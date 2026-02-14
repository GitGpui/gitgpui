use super::*;
use std::hash::{Hash, Hasher};

pub(in super::super) struct ActionBarView {
    store: Arc<AppStore>,
    state: Arc<AppState>,
    theme: AppTheme,
    _ui_model_subscription: gpui::Subscription,
    root_view: WeakEntity<GitGpuiView>,
    tooltip_host: WeakEntity<TooltipHost>,
    notify_fingerprint: u64,
}

impl ActionBarView {
    fn notify_fingerprint(state: &AppState) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        state.active_repo.hash(&mut hasher);

        if let Some(repo_id) = state.active_repo
            && let Some(repo) = state.repos.iter().find(|r| r.id == repo_id)
        {
            repo.spec.workdir.hash(&mut hasher);
            repo.head_branch_rev.hash(&mut hasher);
            match &repo.head_branch {
                Loadable::NotLoaded => 0u8.hash(&mut hasher),
                Loadable::Loading => 1u8.hash(&mut hasher),
                Loadable::Error(err) => {
                    2u8.hash(&mut hasher);
                    err.hash(&mut hasher);
                }
                Loadable::Ready(name) => {
                    3u8.hash(&mut hasher);
                    name.hash(&mut hasher);
                }
            }

            match &repo.upstream_divergence {
                Loadable::NotLoaded => 0u8.hash(&mut hasher),
                Loadable::Loading => 1u8.hash(&mut hasher),
                Loadable::Error(err) => {
                    2u8.hash(&mut hasher);
                    err.hash(&mut hasher);
                }
                Loadable::Ready(None) => 3u8.hash(&mut hasher),
                Loadable::Ready(Some(div)) => {
                    4u8.hash(&mut hasher);
                    div.behind.hash(&mut hasher);
                    div.ahead.hash(&mut hasher);
                }
            }

            repo.pull_in_flight.hash(&mut hasher);
            repo.push_in_flight.hash(&mut hasher);

            let can_stash = match &repo.status {
                Loadable::Ready(status) => !status.staged.is_empty() || !status.unstaged.is_empty(),
                _ => false,
            };
            can_stash.hash(&mut hasher);
        }

        hasher.finish()
    }

    pub(in super::super) fn new(
        store: Arc<AppStore>,
        ui_model: Entity<AppUiModel>,
        theme: AppTheme,
        root_view: WeakEntity<GitGpuiView>,
        tooltip_host: WeakEntity<TooltipHost>,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let state = Arc::clone(&ui_model.read(cx).state);
        let notify_fingerprint = Self::notify_fingerprint(&state);
        let subscription = cx.observe(&ui_model, |this, model, cx| {
            let next = Arc::clone(&model.read(cx).state);
            let next_fingerprint = Self::notify_fingerprint(&next);

            this.state = next;
            if next_fingerprint != this.notify_fingerprint {
                this.notify_fingerprint = next_fingerprint;
                cx.notify();
            }
        });

        Self {
            store,
            state,
            theme,
            _ui_model_subscription: subscription,
            root_view,
            tooltip_host,
            notify_fingerprint,
        }
    }

    pub(in super::super) fn set_theme(&mut self, theme: AppTheme, cx: &mut gpui::Context<Self>) {
        self.theme = theme;
        cx.notify();
    }

    fn active_repo_id(&self) -> Option<RepoId> {
        self.state.active_repo
    }

    fn active_repo(&self) -> Option<&RepoState> {
        let repo_id = self.active_repo_id()?;
        self.state.repos.iter().find(|r| r.id == repo_id)
    }

    fn set_tooltip_text_if_changed(
        &mut self,
        next: Option<SharedString>,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        let _ = self
            .tooltip_host
            .update(cx, |host, cx| host.set_tooltip_text_if_changed(next, cx));
        false
    }

    fn clear_tooltip_if_matches(
        &mut self,
        tooltip: &SharedString,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        let tooltip = tooltip.clone();
        let _ = self
            .tooltip_host
            .update(cx, |host, cx| host.clear_tooltip_if_matches(&tooltip, cx));
        false
    }

    fn open_popover_at(
        &mut self,
        kind: PopoverKind,
        anchor: Point<Pixels>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let _ = self.root_view.update(cx, |root, cx| {
            root.open_popover_at(kind, anchor, window, cx);
        });
    }

    fn push_toast(&mut self, kind: zed::ToastKind, message: String, cx: &mut gpui::Context<Self>) {
        let _ = self.root_view.update(cx, |root, cx| {
            root.push_toast(kind, message, cx);
        });
    }
}

impl Render for ActionBarView {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let hover_bg = with_alpha(theme.colors.text, if theme.is_dark { 0.06 } else { 0.04 });
        let active_bg = with_alpha(theme.colors.text, if theme.is_dark { 0.10 } else { 0.07 });
        let icon_primary = theme.colors.accent;
        let icon_muted = with_alpha(theme.colors.accent, if theme.is_dark { 0.72 } else { 0.82 });
        let icon = |path: &'static str, color: gpui::Rgba| {
            gpui::svg()
                .path(path)
                .w(px(14.0))
                .h(px(14.0))
                .text_color(color)
        };
        let spinner = |id: (&'static str, u64), color: gpui::Rgba| {
            gpui::svg()
                .path("icons/spinner.svg")
                .w(px(14.0))
                .h(px(14.0))
                .text_color(color)
                .with_animation(
                    id,
                    Animation::new(std::time::Duration::from_millis(850)).repeat(),
                    |svg, delta| {
                        svg.with_transformation(gpui::Transformation::rotate(gpui::radians(
                            delta * std::f32::consts::TAU,
                        )))
                    },
                )
        };
        let count_badge = |count: usize, color: gpui::Rgba| {
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(color)
                .child(count.to_string())
                .into_any_element()
        };

        let repo_title: SharedString = self
            .active_repo()
            .map(|r| r.spec.workdir.display().to_string().into())
            .unwrap_or_else(|| "No repository".into());

        let branch: SharedString = self
            .active_repo()
            .map(|r| match &r.head_branch {
                Loadable::Ready(name) => name.clone().into(),
                Loadable::Loading => "".into(),
                Loadable::Error(_) => "error".into(),
                Loadable::NotLoaded => "—".into(),
            })
            .unwrap_or_else(|| "—".into());

        let (pull_count, push_count) = self
            .active_repo()
            .and_then(|r| match &r.upstream_divergence {
                Loadable::Ready(Some(d)) => Some((d.behind, d.ahead)),
                _ => None,
            })
            .unwrap_or((0, 0));
        let (pull_loading, push_loading) = self
            .active_repo()
            .map(|r| (r.pull_in_flight > 0, r.push_in_flight > 0))
            .unwrap_or((false, false));
        let active_repo_key = self.active_repo_id().map(|id| id.0).unwrap_or(0);

        let can_stash = self
            .active_repo()
            .and_then(|r| match &r.status {
                Loadable::Ready(s) => Some(!s.staged.is_empty() || !s.unstaged.is_empty()),
                _ => None,
            })
            .unwrap_or(false);

        let repo_picker = div()
            .id("repo_picker")
            .debug_selector(|| "repo_picker".to_string())
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .rounded(px(theme.radii.row))
            .hover(move |s| s.bg(hover_bg))
            .active(move |s| s.bg(active_bg))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child("Repository"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.colors.text_muted)
                    .line_clamp(1)
                    .child(repo_title),
            )
            .on_click(cx.listener(|this, e: &ClickEvent, window, cx| {
                this.open_popover_at(PopoverKind::RepoPicker, e.position(), window, cx);
            }))
            .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                let text: SharedString = "Select repository".into();
                if *hovering {
                    this.set_tooltip_text_if_changed(Some(text), cx);
                } else {
                    this.clear_tooltip_if_matches(&text, cx);
                }
            }));

        let branch_picker = div()
            .id("branch_picker")
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .rounded(px(theme.radii.row))
            .hover(move |s| s.bg(hover_bg))
            .active(move |s| s.bg(active_bg))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child("Branch"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.colors.text_muted)
                    .child(branch),
            )
            .on_click(cx.listener(|this, e: &ClickEvent, window, cx| {
                this.open_popover_at(PopoverKind::BranchPicker, e.position(), window, cx);
            }))
            .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                let text: SharedString = "Select branch".into();
                if *hovering {
                    this.set_tooltip_text_if_changed(Some(text), cx);
                } else {
                    this.clear_tooltip_if_matches(&text, cx);
                }
            }));

        let pull_color = if pull_count > 0 {
            theme.colors.warning
        } else {
            icon_muted
        };
        let mut pull_main = zed::Button::new("pull_main", "Pull")
            .start_slot(if pull_loading {
                spinner(("pull_spinner", active_repo_key), pull_color).into_any_element()
            } else {
                icon("icons/arrow_down.svg", pull_color).into_any_element()
            })
            .style(zed::ButtonStyle::Subtle);
        if pull_count > 0 {
            pull_main = pull_main.end_slot(count_badge(pull_count, pull_color));
        }
        let pull_menu = zed::Button::new("pull_menu", "")
            .start_slot(icon("icons/chevron_down.svg", icon_muted))
            .style(zed::ButtonStyle::Subtle);

        let pull = div()
            .id("pull")
            .child(
                zed::SplitButton::new(
                    pull_main.on_click(theme, cx, |this, _e, _w, _cx| {
                        if let Some(repo_id) = this.active_repo_id() {
                            this.store.dispatch(Msg::Pull {
                                repo_id,
                                mode: PullMode::Default,
                            });
                        }
                    }),
                    pull_menu.on_click(theme, cx, |this, e, window, cx| {
                        this.open_popover_at(PopoverKind::PullPicker, e.position(), window, cx);
                    }),
                )
                .style(zed::SplitButtonStyle::Outlined)
                .render(theme),
            )
            .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                let text: SharedString = format!("Pull ({pull_count} behind)").into();
                if *hovering {
                    this.set_tooltip_text_if_changed(Some(text), cx);
                } else {
                    this.clear_tooltip_if_matches(&text, cx);
                }
            }));

        let push_color = if push_count > 0 {
            theme.colors.success
        } else {
            icon_muted
        };
        let mut push_main = zed::Button::new("push", "Push")
            .start_slot(if push_loading {
                spinner(("push_spinner", active_repo_key), push_color).into_any_element()
            } else {
                icon("icons/arrow_up.svg", push_color).into_any_element()
            })
            .style(zed::ButtonStyle::Outlined);
        if push_count > 0 {
            push_main = push_main.end_slot(count_badge(push_count, push_color));
        }
        let push_menu = zed::Button::new("push_menu", "")
            .start_slot(icon("icons/chevron_down.svg", icon_muted))
            .style(zed::ButtonStyle::Subtle);

        let push = div()
            .id("push")
            .child(
                zed::SplitButton::new(
                    push_main.on_click(theme, cx, |this, e, window, cx| {
                        let Some(repo) = this.active_repo() else {
                            return;
                        };
                        let repo_id = repo.id;
                        let head = match &repo.head_branch {
                            Loadable::Ready(head) => head.clone(),
                            _ => {
                                this.store.dispatch(Msg::Push { repo_id });
                                return;
                            }
                        };

                        let upstream_missing = match &repo.branches {
                            Loadable::Ready(branches) => branches
                                .iter()
                                .find(|b| b.name == head)
                                .is_some_and(|b| b.upstream.is_none()),
                            _ => false,
                        };

                        if upstream_missing {
                            let remote = match &repo.remotes {
                                Loadable::Ready(remotes) => {
                                    if remotes.is_empty() {
                                        None
                                    } else if remotes.iter().any(|r| r.name == "origin") {
                                        Some("origin".to_string())
                                    } else {
                                        Some(remotes[0].name.clone())
                                    }
                                }
                                _ => Some("origin".to_string()),
                            };

                            if let Some(remote) = remote {
                                this.open_popover_at(
                                    PopoverKind::PushSetUpstreamPrompt { repo_id, remote },
                                    e.position(),
                                    window,
                                    cx,
                                );
                                return;
                            }

                            this.push_toast(
                                zed::ToastKind::Error,
                                "Cannot push: no remotes configured".to_string(),
                                cx,
                            );
                            return;
                        }

                        this.store.dispatch(Msg::Push { repo_id });
                    }),
                    push_menu.on_click(theme, cx, |this, e, window, cx| {
                        this.open_popover_at(PopoverKind::PushPicker, e.position(), window, cx);
                    }),
                )
                .style(zed::SplitButtonStyle::Outlined)
                .render(theme),
            )
            .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                let text: SharedString = format!("Push ({push_count} ahead)").into();
                if *hovering {
                    this.set_tooltip_text_if_changed(Some(text), cx);
                } else {
                    this.clear_tooltip_if_matches(&text, cx);
                }
            }));

        let stash = zed::Button::new("stash", "Stash")
            .start_slot(icon("icons/box.svg", icon_primary))
            .style(zed::ButtonStyle::Outlined)
            .disabled(!can_stash)
            .on_click(theme, cx, |this, e, window, cx| {
                this.open_popover_at(PopoverKind::StashPrompt, e.position(), window, cx);
            })
            .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                let text: SharedString = if can_stash {
                    "Create stash".into()
                } else {
                    "No changes to stash".into()
                };
                if *hovering {
                    this.set_tooltip_text_if_changed(Some(text), cx);
                } else {
                    this.clear_tooltip_if_matches(&text, cx);
                }
            }));

        let create_branch = zed::Button::new("create_branch", "Branch")
            .start_slot(icon("icons/git_branch.svg", icon_primary))
            .style(zed::ButtonStyle::Outlined)
            .on_click(theme, cx, |this, e, window, cx| {
                this.open_popover_at(PopoverKind::CreateBranch, e.position(), window, cx);
            })
            .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                let text: SharedString = "Create branch".into();
                if *hovering {
                    this.set_tooltip_text_if_changed(Some(text), cx);
                } else {
                    this.clear_tooltip_if_matches(&text, cx);
                }
            }));

        div()
            .flex()
            .items_center()
            .justify_between()
            .px_2()
            .py_1()
            .bg(theme.colors.active_section)
            .border_b_1()
            .border_color(theme.colors.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .flex_1()
                    .child(repo_picker)
                    .child(branch_picker),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(pull)
                    .child(push)
                    .child(create_branch)
                    .child(stash),
            )
    }
}
