use crate::assets::GitGpuiAssets;
use crate::view::{FocusedMergetoolViewConfig, GitGpuiView, GitGpuiViewConfig, GitGpuiViewMode};
use gitgpui_core::services::GitBackend;
use gitgpui_state::session;
use gitgpui_state::store::AppStore;
use gpui::{
    App, AppContext, Application, Bounds, KeyBinding, TitlebarOptions, WindowBounds,
    WindowDecorations, WindowOptions, point, px, size,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const WINDOW_MIN_WIDTH_PX: f32 = 820.0;
const WINDOW_MIN_HEIGHT_PX: f32 = 560.0;
const WINDOW_DEFAULT_WIDTH_PX: f32 = 1100.0;
const WINDOW_DEFAULT_HEIGHT_PX: f32 = 720.0;
const FOCUSED_MERGETOOL_EXIT_CANCELED: i32 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FocusedMergetoolConfig {
    pub repo_path: PathBuf,
    pub conflicted_file_path: PathBuf,
    pub label_local: String,
    pub label_remote: String,
    pub label_base: String,
}

#[derive(Clone, Debug)]
struct WindowLaunchConfig {
    title: String,
    app_id: String,
    view_config: GitGpuiViewConfig,
    use_legacy_constructor: bool,
}

pub fn run(backend: Arc<dyn GitBackend>) {
    let initial_path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    run_windowed_app(backend, normal_launch_config(initial_path));
}

/// Launch the unified focused mergetool window using the shared `GitGpuiView`.
///
/// Phase 2 only introduces the focused entrypoint and shared bootstrap; save/exit
/// result wiring is implemented later. For now, a focused window close maps to
/// "canceled" (`1`) per mergetool conventions.
pub fn run_focused_mergetool(backend: Arc<dyn GitBackend>, config: FocusedMergetoolConfig) -> i32 {
    run_windowed_app(backend, focused_mergetool_launch_config(&config));
    FOCUSED_MERGETOOL_EXIT_CANCELED
}

fn normal_launch_config(initial_path: Option<PathBuf>) -> WindowLaunchConfig {
    WindowLaunchConfig {
        title: "GitGpui".to_string(),
        app_id: "gitgpui".to_string(),
        view_config: GitGpuiViewConfig::normal(initial_path),
        use_legacy_constructor: true,
    }
}

fn focused_mergetool_launch_config(config: &FocusedMergetoolConfig) -> WindowLaunchConfig {
    WindowLaunchConfig {
        title: focused_mergetool_window_title(&config.conflicted_file_path),
        app_id: "gitgpui-mergetool".to_string(),
        view_config: GitGpuiViewConfig {
            initial_path: Some(config.repo_path.clone()),
            view_mode: GitGpuiViewMode::FocusedMergetool,
            focused_mergetool: Some(FocusedMergetoolViewConfig {
                repo_path: config.repo_path.clone(),
                conflicted_file_path: config.conflicted_file_path.clone(),
            }),
        },
        use_legacy_constructor: false,
    }
}

fn focused_mergetool_window_title(conflicted_file_path: &Path) -> String {
    let display = conflicted_file_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| conflicted_file_path.display().to_string());
    format!("GitGpui - Mergetool ({display})")
}

fn run_windowed_app(backend: Arc<dyn GitBackend>, launch: WindowLaunchConfig) {
    Application::new()
        .with_assets(GitGpuiAssets)
        .run(move |cx: &mut App| {
            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            bind_text_input_keys(cx);
            let ui_session = session::load();
            let restored_w = ui_session
                .window_width
                .map(|w| px(w as f32))
                .unwrap_or(px(WINDOW_DEFAULT_WIDTH_PX))
                .max(px(WINDOW_MIN_WIDTH_PX));
            let restored_h = ui_session
                .window_height
                .map(|h| px(h as f32))
                .unwrap_or(px(WINDOW_DEFAULT_HEIGHT_PX))
                .max(px(WINDOW_MIN_HEIGHT_PX));

            let bounds = Bounds::centered(None, size(restored_w, restored_h), cx);
            let backend = Arc::clone(&backend);
            let window_title = launch.title.clone();
            let app_id = launch.app_id.clone();
            let view_config = launch.view_config.clone();
            let use_legacy_constructor = launch.use_legacy_constructor;

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(WINDOW_MIN_WIDTH_PX), px(WINDOW_MIN_HEIGHT_PX))),
                    titlebar: Some(TitlebarOptions {
                        title: Some(window_title.into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(9.0), px(9.0))),
                    }),
                    app_id: Some(app_id),
                    window_decorations: Some(WindowDecorations::Client),
                    is_movable: true,
                    is_resizable: true,
                    ..Default::default()
                },
                move |window, cx| {
                    let (store, events) = AppStore::new(Arc::clone(&backend));
                    if use_legacy_constructor {
                        let initial_path = view_config.initial_path.clone();
                        cx.new(|cx| GitGpuiView::new(store, events, initial_path, window, cx))
                    } else {
                        cx.new(|cx| {
                            GitGpuiView::new_with_config(
                                store,
                                events,
                                view_config.clone(),
                                window,
                                cx,
                            )
                        })
                    }
                },
            )
            .unwrap();

            cx.activate(true);
        });
}

fn bind_text_input_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", crate::kit::Backspace, Some("TextInput")),
        KeyBinding::new("delete", crate::kit::Delete, Some("TextInput")),
        KeyBinding::new("enter", crate::kit::Enter, Some("TextInput")),
        KeyBinding::new("left", crate::kit::Left, Some("TextInput")),
        KeyBinding::new("right", crate::kit::Right, Some("TextInput")),
        // Word navigation (Ctrl on Windows/Linux, Option on macOS)
        KeyBinding::new("ctrl-left", crate::kit::WordLeft, Some("TextInput")),
        KeyBinding::new("ctrl-right", crate::kit::WordRight, Some("TextInput")),
        KeyBinding::new(
            "ctrl-shift-left",
            crate::kit::SelectWordLeft,
            Some("TextInput"),
        ),
        KeyBinding::new(
            "ctrl-shift-right",
            crate::kit::SelectWordRight,
            Some("TextInput"),
        ),
        KeyBinding::new("alt-left", crate::kit::WordLeft, Some("TextInput")),
        KeyBinding::new("alt-right", crate::kit::WordRight, Some("TextInput")),
        KeyBinding::new(
            "alt-shift-left",
            crate::kit::SelectWordLeft,
            Some("TextInput"),
        ),
        KeyBinding::new(
            "alt-shift-right",
            crate::kit::SelectWordRight,
            Some("TextInput"),
        ),
        KeyBinding::new("shift-left", crate::kit::SelectLeft, Some("TextInput")),
        KeyBinding::new("shift-right", crate::kit::SelectRight, Some("TextInput")),
        KeyBinding::new("home", crate::kit::Home, Some("TextInput")),
        KeyBinding::new("shift-home", crate::kit::SelectHome, Some("TextInput")),
        KeyBinding::new("end", crate::kit::End, Some("TextInput")),
        KeyBinding::new("shift-end", crate::kit::SelectEnd, Some("TextInput")),
        KeyBinding::new("pageup", crate::kit::PageUp, Some("TextInput")),
        KeyBinding::new("shift-pageup", crate::kit::SelectPageUp, Some("TextInput")),
        KeyBinding::new("pagedown", crate::kit::PageDown, Some("TextInput")),
        KeyBinding::new(
            "shift-pagedown",
            crate::kit::SelectPageDown,
            Some("TextInput"),
        ),
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focused_mergetool_title_uses_file_name_when_available() {
        let title = focused_mergetool_window_title(Path::new("/repo/src/conflict.txt"));
        assert_eq!(title, "GitGpui - Mergetool (conflict.txt)");
    }

    #[test]
    fn focused_mergetool_launch_config_sets_focused_view_mode_and_repo() {
        let config = FocusedMergetoolConfig {
            repo_path: PathBuf::from("/repo"),
            conflicted_file_path: PathBuf::from("/repo/src/conflict.txt"),
            label_local: "LOCAL".to_string(),
            label_remote: "REMOTE".to_string(),
            label_base: "BASE".to_string(),
        };

        let launch = focused_mergetool_launch_config(&config);
        assert_eq!(launch.app_id, "gitgpui-mergetool");
        assert_eq!(launch.title, "GitGpui - Mergetool (conflict.txt)");
        assert_eq!(launch.view_config.initial_path, Some(config.repo_path));
        assert_eq!(
            launch.view_config.view_mode,
            GitGpuiViewMode::FocusedMergetool
        );
        assert_eq!(
            launch.view_config.focused_mergetool,
            Some(FocusedMergetoolViewConfig {
                repo_path: PathBuf::from("/repo"),
                conflicted_file_path: PathBuf::from("/repo/src/conflict.txt"),
            })
        );
    }
}
