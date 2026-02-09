use super::super::*;

pub(in super::super) struct MainPaneView {
    view: WeakEntity<GitGpuiView>,
}

impl MainPaneView {
    pub(in super::super) fn new(view: WeakEntity<GitGpuiView>) -> Self {
        Self { view }
    }
}

impl Render for MainPaneView {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        self.view
            .update(cx, |this, cx| {
                let show_diff = this
                    .active_repo()
                    .and_then(|r| r.diff_target.as_ref())
                    .is_some();
                if show_diff {
                    this.diff_view(cx)
                } else {
                    this.history_view(cx)
                }
            })
            .unwrap_or_else(|_| div())
    }
}
