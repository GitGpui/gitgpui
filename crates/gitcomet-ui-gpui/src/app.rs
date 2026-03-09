use crate::assets::GitCometAssets;
use crate::launch_guard::{UiLaunchError, run_with_panic_guard};
use crate::view::{
    FocusedMergetoolLabels, FocusedMergetoolViewConfig, GitCometView, GitCometViewConfig,
    GitCometViewMode, StartupCrashReport,
};
use gitcomet_core::services::GitBackend;
use gitcomet_state::session;
use gitcomet_state::store::AppStore;
use gpui::{
    App, AppContext, Application, Bounds, KeyBinding, TitlebarOptions, WindowBounds,
    WindowDecorations, WindowOptions, point, px, size,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

const WINDOW_MIN_WIDTH_PX: f32 = 820.0;
const WINDOW_MIN_HEIGHT_PX: f32 = 560.0;
const WINDOW_DEFAULT_WIDTH_PX: f32 = 1100.0;
const WINDOW_DEFAULT_HEIGHT_PX: f32 = 720.0;
const FOCUSED_MERGETOOL_EXIT_CANCELED: i32 = 1;
#[cfg(test)]
const FOCUSED_MERGETOOL_EXIT_SUCCESS: i32 = 0;
const FOCUSED_MERGETOOL_EXIT_ERROR: i32 = 2;

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
    view_config: GitCometViewConfig,
}

pub fn run(backend: Arc<dyn GitBackend>) -> Result<(), UiLaunchError> {
    run_with_startup_crash_report(backend, None, None)
}

pub fn run_with_startup_crash_report(
    backend: Arc<dyn GitBackend>,
    initial_path: Option<PathBuf>,
    startup_crash_report: Option<StartupCrashReport>,
) -> Result<(), UiLaunchError> {
    let launch = normal_launch_config(initial_path, startup_crash_report);
    run_with_panic_guard("main GPUI window launch", move || {
        run_windowed_app(backend, launch)
    })
}

/// Launch the unified focused mergetool window using the shared `GitCometView`.
pub fn run_focused_mergetool(backend: Arc<dyn GitBackend>, config: FocusedMergetoolConfig) -> i32 {
    let exit_code = Arc::new(AtomicI32::new(FOCUSED_MERGETOOL_EXIT_CANCELED));
    let launch = focused_mergetool_launch_config(&config, Some(exit_code.clone()));
    if let Err(err) = run_with_panic_guard("focused mergetool GPUI launch", move || {
        run_windowed_app(backend, launch)
    }) {
        eprintln!("Failed to launch focused mergetool window: {err}");
        return FOCUSED_MERGETOOL_EXIT_ERROR;
    }
    exit_code.load(Ordering::SeqCst)
}

fn normal_launch_config(
    initial_path: Option<PathBuf>,
    startup_crash_report: Option<StartupCrashReport>,
) -> WindowLaunchConfig {
    WindowLaunchConfig {
        title: "GitComet".to_string(),
        app_id: "gitcomet".to_string(),
        view_config: GitCometViewConfig::normal(initial_path, startup_crash_report),
    }
}

fn focused_mergetool_launch_config(
    config: &FocusedMergetoolConfig,
    exit_code: Option<Arc<AtomicI32>>,
) -> WindowLaunchConfig {
    WindowLaunchConfig {
        title: focused_mergetool_window_title(&config.conflicted_file_path),
        app_id: "gitcomet-mergetool".to_string(),
        view_config: GitCometViewConfig {
            initial_path: Some(config.repo_path.clone()),
            view_mode: GitCometViewMode::FocusedMergetool,
            focused_mergetool: Some(FocusedMergetoolViewConfig {
                repo_path: config.repo_path.clone(),
                conflicted_file_path: config.conflicted_file_path.clone(),
                labels: FocusedMergetoolLabels {
                    local: config.label_local.clone(),
                    remote: config.label_remote.clone(),
                    base: config.label_base.clone(),
                },
            }),
            focused_mergetool_exit_code: exit_code,
            startup_crash_report: None,
        },
    }
}

fn focused_mergetool_window_title(conflicted_file_path: &Path) -> String {
    let display = conflicted_file_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| conflicted_file_path.display().to_string());
    format!("GitComet - Mergetool ({display})")
}

fn run_windowed_app(backend: Arc<dyn GitBackend>, launch: WindowLaunchConfig) {
    Application::new()
        .with_assets(GitCometAssets)
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
                    cx.new(|cx| {
                        GitCometView::new_with_config(
                            store,
                            events,
                            view_config.clone(),
                            window,
                            cx,
                        )
                    })
                },
            )
            .expect("failed to open main GitComet window");

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
        KeyBinding::new("up", crate::kit::Up, Some("TextInput")),
        KeyBinding::new("down", crate::kit::Down, Some("TextInput")),
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
        KeyBinding::new("shift-up", crate::kit::SelectUp, Some("TextInput")),
        KeyBinding::new("shift-down", crate::kit::SelectDown, Some("TextInput")),
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
        KeyBinding::new("cmd-z", crate::kit::Undo, Some("TextInput")),
        KeyBinding::new("ctrl-z", crate::kit::Undo, Some("TextInput")),
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
        assert_eq!(title, "GitComet - Mergetool (conflict.txt)");
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

        let launch = focused_mergetool_launch_config(&config, None);
        assert_eq!(launch.app_id, "gitcomet-mergetool");
        assert_eq!(launch.title, "GitComet - Mergetool (conflict.txt)");
        assert_eq!(launch.view_config.initial_path, Some(config.repo_path));
        assert_eq!(
            launch.view_config.view_mode,
            GitCometViewMode::FocusedMergetool
        );
        assert_eq!(
            launch.view_config.focused_mergetool,
            Some(FocusedMergetoolViewConfig {
                repo_path: PathBuf::from("/repo"),
                conflicted_file_path: PathBuf::from("/repo/src/conflict.txt"),
                labels: FocusedMergetoolLabels {
                    local: "LOCAL".to_string(),
                    remote: "REMOTE".to_string(),
                    base: "BASE".to_string(),
                },
            })
        );
        assert!(launch.view_config.focused_mergetool_exit_code.is_none());
    }

    #[test]
    fn focused_mergetool_exit_codes_match_mergetool_contract() {
        assert_eq!(FOCUSED_MERGETOOL_EXIT_SUCCESS, 0);
        assert_eq!(FOCUSED_MERGETOOL_EXIT_CANCELED, 1);
        assert_eq!(FOCUSED_MERGETOOL_EXIT_ERROR, 2);
    }
}
