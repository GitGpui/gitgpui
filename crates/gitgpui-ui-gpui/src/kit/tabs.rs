use crate::theme::AppTheme;
use gpui::{ClickEvent, Div, SharedString, Window, div, px};
use gpui::prelude::*;
use std::sync::Arc;

pub struct Tabs {
    labels: Vec<SharedString>,
    selected: usize,
}

impl Tabs {
    pub fn new(labels: impl IntoIterator<Item = SharedString>) -> Self {
        Self {
            labels: labels.into_iter().collect(),
            selected: 0,
        }
    }

    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = selected;
        self
    }

    pub fn render<V: 'static>(
        self,
        theme: AppTheme,
        cx: &gpui::Context<V>,
        on_select: impl Fn(&mut V, usize, &ClickEvent, &mut Window, &mut gpui::Context<V>) + 'static,
    ) -> Div {
        let on_select: Arc<
            dyn Fn(&mut V, usize, &ClickEvent, &mut Window, &mut gpui::Context<V>) + 'static,
        > = Arc::new(on_select);
        let underline = theme.colors.accent;

        let mut row = div()
            .flex()
            .items_end()
            .gap_1()
            .px_2()
            .py_1()
            .bg(theme.colors.surface_bg_elevated)
            .border_1()
            .border_color(theme.colors.border)
            .rounded(px(theme.radii.panel))
            .shadow_sm();

        for (ix, label) in self.labels.into_iter().enumerate() {
            let on_select = Arc::clone(&on_select);
            let is_selected = ix == self.selected;
            let text_color = if is_selected {
                theme.colors.text
            } else {
                theme.colors.text_muted
            };

            let tab = div()
                .id(("tab", ix))
                .px_2()
                .py_1()
                .rounded(px(theme.radii.row))
                .text_sm()
                .text_color(text_color)
                .hover(move |s| s.bg(theme.colors.hover))
                .child(label)
                .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                    (on_select)(this, ix, event, window, cx);
                }));

            let tab = if is_selected {
                tab.border_b_2().border_color(underline)
            } else {
                tab
            };

            row = row.child(tab);
        }

        row
    }
}
