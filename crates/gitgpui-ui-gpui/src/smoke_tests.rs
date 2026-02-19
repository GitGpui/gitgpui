use crate::{theme::AppTheme, view, zed_port as zed};
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::{GitBackend, GitRepository, Result};
use gitgpui_state::model::RepoId;
use gitgpui_state::msg::Msg;
use gitgpui_state::store::AppStore;
use gpui::prelude::*;
use gpui::{
    ClipboardItem, Decorations, KeyBinding, Modifiers, MouseButton, ScrollHandle, Tiling, div, px,
};
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    let (view, cx) = cx.add_window_view(SmokeView::new);

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

#[gpui::test]
fn text_input_supports_shift_home_end_row_selection(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(SmokeView::new);

    cx.update(|window, app| {
        app.bind_keys([
            KeyBinding::new("left", crate::kit::Left, Some("TextInput")),
            KeyBinding::new("right", crate::kit::Right, Some("TextInput")),
            KeyBinding::new("shift-home", crate::kit::SelectHome, Some("TextInput")),
            KeyBinding::new("shift-end", crate::kit::SelectEnd, Some("TextInput")),
            KeyBinding::new("ctrl-c", crate::kit::Copy, Some("TextInput")),
        ]);

        let focus = view.update(app, |this, cx| this.input.read(cx).focus_handle());
        window.focus(&focus);

        view.update(app, |this, cx| {
            this.input
                .update(cx, |input, cx| input.set_text("abcde\n12345", cx));
        });

        let _ = window.draw(app);
    });

    cx.simulate_keystrokes("left left shift-home ctrl-c");
    assert_eq!(
        cx.read_from_clipboard().and_then(|item| item.text()),
        Some("123".into())
    );

    cx.simulate_keystrokes("right shift-end ctrl-c");
    assert_eq!(
        cx.read_from_clipboard().and_then(|item| item.text()),
        Some("45".into())
    );
}

#[gpui::test]
fn text_input_supports_shift_pageup_pagedown_selection(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(SmokeView::new);

    cx.update(|window, app| {
        app.bind_keys([
            KeyBinding::new("home", crate::kit::Home, Some("TextInput")),
            KeyBinding::new("left", crate::kit::Left, Some("TextInput")),
            KeyBinding::new("right", crate::kit::Right, Some("TextInput")),
            KeyBinding::new("shift-pageup", crate::kit::SelectPageUp, Some("TextInput")),
            KeyBinding::new(
                "shift-pagedown",
                crate::kit::SelectPageDown,
                Some("TextInput"),
            ),
            KeyBinding::new("ctrl-c", crate::kit::Copy, Some("TextInput")),
        ]);

        let focus = view.update(app, |this, cx| this.input.read(cx).focus_handle());
        window.focus(&focus);

        view.update(app, |this, cx| {
            this.input
                .update(cx, |input, cx| input.set_text("abcde\n12345\nxyz", cx));
        });

        let _ = window.draw(app);
    });

    // Move the cursor to the start of the second line.
    cx.simulate_keystrokes("home left home");

    cx.simulate_keystrokes("shift-pageup ctrl-c");
    assert_eq!(
        cx.read_from_clipboard().and_then(|item| item.text()),
        Some("abcde\n".into())
    );

    // Collapse selection back to the start of the second line.
    cx.simulate_keystrokes("right");

    cx.simulate_keystrokes("shift-pagedown ctrl-c");
    assert_eq!(
        cx.read_from_clipboard().and_then(|item| item.text()),
        Some("12345\n".into())
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

fn repo_tab_selector(repo_id: RepoId) -> &'static str {
    Box::leak(format!("repo_tab_{}", repo_id.0).into_boxed_str())
}

fn worktrees_spinner_selector(repo_id: RepoId) -> &'static str {
    Box::leak(format!("worktrees_spinner_{}", repo_id.0).into_boxed_str())
}

fn wait_for_repo_count(store: &AppStore, expected: usize) -> gitgpui_state::model::AppState {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let state = store.snapshot();
        if state.repos.len() == expected {
            return state;
        }
        if Instant::now() >= deadline {
            panic!(
                "timed out waiting for store repos len {expected}, got {}",
                state.repos.len()
            );
        }
        std::thread::yield_now();
    }
}

fn wait_for_repo_order(store: &AppStore, expected: &[RepoId]) {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let state = store.snapshot();
        let got = state.repos.iter().map(|r| r.id).collect::<Vec<_>>();
        if got == expected {
            return;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for repo order {expected:?}, got {got:?}");
        }
        std::thread::yield_now();
    }
}

fn restore_session_and_draw(
    cx: &mut gpui::VisualTestContext,
    store: &AppStore,
    repos: Vec<PathBuf>,
) -> Vec<RepoId> {
    store.dispatch(Msg::RestoreSession {
        open_repos: repos.clone(),
        active_repo: repos.first().cloned(),
    });

    let state = wait_for_repo_count(store, repos.len());
    let ids = state.repos.iter().map(|r| r.id).collect::<Vec<_>>();
    let selectors = ids
        .iter()
        .copied()
        .map(repo_tab_selector)
        .collect::<Vec<_>>();

    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        cx.update(|window, app| {
            let _ = window.draw(app);
        });

        if selectors
            .iter()
            .all(|selector| cx.debug_bounds(selector).is_some())
        {
            return ids;
        }

        if Instant::now() >= deadline {
            panic!(
                "timed out waiting for repo tabs to render; missing selectors: {:?}",
                selectors
                    .into_iter()
                    .filter(|selector| cx.debug_bounds(selector).is_none())
                    .collect::<Vec<_>>()
            );
        }

        cx.run_until_parked();
        std::thread::yield_now();
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
fn repo_tabs_can_drag_reorder_by_right_half(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new(Arc::new(TestBackend));
    let store_for_test = store.clone();
    let (_view, cx) = cx.add_window_view(|window, cx| {
        crate::view::GitGpuiView::new(store, events, None, window, cx)
    });

    let base = std::env::temp_dir().join(format!(
        "gitgpui_ui_test_repo_tabs_right_{}",
        std::process::id()
    ));
    let repo_ids = restore_session_and_draw(
        cx,
        &store_for_test,
        vec![base.join("repo1"), base.join("repo2"), base.join("repo3")],
    );

    let dragged = repo_ids[0];
    let target = repo_ids[1];
    let expected = vec![repo_ids[1], repo_ids[0], repo_ids[2]];

    let dragged_bounds = cx
        .debug_bounds(repo_tab_selector(dragged))
        .expect("expected dragged repo tab bounds");
    let target_bounds = cx
        .debug_bounds(repo_tab_selector(target))
        .expect("expected target repo tab bounds");

    let start = dragged_bounds.center();
    cx.simulate_mouse_move(start, None, Modifiers::default());
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(
        gpui::point(start.x + px(10.0), start.y),
        Some(MouseButton::Left),
        Modifiers::default(),
    );

    let drop = gpui::point(target_bounds.right() - px(5.0), target_bounds.center().y);
    cx.simulate_mouse_move(drop, Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_up(drop, MouseButton::Left, Modifiers::default());

    wait_for_repo_order(&store_for_test, &expected);
}

#[gpui::test]
fn repo_tabs_can_drag_reorder_by_left_half(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new(Arc::new(TestBackend));
    let store_for_test = store.clone();
    let (_view, cx) = cx.add_window_view(|window, cx| {
        crate::view::GitGpuiView::new(store, events, None, window, cx)
    });

    let base = std::env::temp_dir().join(format!(
        "gitgpui_ui_test_repo_tabs_left_{}",
        std::process::id()
    ));
    let repo_ids = restore_session_and_draw(
        cx,
        &store_for_test,
        vec![base.join("repo1"), base.join("repo2"), base.join("repo3")],
    );

    let dragged = repo_ids[2];
    let target = repo_ids[1];
    let expected = vec![repo_ids[0], repo_ids[2], repo_ids[1]];

    let dragged_bounds = cx
        .debug_bounds(repo_tab_selector(dragged))
        .expect("expected dragged repo tab bounds");
    let target_bounds = cx
        .debug_bounds(repo_tab_selector(target))
        .expect("expected target repo tab bounds");

    let start = dragged_bounds.center();
    cx.simulate_mouse_move(start, None, Modifiers::default());
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(
        gpui::point(start.x - px(10.0), start.y),
        Some(MouseButton::Left),
        Modifiers::default(),
    );

    let drop = gpui::point(target_bounds.left() + px(5.0), target_bounds.center().y);
    cx.simulate_mouse_move(drop, Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_up(drop, MouseButton::Left, Modifiers::default());

    wait_for_repo_order(&store_for_test, &expected);
}

#[gpui::test]
fn repo_tabs_drop_on_self_is_noop(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new(Arc::new(TestBackend));
    let store_for_test = store.clone();
    let (_view, cx) = cx.add_window_view(|window, cx| {
        crate::view::GitGpuiView::new(store, events, None, window, cx)
    });

    let base = std::env::temp_dir().join(format!(
        "gitgpui_ui_test_repo_tabs_self_{}",
        std::process::id()
    ));
    let repo_ids = restore_session_and_draw(
        cx,
        &store_for_test,
        vec![base.join("repo1"), base.join("repo2"), base.join("repo3")],
    );

    let dragged = repo_ids[1];
    let dragged_bounds = cx
        .debug_bounds(repo_tab_selector(dragged))
        .expect("expected dragged repo tab bounds");

    let start = dragged_bounds.center();
    cx.simulate_mouse_move(start, None, Modifiers::default());
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());

    let moved_x = (start.x + px(10.0)).min(dragged_bounds.right() - px(1.0));
    let moved = gpui::point(moved_x, start.y);
    cx.simulate_mouse_move(moved, Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_up(moved, MouseButton::Left, Modifiers::default());

    let got = store_for_test
        .snapshot()
        .repos
        .iter()
        .map(|r| r.id)
        .collect::<Vec<_>>();
    assert_eq!(got, repo_ids);
}

#[gpui::test]
fn worktrees_section_shows_spinner_while_removing_worktree(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new(Arc::new(TestBackend));
    let store_for_test = store.clone();
    let (_view, cx) = cx.add_window_view(|window, cx| {
        crate::view::GitGpuiView::new(store, events, None, window, cx)
    });

    let base = std::env::temp_dir().join(format!(
        "gitgpui_ui_test_worktrees_spinner_{}",
        std::process::id()
    ));
    let repo_ids = restore_session_and_draw(cx, &store_for_test, vec![base.join("repo1")]);
    let repo_id = repo_ids[0];

    store_for_test.dispatch(Msg::RemoveWorktree {
        repo_id,
        path: base.join("repo1").join("worktree_to_remove"),
    });

    let selector = worktrees_spinner_selector(repo_id);
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        cx.update(|window, app| {
            let _ = window.draw(app);
        });

        if cx.debug_bounds(selector).is_some() {
            break;
        }

        if Instant::now() >= deadline {
            panic!("timed out waiting for worktrees spinner to render");
        }

        cx.run_until_parked();
        std::thread::yield_now();
    }
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
