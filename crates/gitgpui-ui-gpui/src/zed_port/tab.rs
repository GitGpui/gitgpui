use crate::theme::AppTheme;
use gpui::prelude::*;
use gpui::{AnyElement, Div, ElementId, IntoElement, Stateful, div, px};
use std::cmp::Ordering;

/// The position of a tab within a list.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TabPosition {
    First,
    Middle(Ordering),
    Last,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TabCloseSide {
    Start,
    End,
}

/// Ported/adapted from Zed's `ui::Tab`.
pub struct Tab {
    div: Stateful<Div>,
    selected: bool,
    position: TabPosition,
    close_side: TabCloseSide,
    start_slot: Option<AnyElement>,
    end_slot: Option<AnyElement>,
    children: Vec<AnyElement>,
}

impl Tab {
    const START_TAB_SLOT_SIZE: gpui::Pixels = px(12.0);
    const END_TAB_SLOT_SIZE: gpui::Pixels = px(14.0);

    pub fn new(id: impl Into<ElementId>) -> Self {
        let id = id.into();
        Self {
            div: div().id(id.clone()),
            selected: false,
            position: TabPosition::First,
            close_side: TabCloseSide::End,
            start_slot: None,
            end_slot: None,
            children: Vec::new(),
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn position(mut self, position: TabPosition) -> Self {
        self.position = position;
        self
    }

    pub fn close_side(mut self, close_side: TabCloseSide) -> Self {
        self.close_side = close_side;
        self
    }

    pub fn start_slot(mut self, slot: impl IntoElement) -> Self {
        self.start_slot = Some(slot.into_any_element());
        self
    }

    pub fn end_slot(mut self, slot: impl IntoElement) -> Self {
        self.end_slot = Some(slot.into_any_element());
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    pub fn container_height() -> gpui::Pixels {
        px(32.0)
    }

    pub fn render(self, theme: AppTheme) -> Stateful<Div> {
        let (text_color, tab_bg) = if self.selected {
            (theme.colors.text, theme.colors.window_bg)
        } else {
            (theme.colors.text_muted, theme.colors.surface_bg)
        };
        let hover_bg = theme.colors.hover;

        let (start_slot, end_slot) = match self.close_side {
            TabCloseSide::End => (self.start_slot, self.end_slot),
            TabCloseSide::Start => (self.end_slot, self.start_slot),
        };

        let start_slot = div()
            .flex_none()
            .size(Self::START_TAB_SLOT_SIZE)
            .flex()
            .items_center()
            .justify_center()
            .children(start_slot);

        let end_slot = div()
            .flex_none()
            .size(Self::END_TAB_SLOT_SIZE)
            .flex()
            .items_center()
            .justify_center()
            .children(end_slot);

        let mut base = self
            .div
            .group("tab")
            .h(Self::container_height())
            .bg(tab_bg)
            .border_color(theme.colors.border)
            .cursor_pointer()
            .hover(move |s| s.bg(hover_bg))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(px(31.0))
                    .px_2()
                    .text_color(text_color)
                    .child(start_slot)
                    .children(self.children)
                    .child(end_slot),
            );

        base = match self.position {
            TabPosition::First => {
                if self.selected {
                    base.pl(px(1.0)).border_r_1().pb(px(1.0))
                } else {
                    base.pl(px(1.0)).pr(px(1.0)).border_b_1()
                }
            }
            TabPosition::Last => {
                if self.selected {
                    base.border_l_1().border_r_1().pb(px(1.0))
                } else {
                    base.pl(px(1.0)).border_b_1().border_r_1()
                }
            }
            TabPosition::Middle(Ordering::Equal) => base.border_l_1().border_r_1().pb(px(1.0)),
            TabPosition::Middle(Ordering::Less) => base.border_l_1().pr(px(1.0)).border_b_1(),
            TabPosition::Middle(Ordering::Greater) => base.border_r_1().pl(px(1.0)).border_b_1(),
        };

        base
    }
}
