use super::*;
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::{GitBackend, GitRepository, Result};
use gitgpui_state::store::AppStore;
use gpui::px;
use std::path::Path;
use std::sync::Arc;

#[test]
fn commit_details_files_list_has_reasonable_max_height() {
    assert!(COMMIT_DETAILS_FILES_MAX_HEIGHT_PX > 0.0);
    assert!(COMMIT_DETAILS_FILES_MAX_HEIGHT_PX <= 400.0);
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
fn file_preview_renders_scrollable_syntax_highlighted_rows(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new(Arc::new(TestBackend));
    let (view, cx) = cx.add_window_view(|window, cx| {
        super::super::GitGpuiView::new(store, events, None, window, cx)
    });

    let repo_id = gitgpui_state::model::RepoId(1);
    let workdir = std::env::temp_dir().join(format!("gitgpui_ui_test_{}", std::process::id()));
    let file_rel = std::path::PathBuf::from("preview.rs");
    let lines: Arc<Vec<String>> = Arc::new(
        (0..300)
            .map(|_| "fn main() { let x = 1; }".to_string())
            .collect(),
    );

    cx.update(|_window, app| {
        view.update(app, |this, cx| {
            let mut repo = gitgpui_state::model::RepoState::new_opening(
                repo_id,
                gitgpui_core::domain::RepoSpec {
                    workdir: workdir.clone(),
                },
            );
            repo.status = gitgpui_state::model::Loadable::Ready(
                gitgpui_core::domain::RepoStatus {
                    staged: vec![],
                    unstaged: vec![gitgpui_core::domain::FileStatus {
                        path: file_rel.clone(),
                        kind: gitgpui_core::domain::FileStatusKind::Untracked,
                    }],
                }
                .into(),
            );
            repo.diff_target = Some(gitgpui_core::domain::DiffTarget::WorkingTree {
                path: file_rel.clone(),
                area: gitgpui_core::domain::DiffArea::Unstaged,
            });

            this.state = Arc::new(AppState {
                repos: vec![repo],
                active_repo: Some(repo_id),
                ..Default::default()
            });

            this.worktree_preview_path = Some(workdir.join(&file_rel));
            this.worktree_preview = gitgpui_state::model::Loadable::Ready(Arc::clone(&lines));
            this.worktree_preview_segments_cache_path = None;
            this.worktree_preview_segments_cache.clear();
            this.worktree_preview_scroll
                .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
            cx.notify();
        });
    });

    cx.update(|window, app| {
        let _ = window.draw(app);
    });

    cx.update(|_window, app| {
        let this = view.read(app);
        let max_offset = this
            .worktree_preview_scroll
            .0
            .borrow()
            .base_handle
            .max_offset()
            .height;
        assert!(
            max_offset > px(0.0),
            "expected file preview to overflow and be scrollable"
        );

        let Some(styled) = this.worktree_preview_segments_cache.get(&0) else {
            panic!("expected first visible preview row to populate segment cache");
        };
        assert!(
            !styled.highlights.is_empty(),
            "expected syntax highlighting highlights for preview row"
        );
    });
}

#[gpui::test]
fn patch_view_applies_syntax_highlighting_to_context_lines(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new(Arc::new(TestBackend));
    let (view, cx) = cx.add_window_view(|window, cx| {
        super::super::GitGpuiView::new(store, events, None, window, cx)
    });

    let repo_id = gitgpui_state::model::RepoId(2);
    let workdir =
        std::env::temp_dir().join(format!("gitgpui_ui_test_{}_patch", std::process::id()));

    cx.update(|_window, app| {
        view.update(app, |this, cx| {
            let target = gitgpui_core::domain::DiffTarget::Commit {
                commit_id: gitgpui_core::domain::CommitId("deadbeef".to_string()),
                path: None,
            };

            let diff = gitgpui_core::domain::Diff {
                target: target.clone(),
                lines: vec![
                    gitgpui_core::domain::DiffLine {
                        kind: gitgpui_core::domain::DiffLineKind::Header,
                        text: "diff --git a/foo.rs b/foo.rs".to_string(),
                    },
                    gitgpui_core::domain::DiffLine {
                        kind: gitgpui_core::domain::DiffLineKind::Hunk,
                        text: "@@ -1,1 +1,1 @@".to_string(),
                    },
                    gitgpui_core::domain::DiffLine {
                        kind: gitgpui_core::domain::DiffLineKind::Context,
                        text: " fn main() { let x = 1; }".to_string(),
                    },
                ],
            };

            let mut repo = gitgpui_state::model::RepoState::new_opening(
                repo_id,
                gitgpui_core::domain::RepoSpec {
                    workdir: workdir.clone(),
                },
            );
            repo.status = gitgpui_state::model::Loadable::Ready(
                gitgpui_core::domain::RepoStatus::default().into(),
            );
            repo.diff_target = Some(target);
            repo.diff_rev = 1;
            repo.diff = gitgpui_state::model::Loadable::Ready(diff.into());

            this.state = Arc::new(AppState {
                repos: vec![repo],
                active_repo: Some(repo_id),
                ..Default::default()
            });

            // Ensure a clean render path.
            this.rebuild_diff_cache();
            this.diff_text_segments_cache.clear();
            cx.notify();
        });
    });

    cx.update(|window, app| {
        let _ = window.draw(app);
    });

    cx.update(|_window, app| {
        let this = view.read(app);
        let styled = this
            .diff_text_segments_cache
            .get(2)
            .and_then(|v| v.as_ref())
            .expect("expected context line to be syntax-highlighted and cached");
        assert!(
            !styled.highlights.is_empty(),
            "expected syntax highlighting highlights for context line"
        );
    });
}

#[gpui::test]
fn staged_deleted_file_preview_uses_old_contents(cx: &mut gpui::TestAppContext) {
    let (store, events) = AppStore::new(Arc::new(TestBackend));
    let (view, cx) = cx.add_window_view(|window, cx| {
        super::super::GitGpuiView::new(store, events, None, window, cx)
    });

    let repo_id = gitgpui_state::model::RepoId(3);
    let workdir =
        std::env::temp_dir().join(format!("gitgpui_ui_test_{}_deleted", std::process::id()));
    let file_rel = std::path::PathBuf::from("deleted.rs");

    cx.update(|_window, app| {
        view.update(app, |this, cx| {
            let mut repo = gitgpui_state::model::RepoState::new_opening(
                repo_id,
                gitgpui_core::domain::RepoSpec {
                    workdir: workdir.clone(),
                },
            );

            repo.status = gitgpui_state::model::Loadable::Ready(
                gitgpui_core::domain::RepoStatus {
                    staged: vec![gitgpui_core::domain::FileStatus {
                        path: file_rel.clone(),
                        kind: gitgpui_core::domain::FileStatusKind::Deleted,
                    }],
                    unstaged: vec![],
                }
                .into(),
            );
            repo.diff_target = Some(gitgpui_core::domain::DiffTarget::WorkingTree {
                path: file_rel.clone(),
                area: gitgpui_core::domain::DiffArea::Staged,
            });
            repo.diff_file = gitgpui_state::model::Loadable::Ready(Some(
                gitgpui_core::domain::FileDiffText {
                    path: file_rel.clone(),
                    old: Some("one\ntwo\n".to_string()),
                    new: None,
                },
            ));

            this.state = Arc::new(AppState {
                repos: vec![repo],
                active_repo: Some(repo_id),
                ..Default::default()
            });

            this.try_populate_worktree_preview_from_diff_file();
            cx.notify();
        });
    });

    cx.update(|_window, app| {
        let this = view.read(app);
        assert_eq!(
            this.deleted_file_preview_abs_path(),
            Some(workdir.join(&file_rel))
        );
        let gitgpui_state::model::Loadable::Ready(lines) = &this.worktree_preview else {
            panic!("expected worktree preview to be ready");
        };
        assert_eq!(lines.as_ref(), &vec!["one".to_string(), "two".to_string()]);
    });
}
