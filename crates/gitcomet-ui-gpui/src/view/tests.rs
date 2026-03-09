use super::*;
use gitcomet_core::domain::{Branch, CommitId, Remote, RemoteBranch, RepoSpec, Upstream};
use std::path::PathBuf;

#[test]
fn toast_total_lifetime_includes_fade_in_and_out() {
    let ttl = Duration::from_secs(6);
    assert_eq!(
        toast_total_lifetime(ttl),
        ttl + Duration::from_millis(TOAST_FADE_IN_MS + TOAST_FADE_OUT_MS)
    );
}

#[test]
fn reconcile_status_multi_selection_prunes_missing_paths_and_anchors() {
    let a = PathBuf::from("a.txt");
    let b = PathBuf::from("b.txt");
    let c = PathBuf::from("c.txt");

    let status = RepoStatus {
        staged: vec![],
        unstaged: vec![FileStatus {
            path: a.clone(),
            kind: FileStatusKind::Modified,
            conflict: None,
        }],
    };

    let mut selection = StatusMultiSelection {
        unstaged: vec![a.clone(), b.clone()],
        unstaged_anchor: Some(b),
        staged: vec![c.clone()],
        staged_anchor: Some(c),
    };

    reconcile_status_multi_selection(&mut selection, &status);

    assert_eq!(selection.unstaged, vec![a]);
    assert!(selection.unstaged_anchor.is_none());
    assert!(selection.staged.is_empty());
    assert!(selection.staged_anchor.is_none());
}

#[test]
fn remote_rows_groups_and_sorts() {
    let mut repo = RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::new(),
        },
    );
    repo.remote_branches = Loadable::Ready(Arc::new(vec![
        RemoteBranch {
            remote: "origin".to_string(),
            name: "b".to_string(),
            target: CommitId("b0".to_string()),
        },
        RemoteBranch {
            remote: "origin".to_string(),
            name: "a".to_string(),
            target: CommitId("a0".to_string()),
        },
        RemoteBranch {
            remote: "upstream".to_string(),
            name: "main".to_string(),
            target: CommitId("c0".to_string()),
        },
    ]));

    let rows = GitCometView::remote_rows(&repo);
    assert_eq!(
        rows,
        vec![
            RemoteRow::Header("origin".to_string()),
            RemoteRow::Branch {
                remote: "origin".to_string(),
                name: "a".to_string()
            },
            RemoteRow::Branch {
                remote: "origin".to_string(),
                name: "b".to_string()
            },
            RemoteRow::Header("upstream".to_string()),
            RemoteRow::Branch {
                remote: "upstream".to_string(),
                name: "main".to_string()
            },
        ]
    );
}

#[test]
fn remote_headers_include_remotes_with_no_branches() {
    let mut repo = RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::new(),
        },
    );

    repo.remotes = Loadable::Ready(Arc::new(vec![
        Remote {
            name: "origin".to_string(),
            url: Some("https://example.com/origin.git".to_string()),
        },
        Remote {
            name: "upstream".to_string(),
            url: Some("https://example.com/upstream.git".to_string()),
        },
    ]));
    repo.remote_branches = Loadable::Ready(Arc::new(vec![RemoteBranch {
        remote: "origin".to_string(),
        name: "main".to_string(),
        target: CommitId("deadbeef".to_string()),
    }]));

    let rows = GitCometView::branch_sidebar_rows(&repo);
    let mut headers = rows
        .iter()
        .filter_map(|r| match r {
            BranchSidebarRow::RemoteHeader { name } => Some(name.as_ref().to_owned()),
            _ => None,
        })
        .collect::<Vec<_>>();
    headers.sort();
    headers.dedup();

    assert!(
        headers.contains(&"origin".to_string()),
        "expected origin remote header"
    );
    assert!(
        headers.contains(&"upstream".to_string()),
        "expected upstream remote header"
    );
}

#[test]
fn remote_upstream_branch_is_marked() {
    let mut repo = RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::new(),
        },
    );

    repo.head_branch = Loadable::Ready("main".to_string());
    repo.branches = Loadable::Ready(Arc::new(vec![Branch {
        name: "main".to_string(),
        target: CommitId("deadbeef".to_string()),
        upstream: Some(Upstream {
            remote: "origin".to_string(),
            branch: "main".to_string(),
        }),
        divergence: None,
    }]));
    repo.remote_branches = Loadable::Ready(Arc::new(vec![RemoteBranch {
        remote: "origin".to_string(),
        name: "main".to_string(),
        target: CommitId("deadbeef".to_string()),
    }]));

    let rows = GitCometView::branch_sidebar_rows(&repo);
    let upstream_row = rows.iter().find(|r| {
        matches!(
            r,
            BranchSidebarRow::Branch {
                section: BranchSection::Remote,
                name,
                is_upstream: true,
                ..
            } if name.as_ref() == "origin/main"
        )
    });
    assert!(
        upstream_row.is_some(),
        "expected origin/main to be marked as upstream"
    );
}

#[test]
fn resize_edge_detects_edges_and_corners() {
    let window_size = size(px(100.0), px(100.0));
    let tiling = Tiling::default();
    let inset = px(10.0);

    assert_eq!(
        resize_edge(point(px(0.0), px(0.0)), inset, window_size, tiling),
        Some(ResizeEdge::TopLeft)
    );
    assert_eq!(
        resize_edge(point(px(99.0), px(0.0)), inset, window_size, tiling),
        Some(ResizeEdge::TopRight)
    );
    assert_eq!(
        resize_edge(point(px(0.0), px(99.0)), inset, window_size, tiling),
        Some(ResizeEdge::BottomLeft)
    );
    assert_eq!(
        resize_edge(point(px(99.0), px(99.0)), inset, window_size, tiling),
        Some(ResizeEdge::BottomRight)
    );

    assert_eq!(
        resize_edge(point(px(50.0), px(0.0)), inset, window_size, tiling),
        Some(ResizeEdge::Top)
    );
    assert_eq!(
        resize_edge(point(px(50.0), px(99.0)), inset, window_size, tiling),
        Some(ResizeEdge::Bottom)
    );
    assert_eq!(
        resize_edge(point(px(0.0), px(50.0)), inset, window_size, tiling),
        Some(ResizeEdge::Left)
    );
    assert_eq!(
        resize_edge(point(px(99.0), px(50.0)), inset, window_size, tiling),
        Some(ResizeEdge::Right)
    );

    assert_eq!(
        resize_edge(point(px(50.0), px(50.0)), inset, window_size, tiling),
        None
    );
}

#[test]
fn resize_edge_respects_tiling() {
    let window_size = size(px(100.0), px(100.0));
    let inset = px(10.0);
    let tiling = Tiling {
        top: true,
        left: false,
        right: false,
        bottom: false,
    };

    assert_eq!(
        resize_edge(point(px(0.0), px(0.0)), inset, window_size, tiling),
        Some(ResizeEdge::Left)
    );
    assert_eq!(
        resize_edge(point(px(50.0), px(0.0)), inset, window_size, tiling),
        None
    );
    assert_eq!(
        resize_edge(point(px(0.0), px(50.0)), inset, window_size, tiling),
        Some(ResizeEdge::Left)
    );
}

#[test]
fn cursor_style_matches_resize_edge() {
    assert_eq!(
        cursor_style_for_resize_edge(ResizeEdge::Left),
        CursorStyle::ResizeLeftRight
    );
    assert_eq!(
        cursor_style_for_resize_edge(ResizeEdge::Top),
        CursorStyle::ResizeUpDown
    );
    assert_eq!(
        cursor_style_for_resize_edge(ResizeEdge::TopLeft),
        CursorStyle::ResizeUpLeftDownRight
    );
    assert_eq!(
        cursor_style_for_resize_edge(ResizeEdge::TopRight),
        CursorStyle::ResizeUpRightDownLeft
    );
}

#[test]
fn is_markdown_path_detects_common_extensions() {
    use std::path::Path;
    assert!(is_markdown_path(Path::new("README.md")));
    assert!(is_markdown_path(Path::new("doc.markdown")));
    assert!(is_markdown_path(Path::new("notes.mdown")));
    assert!(is_markdown_path(Path::new("CHANGES.mkd")));
    assert!(is_markdown_path(Path::new("file.mkdn")));
    assert!(is_markdown_path(Path::new("file.mdwn")));
    assert!(is_markdown_path(Path::new("UPPER.MD")));
}

#[test]
fn is_markdown_path_rejects_non_markdown() {
    use std::path::Path;
    assert!(!is_markdown_path(Path::new("file.txt")));
    assert!(!is_markdown_path(Path::new("file.rs")));
    assert!(!is_markdown_path(Path::new("file")));
}

#[test]
fn conflict_resolver_preview_mode_defaults_to_text() {
    assert_eq!(
        ConflictResolverPreviewMode::default(),
        ConflictResolverPreviewMode::Text
    );
}

fn focused_bootstrap(
    repo_path: PathBuf,
    conflicted_file_path: PathBuf,
) -> FocusedMergetoolBootstrap {
    FocusedMergetoolBootstrap::from_view_config(FocusedMergetoolViewConfig {
        repo_path,
        conflicted_file_path,
        labels: FocusedMergetoolLabels {
            local: "LOCAL".to_string(),
            remote: "REMOTE".to_string(),
            base: "BASE".to_string(),
        },
    })
}

fn open_repo_state_with_workdir(workdir: &str) -> RepoState {
    let mut repo = RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: normalize_bootstrap_repo_path(PathBuf::from(workdir)),
        },
    );
    repo.open = Loadable::Ready(());
    repo
}

#[test]
fn focused_mergetool_target_path_prefers_repo_relative_path() {
    let repo = normalize_bootstrap_repo_path(PathBuf::from("/repo"));
    let target = focused_mergetool_target_path(&repo, &repo.join("src/conflict.txt"));
    assert_eq!(target, PathBuf::from("src/conflict.txt"));
}

#[test]
fn focused_mergetool_bootstrap_requests_open_repo_when_missing() {
    let repo = normalize_bootstrap_repo_path(PathBuf::from("/repo"));
    let bootstrap = focused_bootstrap(repo.clone(), repo.join("src/conflict.txt"));
    let state = AppState::default();

    assert_eq!(
        focused_mergetool_bootstrap_action(&state, &bootstrap),
        Some(FocusedMergetoolBootstrapAction::OpenRepo(repo))
    );
}

#[test]
fn focused_mergetool_bootstrap_selects_worktree_diff_target() {
    let repo = normalize_bootstrap_repo_path(PathBuf::from("/repo"));
    let bootstrap = focused_bootstrap(repo.clone(), repo.join("src/conflict.txt"));
    let mut state = AppState::default();
    state.active_repo = Some(RepoId(1));
    state
        .repos
        .push(open_repo_state_with_workdir(&repo.to_string_lossy()));

    assert_eq!(
        focused_mergetool_bootstrap_action(&state, &bootstrap),
        Some(FocusedMergetoolBootstrapAction::SelectDiff {
            repo_id: RepoId(1),
            target: DiffTarget::WorkingTree {
                area: DiffArea::Unstaged,
                path: PathBuf::from("src/conflict.txt"),
            },
        })
    );
}

#[test]
fn focused_mergetool_bootstrap_loads_conflict_file_after_diff_target() {
    let repo = normalize_bootstrap_repo_path(PathBuf::from("/repo"));
    let bootstrap = focused_bootstrap(repo.clone(), repo.join("src/conflict.txt"));
    let mut state = AppState::default();
    state.active_repo = Some(RepoId(1));
    let mut repo_state = open_repo_state_with_workdir(&repo.to_string_lossy());
    repo_state.diff_state.diff_target = Some(DiffTarget::WorkingTree {
        area: DiffArea::Unstaged,
        path: PathBuf::from("src/conflict.txt"),
    });
    state.repos.push(repo_state);

    assert_eq!(
        focused_mergetool_bootstrap_action(&state, &bootstrap),
        Some(FocusedMergetoolBootstrapAction::LoadConflictFile {
            repo_id: RepoId(1),
            path: PathBuf::from("src/conflict.txt"),
        })
    );
}

#[test]
fn focused_mergetool_bootstrap_completes_after_conflict_file_target_set() {
    let repo = normalize_bootstrap_repo_path(PathBuf::from("/repo"));
    let bootstrap = focused_bootstrap(repo.clone(), repo.join("src/conflict.txt"));
    let mut state = AppState::default();
    state.active_repo = Some(RepoId(1));
    let mut repo_state = open_repo_state_with_workdir(&repo.to_string_lossy());
    repo_state.diff_state.diff_target = Some(DiffTarget::WorkingTree {
        area: DiffArea::Unstaged,
        path: PathBuf::from("src/conflict.txt"),
    });
    repo_state.conflict_state.conflict_file_path = Some(PathBuf::from("src/conflict.txt"));
    repo_state.conflict_state.conflict_file = Loadable::Loading;
    state.repos.push(repo_state);

    assert_eq!(
        focused_mergetool_bootstrap_action(&state, &bootstrap),
        Some(FocusedMergetoolBootstrapAction::Complete)
    );
}

#[test]
fn focused_mergetool_mode_hides_full_chrome() {
    assert!(renders_full_chrome(GitCometViewMode::Normal));
    assert!(!renders_full_chrome(GitCometViewMode::FocusedMergetool));
}
