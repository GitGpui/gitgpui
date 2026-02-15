use crate::{theme::AppTheme, view, zed_port as zed};
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::{GitBackend, GitRepository, Result};
use gitgpui_state::store::AppStore;
use gpui::prelude::*;
use gpui::{
    ClipboardItem, Decorations, KeyBinding, Modifiers, MouseButton, ScrollHandle, Tiling, div, px,
};
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
        assert_no_panic("zed::pill", || {
            let _ = zed::pill(theme, "Label", theme.colors.accent);
        });

        assert_no_panic("zed::empty_state", || {
            let _ = zed::empty_state(theme, "Title", "Message");
        });

        assert_no_panic("zed::panel", || {
            let _ = zed::panel(theme, "Panel", None, div().child("body"));
        });

        assert_no_panic("zed::diff_stat", || {
            let _ = zed::diff_stat(theme, 12, 4);
        });

        assert_no_panic("zed::toast", || {
            let _ = zed::toast(theme, zed::ToastKind::Success, "Hello");
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
    input: gpui::Entity<zed::TextInput>,
}

impl SmokeView {
    fn new(window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> Self {
        let input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "Enter".into(),
                    multiline: false,
                    read_only: false,
                    chromeless: false,
                    soft_wrap: false,
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
        _cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let tabs = zed::TabBar::new("smoke_tabs")
            .tab(
                zed::Tab::new(("t", 0u64))
                    .selected(true)
                    .child(div().child("One"))
                    .render(theme),
            )
            .tab(
                zed::Tab::new(("t", 1u64))
                    .selected(false)
                    .child(div().child("Two"))
                    .render(theme),
            )
            .render(theme);

        let content = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(zed::panel(theme, "Tabs", None, tabs))
            .child(zed::panel(theme, "Input", None, self.input.clone()))
            .child(zed::panel(
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
                zed::TextInput::new(
                    zed::TextInputOptions {
                        placeholder: "Commit message".into(),
                        multiline: false,
                        read_only: false,
                        chromeless: false,
                        soft_wrap: false,
                    },
                    window,
                    cx,
                )
            })
        })
        .unwrap();
    });
}

#[gpui::test]
fn text_input_supports_basic_clipboard_and_word_shortcuts(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| SmokeView::new(window, cx));

    cx.update(|window, app| {
        app.bind_keys([
            KeyBinding::new("ctrl-a", crate::kit::SelectAll, Some("TextInput")),
            KeyBinding::new("ctrl-c", crate::kit::Copy, Some("TextInput")),
            KeyBinding::new("ctrl-x", crate::kit::Cut, Some("TextInput")),
            KeyBinding::new("ctrl-v", crate::kit::Paste, Some("TextInput")),
            KeyBinding::new(
                "ctrl-shift-left",
                crate::kit::SelectWordLeft,
                Some("TextInput"),
            ),
        ]);

        let focus = view.update(app, |this, cx| this.input.read(cx).focus_handle());
        window.focus(&focus);

        view.update(app, |this, cx| {
            this.input
                .update(cx, |input, cx| input.set_text("hello world", cx));
        });
    });

    cx.simulate_keystrokes("ctrl-a ctrl-c");
    assert_eq!(
        cx.read_from_clipboard().and_then(|item| item.text()),
        Some("hello world".into())
    );

    cx.simulate_keystrokes("ctrl-x");
    let text = cx.update(|_window, app| view.read(app).input.read(app).text().to_string());
    assert_eq!(text, "");

    cx.write_to_clipboard(ClipboardItem::new_string("abc".to_string()));
    cx.simulate_keystrokes("ctrl-v");
    let text = cx.update(|_window, app| view.read(app).input.read(app).text().to_string());
    assert_eq!(text, "abc");

    cx.update(|window, app| {
        let focus = view.update(app, |this, cx| this.input.read(cx).focus_handle());
        window.focus(&focus);
        view.update(app, |this, cx| {
            this.input
                .update(cx, |input, cx| input.set_text("hello world", cx));
        });
    });
    cx.simulate_keystrokes("ctrl-shift-left ctrl-c");
    assert_eq!(
        cx.read_from_clipboard().and_then(|item| item.text()),
        Some("world".into())
    );
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

struct PanelLayoutTestView {
    theme: AppTheme,
    handle: gpui::UniformListScrollHandle,
}

impl PanelLayoutTestView {
    fn new() -> Self {
        Self {
            theme: AppTheme::zed_ayu_dark(),
            handle: gpui::UniformListScrollHandle::default(),
        }
    }
}

impl gpui::Render for PanelLayoutTestView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;

        let header = div().id("diff_header").h(px(24.0)).child("Header");
        let list = gpui::uniform_list(
            "diff_list",
            50,
            cx.processor(
                |_this: &mut PanelLayoutTestView,
                 range: std::ops::Range<usize>,
                 _window: &mut gpui::Window,
                 _cx: &mut gpui::Context<PanelLayoutTestView>| {
                    range
                        .map(|ix| {
                            div()
                                .id(ix)
                                .h(px(20.0))
                                .px_2()
                                .child(format!("Row {ix}"))
                                .into_any_element()
                        })
                        .collect::<Vec<_>>()
                },
            ),
        )
        .h_full()
        .track_scroll(self.handle.clone());

        let scroll_handle = self.handle.0.borrow().base_handle.clone();

        let body =
            div()
                .id("diff_body")
                .debug_selector(|| "diff_body".to_string())
                .flex()
                .flex_col()
                .h_full()
                .child(header)
                .child(div().flex_1().min_h(px(0.0)).relative().child(list).child(
                    zed::Scrollbar::new("diff_scrollbar_test", scroll_handle).render(theme),
                ));

        div()
            .size_full()
            .bg(theme.colors.window_bg)
            .child(zed::panel(theme, "Panel", None, body).flex_1().h_full())
    }
}

#[gpui::test]
fn panel_allows_flex_body_to_have_height(cx: &mut gpui::TestAppContext) {
    let (_view, cx) = cx.add_window_view(|_window, _cx| PanelLayoutTestView::new());
    cx.update(|window, app| {
        let _ = window.draw(app);
    });
    let bounds = cx
        .debug_bounds("diff_body")
        .expect("expected diff_body to be painted");
    assert!(bounds.size.height > px(50.0));
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
            !view.read(app).is_popover_open(app),
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
        assert!(
            view.read(app).is_popover_open(app),
            "expected popover to open"
        );
    });

    // Click somewhere in the main content area (outside the popover).
    let outside = gpui::point(px(900.0), px(700.0));
    cx.simulate_mouse_move(outside, None, Modifiers::default());
    cx.simulate_mouse_down(outside, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();

    cx.update(|_window, app| {
        assert!(
            !view.read(app).is_popover_open(app),
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
                    zed::Scrollbar::new("test_scrollbar", self.handle.clone())
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
            zed::Scrollbar::thumb_visible_for_test(handle, px(120.0)),
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
            !zed::Scrollbar::thumb_visible_for_test(handle, px(120.0)),
            "expected scrollbar thumb to be hidden when not overflowing"
        );
    });
}

#[gpui::test]
fn scrollbar_allows_dragging_thumb_to_scroll(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(|_window, _cx| ScrollbarTestView::new(50));
    cx.update(|window, app| {
        let _ = window.draw(app);
    });

    let bounds = cx
        .debug_bounds("test_scrollbar")
        .expect("expected test_scrollbar in debug bounds");

    let start = gpui::point(bounds.right() - px(2.0), bounds.top() + px(6.0));
    cx.simulate_mouse_move(start, None, Modifiers::default());
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());

    // First move crosses the drag threshold and starts the drag.
    cx.simulate_mouse_move(
        gpui::point(start.x, start.y + px(5.0)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    // Second move should scroll.
    cx.simulate_mouse_move(
        gpui::point(start.x, start.y + px(60.0)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        gpui::point(start.x, start.y + px(60.0)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.run_until_parked();

    cx.update(|window, app| {
        let _ = window.draw(app);
        let offset_y = view.read(app).handle.offset().y;
        assert!(
            offset_y < px(0.0),
            "expected scrollbar drag to scroll (offset should become negative)"
        );
    });
}

struct ScrollbarMismatchedBoundsView {
    theme: AppTheme,
    handle: ScrollHandle,
    rows: usize,
}

impl ScrollbarMismatchedBoundsView {
    fn new(rows: usize) -> Self {
        Self {
            theme: AppTheme::zed_ayu_dark(),
            handle: ScrollHandle::new(),
            rows,
        }
    }
}

impl gpui::Render for ScrollbarMismatchedBoundsView {
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

        // Render the scrollbar in a *larger* container than the scroll surface to ensure the
        // scrollbar uses its own bounds (not the scroll handle's bounds) for hit-testing/metrics.
        div().size_full().bg(theme.colors.window_bg).child(
            div()
                .id("outer_scrollbar_container")
                .relative()
                .w(px(200.0))
                .h(px(200.0))
                .child(
                    div()
                        .id("inner_scroll_surface")
                        .relative()
                        .w_full()
                        .h(px(120.0))
                        .overflow_y_scroll()
                        .track_scroll(&self.handle)
                        .child(div().flex().flex_col().children(rows)),
                )
                .child(
                    zed::Scrollbar::new("outer_scrollbar", self.handle.clone())
                        .debug_selector("outer_scrollbar")
                        .render(theme),
                ),
        )
    }
}

#[gpui::test]
fn scrollbar_track_uses_own_bounds_when_larger_than_surface(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(|_window, _cx| ScrollbarMismatchedBoundsView::new(100));
    cx.update(|window, app| {
        let _ = window.draw(app);
    });

    let bounds = cx
        .debug_bounds("outer_scrollbar")
        .expect("expected outer_scrollbar in debug bounds");

    // Scrollbar track uses a 4px margin at top/bottom.
    let click = gpui::point(bounds.right() - px(2.0), bounds.bottom() - px(6.0));
    cx.simulate_mouse_move(click, None, Modifiers::default());
    cx.simulate_mouse_down(click, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(click, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();

    cx.update(|window, app| {
        let _ = window.draw(app);
        let offset_y = view.read(app).handle.offset().y;
        assert!(
            offset_y != px(0.0),
            "expected track click near bottom to scroll even when scrollbar is taller than the scroll surface"
        );
    });
}
