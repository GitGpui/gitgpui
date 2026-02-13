use super::util::diff_target_wants_image_preview;
use crate::model::{AppState, DiagnosticKind, Loadable, RepoId};
use crate::msg::Effect;
use gitgpui_core::domain::{Diff, DiffTarget, FileDiffImage, FileDiffText};
use gitgpui_core::error::Error;
use std::sync::Arc;

pub(super) fn select_diff(
    state: &mut AppState,
    repo_id: RepoId,
    target: DiffTarget,
) -> Vec<Effect> {
    let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
        return Vec::new();
    };

    repo_state.diff_target = Some(target.clone());
    repo_state.diff = Loadable::Loading;
    let supports_file = matches!(
        &target,
        DiffTarget::WorkingTree { .. } | DiffTarget::Commit { path: Some(_), .. }
    );
    let wants_image = diff_target_wants_image_preview(&target);
    repo_state.diff_file = if supports_file && !wants_image {
        Loadable::Loading
    } else {
        Loadable::NotLoaded
    };
    repo_state.diff_file_image = if supports_file && wants_image {
        Loadable::Loading
    } else {
        Loadable::NotLoaded
    };

    if supports_file {
        if wants_image {
            vec![
                Effect::LoadDiffFileImage {
                    repo_id,
                    target: target.clone(),
                },
                Effect::LoadDiff { repo_id, target },
            ]
        } else {
            vec![
                Effect::LoadDiffFile {
                    repo_id,
                    target: target.clone(),
                },
                Effect::LoadDiff { repo_id, target },
            ]
        }
    } else {
        vec![Effect::LoadDiff { repo_id, target }]
    }
}

pub(super) fn clear_diff_selection(state: &mut AppState, repo_id: RepoId) -> Vec<Effect> {
    let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
        return Vec::new();
    };

    repo_state.diff_target = None;
    repo_state.diff = Loadable::NotLoaded;
    repo_state.diff_file = Loadable::NotLoaded;
    repo_state.diff_file_image = Loadable::NotLoaded;
    Vec::new()
}

pub(super) fn stage_hunk(repo_id: RepoId, patch: String) -> Vec<Effect> {
    vec![Effect::StageHunk { repo_id, patch }]
}

pub(super) fn unstage_hunk(repo_id: RepoId, patch: String) -> Vec<Effect> {
    vec![Effect::UnstageHunk { repo_id, patch }]
}

pub(super) fn apply_worktree_patch(repo_id: RepoId, patch: String, reverse: bool) -> Vec<Effect> {
    vec![Effect::ApplyWorktreePatch {
        repo_id,
        patch,
        reverse,
    }]
}

pub(super) fn diff_loaded(
    state: &mut AppState,
    repo_id: RepoId,
    target: DiffTarget,
    result: std::result::Result<Diff, Error>,
) -> Vec<Effect> {
    if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id)
        && repo_state.diff_target.as_ref() == Some(&target)
    {
        repo_state.diff_rev = repo_state.diff_rev.wrapping_add(1);
        repo_state.diff = match result {
            Ok(v) => Loadable::Ready(Arc::new(v)),
            Err(e) => {
                super::util::push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                Loadable::Error(e.to_string())
            }
        };
    }
    Vec::new()
}

pub(super) fn diff_file_loaded(
    state: &mut AppState,
    repo_id: RepoId,
    target: DiffTarget,
    result: std::result::Result<Option<FileDiffText>, Error>,
) -> Vec<Effect> {
    if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id)
        && repo_state.diff_target.as_ref() == Some(&target)
    {
        repo_state.diff_file_rev = repo_state.diff_file_rev.wrapping_add(1);
        repo_state.diff_file = match result {
            Ok(v) => Loadable::Ready(v),
            Err(e) => {
                super::util::push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                Loadable::Error(e.to_string())
            }
        };
    }
    Vec::new()
}

pub(super) fn diff_file_image_loaded(
    state: &mut AppState,
    repo_id: RepoId,
    target: DiffTarget,
    result: std::result::Result<Option<FileDiffImage>, Error>,
) -> Vec<Effect> {
    if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id)
        && repo_state.diff_target.as_ref() == Some(&target)
    {
        repo_state.diff_file_rev = repo_state.diff_file_rev.wrapping_add(1);
        repo_state.diff_file_image = match result {
            Ok(v) => Loadable::Ready(v),
            Err(e) => {
                super::util::push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                Loadable::Error(e.to_string())
            }
        };
    }
    Vec::new()
}
