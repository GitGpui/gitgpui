//! Small reusable UI building blocks.
//!
//! These are kept in `zed_port` so the rest of the UI can consistently depend on
//! a single "Zed-style" component surface without pulling in Zed's crate graph.

use crate::theme::AppTheme;
use gpui::prelude::*;
use gpui::{Div, FontWeight, IntoElement, SharedString, div, px};

pub fn panel(
    theme: AppTheme,
    title: impl Into<SharedString>,
    subtitle: Option<SharedString>,
    content: impl IntoElement,
) -> Div {
    let mut header = div()
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(theme.colors.border)
        .child(div().text_sm().font_weight(FontWeight::BOLD).child(title.into()));

    if let Some(subtitle) = subtitle {
        header = header.child(
            div()
                .text_xs()
                .text_color(theme.colors.text_muted)
                .child(subtitle),
        );
    }

    div()
        .flex()
        .flex_col()
        .bg(theme.colors.surface_bg)
        .border_1()
        .border_color(theme.colors.border)
        .rounded(px(theme.radii.panel))
        .overflow_hidden()
        .child(header)
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.0))
                .p_3()
                .child(div().flex_1().min_h(px(0.0)).child(content)),
        )
}

pub fn pill(theme: AppTheme, label: impl Into<SharedString>, bg: gpui::Rgba) -> Div {
    div()
        .px_2()
        .py_1()
        .rounded(px(theme.radii.pill))
        .bg(bg)
        .text_xs()
        .text_color(theme.colors.text)
        .child(label.into())
}

pub fn key_value(theme: AppTheme, key: impl Into<SharedString>, value: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            div()
                .text_sm()
                .text_color(theme.colors.text_muted)
                .child(key.into()),
        )
        .child(div().text_sm().child(value.into()))
}

pub fn empty_state(
    theme: AppTheme,
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
) -> Div {
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .py_6()
        .child(div().text_lg().child(title.into()))
        .child(
            div()
                .text_sm()
                .text_color(theme.colors.text_muted)
                .child(message.into()),
        )
}
