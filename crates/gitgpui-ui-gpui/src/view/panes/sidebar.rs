use super::super::*;

pub(in super::super) struct SidebarPaneView {
    view: WeakEntity<GitGpuiView>,
}

impl SidebarPaneView {
    pub(in super::super) fn new(view: WeakEntity<GitGpuiView>) -> Self {
        Self { view }
    }
}

impl Render for SidebarPaneView {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        self.view
            .update(cx, |this, cx| this.sidebar(cx))
            .unwrap_or_else(|_| div())
    }
}
