//! Context menu building blocks (Zed-inspired styling).

use crate::theme::AppTheme;
use gpui::prelude::*;
use gpui::{Div, ElementId, FontWeight, SharedString, Stateful, div, px};

use super::CONTROL_HEIGHT_MD_PX;

pub fn context_menu(theme: AppTheme, content: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .text_color(theme.colors.text)
        .child(content)
}

pub fn context_menu_header(theme: AppTheme, title: impl Into<SharedString>) -> Div {
    div()
        .px_2()
        .py_1()
        .text_xs()
        .text_color(theme.colors.text_muted)
        .child(title.into())
}

pub fn context_menu_label(theme: AppTheme, text: impl Into<SharedString>) -> Div {
    div()
        .px_2()
        .pb_1()
        .text_sm()
        .text_color(theme.colors.text)
        .line_clamp(2)
        .child(text.into())
}

pub fn context_menu_separator(theme: AppTheme) -> Div {
    div().border_t_1().border_color(theme.colors.border)
}

pub fn context_menu_entry(
    id: impl Into<ElementId>,
    theme: AppTheme,
    selected: bool,
    disabled: bool,
    icon: Option<SharedString>,
    label: impl Into<SharedString>,
    shortcut: Option<SharedString>,
    has_submenu: bool,
) -> Stateful<Div> {
    let label: SharedString = label.into();

    let mut row = div()
        .id(id)
        .h(px(CONTROL_HEIGHT_MD_PX))
        .w_full()
        .px_2()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .rounded(px(theme.radii.row))
        .text_color(theme.colors.text)
        .when(selected, |s| s.bg(theme.colors.hover))
        .hover(move |s| s.bg(theme.colors.hover))
        .active(move |s| s.bg(theme.colors.active))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .flex_1()
                .min_w(px(0.0))
                .child(
                    div()
                        .w(px(16.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when_some(icon, |this, icon| {
                            this.child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.colors.text_muted)
                                    .child(icon),
                            )
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .text_sm()
                        .line_clamp(1)
                        .child(label),
                ),
        );

    let mut end = div()
        .flex()
        .items_center()
        .gap_2()
        .font_family("monospace")
        .text_xs()
        .text_color(theme.colors.text_muted);

    if let Some(shortcut) = shortcut {
        end = end.child(shortcut);
    }
    if has_submenu {
        end = end.child("›");
    }
    row = row.child(end);

    if disabled {
        row = row.text_color(theme.colors.text_muted);
    }

    row
}
