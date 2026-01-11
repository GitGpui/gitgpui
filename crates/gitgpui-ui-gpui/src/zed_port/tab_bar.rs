use crate::theme::AppTheme;
use gpui::{AnyElement, Div, ElementId, IntoElement, Stateful, div};
use gpui::prelude::*;

use super::Tab;

/// Ported/adapted from Zed's `ui::TabBar`.
pub struct TabBar {
    id: ElementId,
    start: Vec<AnyElement>,
    tabs: Vec<AnyElement>,
    end: Vec<AnyElement>,
}

impl TabBar {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            start: Vec::new(),
            tabs: Vec::new(),
            end: Vec::new(),
        }
    }

    pub fn start_child(mut self, child: impl IntoElement) -> Self {
        self.start.push(child.into_any_element());
        self
    }

    pub fn tab(mut self, tab: impl IntoElement) -> Self {
        self.tabs.push(tab.into_any_element());
        self
    }

    pub fn end_child(mut self, child: impl IntoElement) -> Self {
        self.end.push(child.into_any_element());
        self
    }

    pub fn render(self, theme: AppTheme) -> Stateful<Div> {
        div()
            .id(self.id)
            .group("tab_bar")
            .flex()
            .items_center()
            .w_full()
            .h(Tab::container_height())
            .bg(theme.colors.surface_bg)
            .when(!self.start.is_empty(), |this| {
                this.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .h_full()
                        .border_b_1()
                        .border_r_1()
                        .border_color(theme.colors.border)
                        .children(self.start),
                )
            })
            .child(
                div()
                    .relative()
                    .flex_1()
                    .h_full()
                    .overflow_x_hidden()
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .border_b_1()
                            .border_color(theme.colors.border),
                    )
                    .child(div().flex().items_center().h_full().children(self.tabs)),
            )
            .when(!self.end.is_empty(), |this| {
                this.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .h_full()
                        .border_b_1()
                        .border_l_1()
                        .border_color(theme.colors.border)
                        .children(self.end),
                )
            })
    }
}
