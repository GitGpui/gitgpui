use crate::theme::AppTheme;
use gpui::prelude::*;
use gpui::{AnyElement, Div, ElementId, IntoElement, ScrollHandle, Stateful, div, px};

use super::Tab;

/// Ported/adapted from Zed's `ui::TabBar`.
pub struct TabBar {
    id: ElementId,
    start: Vec<AnyElement>,
    tabs: Vec<AnyElement>,
    end: Vec<AnyElement>,
    scroll_handle: Option<ScrollHandle>,
}

impl TabBar {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            start: Vec::new(),
            tabs: Vec::new(),
            end: Vec::new(),
            scroll_handle: None,
        }
    }

    pub fn track_scroll(mut self, scroll_handle: &ScrollHandle) -> Self {
        self.scroll_handle = Some(scroll_handle.clone());
        self
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
        let mut tabs = div()
            .id((self.id.clone(), "tabs"))
            .flex()
            .items_center()
            .h_full()
            .overflow_x_scroll()
            .scrollbar_width(px(0.0))
            .children(self.tabs);

        if let Some(scroll_handle) = self.scroll_handle {
            tabs = tabs.track_scroll(&scroll_handle);
        }

        div()
            .id(self.id)
            .group("tab_bar")
            .flex()
            .flex_none()
            .items_center()
            .w_full()
            .h(Tab::container_height())
            .bg(theme.colors.surface_bg)
            .when(!self.start.is_empty(), |this| {
                this.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(0.0))
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
                    .child(tabs),
            )
            .when(!self.end.is_empty(), |this| {
                this.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(0.0))
                        .h_full()
                        .border_b_1()
                        .border_l_1()
                        .border_color(theme.colors.border)
                        .children(self.end),
                )
            })
    }
}
