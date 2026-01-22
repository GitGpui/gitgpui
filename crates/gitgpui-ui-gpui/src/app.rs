use crate::assets::GitGpuiAssets;
use crate::view::GitGpuiView;
use gitgpui_core::services::GitBackend;
use gitgpui_state::session;
use gitgpui_state::store::AppStore;
use gpui::{
    App, AppContext, Application, Bounds, KeyBinding, TitlebarOptions, WindowBounds,
    WindowDecorations, WindowOptions, point, px, size,
};
use std::sync::Arc;

pub fn run(backend: Arc<dyn GitBackend>) {
    let initial_path = std::env::args_os().nth(1).map(std::path::PathBuf::from);

    Application::new()
        .with_assets(GitGpuiAssets)
        .run(move |cx: &mut App| {
            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            cx.bind_keys([
                KeyBinding::new("backspace", crate::kit::Backspace, Some("TextInput")),
                KeyBinding::new("delete", crate::kit::Delete, Some("TextInput")),
                KeyBinding::new("left", crate::kit::Left, Some("TextInput")),
                KeyBinding::new("right", crate::kit::Right, Some("TextInput")),
                KeyBinding::new("shift-left", crate::kit::SelectLeft, Some("TextInput")),
                KeyBinding::new("shift-right", crate::kit::SelectRight, Some("TextInput")),
                KeyBinding::new("home", crate::kit::Home, Some("TextInput")),
                KeyBinding::new("end", crate::kit::End, Some("TextInput")),
                KeyBinding::new("cmd-a", crate::kit::SelectAll, Some("TextInput")),
                KeyBinding::new("ctrl-a", crate::kit::SelectAll, Some("TextInput")),
                KeyBinding::new("cmd-v", crate::kit::Paste, Some("TextInput")),
                KeyBinding::new("ctrl-v", crate::kit::Paste, Some("TextInput")),
                KeyBinding::new("cmd-c", crate::kit::Copy, Some("TextInput")),
                KeyBinding::new("ctrl-c", crate::kit::Copy, Some("TextInput")),
                KeyBinding::new("cmd-x", crate::kit::Cut, Some("TextInput")),
                KeyBinding::new("ctrl-x", crate::kit::Cut, Some("TextInput")),
                #[cfg(target_os = "macos")]
                KeyBinding::new(
                    "ctrl-cmd-space",
                    crate::kit::ShowCharacterPalette,
                    Some("TextInput"),
                ),
            ]);

            const WINDOW_MIN_WIDTH_PX: f32 = 820.0;
            const WINDOW_MIN_HEIGHT_PX: f32 = 560.0;

            let ui_session = session::load();
            let restored_w = ui_session
                .window_width
                .map(|w| px(w as f32))
                .unwrap_or(px(1100.0))
                .max(px(WINDOW_MIN_WIDTH_PX));
            let restored_h = ui_session
                .window_height
                .map(|h| px(h as f32))
                .unwrap_or(px(720.0))
                .max(px(WINDOW_MIN_HEIGHT_PX));

            let bounds = Bounds::centered(None, size(restored_w, restored_h), cx);
            let backend = Arc::clone(&backend);

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(WINDOW_MIN_WIDTH_PX), px(WINDOW_MIN_HEIGHT_PX))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("GitGpui".into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(9.0), px(9.0))),
                    }),
                    window_decorations: Some(WindowDecorations::Client),
                    is_movable: true,
                    is_resizable: true,
                    ..Default::default()
                },
                move |window, cx| {
                    let (store, events) = AppStore::new(Arc::clone(&backend));
                    cx.new(|cx| GitGpuiView::new(store, events, initial_path.clone(), window, cx))
                },
            )
            .unwrap();

            cx.activate(true);
        });
}
