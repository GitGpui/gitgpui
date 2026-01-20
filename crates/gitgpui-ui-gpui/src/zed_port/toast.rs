use crate::theme::AppTheme;
use gpui::prelude::*;
use gpui::{Div, SharedString, div, px};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

pub fn toast(theme: AppTheme, kind: ToastKind, message: impl Into<SharedString>) -> Div {
    let message: SharedString = message.into();
    let (accent, bg, border) = match kind {
        ToastKind::Info => (
            theme.colors.accent,
            with_alpha(
                theme.colors.surface_bg_elevated,
                if theme.is_dark { 0.96 } else { 0.98 },
            ),
            with_alpha(theme.colors.border, if theme.is_dark { 0.9 } else { 1.0 }),
        ),
        ToastKind::Success => (
            theme.colors.success,
            with_alpha(
                theme.colors.surface_bg_elevated,
                if theme.is_dark { 0.96 } else { 0.98 },
            ),
            with_alpha(theme.colors.border, if theme.is_dark { 0.9 } else { 1.0 }),
        ),
        ToastKind::Error => (
            theme.colors.danger,
            with_alpha(
                theme.colors.surface_bg_elevated,
                if theme.is_dark { 0.96 } else { 0.98 },
            ),
            with_alpha(theme.colors.border, if theme.is_dark { 0.9 } else { 1.0 }),
        ),
    };

    let accent = with_alpha(accent, if theme.is_dark { 0.85 } else { 0.75 });

    div()
        .min_w(px(260.0))
        .max_w(px(520.0))
        .flex()
        .items_center()
        .gap_2()
        .bg(bg)
        .border_1()
        .border_color(border)
        .rounded(px(theme.radii.panel))
        .shadow_sm()
        .text_sm()
        .child(div().w(px(3.0)).h(px(18.0)).bg(accent).rounded(px(2.0)))
        .child(div().flex_1().px_2().py_1().child(message))
}

fn with_alpha(mut color: gpui::Rgba, alpha: f32) -> gpui::Rgba {
    color.a = alpha;
    color
}
