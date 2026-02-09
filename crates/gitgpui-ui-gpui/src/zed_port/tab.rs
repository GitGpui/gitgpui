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
/// Ported/adapted from Zed's `ui::Tab`.
pub struct Tab {
    div: Stateful<Div>,
    selected: bool,
    position: TabPosition,
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
            (theme.colors.text, theme.colors.active_section)
        } else {
            (theme.colors.text_muted, theme.colors.surface_bg)
        };
        let hover_bg = theme.colors.hover;
        let active_bg = theme.colors.active;

        let start_slot = div().flex_none().size(Self::START_TAB_SLOT_SIZE);

        let end_slot = div()
            .flex_none()
            .size(Self::END_TAB_SLOT_SIZE)
            .flex()
            .items_center()
            .justify_center()
            .children(self.end_slot);

        let mut base = self
            .div
            .group("tab")
            .tab_index(0)
            .h(Self::container_height())
            .bg(tab_bg)
            .border_color(theme.colors.border)
            .cursor_pointer()
            .hover(move |s| s.bg(hover_bg))
            .active(move |s| s.bg(active_bg))
            .focus(move |s| s.border_color(theme.colors.focus_ring))
            .on_key_down(|event, window, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "left" => {
                        window.focus_prev();
                        cx.stop_propagation();
                    }
                    "right" => {
                        window.focus_next();
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            })
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
                    base.pl(px(1.0)).pb(px(1.0))
                } else {
                    base.pl(px(1.0)).pr(px(1.0)).border_b_1()
                }
            }
            TabPosition::Last => {
                if self.selected {
                    base.pb(px(1.0))
                } else {
                    base.pl(px(1.0)).border_b_1().border_r_1()
                }
            }
            TabPosition::Middle(Ordering::Equal) => {
                if self.selected {
                    base.pb(px(1.0))
                } else {
                    base.border_l_1().border_r_1().pb(px(1.0))
                }
            }
            TabPosition::Middle(Ordering::Less) => base.border_l_1().pr(px(1.0)).border_b_1(),
            TabPosition::Middle(Ordering::Greater) => base.border_r_1().pl(px(1.0)).border_b_1(),
        };

        base
    }
}
