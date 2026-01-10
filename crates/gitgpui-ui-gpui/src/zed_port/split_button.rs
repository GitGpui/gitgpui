use crate::theme::AppTheme;
use gpui::{AnyElement, Div, IntoElement, div, px};
use gpui::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SplitButtonStyle {
    Filled,
    Outlined,
    Transparent,
}

/// A button composed of a primary action and a secondary affordance (typically a menu).
///
/// Ported/adapted from Zed's `ui::SplitButton`.
pub struct SplitButton {
    left: AnyElement,
    right: AnyElement,
    style: SplitButtonStyle,
}

impl SplitButton {
    pub fn new(left: impl IntoElement, right: impl IntoElement) -> Self {
        Self {
            left: left.into_any_element(),
            right: right.into_any_element(),
            style: SplitButtonStyle::Filled,
        }
    }

    pub fn style(mut self, style: SplitButtonStyle) -> Self {
        self.style = style;
        self
    }

    pub fn render(self, theme: AppTheme) -> Div {
        let bordered = matches!(self.style, SplitButtonStyle::Filled | SplitButtonStyle::Outlined);
        let bg = match self.style {
            SplitButtonStyle::Filled => theme.colors.surface_bg,
            SplitButtonStyle::Outlined | SplitButtonStyle::Transparent => gpui::rgba(0x00000000),
        };

        div()
            .flex()
            .items_center()
            .rounded(px(theme.radii.row))
            .bg(bg)
            .when(bordered, |this| this.border_1().border_color(with_alpha(theme.colors.border, 0.8)))
            .child(div().flex_1().child(self.left))
            .child(div().h_full().w(px(1.0)).bg(with_alpha(theme.colors.border, 0.5)))
            .child(self.right)
    }
}

fn with_alpha(mut color: gpui::Rgba, alpha: f32) -> gpui::Rgba {
    color.a = alpha;
    color
}
