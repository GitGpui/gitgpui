use crate::{components, kit, theme::AppTheme, view, zed_port as zed};
use gpui::prelude::*;
use gpui::{Decorations, Tiling, div, px};

fn assert_no_panic(label: &str, f: impl FnOnce()) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err() {
        panic!("component build panicked: {label}");
    }
}

#[test]
fn builds_pure_components_without_panics() {
    let theme = AppTheme::zed_one_dark();

    assert_no_panic("components::pill", || {
        let _ = components::pill(theme, "Label", theme.colors.accent);
    });

    assert_no_panic("components::empty_state", || {
        let _ = components::empty_state(theme, "Title", "Message");
    });

    assert_no_panic("components::panel", || {
        let _ = components::panel(theme, "Panel", None, div().child("body"));
    });

    assert_no_panic("kit::Button render variants", || {
        let _ = kit::Button::new("k1", "Primary")
            .style(kit::ButtonStyle::Primary)
            .render(theme);
        let _ = kit::Button::new("k2", "Secondary")
            .style(kit::ButtonStyle::Secondary)
            .render(theme);
        let _ = kit::Button::new("k3", "Danger")
            .style(kit::ButtonStyle::Danger)
            .render(theme);
        let _ = kit::Button::new("k4", "Disabled")
            .style(kit::ButtonStyle::Secondary)
            .disabled(true)
            .render(theme);
    });

    assert_no_panic("zed::Button render variants", || {
        let _ = zed::Button::new("z1", "Filled")
            .style(zed::ButtonStyle::Filled)
            .render(theme);
        let _ = zed::Button::new("z2", "Outlined")
            .style(zed::ButtonStyle::Outlined)
            .render(theme);
        let _ = zed::Button::new("z3", "Subtle")
            .style(zed::ButtonStyle::Subtle)
            .render(theme);
        let _ = zed::Button::new("z4", "Disabled")
            .style(zed::ButtonStyle::Outlined)
            .disabled(true)
            .render(theme);
    });

    assert_no_panic("zed::SplitButton", || {
        let left = zed::Button::new("s1", "Left")
            .style(zed::ButtonStyle::Outlined)
            .render(theme);
        let right = zed::Button::new("s2", "Right")
            .style(zed::ButtonStyle::Outlined)
            .render(theme);
        let _ = zed::SplitButton::new(left, right)
            .style(zed::SplitButtonStyle::Outlined)
            .render(theme);
    });

    assert_no_panic("zed::Tab + TabBar", || {
        let tab = zed::Tab::new(("t", 1u64))
            .selected(true)
            .child(div().child("Repo"))
            .render(theme);
        let _ = zed::TabBar::new("tb").tab(tab).render(theme);
    });

    assert_no_panic("view::window_frame", || {
        let content = div().child("content").into_any_element();
        let _ = view::window_frame(theme, Decorations::Server, content);
        let _ = view::window_frame(
            theme,
            Decorations::Client {
                tiling: Tiling::default(),
            },
            div().child("content").into_any_element(),
        );
    });

    assert_no_panic("window-frame uses shadow/rounding", || {
        let _ = div()
            .rounded(px(theme.radii.panel))
            .shadow_lg()
            .border_1()
            .child("x");
    });
}

struct SmokeView {
    theme: AppTheme,
    input: gpui::Entity<kit::TextInput>,
}

impl SmokeView {
    fn new(window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> Self {
        let input = cx.new(|cx| {
            kit::TextInput::new(
                kit::TextInputOptions {
                    placeholder: "Enter…".into(),
                    multiline: false,
                },
                window,
                cx,
            )
        });

        Self {
            theme: AppTheme::zed_one_dark(),
            input,
        }
    }
}

impl gpui::Render for SmokeView {
    fn render(&mut self, window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let tabs = kit::Tabs::new(vec!["One".into(), "Two".into()])
            .selected(0)
            .render(theme, cx, |_this, _ix, _e, _w, _cx| {});

        let content = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(components::panel(theme, "Tabs", None, tabs))
            .child(components::panel(theme, "Input", None, self.input.clone()))
            .child(
                components::panel(
                    theme,
                    "Buttons",
                    None,
                    div()
                        .flex()
                        .gap_2()
                        .child(zed::Button::new("b1", "Primary").style(zed::ButtonStyle::Filled).render(theme))
                        .child(zed::Button::new("b2", "Secondary").style(zed::ButtonStyle::Outlined).render(theme)),
                ),
            )
            .into_any_element();

        view::window_frame(theme, window.window_decorations(), content)
    }
}

#[gpui::test]
fn smoke_view_renders_without_panicking(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| cx.new(|cx| SmokeView::new(window, cx)))
            .unwrap();
    });
}

#[gpui::test]
fn text_input_constructs_without_panicking(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            cx.new(|cx| {
                kit::TextInput::new(
                    kit::TextInputOptions {
                        placeholder: "Commit message…".into(),
                        multiline: false,
                    },
                    window,
                    cx,
                )
            })
        })
        .unwrap();
    });
}
