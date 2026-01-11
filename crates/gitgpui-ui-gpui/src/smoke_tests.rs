use crate::{components, kit, theme::AppTheme, view, zed_port as zed};
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::{GitBackend, GitRepository, Result};
use gitgpui_state::store::AppStore;
use gpui::prelude::*;
use gpui::{Decorations, Modifiers, MouseButton, ScrollHandle, Tiling, div, px};
use std::path::Path;
use std::sync::Arc;

fn assert_no_panic(label: &str, f: impl FnOnce()) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err() {
        panic!("component build panicked: {label}");
    }
}

#[test]
fn builds_pure_components_without_panics() {
    for theme in [AppTheme::zed_ayu_dark(), AppTheme::zed_one_light()] {
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
            theme: AppTheme::zed_ayu_dark(),
            input,
        }
    }
}

impl gpui::Render for SmokeView {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
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
            .child(components::panel(
                theme,
                "Buttons",
                None,
                div()
                    .flex()
                    .gap_2()
                    .child(
                        zed::Button::new("b1", "Primary")
                            .style(zed::ButtonStyle::Filled)
                            .render(theme),
                    )
                    .child(
                        zed::Button::new("b2", "Secondary")
                            .style(zed::ButtonStyle::Outlined)
                            .render(theme),
                    ),
            ))
            .into_any_element();

        view::window_frame(theme, window.window_decorations(), content)
    }
}

#[gpui::test]
fn smoke_view_renders_without_panicking(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            cx.new(|cx| SmokeView::new(window, cx))
        })
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

struct TestBackend;

impl GitBackend for TestBackend {
    fn open(&self, _workdir: &Path) -> Result<Arc<dyn GitRepository>> {
        Err(Error::new(ErrorKind::Unsupported(
            "Test backend does not open repositories",
        )))
    }
}

#[gpui::test]
fn gitgpui_view_renders_without_panicking(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let (store, events) = AppStore::new(Arc::new(TestBackend));
        cx.open_window(Default::default(), |window, cx| {
            cx.new(|cx| crate::view::GitGpuiView::new(store, events, None, window, cx))
        })
        .unwrap();
    });
}

#[gpui::test]
fn popover_is_clickable_above_content(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new(Arc::new(TestBackend));
    let (view, cx) = cx.add_window_view(|window, cx| {
        crate::view::GitGpuiView::new(store, events, None, window, cx)
    });

    // Open the repo picker dropdown in the action bar, which should overlay the rest of the UI.
    let picker_bounds = cx
        .debug_bounds("repo_picker")
        .expect("expected repo_picker in debug bounds");
    cx.simulate_mouse_move(picker_bounds.center(), None, Modifiers::default());
    cx.simulate_mouse_down(
        picker_bounds.center(),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        picker_bounds.center(),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.run_until_parked();
    cx.update(|window, app| {
        let _ = window.draw(app);
    });

    let close_bounds = cx
        .debug_bounds("repo_popover_close")
        .expect("expected repo_popover_close in debug bounds");
    cx.simulate_mouse_move(close_bounds.center(), None, Modifiers::default());
    cx.simulate_mouse_down(
        close_bounds.center(),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        close_bounds.center(),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.run_until_parked();
    cx.update(|window, app| {
        let _ = window.draw(app);
    });

    cx.update(|_window, app| {
        assert!(
            !view.read(app).is_popover_open(),
            "expected popover to close on click"
        );
    });
}

#[gpui::test]
fn popover_closes_when_clicking_outside(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new(Arc::new(TestBackend));
    let (view, cx) = cx.add_window_view(|window, cx| {
        crate::view::GitGpuiView::new(store, events, None, window, cx)
    });

    let picker_bounds = cx
        .debug_bounds("repo_picker")
        .expect("expected repo_picker in debug bounds");
    cx.simulate_mouse_move(picker_bounds.center(), None, Modifiers::default());
    cx.simulate_mouse_down(
        picker_bounds.center(),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        picker_bounds.center(),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.run_until_parked();

    cx.update(|_window, app| {
        assert!(view.read(app).is_popover_open(), "expected popover to open");
    });

    // Click somewhere in the main content area (outside the popover).
    let outside = gpui::point(px(900.0), px(700.0));
    cx.simulate_mouse_move(outside, None, Modifiers::default());
    cx.simulate_mouse_down(outside, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();

    cx.update(|_window, app| {
        assert!(
            !view.read(app).is_popover_open(),
            "expected popover to close when clicking outside"
        );
    });
}

struct ScrollbarTestView {
    theme: AppTheme,
    handle: ScrollHandle,
    rows: usize,
}

impl ScrollbarTestView {
    fn new(rows: usize) -> Self {
        Self {
            theme: AppTheme::zed_ayu_dark(),
            handle: ScrollHandle::new(),
            rows,
        }
    }
}

impl gpui::Render for ScrollbarTestView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let rows = (0..self.rows)
            .map(|ix| {
                div()
                    .id(ix)
                    .h(px(20.0))
                    .px_2()
                    .child(format!("Row {ix}"))
                    .into_any_element()
            })
            .collect::<Vec<_>>();

        div().size_full().bg(theme.colors.window_bg).child(
            div()
                .id("scroll_container")
                .relative()
                .w(px(200.0))
                .h(px(120.0))
                .overflow_y_scroll()
                .track_scroll(&self.handle)
                .child(div().flex().flex_col().children(rows))
                .child(
                    kit::Scrollbar::new("test_scrollbar", self.handle.clone())
                        .debug_selector("test_scrollbar")
                        .render(theme),
                ),
        )
    }
}

#[gpui::test]
fn scrollbar_thumb_visible_when_overflowing(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(|_window, _cx| ScrollbarTestView::new(50));
    cx.update(|window, app| {
        let _ = window.draw(app);
    });
    cx.update(|_window, app| {
        let handle = &view.read(app).handle;
        assert!(
            kit::Scrollbar::thumb_visible_for_test(handle, px(120.0)),
            "expected scrollbar thumb to be visible when overflowing"
        );
    });
}

#[gpui::test]
fn scrollbar_thumb_hidden_when_not_overflowing(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(|_window, _cx| ScrollbarTestView::new(2));
    cx.update(|window, app| {
        let _ = window.draw(app);
    });
    cx.update(|_window, app| {
        let handle = &view.read(app).handle;
        assert!(
            !kit::Scrollbar::thumb_visible_for_test(handle, px(120.0)),
            "expected scrollbar thumb to be hidden when not overflowing"
        );
    });
}
