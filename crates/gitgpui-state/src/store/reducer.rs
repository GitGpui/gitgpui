use crate::model::{
    AppNotification, AppNotificationKind, AppState, CloneOpState, CloneOpStatus, CommandLogEntry,
    DiagnosticEntry, DiagnosticKind, Loadable, RepoId, RepoLoadsInFlight, RepoState,
};
use crate::msg::RepoExternalChange;
use crate::msg::{Effect, Msg};
use crate::session;
use gitgpui_core::domain::{DiffTarget, RepoSpec};
use gitgpui_core::error::Error;
use gitgpui_core::services::{CommandOutput, GitRepository};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

fn is_supported_image_path(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "tif" | "tiff"
    )
}

fn diff_target_wants_image_preview(target: &DiffTarget) -> bool {
    match target {
        DiffTarget::WorkingTree { path, .. } => is_supported_image_path(path),
        DiffTarget::Commit {
            path: Some(path), ..
        } => is_supported_image_path(path),
        _ => false,
    }
}

fn diff_reload_effects(repo_id: RepoId, target: DiffTarget) -> Vec<Effect> {
    let supports_file = matches!(
        &target,
        DiffTarget::WorkingTree { .. } | DiffTarget::Commit { path: Some(_), .. }
    );
    let wants_image = diff_target_wants_image_preview(&target);

    let mut effects = vec![Effect::LoadDiff {
        repo_id,
        target: target.clone(),
    }];
    if supports_file {
        if wants_image {
            effects.push(Effect::LoadDiffFileImage { repo_id, target });
        } else {
            effects.push(Effect::LoadDiffFile { repo_id, target });
        }
    }

    effects
}

fn refresh_primary_effects(repo_state: &mut RepoState) -> Vec<Effect> {
    let repo_id = repo_state.id;
    let mut effects = Vec::new();

    if repo_state
        .loads_in_flight
        .request(RepoLoadsInFlight::HEAD_BRANCH)
    {
        effects.push(Effect::LoadHeadBranch { repo_id });
    }
    if repo_state
        .loads_in_flight
        .request(RepoLoadsInFlight::UPSTREAM_DIVERGENCE)
    {
        effects.push(Effect::LoadUpstreamDivergence { repo_id });
    }
    if repo_state
        .loads_in_flight
        .request(RepoLoadsInFlight::REBASE_STATE)
    {
        effects.push(Effect::LoadRebaseState { repo_id });
    }
    if repo_state
        .loads_in_flight
        .request(RepoLoadsInFlight::STATUS)
    {
        effects.push(Effect::LoadStatus { repo_id });
    }
    if repo_state
        .loads_in_flight
        .request_log(repo_state.history_scope, 200, None)
    {
        // Block pagination while a refresh log load is in flight, to avoid concurrent LogLoaded
        // merges with different cursors.
        repo_state.log_loading_more = false;
        effects.push(Effect::LoadLog {
            repo_id,
            scope: repo_state.history_scope,
            limit: 200,
            cursor: None,
        });
    }

    effects
}

fn refresh_full_effects(repo_state: &mut RepoState) -> Vec<Effect> {
    let repo_id = repo_state.id;
    let mut effects = Vec::new();

    // Keep ordering stable with historical `refresh_effects` to minimize behavioral churn and
    // make the refresh fan-out predictable.
    if repo_state
        .loads_in_flight
        .request(RepoLoadsInFlight::HEAD_BRANCH)
    {
        effects.push(Effect::LoadHeadBranch { repo_id });
    }
    if repo_state
        .loads_in_flight
        .request(RepoLoadsInFlight::UPSTREAM_DIVERGENCE)
    {
        effects.push(Effect::LoadUpstreamDivergence { repo_id });
    }
    if repo_state
        .loads_in_flight
        .request(RepoLoadsInFlight::BRANCHES)
    {
        effects.push(Effect::LoadBranches { repo_id });
    }
    if repo_state.loads_in_flight.request(RepoLoadsInFlight::TAGS) {
        effects.push(Effect::LoadTags { repo_id });
    }
    if repo_state
        .loads_in_flight
        .request(RepoLoadsInFlight::REMOTES)
    {
        effects.push(Effect::LoadRemotes { repo_id });
    }
    if repo_state
        .loads_in_flight
        .request(RepoLoadsInFlight::REMOTE_BRANCHES)
    {
        effects.push(Effect::LoadRemoteBranches { repo_id });
    }
    if repo_state
        .loads_in_flight
        .request(RepoLoadsInFlight::STATUS)
    {
        effects.push(Effect::LoadStatus { repo_id });
    }
    if repo_state
        .loads_in_flight
        .request(RepoLoadsInFlight::STASHES)
    {
        effects.push(Effect::LoadStashes { repo_id, limit: 50 });
    }
    if repo_state
        .loads_in_flight
        .request(RepoLoadsInFlight::REFLOG)
    {
        effects.push(Effect::LoadReflog {
            repo_id,
            limit: 200,
        });
    }
    if repo_state
        .loads_in_flight
        .request(RepoLoadsInFlight::REBASE_STATE)
    {
        effects.push(Effect::LoadRebaseState { repo_id });
    }
    if repo_state
        .loads_in_flight
        .request_log(repo_state.history_scope, 200, None)
    {
        repo_state.log_loading_more = false;
        effects.push(Effect::LoadLog {
            repo_id,
            scope: repo_state.history_scope,
            limit: 200,
            cursor: None,
        });
    }

    effects
}

pub(super) fn reduce(
    repos: &mut HashMap<RepoId, Arc<dyn GitRepository>>,
    id_alloc: &AtomicU64,
    state: &mut AppState,
    msg: Msg,
) -> Vec<Effect> {
    match msg {
        Msg::OpenRepo(path) => {
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
                .push(RepoState::new_opening(repo_id, spec.clone()));
            state.active_repo = Some(repo_id);
            let effects = vec![Effect::OpenRepo {
                repo_id,
                path: spec.workdir.clone(),
            }];
            let _ = session::persist_from_state(state);
            effects
        }

        Msg::RestoreSession {
            open_repos,
            active_repo,
        } => {
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
                    .push(RepoState::new_opening(repo_id, spec.clone()));
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

        Msg::CloseRepo { repo_id } => {
            state.repos.retain(|r| r.id != repo_id);
            repos.remove(&repo_id);
            if state.active_repo == Some(repo_id) {
                state.active_repo = state.repos.first().map(|r| r.id);
            }
            let _ = session::persist_from_state(state);
            Vec::new()
        }

        Msg::SetActiveRepo { repo_id } => {
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

        Msg::ReloadRepo { repo_id } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };

            repo_state.head_branch = Loadable::Loading;
            repo_state.branches = Loadable::Loading;
            repo_state.tags = Loadable::Loading;
            repo_state.remotes = Loadable::Loading;
            repo_state.remote_branches = Loadable::Loading;
            repo_state.status = Loadable::Loading;
            repo_state.log = Loadable::Loading;
            repo_state.log_loading_more = false;
            repo_state.stashes = Loadable::Loading;
            repo_state.reflog = Loadable::Loading;
            repo_state.rebase_in_progress = Loadable::Loading;
            repo_state.file_history_path = None;
            repo_state.file_history = Loadable::NotLoaded;
            repo_state.blame_path = None;
            repo_state.blame_rev = None;
            repo_state.blame = Loadable::NotLoaded;
            repo_state.worktrees = Loadable::NotLoaded;
            repo_state.submodules = Loadable::NotLoaded;
            repo_state.selected_commit = None;
            repo_state.commit_details = Loadable::NotLoaded;

            refresh_full_effects(repo_state)
        }

        Msg::RepoExternallyChanged { repo_id, change } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };

            // Coalesce refreshes while a refresh is already in flight.
            let mut effects = match change {
                RepoExternalChange::Worktree => {
                    if repo_state
                        .loads_in_flight
                        .request(RepoLoadsInFlight::STATUS)
                    {
                        vec![Effect::LoadStatus { repo_id }]
                    } else {
                        Vec::new()
                    }
                }
                RepoExternalChange::GitState | RepoExternalChange::Both => {
                    refresh_primary_effects(repo_state)
                }
            };

            if let Some(target) = repo_state.diff_target.clone() {
                effects.extend(diff_reload_effects(repo_id, target));
            }

            effects
        }

        Msg::SetHistoryScope { repo_id, scope } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };

            if repo_state.history_scope == scope {
                return Vec::new();
            }

            repo_state.history_scope = scope;
            repo_state.log = Loadable::Loading;
            repo_state.log_loading_more = false;

            if repo_state.loads_in_flight.request_log(scope, 200, None) {
                vec![Effect::LoadLog {
                    repo_id,
                    scope,
                    limit: 200,
                    cursor: None,
                }]
            } else {
                Vec::new()
            }
        }

        Msg::LoadMoreHistory { repo_id } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };

            if repo_state.log_loading_more {
                return Vec::new();
            }

            let Loadable::Ready(page) = &repo_state.log else {
                return Vec::new();
            };
            let Some(cursor) = page.next_cursor.clone() else {
                return Vec::new();
            };

            repo_state.log_loading_more = true;
            if repo_state.loads_in_flight.request_log(
                repo_state.history_scope,
                200,
                Some(cursor.clone()),
            ) {
                vec![Effect::LoadLog {
                    repo_id,
                    scope: repo_state.history_scope,
                    limit: 200,
                    cursor: Some(cursor),
                }]
            } else {
                Vec::new()
            }
        }

        Msg::RebaseStateLoaded { repo_id, result } => {
            let mut effects = Vec::new();
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.rebase_in_progress = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
                if repo_state
                    .loads_in_flight
                    .finish(RepoLoadsInFlight::REBASE_STATE)
                {
                    effects.push(Effect::LoadRebaseState { repo_id });
                }
            }
            effects
        }

        Msg::FileHistoryLoaded {
            repo_id,
            path,
            result,
        } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id)
                && repo_state.file_history_path.as_ref() == Some(&path)
            {
                repo_state.file_history = match result {
                    Ok(v) => Loadable::Ready(Arc::new(v)),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::BlameLoaded {
            repo_id,
            path,
            rev,
            result,
        } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id)
                && repo_state.blame_path.as_ref() == Some(&path)
                && repo_state.blame_rev == rev
            {
                repo_state.blame = match result {
                    Ok(v) => Loadable::Ready(Arc::new(v)),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::ConflictFileLoaded {
            repo_id,
            path,
            result,
        } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id)
                && repo_state.conflict_file_path.as_ref() == Some(&path)
            {
                repo_state.conflict_file = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::WorktreesLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.worktrees = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::SubmodulesLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.submodules = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::SelectCommit { repo_id, commit_id } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };

            if repo_state.selected_commit.as_ref() == Some(&commit_id) {
                return Vec::new();
            }

            repo_state.selected_commit = Some(commit_id.clone());
            let already_loaded = matches!(
                &repo_state.commit_details,
                Loadable::Ready(details) if details.id == commit_id
            );
            if already_loaded {
                return Vec::new();
            }

            if matches!(
                repo_state.commit_details,
                Loadable::Error(_) | Loadable::NotLoaded
            ) {
                repo_state.commit_details = Loadable::NotLoaded;
            }
            vec![Effect::LoadCommitDetails { repo_id, commit_id }]
        }

        Msg::ClearCommitSelection { repo_id } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };

            repo_state.selected_commit = None;
            repo_state.commit_details = Loadable::NotLoaded;
            Vec::new()
        }

        Msg::SelectDiff { repo_id, target } => {
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

            let mut effects = vec![Effect::LoadDiff {
                repo_id,
                target: target.clone(),
            }];
            if supports_file {
                if wants_image {
                    effects.push(Effect::LoadDiffFileImage { repo_id, target });
                } else {
                    effects.push(Effect::LoadDiffFile { repo_id, target });
                }
            }
            effects
        }

        Msg::ClearDiffSelection { repo_id } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };

            repo_state.diff_target = None;
            repo_state.diff = Loadable::NotLoaded;
            repo_state.diff_file = Loadable::NotLoaded;
            repo_state.diff_file_image = Loadable::NotLoaded;
            Vec::new()
        }

        Msg::LoadStashes { repo_id } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };
            repo_state.stashes = Loadable::Loading;
            if repo_state
                .loads_in_flight
                .request(RepoLoadsInFlight::STASHES)
            {
                vec![Effect::LoadStashes { repo_id, limit: 50 }]
            } else {
                Vec::new()
            }
        }

        Msg::LoadConflictFile { repo_id, path } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };
            repo_state.conflict_file_path = Some(path.clone());
            repo_state.conflict_file = Loadable::Loading;
            vec![Effect::LoadConflictFile { repo_id, path }]
        }

        Msg::LoadReflog { repo_id } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };
            repo_state.reflog = Loadable::Loading;
            if repo_state
                .loads_in_flight
                .request(RepoLoadsInFlight::REFLOG)
            {
                vec![Effect::LoadReflog {
                    repo_id,
                    limit: 200,
                }]
            } else {
                Vec::new()
            }
        }

        Msg::LoadFileHistory {
            repo_id,
            path,
            limit,
        } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };
            repo_state.file_history_path = Some(path.clone());
            repo_state.file_history = Loadable::Loading;
            vec![Effect::LoadFileHistory {
                repo_id,
                path,
                limit,
            }]
        }

        Msg::LoadBlame { repo_id, path, rev } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };
            repo_state.blame_path = Some(path.clone());
            repo_state.blame_rev = rev.clone();
            repo_state.blame = Loadable::Loading;
            vec![Effect::LoadBlame { repo_id, path, rev }]
        }

        Msg::LoadWorktrees { repo_id } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };
            repo_state.worktrees = Loadable::Loading;
            vec![Effect::LoadWorktrees { repo_id }]
        }

        Msg::LoadSubmodules { repo_id } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };
            repo_state.submodules = Loadable::Loading;
            vec![Effect::LoadSubmodules { repo_id }]
        }

        Msg::StageHunk { repo_id, patch } => vec![Effect::StageHunk { repo_id, patch }],
        Msg::UnstageHunk { repo_id, patch } => vec![Effect::UnstageHunk { repo_id, patch }],

        Msg::CheckoutBranch { repo_id, name } => vec![Effect::CheckoutBranch { repo_id, name }],
        Msg::CheckoutRemoteBranch {
            repo_id,
            remote,
            name,
        } => vec![Effect::CheckoutRemoteBranch {
            repo_id,
            remote,
            name,
        }],
        Msg::CheckoutCommit { repo_id, commit_id } => {
            vec![Effect::CheckoutCommit { repo_id, commit_id }]
        }
        Msg::CherryPickCommit { repo_id, commit_id } => {
            vec![Effect::CherryPickCommit { repo_id, commit_id }]
        }
        Msg::RevertCommit { repo_id, commit_id } => {
            vec![Effect::RevertCommit { repo_id, commit_id }]
        }
        Msg::CreateBranch { repo_id, name } => vec![Effect::CreateBranch { repo_id, name }],
        Msg::CreateBranchAndCheckout { repo_id, name } => {
            vec![Effect::CreateBranchAndCheckout { repo_id, name }]
        }
        Msg::DeleteBranch { repo_id, name } => vec![Effect::DeleteBranch { repo_id, name }],
        Msg::CloneRepo { url, dest } => {
            state.clone = Some(CloneOpState {
                url: url.clone(),
                dest: dest.clone(),
                status: CloneOpStatus::Running,
                seq: 0,
                output_tail: Vec::new(),
            });
            vec![Effect::CloneRepo { url, dest }]
        }
        Msg::CloneRepoProgress { dest, line } => {
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
        Msg::CloneRepoFinished { url, dest, result } => {
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
        Msg::ExportPatch {
            repo_id,
            commit_id,
            dest,
        } => vec![Effect::ExportPatch {
            repo_id,
            commit_id,
            dest,
        }],
        Msg::ApplyPatch { repo_id, patch } => vec![Effect::ApplyPatch { repo_id, patch }],
        Msg::AddWorktree {
            repo_id,
            path,
            reference,
        } => vec![Effect::AddWorktree {
            repo_id,
            path,
            reference,
        }],
        Msg::RemoveWorktree { repo_id, path } => vec![Effect::RemoveWorktree { repo_id, path }],
        Msg::AddSubmodule { repo_id, url, path } => {
            vec![Effect::AddSubmodule { repo_id, url, path }]
        }
        Msg::UpdateSubmodules { repo_id } => vec![Effect::UpdateSubmodules { repo_id }],
        Msg::RemoveSubmodule { repo_id, path } => vec![Effect::RemoveSubmodule { repo_id, path }],
        Msg::StagePath { repo_id, path } => vec![Effect::StagePath { repo_id, path }],
        Msg::StagePaths { repo_id, paths } => vec![Effect::StagePaths { repo_id, paths }],
        Msg::UnstagePath { repo_id, path } => vec![Effect::UnstagePath { repo_id, path }],
        Msg::UnstagePaths { repo_id, paths } => vec![Effect::UnstagePaths { repo_id, paths }],
        Msg::DiscardWorktreeChangesPath { repo_id, path } => {
            vec![Effect::DiscardWorktreeChangesPath { repo_id, path }]
        }
        Msg::DiscardWorktreeChangesPaths { repo_id, paths } => {
            vec![Effect::DiscardWorktreeChangesPaths { repo_id, paths }]
        }

        Msg::SaveWorktreeFile {
            repo_id,
            path,
            contents,
            stage,
        } => vec![Effect::SaveWorktreeFile {
            repo_id,
            path,
            contents,
            stage,
        }],
        Msg::Commit { repo_id, message } => vec![Effect::Commit { repo_id, message }],
        Msg::CommitAmend { repo_id, message } => vec![Effect::CommitAmend { repo_id, message }],
        Msg::FetchAll { repo_id } => vec![Effect::FetchAll { repo_id }],
        Msg::Pull { repo_id, mode } => vec![Effect::Pull { repo_id, mode }],
        Msg::PullBranch {
            repo_id,
            remote,
            branch,
        } => vec![Effect::PullBranch {
            repo_id,
            remote,
            branch,
        }],
        Msg::MergeRef { repo_id, reference } => vec![Effect::MergeRef { repo_id, reference }],
        Msg::Push { repo_id } => vec![Effect::Push { repo_id }],
        Msg::ForcePush { repo_id } => vec![Effect::ForcePush { repo_id }],
        Msg::PushSetUpstream {
            repo_id,
            remote,
            branch,
        } => vec![Effect::PushSetUpstream {
            repo_id,
            remote,
            branch,
        }],
        Msg::Reset {
            repo_id,
            target,
            mode,
        } => vec![Effect::Reset {
            repo_id,
            target,
            mode,
        }],
        Msg::Rebase { repo_id, onto } => vec![Effect::Rebase { repo_id, onto }],
        Msg::RebaseContinue { repo_id } => vec![Effect::RebaseContinue { repo_id }],
        Msg::RebaseAbort { repo_id } => vec![Effect::RebaseAbort { repo_id }],
        Msg::CreateTag {
            repo_id,
            name,
            target,
        } => vec![Effect::CreateTag {
            repo_id,
            name,
            target,
        }],
        Msg::DeleteTag { repo_id, name } => vec![Effect::DeleteTag { repo_id, name }],
        Msg::AddRemote { repo_id, name, url } => vec![Effect::AddRemote { repo_id, name, url }],
        Msg::RemoveRemote { repo_id, name } => vec![Effect::RemoveRemote { repo_id, name }],
        Msg::SetRemoteUrl {
            repo_id,
            name,
            url,
            kind,
        } => vec![Effect::SetRemoteUrl {
            repo_id,
            name,
            url,
            kind,
        }],
        Msg::CheckoutConflictSide {
            repo_id,
            path,
            side,
        } => vec![Effect::CheckoutConflictSide {
            repo_id,
            path,
            side,
        }],
        Msg::Stash {
            repo_id,
            message,
            include_untracked,
        } => vec![Effect::Stash {
            repo_id,
            message,
            include_untracked,
        }],
        Msg::ApplyStash { repo_id, index } => vec![Effect::ApplyStash { repo_id, index }],
        Msg::DropStash { repo_id, index } => vec![Effect::DropStash { repo_id, index }],
        Msg::PopStash { repo_id, index } => vec![Effect::PopStash { repo_id, index }],

        Msg::RepoOpenedOk {
            repo_id,
            spec,
            repo,
        } => {
            repos.insert(repo_id, repo);

            let spec = RepoSpec {
                workdir: normalize_repo_path(spec.workdir),
            };
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.spec = spec;
                repo_state.open = Loadable::Ready(());
                repo_state.head_branch = Loadable::Loading;
                repo_state.upstream_divergence = Loadable::Loading;
                repo_state.branches = Loadable::Loading;
                repo_state.tags = Loadable::Loading;
                repo_state.remotes = Loadable::Loading;
                repo_state.remote_branches = Loadable::Loading;
                repo_state.status = Loadable::Loading;
                repo_state.log = Loadable::Loading;
                repo_state.log_loading_more = false;
                repo_state.stashes = Loadable::Loading;
                repo_state.reflog = Loadable::Loading;
                repo_state.rebase_in_progress = Loadable::Loading;
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

        Msg::RepoOpenedErr {
            repo_id,
            spec,
            error,
        } => {
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

        Msg::BranchesLoaded { repo_id, result } => {
            let mut effects = Vec::new();
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.branches = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
                if repo_state
                    .loads_in_flight
                    .finish(RepoLoadsInFlight::BRANCHES)
                {
                    effects.push(Effect::LoadBranches { repo_id });
                }
            }
            effects
        }

        Msg::RemotesLoaded { repo_id, result } => {
            let mut effects = Vec::new();
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.remotes = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
                if repo_state
                    .loads_in_flight
                    .finish(RepoLoadsInFlight::REMOTES)
                {
                    effects.push(Effect::LoadRemotes { repo_id });
                }
            }
            effects
        }

        Msg::RemoteBranchesLoaded { repo_id, result } => {
            let mut effects = Vec::new();
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.remote_branches = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
                if repo_state
                    .loads_in_flight
                    .finish(RepoLoadsInFlight::REMOTE_BRANCHES)
                {
                    effects.push(Effect::LoadRemoteBranches { repo_id });
                }
            }
            effects
        }

        Msg::StatusLoaded { repo_id, result } => {
            let mut effects = Vec::new();
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.status = match result {
                    Ok(v) => Loadable::Ready(Arc::new(v)),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
                if repo_state.loads_in_flight.finish(RepoLoadsInFlight::STATUS) {
                    effects.push(Effect::LoadStatus { repo_id });
                }
            }
            effects
        }

        Msg::HeadBranchLoaded { repo_id, result } => {
            let mut effects = Vec::new();
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.head_branch = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
                if repo_state
                    .loads_in_flight
                    .finish(RepoLoadsInFlight::HEAD_BRANCH)
                {
                    effects.push(Effect::LoadHeadBranch { repo_id });
                }
            }
            effects
        }

        Msg::UpstreamDivergenceLoaded { repo_id, result } => {
            let mut effects = Vec::new();
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.upstream_divergence = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
                if repo_state
                    .loads_in_flight
                    .finish(RepoLoadsInFlight::UPSTREAM_DIVERGENCE)
                {
                    effects.push(Effect::LoadUpstreamDivergence { repo_id });
                }
            }
            effects
        }

        Msg::LogLoaded {
            repo_id,
            scope,
            cursor,
            result,
        } => {
            let mut effects = Vec::new();
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                let is_load_more = cursor.is_some();

                if repo_state.history_scope != scope {
                    if is_load_more {
                        repo_state.log_loading_more = false;
                    }
                    if let Some(next) = repo_state.loads_in_flight.finish_log() {
                        repo_state.log_loading_more = next.cursor.is_some();
                        effects.push(Effect::LoadLog {
                            repo_id,
                            scope: next.scope,
                            limit: next.limit,
                            cursor: next.cursor,
                        });
                    }
                    return effects;
                }

                match result {
                    Ok(mut page) => {
                        if is_load_more && let Loadable::Ready(existing) = &mut repo_state.log {
                            let existing = Arc::make_mut(existing);
                            existing.commits.extend(page.commits.drain(..));
                            existing.next_cursor = page.next_cursor;
                        } else {
                            repo_state.log = Loadable::Ready(Arc::new(page));
                        }
                    }
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        if !is_load_more {
                            repo_state.log = Loadable::Error(e.to_string());
                        }
                    }
                }

                if is_load_more {
                    repo_state.log_loading_more = false;
                }

                if let Some(next) = repo_state.loads_in_flight.finish_log() {
                    repo_state.log_loading_more = next.cursor.is_some();
                    effects.push(Effect::LoadLog {
                        repo_id,
                        scope: next.scope,
                        limit: next.limit,
                        cursor: next.cursor,
                    });
                }
            }
            effects
        }

        Msg::TagsLoaded { repo_id, result } => {
            let mut effects = Vec::new();
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.tags = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        if matches!(e.kind(), gitgpui_core::error::ErrorKind::Unsupported(_)) {
                            Loadable::Ready(Vec::new())
                        } else {
                            push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                            Loadable::Error(e.to_string())
                        }
                    }
                };
                if repo_state.loads_in_flight.finish(RepoLoadsInFlight::TAGS) {
                    effects.push(Effect::LoadTags { repo_id });
                }
            }
            effects
        }

        Msg::StashesLoaded { repo_id, result } => {
            let mut effects = Vec::new();
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.stashes = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
                if repo_state
                    .loads_in_flight
                    .finish(RepoLoadsInFlight::STASHES)
                {
                    effects.push(Effect::LoadStashes { repo_id, limit: 50 });
                }
            }
            effects
        }

        Msg::ReflogLoaded { repo_id, result } => {
            let mut effects = Vec::new();
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.reflog = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
                if repo_state.loads_in_flight.finish(RepoLoadsInFlight::REFLOG) {
                    effects.push(Effect::LoadReflog {
                        repo_id,
                        limit: 200,
                    });
                }
            }
            effects
        }

        Msg::CommitDetailsLoaded {
            repo_id,
            commit_id,
            result,
        } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id)
                && repo_state.selected_commit.as_ref() == Some(&commit_id)
            {
                repo_state.commit_details = match result {
                    Ok(v) => Loadable::Ready(Arc::new(v)),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::DiffLoaded {
            repo_id,
            target,
            result,
        } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id)
                && repo_state.diff_target.as_ref() == Some(&target)
            {
                repo_state.diff_rev = repo_state.diff_rev.wrapping_add(1);
                repo_state.diff = match result {
                    Ok(v) => Loadable::Ready(Arc::new(v)),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::DiffFileLoaded {
            repo_id,
            target,
            result,
        } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id)
                && repo_state.diff_target.as_ref() == Some(&target)
            {
                repo_state.diff_file_rev = repo_state.diff_file_rev.wrapping_add(1);
                repo_state.diff_file = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::DiffFileImageLoaded {
            repo_id,
            target,
            result,
        } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id)
                && repo_state.diff_target.as_ref() == Some(&target)
            {
                repo_state.diff_file_rev = repo_state.diff_file_rev.wrapping_add(1);
                repo_state.diff_file_image = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::RepoActionFinished { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                match result {
                    Ok(()) => repo_state.last_error = None,
                    Err(e) => {
                        repo_state.last_error = Some(e.to_string());
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                    }
                }
            }
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };

            let mut effects = refresh_primary_effects(repo_state);
            if let Some(target) = repo_state.diff_target.clone() {
                effects.extend(diff_reload_effects(repo_id, target));
            }
            effects
        }

        Msg::CommitFinished { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                match result {
                    Ok(()) => {
                        repo_state.last_error = None;
                        repo_state.diff_target = None;
                        repo_state.diff = Loadable::NotLoaded;
                        repo_state.diff_file = Loadable::NotLoaded;
                        repo_state.diff_file_image = Loadable::NotLoaded;
                        push_action_log(
                            repo_state,
                            true,
                            "Commit".to_string(),
                            "Commit: Completed".to_string(),
                            None,
                        );
                    }
                    Err(e) => {
                        repo_state.last_error = Some(e.to_string());
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        push_action_log(
                            repo_state,
                            false,
                            "Commit".to_string(),
                            format!("Commit failed: {e}"),
                            Some(&e),
                        );
                    }
                }
            }
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };
            refresh_primary_effects(repo_state)
        }

        Msg::CommitAmendFinished { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                match result {
                    Ok(()) => {
                        repo_state.last_error = None;
                        repo_state.diff_target = None;
                        repo_state.diff = Loadable::NotLoaded;
                        repo_state.diff_file = Loadable::NotLoaded;
                        repo_state.diff_file_image = Loadable::NotLoaded;
                        push_action_log(
                            repo_state,
                            true,
                            "Amend".to_string(),
                            "Amend: Completed".to_string(),
                            None,
                        );
                    }
                    Err(e) => {
                        repo_state.last_error = Some(e.to_string());
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        push_action_log(
                            repo_state,
                            false,
                            "Amend".to_string(),
                            format!("Amend failed: {e}"),
                            Some(&e),
                        );
                    }
                }
            }
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };
            refresh_primary_effects(repo_state)
        }

        Msg::RepoCommandFinished {
            repo_id,
            command,
            result,
        } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                match result {
                    Ok(output) => {
                        repo_state.last_error = None;
                        if matches!(
                            &command,
                            crate::msg::RepoCommandKind::Reset { .. }
                                | crate::msg::RepoCommandKind::Rebase { .. }
                                | crate::msg::RepoCommandKind::RebaseContinue
                                | crate::msg::RepoCommandKind::RebaseAbort
                        ) {
                            repo_state.diff_target = None;
                            repo_state.diff = Loadable::NotLoaded;
                            repo_state.diff_file = Loadable::NotLoaded;
                            repo_state.diff_file_image = Loadable::NotLoaded;
                        }
                        push_command_log(repo_state, true, &command, &output, None);
                    }
                    Err(e) => {
                        repo_state.last_error = Some(e.to_string());
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        push_command_log(
                            repo_state,
                            false,
                            &command,
                            &CommandOutput::default(),
                            Some(&e),
                        );
                    }
                }
            }
            let mut extra_effects = Vec::new();
            if matches!(
                &command,
                crate::msg::RepoCommandKind::StageHunk | crate::msg::RepoCommandKind::UnstageHunk
            ) {
                if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id)
                    && let Some(target) = repo_state.diff_target.clone()
                {
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
                    extra_effects.push(Effect::LoadDiff {
                        repo_id,
                        target: target.clone(),
                    });
                    if supports_file {
                        if wants_image {
                            extra_effects.push(Effect::LoadDiffFileImage { repo_id, target });
                        } else {
                            extra_effects.push(Effect::LoadDiffFile { repo_id, target });
                        }
                    }
                }
            }
            let mut effects =
                if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                    refresh_full_effects(repo_state)
                } else {
                    Vec::new()
                };
            effects.extend(extra_effects);
            effects
        }
    }
}

fn dedup_paths_in_order(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for p in paths {
        if out.iter().any(|x| x == &p) {
            continue;
        }
        out.push(p);
    }
    out
}

pub(super) fn normalize_repo_path(path: PathBuf) -> PathBuf {
    let path = if path.is_relative() {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    } else {
        path
    };

    std::fs::canonicalize(&path).unwrap_or(path)
}

pub(super) fn push_notification(state: &mut AppState, kind: AppNotificationKind, message: String) {
    const MAX_NOTIFICATIONS: usize = 200;
    state.notifications.push(AppNotification {
        time: SystemTime::now(),
        kind,
        message,
    });
    if state.notifications.len() > MAX_NOTIFICATIONS {
        let extra = state.notifications.len() - MAX_NOTIFICATIONS;
        state.notifications.drain(0..extra);
    }
}

pub(super) fn push_diagnostic(repo_state: &mut RepoState, kind: DiagnosticKind, message: String) {
    const MAX_DIAGNOSTICS: usize = 200;
    repo_state.diagnostics.push(DiagnosticEntry {
        time: SystemTime::now(),
        kind,
        message,
    });
    if repo_state.diagnostics.len() > MAX_DIAGNOSTICS {
        let extra = repo_state.diagnostics.len() - MAX_DIAGNOSTICS;
        repo_state.diagnostics.drain(0..extra);
    }
}

fn push_command_log(
    repo_state: &mut RepoState,
    ok: bool,
    command: &crate::msg::RepoCommandKind,
    output: &CommandOutput,
    error: Option<&Error>,
) {
    const MAX_COMMAND_LOG: usize = 200;

    let (command_text, summary) = summarize_command(command, output, ok, error);

    repo_state.command_log.push(CommandLogEntry {
        time: SystemTime::now(),
        ok,
        command: command_text,
        summary,
        stdout: output.stdout.clone(),
        stderr: if output.stderr.is_empty() {
            error.map(|e| e.to_string()).unwrap_or_default()
        } else {
            output.stderr.clone()
        },
    });
    if repo_state.command_log.len() > MAX_COMMAND_LOG {
        let extra = repo_state.command_log.len() - MAX_COMMAND_LOG;
        repo_state.command_log.drain(0..extra);
    }
}

fn push_action_log(
    repo_state: &mut RepoState,
    ok: bool,
    command: String,
    summary: String,
    error: Option<&Error>,
) {
    const MAX_COMMAND_LOG: usize = 200;

    repo_state.command_log.push(CommandLogEntry {
        time: SystemTime::now(),
        ok,
        command,
        summary,
        stdout: String::new(),
        stderr: error.map(|e| e.to_string()).unwrap_or_default(),
    });
    if repo_state.command_log.len() > MAX_COMMAND_LOG {
        let extra = repo_state.command_log.len() - MAX_COMMAND_LOG;
        repo_state.command_log.drain(0..extra);
    }
}

fn summarize_command(
    command: &crate::msg::RepoCommandKind,
    output: &CommandOutput,
    ok: bool,
    error: Option<&Error>,
) -> (String, String) {
    use crate::msg::RepoCommandKind;
    use gitgpui_core::services::ConflictSide;

    if !ok {
        let label = match command {
            RepoCommandKind::FetchAll => "Fetch",
            RepoCommandKind::Pull { .. } => "Pull",
            RepoCommandKind::PullBranch { .. } => "Pull",
            RepoCommandKind::MergeRef { .. } => "Merge",
            RepoCommandKind::Push => "Push",
            RepoCommandKind::ForcePush => "Force push",
            RepoCommandKind::PushSetUpstream { .. } => "Push",
            RepoCommandKind::Reset { .. } => "Reset",
            RepoCommandKind::Rebase { .. } => "Rebase",
            RepoCommandKind::RebaseContinue => "Rebase",
            RepoCommandKind::RebaseAbort => "Rebase",
            RepoCommandKind::CreateTag { .. } => "Tag",
            RepoCommandKind::DeleteTag { .. } => "Tag",
            RepoCommandKind::AddRemote { .. } => "Remote",
            RepoCommandKind::RemoveRemote { .. } => "Remote",
            RepoCommandKind::SetRemoteUrl { .. } => "Remote",
            RepoCommandKind::CheckoutConflict { side, .. } => match side {
                ConflictSide::Ours => "Checkout ours",
                ConflictSide::Theirs => "Checkout theirs",
            },
            RepoCommandKind::SaveWorktreeFile { .. } => "Save file",
            RepoCommandKind::ExportPatch { .. } | RepoCommandKind::ApplyPatch { .. } => "Patch",
            RepoCommandKind::AddWorktree { .. } | RepoCommandKind::RemoveWorktree { .. } => {
                "Worktree"
            }
            RepoCommandKind::AddSubmodule { .. }
            | RepoCommandKind::UpdateSubmodules
            | RepoCommandKind::RemoveSubmodule { .. } => "Submodule",
            RepoCommandKind::StageHunk | RepoCommandKind::UnstageHunk => "Hunk",
        };
        return (
            output.command.clone().if_empty_else(|| label.to_string()),
            error
                .map(|e| format!("{label} failed: {e}"))
                .unwrap_or_else(|| format!("{label} failed")),
        );
    }

    let summary = match command {
        RepoCommandKind::FetchAll => {
            if output.stderr.trim().is_empty() && output.stdout.trim().is_empty() {
                "Fetch: Already up to date".to_string()
            } else {
                "Fetch: Synchronized".to_string()
            }
        }
        RepoCommandKind::Pull { .. } => {
            if output.stdout.contains("Already up to date") {
                "Pull: Already up to date".to_string()
            } else if output.stdout.starts_with("Updating") {
                "Pull: Fast-forwarded".to_string()
            } else if output.stdout.starts_with("Merge") {
                "Pull: Merged".to_string()
            } else if output.stdout.contains("Successfully rebased") {
                "Pull: Rebasing complete".to_string()
            } else {
                "Pull: Completed".to_string()
            }
        }
        RepoCommandKind::PullBranch { remote, branch } => {
            let base = if output.stdout.contains("Already up to date") {
                "Already up to date"
            } else if output.stdout.starts_with("Updating") {
                "Fast-forwarded"
            } else if output.stdout.starts_with("Merge") {
                "Merged"
            } else {
                "Completed"
            };
            format!("Pull {remote}/{branch}: {base}")
        }
        RepoCommandKind::MergeRef { reference } => {
            let base = if output.stdout.contains("Already up to date") {
                "Already up to date"
            } else if output.stdout.contains("Fast-forward")
                || output.stdout.starts_with("Updating")
            {
                "Fast-forwarded"
            } else if output.stdout.contains("Merge made by") {
                "Merged"
            } else {
                "Completed"
            };
            format!("Merge {reference}: {base}")
        }
        RepoCommandKind::Push => {
            if output.stderr.contains("Everything up-to-date") {
                "Push: Everything up-to-date".to_string()
            } else {
                "Push: Completed".to_string()
            }
        }
        RepoCommandKind::ForcePush => {
            if output.stderr.contains("Everything up-to-date") {
                "Force push: Everything up-to-date".to_string()
            } else {
                "Force push: Completed".to_string()
            }
        }
        RepoCommandKind::PushSetUpstream { remote, branch } => {
            let base = if output.stderr.contains("Everything up-to-date") {
                "Everything up-to-date"
            } else {
                "Completed"
            };
            format!("Push -u {remote}/{branch}: {base}")
        }
        RepoCommandKind::CheckoutConflict { side, .. } => match side {
            ConflictSide::Ours => "Resolved using ours".to_string(),
            ConflictSide::Theirs => "Resolved using theirs".to_string(),
        },
        RepoCommandKind::SaveWorktreeFile { path, stage } => {
            if *stage {
                format!("Saved and staged → {}", path.display())
            } else {
                format!("Saved → {}", path.display())
            }
        }
        RepoCommandKind::Reset { mode, target } => {
            let mode = match mode {
                gitgpui_core::services::ResetMode::Soft => "soft",
                gitgpui_core::services::ResetMode::Mixed => "mixed",
                gitgpui_core::services::ResetMode::Hard => "hard",
            };
            format!("Reset (--{mode}) {target}: Completed")
        }
        RepoCommandKind::Rebase { onto } => format!("Rebase onto {onto}: Completed"),
        RepoCommandKind::RebaseContinue => "Rebase: Continued".to_string(),
        RepoCommandKind::RebaseAbort => "Rebase: Aborted".to_string(),
        RepoCommandKind::CreateTag { name, target } => format!("Tag {name} → {target}: Created"),
        RepoCommandKind::DeleteTag { name } => format!("Tag {name}: Deleted"),
        RepoCommandKind::AddRemote { name, .. } => format!("Remote {name}: Added"),
        RepoCommandKind::RemoveRemote { name } => format!("Remote {name}: Removed"),
        RepoCommandKind::SetRemoteUrl { name, kind, .. } => {
            let kind = match kind {
                gitgpui_core::services::RemoteUrlKind::Fetch => "fetch",
                gitgpui_core::services::RemoteUrlKind::Push => "push",
            };
            format!("Remote {name} ({kind}): URL updated")
        }
        RepoCommandKind::ExportPatch { dest, .. } => {
            format!("Patch exported → {}", dest.display())
        }
        RepoCommandKind::ApplyPatch { patch } => format!("Patch applied → {}", patch.display()),
        RepoCommandKind::AddWorktree { path, reference } => {
            if let Some(reference) = reference {
                format!("Worktree added → {} ({reference})", path.display())
            } else {
                format!("Worktree added → {}", path.display())
            }
        }
        RepoCommandKind::RemoveWorktree { path } => {
            format!("Worktree removed → {}", path.display())
        }
        RepoCommandKind::AddSubmodule { path, .. } => {
            format!("Submodule added → {}", path.display())
        }
        RepoCommandKind::UpdateSubmodules => "Submodules: Updated".to_string(),
        RepoCommandKind::RemoveSubmodule { path } => {
            format!("Submodule removed → {}", path.display())
        }
        RepoCommandKind::StageHunk => "Hunk staged".to_string(),
        RepoCommandKind::UnstageHunk => "Hunk unstaged".to_string(),
    };

    (output.command.clone(), summary)
}

trait IfEmptyElse {
    fn if_empty_else(self, f: impl FnOnce() -> String) -> String;
}

impl IfEmptyElse for String {
    fn if_empty_else(self, f: impl FnOnce() -> String) -> String {
        if self.trim().is_empty() { f() } else { self }
    }
}

// Note: repo refresh scheduling is handled by `refresh_primary_effects` / `refresh_full_effects`
// with coalescing/backpressure (`RepoLoadsInFlight`).
