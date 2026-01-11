use crate::theme::AppTheme;
use gpui::{ClickEvent, Div, SharedString, Stateful, Window, div, px};
use gpui::prelude::*;
use std::hash::{Hash as _, Hasher as _};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonStyle {
    Filled,
    Outlined,
    Subtle,
    Transparent,
}

pub struct Button {
    id: SharedString,
    label: SharedString,
    style: ButtonStyle,
    disabled: bool,
}

impl Button {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            style: ButtonStyle::Subtle,
            disabled: false,
        }
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
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.id.hash(&mut hasher);
        let id_hash = hasher.finish();
        let disabled = self.disabled;

        self.render(theme)
            .id(("zed_btn", id_hash))
            .when(!disabled, |this| this.on_click(cx.listener(f)))
    }

    pub fn render(self, theme: AppTheme) -> Div {
        let transparent = gpui::rgba(0x00000000);
        let (bg, hover_bg, border, text) = match self.style {
            ButtonStyle::Filled => (
                theme.colors.accent,
                with_alpha(theme.colors.accent, 0.85),
                with_alpha(theme.colors.accent, 0.9),
                theme.colors.window_bg,
            ),
            ButtonStyle::Outlined => (
                transparent,
                with_alpha(theme.colors.hover, 0.65),
                theme.colors.border,
                theme.colors.text,
            ),
            ButtonStyle::Subtle => (
                transparent,
                with_alpha(theme.colors.hover, 0.65),
                transparent,
                theme.colors.text,
            ),
            ButtonStyle::Transparent => (
                transparent,
                transparent,
                transparent,
                theme.colors.text_muted,
            ),
        };

        let mut base = div()
            .px_3()
            .py_2()
            .rounded(px(theme.radii.row))
            .bg(bg)
            .border_1()
            .border_color(border)
            .text_sm()
            .text_color(text)
            .child(self.label);

        if self.disabled {
            base = base.opacity(0.5);
        } else {
            base = base.hover(move |s| s.bg(hover_bg));
        }

        base
    }
}

fn with_alpha(mut color: gpui::Rgba, alpha: f32) -> gpui::Rgba {
    color.a = alpha;
    color
}
