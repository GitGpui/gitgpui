use super::super::caches::BranchSidebarFingerprint;
use super::super::*;

pub(in super::super) struct SidebarPaneView {
    pub(in super::super) store: Arc<AppStore>,
    state: Arc<AppState>,
    pub(in super::super) theme: AppTheme,
    _ui_model_subscription: gpui::Subscription,
    branches_scroll: UniformListScrollHandle,
    branch_sidebar_cache: Option<BranchSidebarCache>,
    root_view: WeakEntity<GitGpuiView>,
    tooltip_host: WeakEntity<TooltipHost>,
}

impl SidebarPaneView {
    pub(in super::super) fn new(
        store: Arc<AppStore>,
        ui_model: Entity<AppUiModel>,
        theme: AppTheme,
        root_view: WeakEntity<GitGpuiView>,
        tooltip_host: WeakEntity<TooltipHost>,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let state = Arc::clone(&ui_model.read(cx).state);
        let subscription = cx.observe(&ui_model, |this, model, cx| {
            this.state = Arc::clone(&model.read(cx).state);
            cx.notify();
        });

        Self {
            store,
            state,
            theme,
            _ui_model_subscription: subscription,
            branches_scroll: UniformListScrollHandle::default(),
            branch_sidebar_cache: None,
            root_view,
            tooltip_host,
        }
    }

    pub(in super::super) fn set_theme(&mut self, theme: AppTheme, cx: &mut gpui::Context<Self>) {
        self.theme = theme;
        cx.notify();
    }

    pub(in super::super) fn active_repo_id(&self) -> Option<RepoId> {
        self.state.active_repo
    }

    pub(in super::super) fn active_repo(&self) -> Option<&RepoState> {
        let repo_id = self.active_repo_id()?;
        self.state.repos.iter().find(|r| r.id == repo_id)
    }

    pub(in super::super) fn branch_sidebar_rows_cached(
        &mut self,
    ) -> Option<Arc<[BranchSidebarRow]>> {
        let repo = self.active_repo();
        if repo.is_none() {
            self.branch_sidebar_cache = None;
            return None;
        }

        let (repo_id, fingerprint, rows) = {
            let repo = repo?;
            let fingerprint = BranchSidebarFingerprint::from_repo(repo);
            if let Some(cache) = &self.branch_sidebar_cache
                && cache.repo_id == repo.id
                && cache.fingerprint == fingerprint
            {
                return Some(Arc::clone(&cache.rows));
            }

            let rows: Arc<[BranchSidebarRow]> = branch_sidebar::branch_sidebar_rows(repo).into();
            (repo.id, fingerprint, rows)
        };

        self.branch_sidebar_cache = Some(BranchSidebarCache {
            repo_id,
            fingerprint,
            rows: Arc::clone(&rows),
        });
        Some(rows)
    }

    pub(in super::super) fn sidebar(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let Some(rows) = self.branch_sidebar_rows_cached() else {
            return div()
                .flex()
                .flex_col()
                .h_full()
                .min_h(px(0.0))
                .child(zed::empty_state(
                    theme,
                    "Branches",
                    "No repository selected.",
                ));
        };

        let row_count = rows.len();
        let list = uniform_list(
            "branch_sidebar",
            row_count,
            cx.processor(Self::render_branch_sidebar_rows),
        )
        .h_full()
        .min_h(px(0.0))
        .track_scroll(self.branches_scroll.clone());
        let scroll_handle = self.branches_scroll.0.borrow().base_handle.clone();
        let list = div().flex_1().min_h(px(0.0)).px(px(2.0)).child(list);
        let panel_body: AnyElement = div()
            .id("branch_sidebar_scroll_container")
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .child(list.into_any_element())
            .child(zed::Scrollbar::new("branch_sidebar_scrollbar", scroll_handle).render(theme))
            .into_any_element();

        div()
            .flex()
            .flex_col()
            .h_full()
            .min_h(px(0.0))
            .child(panel_body)
    }

    pub(in super::super) fn set_tooltip_text_if_changed(
        &mut self,
        next: Option<SharedString>,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        let _ = self
            .tooltip_host
            .update(cx, |host, cx| host.set_tooltip_text_if_changed(next, cx));
        false
    }

    pub(in super::super) fn clear_tooltip_if_matches(
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

    pub(in super::super) fn open_popover_at(
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

    pub(in super::super) fn rebuild_diff_cache(&mut self, cx: &mut gpui::Context<Self>) {
        let _ = self.root_view.update(cx, |root, cx| {
            root.rebuild_diff_cache();
            cx.notify();
        });
    }
}

impl Render for SidebarPaneView {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        self.sidebar(cx)
    }
}
