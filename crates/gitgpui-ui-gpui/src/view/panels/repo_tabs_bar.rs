use super::*;
use std::hash::{Hash, Hasher};

pub(in super::super) struct RepoTabsBarView {
    store: Arc<AppStore>,
    state: Arc<AppState>,
    theme: AppTheme,
    _ui_model_subscription: gpui::Subscription,
    root_view: WeakEntity<GitGpuiView>,
    tooltip_host: WeakEntity<TooltipHost>,

    hovered_repo_tab: Option<RepoId>,
    notify_fingerprint: u64,
}

impl RepoTabsBarView {
    fn notify_fingerprint(state: &AppState) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        state.active_repo.hash(&mut hasher);
        state.repos.len().hash(&mut hasher);
        for repo in &state.repos {
            repo.id.hash(&mut hasher);
            repo.spec.workdir.hash(&mut hasher);
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

            if this
                .hovered_repo_tab
                .is_some_and(|id| !this.state.repos.iter().any(|r| r.id == id))
            {
                this.hovered_repo_tab = None;
            }

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
            hovered_repo_tab: None,
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
}

impl Render for RepoTabsBarView {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
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
                            this.set_tooltip_text_if_changed(Some(close_tooltip.clone()), cx);
                            return;
                        }

                        let cleared = this.clear_tooltip_if_matches(&close_tooltip, cx);
                        if cleared && this.hovered_repo_tab == Some(repo_id) {
                            this.set_tooltip_text_if_changed(Some(tooltip.clone()), cx);
                        }
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
                            this.set_tooltip_text_if_changed(Some(tooltip.clone()), cx);
                        } else {
                            if this.hovered_repo_tab == Some(repo_id) {
                                this.hovered_repo_tab = None;
                            }
                            this.clear_tooltip_if_matches(&tooltip, cx);
                            this.clear_tooltip_if_matches(&close_tooltip, cx);
                        }
                        cx.notify();
                    }
                }))
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, _cx| {
                    this.store.dispatch(Msg::SetActiveRepo { repo_id });
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

        let root_view = self.root_view.clone();
        let open_repo = zed::Button::new("open_repo", "")
            .start_slot(icon("icons/folder.svg"))
            .style(zed::ButtonStyle::Subtle)
            .on_click(theme, cx, move |_this, _e, window, cx| {
                let _ = root_view.update(cx, |root, cx| root.prompt_open_repo(window, cx));
            })
            .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                let text: SharedString = "Open repository".into();
                if *hovering {
                    this.set_tooltip_text_if_changed(Some(text), cx);
                } else {
                    this.clear_tooltip_if_matches(&text, cx);
                }
            }));

        let root_view = self.root_view.clone();
        let clone_repo = zed::Button::new("clone_repo", "")
            .start_slot(icon("icons/cloud.svg"))
            .style(zed::ButtonStyle::Subtle)
            .on_click(theme, cx, move |_this, e, window, cx| {
                let _ = root_view.update(cx, |root, cx| {
                    root.open_popover_at(PopoverKind::CloneRepo, e.position(), window, cx);
                });
            })
            .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                let text: SharedString = "Clone repository".into();
                if *hovering {
                    this.set_tooltip_text_if_changed(Some(text), cx);
                } else {
                    this.clear_tooltip_if_matches(&text, cx);
                }
            }));

        bar.end_child(
            div()
                .id("add_repo_container")
                .relative()
                .h_full()
                .flex()
                .items_center()
                .gap_1()
                .child(open_repo)
                .child(clone_repo),
        )
        .render(theme)
    }
}
