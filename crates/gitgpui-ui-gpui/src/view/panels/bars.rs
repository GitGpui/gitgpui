use super::*;

impl GitGpuiView {
    pub(in super::super) fn repo_tabs_bar(
        &mut self,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let active = self.active_repo_id();
        let repos_len = self.state.repos.len();
        let active_ix = active.and_then(|id| self.state.repos.iter().position(|r| r.id == id));

        let mut bar = zed::TabBar::new("repo_tab_bar");
        for (ix, repo) in self.state.repos.iter().enumerate() {
            let repo_id = repo.id;
            let is_active = Some(repo_id) == active;
            let show_close = self.hovered_repo_tab == Some(repo_id);
            let label: SharedString = repo
                .spec
                .workdir
                .file_name()
                .and_then(|s| s.to_str())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| repo.spec.workdir.display().to_string())
                .into();

            let position = if ix == 0 {
                zed::TabPosition::First
            } else if ix + 1 == repos_len {
                zed::TabPosition::Last
            } else {
                let ordering = match (is_active, active_ix) {
                    (true, _) => std::cmp::Ordering::Equal,
                    (false, Some(active_ix)) => ix.cmp(&active_ix),
                    (false, None) => std::cmp::Ordering::Equal,
                };
                zed::TabPosition::Middle(ordering)
            };

            let tooltip: SharedString = repo.spec.workdir.display().to_string().into();
            let close_tooltip: SharedString = "Close repository".into();
            let close_button = div()
                .id(("repo_tab_close", repo_id.0))
                .flex()
                .items_center()
                .justify_center()
                .size(px(14.0))
                .rounded(px(theme.radii.row))
                .text_xs()
                .text_color(theme.colors.text_muted)
                .cursor_pointer()
                .hover(move |s| s.bg(theme.colors.hover).text_color(theme.colors.text))
                .active(move |s| s.bg(theme.colors.active).text_color(theme.colors.text))
                .child("✕")
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    cx.stop_propagation();
                    this.hovered_repo_tab = None;
                    this.store.dispatch(Msg::CloseRepo { repo_id });
                    cx.notify();
                }))
                .on_hover(cx.listener({
                    let tooltip = tooltip.clone();
                    let close_tooltip = close_tooltip.clone();
                    move |this, hovering: &bool, _w, cx| {
                        if *hovering {
                            this.tooltip_text = Some(close_tooltip.clone());
                        } else if this.tooltip_text.as_ref() == Some(&close_tooltip) {
                            if this.hovered_repo_tab == Some(repo_id) {
                                this.tooltip_text = Some(tooltip.clone());
                            } else {
                                this.tooltip_text = None;
                            }
                        }
                        cx.notify();
                    }
                }));

            let mut tab = zed::Tab::new(("repo_tab", repo_id.0))
                .selected(is_active)
                .position(position);
            if show_close {
                tab = tab.end_slot(close_button);
            }

            let tab = tab
                .child(div().text_sm().line_clamp(1).child(label))
                .render(theme)
                .on_hover(cx.listener({
                    move |this, hovering: &bool, _w, cx| {
                        if *hovering {
                            this.hovered_repo_tab = Some(repo_id);
                            this.tooltip_text = Some(tooltip.clone());
                        } else {
                            if this.hovered_repo_tab == Some(repo_id) {
                                this.hovered_repo_tab = None;
                            }
                            if this.tooltip_text.as_ref() == Some(&tooltip)
                                || this.tooltip_text.as_ref() == Some(&close_tooltip)
                            {
                                this.tooltip_text = None;
                            }
                        }
                        cx.notify();
                    }
                }))
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.store.dispatch(Msg::SetActiveRepo { repo_id });
                    this.rebuild_diff_cache();
                    cx.notify();
                }));

            bar = bar.tab(tab);
        }

        let icon = |path: &'static str| {
            gpui::svg()
                .path(path)
                .w(px(14.0))
                .h(px(14.0))
                .text_color(theme.colors.text)
        };

        bar.end_child(
            div()
                .id("add_repo_container")
                .relative()
                .h_full()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    zed::Button::new("open_repo", "")
                        .start_slot(icon("icons/folder.svg"))
                        .style(zed::ButtonStyle::Subtle)
                        .on_click(theme, cx, |this, _e, window, cx| {
                            this.prompt_open_repo(window, cx)
                        })
                        .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                            let text: SharedString = "Open repository".into();
                            if *hovering {
                                this.tooltip_text = Some(text);
                            } else if this.tooltip_text.as_ref() == Some(&text) {
                                this.tooltip_text = None;
                            }
                            cx.notify();
                        })),
                )
                .child(
                    zed::Button::new("clone_repo", "")
                        .start_slot(icon("icons/cloud.svg"))
                        .style(zed::ButtonStyle::Subtle)
                        .on_click(theme, cx, move |this, e, window, cx| {
                            this.clone_repo_url_input.update(cx, |input, cx| {
                                input.set_theme(theme, cx);
                                input.set_text("", cx);
                            });
                            this.clone_repo_parent_dir_input.update(cx, |input, cx| {
                                input.set_theme(theme, cx);
                            });
                            this.open_popover_at(PopoverKind::CloneRepo, e.position(), window, cx);
                        })
                        .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                            let text: SharedString = "Clone repository".into();
                            if *hovering {
                                this.tooltip_text = Some(text);
                            } else if this.tooltip_text.as_ref() == Some(&text) {
                                this.tooltip_text = None;
                            }
                            cx.notify();
                        })),
                ),
        )
        .render(theme)
    }

    pub(in super::super) fn open_repo_panel(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        if !self.open_repo_panel {
            return div();
        }

        div()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .bg(theme.colors.surface_bg)
            .border_1()
            .border_color(theme.colors.border)
            .rounded(px(theme.radii.panel))
            .shadow_sm()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.colors.text_muted)
                    .child("Path"),
            )
            .child(div().flex_1().child(self.open_repo_input.clone()))
            .child(
                zed::Button::new("open_repo_go", "Open")
                    .style(zed::ButtonStyle::Filled)
                    .on_click(theme, cx, |this, _e, _w, cx| {
                        let path = this
                            .open_repo_input
                            .read_with(cx, |input, _| input.text().trim().to_string());
                        if !path.is_empty() {
                            this.store.dispatch(Msg::OpenRepo(path.into()));
                            this.open_repo_panel = false;
                        }
                        cx.notify();
                    }),
            )
            .child(zed::Button::new("open_repo_cancel", "Cancel").on_click(
                theme,
                cx,
                |this, _e, _w, cx| {
                    this.open_repo_panel = false;
                    cx.notify();
                },
            ))
    }

    pub(in super::super) fn action_bar(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let hover_bg = with_alpha(theme.colors.text, if theme.is_dark { 0.06 } else { 0.04 });
        let active_bg = with_alpha(theme.colors.text, if theme.is_dark { 0.10 } else { 0.07 });
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
                let _ = this.ensure_repo_picker_search_input(window, cx);
                this.popover = Some(PopoverKind::RepoPicker);
                this.popover_anchor = Some(e.position());
                cx.notify();
            }))
            .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                let text: SharedString = "Select repository".into();
                if *hovering {
                    this.tooltip_text = Some(text);
                } else if this.tooltip_text.as_ref() == Some(&text) {
                    this.tooltip_text = None;
                }
                cx.notify();
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
                let _ = this.ensure_branch_picker_search_input(window, cx);
                this.popover = Some(PopoverKind::BranchPicker);
                this.popover_anchor = Some(e.position());
                cx.notify();
            }))
            .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                let text: SharedString = "Select branch".into();
                if *hovering {
                    this.tooltip_text = Some(text);
                } else if this.tooltip_text.as_ref() == Some(&text) {
                    this.tooltip_text = None;
                }
                cx.notify();
            }));

        let pull_color = if pull_count > 0 {
            theme.colors.warning
        } else {
            theme.colors.text
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
            .start_slot(icon("icons/chevron_down.svg", theme.colors.text))
            .style(zed::ButtonStyle::Subtle);

        let pull = div()
            .id("pull")
            .child(
                zed::SplitButton::new(
                    pull_main.on_click(theme, cx, |this, _e, _w, cx| {
                        if let Some(repo_id) = this.active_repo_id() {
                            this.store.dispatch(Msg::Pull {
                                repo_id,
                                mode: PullMode::Default,
                            });
                        }
                        cx.notify();
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
                    this.tooltip_text = Some(text);
                } else if this.tooltip_text.as_ref() == Some(&text) {
                    this.tooltip_text = None;
                }
                cx.notify();
            }));

        let push_color = if push_count > 0 {
            theme.colors.success
        } else {
            theme.colors.text
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
            .start_slot(icon("icons/chevron_down.svg", theme.colors.text))
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
                                cx.notify();
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
                                this.push_upstream_branch_input
                                    .update(cx, |i, cx| i.set_text(head, cx));
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
                        cx.notify();
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
                    this.tooltip_text = Some(text);
                } else if this.tooltip_text.as_ref() == Some(&text) {
                    this.tooltip_text = None;
                }
                cx.notify();
            }));

        let stash = zed::Button::new("stash", "Stash")
            .start_slot(icon("icons/box.svg", theme.colors.text))
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
                    this.tooltip_text = Some(text);
                } else if this.tooltip_text.as_ref() == Some(&text) {
                    this.tooltip_text = None;
                }
                cx.notify();
            }));

        let create_branch = zed::Button::new("create_branch", "Branch")
            .start_slot(icon("icons/git_branch.svg", theme.colors.text))
            .style(zed::ButtonStyle::Outlined)
            .on_click(theme, cx, |this, e, window, cx| {
                this.open_popover_at(PopoverKind::CreateBranch, e.position(), window, cx);
            })
            .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                let text: SharedString = "Create branch".into();
                if *hovering {
                    this.tooltip_text = Some(text);
                } else if this.tooltip_text.as_ref() == Some(&text) {
                    this.tooltip_text = None;
                }
                cx.notify();
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
