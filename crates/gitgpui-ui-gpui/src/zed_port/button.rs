use crate::theme::AppTheme;
use gpui::prelude::*;
use gpui::{AnyElement, ClickEvent, CursorStyle, Div, IntoElement, SharedString, Stateful, Window, div, px};

use super::{CONTROL_HEIGHT_PX, CONTROL_PAD_X_PX, CONTROL_PAD_Y_PX, ICON_PAD_X_PX};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonStyle {
    Filled,
    Outlined,
    Subtle,
    Transparent,
    Danger,
}

pub struct Button {
    id: SharedString,
    label: SharedString,
    style: ButtonStyle,
    disabled: bool,
    start_slot: Option<AnyElement>,
    end_slot: Option<AnyElement>,
}

impl Button {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            style: ButtonStyle::Subtle,
            disabled: false,
            start_slot: None,
            end_slot: None,
        }
    }

    pub fn start_slot(mut self, slot: impl IntoElement) -> Self {
        self.start_slot = Some(slot.into_any_element());
        self
    }

    pub fn end_slot(mut self, slot: impl IntoElement) -> Self {
        self.end_slot = Some(slot.into_any_element());
        self
    }

    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click<V: 'static>(
        self,
        theme: AppTheme,
        cx: &gpui::Context<V>,
        f: impl Fn(&mut V, &ClickEvent, &mut Window, &mut gpui::Context<V>) + 'static,
    ) -> Stateful<Div> {
        let disabled = self.disabled;

        self.render(theme)
            .when(!disabled, |this| this.on_click(cx.listener(f)))
    }

    pub fn render(self, theme: AppTheme) -> Stateful<Div> {
        let transparent = gpui::rgba(0x00000000);
        let outlined_border = with_alpha(
            theme.colors.text_muted,
            if theme.is_dark { 0.38 } else { 0.28 },
        );
        let (bg, hover_bg, active_bg, border, text) = match self.style {
            ButtonStyle::Filled => (
                theme.colors.accent,
                with_alpha(theme.colors.accent, 0.85),
                with_alpha(theme.colors.accent, 0.78),
                with_alpha(theme.colors.accent, 0.9),
                theme.colors.window_bg,
            ),
            ButtonStyle::Outlined => (
                transparent,
                with_alpha(theme.colors.hover, 0.65),
                theme.colors.active,
                outlined_border,
                theme.colors.text,
            ),
            ButtonStyle::Subtle => (
                transparent,
                with_alpha(theme.colors.hover, 0.65),
                theme.colors.active,
                transparent,
                theme.colors.text,
            ),
            ButtonStyle::Transparent => (
                transparent,
                with_alpha(theme.colors.hover, 0.55),
                theme.colors.active,
                transparent,
                theme.colors.text_muted,
            ),
            ButtonStyle::Danger => (
                with_alpha(theme.colors.danger, if theme.is_dark { 0.18 } else { 0.14 }),
                with_alpha(theme.colors.danger, if theme.is_dark { 0.26 } else { 0.20 }),
                with_alpha(theme.colors.danger, if theme.is_dark { 0.32 } else { 0.26 }),
                with_alpha(theme.colors.danger, if theme.is_dark { 0.42 } else { 0.32 }),
                theme.colors.text,
            ),
        };

        let label = self.label.to_string();
        let icon_only = looks_like_icon_button(&label);

        let mut inner = div().flex().items_center().gap_1();
        if let Some(start_slot) = self.start_slot {
            inner = inner.child(start_slot);
        }
        if !label.is_empty() {
            inner = inner.child(label);
        }
        if let Some(end_slot) = self.end_slot {
            inner = inner.child(end_slot);
        }

        let mut base = div()
            .id(self.id.clone())
            .tab_index(0)
            .h(px(CONTROL_HEIGHT_PX))
            .px(px(if icon_only {
                ICON_PAD_X_PX
            } else {
                CONTROL_PAD_X_PX
            }))
            .py(px(CONTROL_PAD_Y_PX))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme.radii.row))
            .bg(bg)
            .border_1()
            .border_color(border)
            .focus(move |s| {
                s.border_color(theme.colors.focus_ring)
                    .bg(theme.colors.focus_ring_bg)
            })
            .text_sm()
            .text_color(text)
            .cursor(CursorStyle::PointingHand)
            .child(inner);

        if self.disabled {
            base = base.opacity(0.5).cursor(CursorStyle::Arrow);
        } else {
            base = base
                .hover(move |s| s.bg(hover_bg))
                .active(move |s| s.bg(active_bg));
        }

        base
    }
}

fn looks_like_icon_button(label: &str) -> bool {
    matches!(label.trim(), "✕" | "＋" | "▾" | "≡" | "…" | "⋯" | "⟳" | "↻")
        || (label.chars().count() <= 2 && !label.chars().any(|c| c.is_alphanumeric()))
}

fn with_alpha(mut color: gpui::Rgba, alpha: f32) -> gpui::Rgba {
    color.a = alpha;
    color
}
