use super::super::*;

pub(in super::super) struct DetailsPaneView {
    view: WeakEntity<GitGpuiView>,
}

impl DetailsPaneView {
    pub(in super::super) fn new(view: WeakEntity<GitGpuiView>) -> Self {
        Self { view }
    }
}

impl Render for DetailsPaneView {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        self.view
            .update(cx, |this, cx| this.commit_details_view(cx))
            .unwrap_or_else(|_| div().into_any_element())
    }
}
