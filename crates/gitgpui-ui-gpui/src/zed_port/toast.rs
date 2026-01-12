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
    let (bg, border) = match kind {
        ToastKind::Info => (
            with_alpha(theme.colors.accent, if theme.is_dark { 0.10 } else { 0.08 }),
            with_alpha(theme.colors.accent, if theme.is_dark { 0.25 } else { 0.18 }),
        ),
        ToastKind::Success => (
            with_alpha(theme.colors.success, if theme.is_dark { 0.12 } else { 0.10 }),
            with_alpha(theme.colors.success, if theme.is_dark { 0.28 } else { 0.20 }),
        ),
        ToastKind::Error => (
            with_alpha(theme.colors.danger, if theme.is_dark { 0.12 } else { 0.10 }),
            with_alpha(theme.colors.danger, if theme.is_dark { 0.28 } else { 0.20 }),
        ),
    };

    div()
        .min_w(px(260.0))
        .max_w(px(520.0))
        .px_3()
        .py_2()
        .bg(bg)
        .border_1()
        .border_color(border)
        .rounded(px(theme.radii.panel))
        .shadow_sm()
        .text_sm()
        .child(message)
}

fn with_alpha(mut color: gpui::Rgba, alpha: f32) -> gpui::Rgba {
    color.a = alpha;
    color
}
