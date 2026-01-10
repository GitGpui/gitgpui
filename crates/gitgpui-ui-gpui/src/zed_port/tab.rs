use crate::theme::AppTheme;
use gpui::{AnyElement, Div, ElementId, IntoElement, Stateful, div, px};
use gpui::prelude::*;
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
        px(30.0)
    }

    pub fn render(self, theme: AppTheme) -> Stateful<Div> {
        let (text_color, tab_bg, hover_bg) = if self.selected {
            (theme.colors.text, theme.colors.window_bg, theme.colors.hover)
        } else {
            (
                theme.colors.text_muted,
                theme.colors.surface_bg,
                with_alpha(theme.colors.hover, 0.8),
            )
        };

        let (start_slot, end_slot) = match self.close_side {
            TabCloseSide::End => (self.start_slot, self.end_slot),
            TabCloseSide::Start => (self.end_slot, self.start_slot),
        };

        let mut base = self
            .div
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
                    .h_full()
                    .px_2()
                    .text_color(text_color)
                    .children(start_slot)
                    .children(self.children)
                    .children(end_slot),
            );

        base = match self.position {
            TabPosition::First => base.border_b_1().border_r_1(),
            TabPosition::Last => base.border_b_1().border_l_1().border_r_1(),
            TabPosition::Middle(_) => base.border_b_1().border_l_1().border_r_1(),
        };

        if self.selected {
            base = base.border_b_0();
        }

        base
    }
}

fn with_alpha(mut color: gpui::Rgba, alpha: f32) -> gpui::Rgba {
    color.a = alpha;
    color
}

