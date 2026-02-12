use super::util::{
    dedup_paths_in_order, diff_target_wants_image_preview, normalize_repo_path, push_diagnostic,
    push_notification, refresh_full_effects, refresh_primary_effects,
};
use crate::model::{
    AppNotificationKind, AppState, CloneOpState, CloneOpStatus, DiagnosticKind, Loadable, RepoId,
};
use crate::msg::Effect;
use crate::session;
use gitgpui_core::domain::{DiffTarget, RepoSpec};
use gitgpui_core::error::Error;
use gitgpui_core::services::{CommandOutput, GitRepository};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) fn open_repo(id_alloc: &AtomicU64, state: &mut AppState, path: PathBuf) -> Vec<Effect> {
    let path = normalize_repo_path(path);
    if let Some(repo_id) = state
        .repos
        .iter()
        .find(|r| r.spec.workdir == path)
        .map(|r| r.id)
    {
        state.active_repo = Some(repo_id);
        let _ = session::persist_from_state(state);
        return Vec::new();
    }

    let repo_id = RepoId(id_alloc.fetch_add(1, Ordering::Relaxed));
    let spec = RepoSpec { workdir: path };

    state
        .repos
        .push(crate::model::RepoState::new_opening(repo_id, spec.clone()));
    state.active_repo = Some(repo_id);
    let effects = vec![Effect::OpenRepo {
        repo_id,
        path: spec.workdir.clone(),
    }];
    let _ = session::persist_from_state(state);
    effects
}

pub(super) fn restore_session(
    repos: &mut HashMap<RepoId, Arc<dyn GitRepository>>,
    id_alloc: &AtomicU64,
    state: &mut AppState,
    open_repos: Vec<PathBuf>,
    active_repo: Option<PathBuf>,
) -> Vec<Effect> {
    repos.clear();
    state.repos.clear();
    state.active_repo = None;

    let active_repo = active_repo.map(normalize_repo_path);
    let mut active_repo_id: Option<RepoId> = None;

    let mut effects = Vec::new();
    for path in dedup_paths_in_order(open_repos)
        .into_iter()
        .map(normalize_repo_path)
    {
        if state.repos.iter().any(|r| r.spec.workdir == path) {
            continue;
        }
        let repo_id = RepoId(id_alloc.fetch_add(1, Ordering::Relaxed));
        let spec = RepoSpec { workdir: path };
        if active_repo_id.is_none()
            && active_repo
                .as_ref()
                .is_some_and(|active| active == &spec.workdir)
        {
            active_repo_id = Some(repo_id);
        }

        state
            .repos
            .push(crate::model::RepoState::new_opening(repo_id, spec.clone()));
        effects.push(Effect::OpenRepo {
            repo_id,
            path: spec.workdir.clone(),
        });
    }

    state.active_repo = if let Some(active_repo_id) = active_repo_id {
        Some(active_repo_id)
    } else {
        state.repos.last().map(|r| r.id)
    };

    let _ = session::persist_from_state(state);
    effects
}

pub(super) fn close_repo(
    repos: &mut HashMap<RepoId, Arc<dyn GitRepository>>,
    state: &mut AppState,
    repo_id: RepoId,
) -> Vec<Effect> {
    state.repos.retain(|r| r.id != repo_id);
    repos.remove(&repo_id);
    if state.active_repo == Some(repo_id) {
        state.active_repo = state.repos.first().map(|r| r.id);
    }
    let _ = session::persist_from_state(state);
    Vec::new()
}

pub(super) fn set_active_repo(state: &mut AppState, repo_id: RepoId) -> Vec<Effect> {
    if !state.repos.iter().any(|r| r.id == repo_id) {
        return Vec::new();
    }

    let changed = state.active_repo != Some(repo_id);
    state.active_repo = Some(repo_id);
    if changed {
        let _ = session::persist_from_state(state);
    }

    let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
        return Vec::new();
    };

    // On focus events the UI can re-send SetActiveRepo for the already-active repo. Avoid
    // re-running the full refresh fan-out in that case: prioritize the minimum set that
    // keeps the UI correct and responsive.
    let mut effects = if changed {
        refresh_full_effects(repo_state)
    } else {
        refresh_primary_effects(repo_state)
    };

    // Reload the selected diff when switching repos; steady-state refreshes rely on the
    // filesystem watcher (`RepoExternallyChanged`) for diff invalidation.
    if changed && let Some(target) = repo_state.diff_target.clone() {
        let supports_file = matches!(
            &target,
            DiffTarget::WorkingTree { .. } | DiffTarget::Commit { path: Some(_), .. }
        );
        let wants_image = diff_target_wants_image_preview(&target);
        effects.push(Effect::LoadDiff {
            repo_id,
            target: target.clone(),
        });
        if supports_file {
            if wants_image {
                effects.push(Effect::LoadDiffFileImage { repo_id, target });
            } else {
                effects.push(Effect::LoadDiffFile { repo_id, target });
            }
        }
    }
    effects
}

pub(super) fn clone_repo(state: &mut AppState, url: String, dest: PathBuf) -> Vec<Effect> {
    state.clone = Some(CloneOpState {
        url: url.clone(),
        dest: dest.clone(),
        status: CloneOpStatus::Running,
        seq: 0,
        output_tail: Vec::new(),
    });
    vec![Effect::CloneRepo { url, dest }]
}

pub(super) fn clone_repo_progress(
    state: &mut AppState,
    dest: PathBuf,
    line: String,
) -> Vec<Effect> {
    if let Some(op) = state.clone.as_mut()
        && matches!(op.status, CloneOpStatus::Running)
        && op.dest == dest
    {
        op.seq = op.seq.wrapping_add(1);
        if !line.trim().is_empty() {
            op.output_tail.push(line);
            const MAX_LINES: usize = 80;
            if op.output_tail.len() > MAX_LINES {
                let drain = op.output_tail.len() - MAX_LINES;
                op.output_tail.drain(0..drain);
            }
        }
    }
    Vec::new()
}

pub(super) fn clone_repo_finished(
    state: &mut AppState,
    url: String,
    dest: PathBuf,
    result: std::result::Result<CommandOutput, Error>,
) -> Vec<Effect> {
    if let Some(op) = state.clone.as_mut()
        && op.dest == dest
    {
        op.url = url;
        op.status = match result {
            Ok(_) => CloneOpStatus::FinishedOk,
            Err(e) => CloneOpStatus::FinishedErr(e.to_string()),
        };
        op.seq = op.seq.wrapping_add(1);
    } else {
        state.clone = Some(CloneOpState {
            url,
            dest,
            status: match result {
                Ok(_) => CloneOpStatus::FinishedOk,
                Err(e) => CloneOpStatus::FinishedErr(e.to_string()),
            },
            seq: 1,
            output_tail: Vec::new(),
        });
    }
    Vec::new()
}

pub(super) fn repo_opened_ok(
    repos: &mut HashMap<RepoId, Arc<dyn GitRepository>>,
    state: &mut AppState,
    repo_id: RepoId,
    spec: RepoSpec,
    repo: Arc<dyn GitRepository>,
) -> Vec<Effect> {
    repos.insert(repo_id, repo);

    let spec = RepoSpec {
        workdir: normalize_repo_path(spec.workdir),
    };
    if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
        repo_state.spec = spec;
        repo_state.open = Loadable::Ready(());
        repo_state.set_head_branch(Loadable::Loading);
        repo_state.upstream_divergence = Loadable::Loading;
        repo_state.set_branches(Loadable::Loading);
        repo_state.tags = Loadable::Loading;
        repo_state.set_remotes(Loadable::Loading);
        repo_state.set_remote_branches(Loadable::Loading);
        repo_state.status = Loadable::Loading;
        repo_state.log = Loadable::Loading;
        repo_state.log_loading_more = false;
        repo_state.set_stashes(Loadable::Loading);
        repo_state.reflog = Loadable::Loading;
        repo_state.rebase_in_progress = Loadable::Loading;
        repo_state.merge_commit_message = Loadable::Loading;
        repo_state.file_history_path = None;
        repo_state.file_history = Loadable::NotLoaded;
        repo_state.blame_path = None;
        repo_state.blame_rev = None;
        repo_state.blame = Loadable::NotLoaded;
        repo_state.worktrees = Loadable::NotLoaded;
        repo_state.submodules = Loadable::NotLoaded;
        repo_state.selected_commit = None;
        repo_state.commit_details = Loadable::NotLoaded;
        repo_state.diff_target = None;
        repo_state.diff = Loadable::NotLoaded;
        repo_state.diff_file = Loadable::NotLoaded;
        repo_state.diff_file_image = Loadable::NotLoaded;
        repo_state.last_error = None;

        return refresh_full_effects(repo_state);
    }

    Vec::new()
}

pub(super) fn repo_opened_err(
    repos: &mut HashMap<RepoId, Arc<dyn GitRepository>>,
    state: &mut AppState,
    repo_id: RepoId,
    spec: RepoSpec,
    error: Error,
) -> Vec<Effect> {
    let spec = RepoSpec {
        workdir: normalize_repo_path(spec.workdir),
    };
    if matches!(error.kind(), gitgpui_core::error::ErrorKind::NotARepository) {
        push_notification(
            state,
            AppNotificationKind::Error,
            format!("Folder is not a git repository: {}", spec.workdir.display()),
        );

        repos.remove(&repo_id);
        if let Some(ix) = state.repos.iter().position(|r| r.id == repo_id) {
            let was_active = state.active_repo == Some(repo_id);
            state.repos.remove(ix);
            if was_active {
                state.active_repo = if ix > 0 {
                    state.repos.get(ix - 1).map(|r| r.id)
                } else {
                    state.repos.get(ix).map(|r| r.id)
                };
            }
            let _ = session::persist_from_state(state);
        }
        return Vec::new();
    }

    if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
        repo_state.spec = spec;
        repo_state.open = Loadable::Error(error.to_string());
        repo_state.last_error = Some(error.to_string());
        push_diagnostic(repo_state, DiagnosticKind::Error, error.to_string());
    }
    Vec::new()
}
