use crate::model::{
    AppState, CommandLogEntry, DiagnosticEntry, DiagnosticKind, Loadable, RepoId, RepoState,
};
use crate::msg::{Effect, Msg};
use crate::session;
use gitgpui_core::domain::{DiffTarget, RepoSpec};
use gitgpui_core::error::Error;
use gitgpui_core::services::{CommandOutput, GitRepository};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

pub(super) fn reduce(
    repos: &mut HashMap<RepoId, Arc<dyn GitRepository>>,
    id_alloc: &AtomicU64,
    state: &mut AppState,
    msg: Msg,
) -> Vec<Effect> {
    match msg {
        Msg::OpenRepo(path) => {
            let path = normalize_repo_path(path);
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
            if state.repos.iter().any(|r| r.id == repo_id) {
                state.active_repo = Some(repo_id);
                let _ = session::persist_from_state(state);
            }
            Vec::new()
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
            repo_state.selected_commit = None;
            repo_state.commit_details = Loadable::NotLoaded;

            refresh_effects(repo_id, repo_state.history_scope)
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

            vec![Effect::LoadLog {
                repo_id,
                scope,
                limit: 200,
                cursor: None,
            }]
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
            vec![Effect::LoadLog {
                repo_id,
                scope: repo_state.history_scope,
                limit: 200,
                cursor: Some(cursor),
            }]
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
            repo_state.diff_file = if supports_file {
                Loadable::Loading
            } else {
                Loadable::NotLoaded
            };

            let mut effects = vec![Effect::LoadDiff {
                repo_id,
                target: target.clone(),
            }];
            if supports_file {
                effects.push(Effect::LoadDiffFile { repo_id, target });
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
            Vec::new()
        }

        Msg::LoadStashes { repo_id } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };
            repo_state.stashes = Loadable::Loading;
            vec![Effect::LoadStashes { repo_id, limit: 50 }]
        }

        Msg::LoadReflog { repo_id } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };
            repo_state.reflog = Loadable::Loading;
            vec![Effect::LoadReflog {
                repo_id,
                limit: 200,
            }]
        }

        Msg::CheckoutBranch { repo_id, name } => vec![Effect::CheckoutBranch { repo_id, name }],
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
        Msg::StagePath { repo_id, path } => vec![Effect::StagePath { repo_id, path }],
        Msg::StagePaths { repo_id, paths } => vec![Effect::StagePaths { repo_id, paths }],
        Msg::UnstagePath { repo_id, path } => vec![Effect::UnstagePath { repo_id, path }],
        Msg::UnstagePaths { repo_id, paths } => vec![Effect::UnstagePaths { repo_id, paths }],
        Msg::Commit { repo_id, message } => vec![Effect::Commit { repo_id, message }],
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
                repo_state.selected_commit = None;
                repo_state.commit_details = Loadable::NotLoaded;
                repo_state.diff_target = None;
                repo_state.diff = Loadable::NotLoaded;
                repo_state.last_error = None;

                return refresh_effects(repo_id, repo_state.history_scope);
            }

            refresh_effects(repo_id, gitgpui_core::domain::LogScope::CurrentBranch)
        }

        Msg::RepoOpenedErr {
            repo_id,
            spec,
            error,
        } => {
            let spec = RepoSpec {
                workdir: normalize_repo_path(spec.workdir),
            };
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.spec = spec;
                repo_state.open = Loadable::Error(error.to_string());
                repo_state.last_error = Some(error.to_string());
                push_diagnostic(repo_state, DiagnosticKind::Error, error.to_string());
            }
            Vec::new()
        }

        Msg::BranchesLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.branches = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::RemotesLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.remotes = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::RemoteBranchesLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.remote_branches = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::StatusLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.status = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::HeadBranchLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.head_branch = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::UpstreamDivergenceLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.upstream_divergence = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::LogLoaded {
            repo_id,
            scope,
            result,
        } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                if repo_state.history_scope != scope {
                    repo_state.log_loading_more = false;
                    return Vec::new();
                }
                let loading_more = repo_state.log_loading_more;
                repo_state.log_loading_more = false;

                match result {
                    Ok(mut page) => {
                        if loading_more
                            && let Loadable::Ready(existing) = &mut repo_state.log
                        {
                            existing.commits.extend(page.commits.drain(..));
                            existing.next_cursor = page.next_cursor;
                        } else {
                            repo_state.log = Loadable::Ready(page);
                        }
                    }
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        if !loading_more {
                            repo_state.log = Loadable::Error(e.to_string());
                        }
                    }
                }
            }
            Vec::new()
        }

        Msg::TagsLoaded { repo_id, result } => {
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
            }
            Vec::new()
        }

        Msg::StashesLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.stashes = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::ReflogLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.reflog = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
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
                    Ok(v) => Loadable::Ready(v),
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
                    Ok(v) => Loadable::Ready(v),
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
            let scope = state
                .repos
                .iter()
                .find(|r| r.id == repo_id)
                .map(|r| r.history_scope)
                .unwrap_or(gitgpui_core::domain::LogScope::CurrentBranch);
            let diff_target = state
                .repos
                .iter()
                .find(|r| r.id == repo_id)
                .and_then(|r| r.diff_target.clone());

            let mut effects = refresh_effects(repo_id, scope);
            if let Some(target) = diff_target {
                let supports_file = matches!(
                    &target,
                    DiffTarget::WorkingTree { .. } | DiffTarget::Commit { path: Some(_), .. }
                );
                effects.push(Effect::LoadDiff {
                    repo_id,
                    target: target.clone(),
                });
                if supports_file {
                    effects.push(Effect::LoadDiffFile { repo_id, target });
                }
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

            let scope = state
                .repos
                .iter()
                .find(|r| r.id == repo_id)
                .map(|r| r.history_scope)
                .unwrap_or(gitgpui_core::domain::LogScope::CurrentBranch);
            refresh_effects(repo_id, scope)
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
            let scope = state
                .repos
                .iter()
                .find(|r| r.id == repo_id)
                .map(|r| r.history_scope)
                .unwrap_or(gitgpui_core::domain::LogScope::CurrentBranch);
            refresh_effects(repo_id, scope)
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
            RepoCommandKind::CheckoutConflict { side, .. } => match side {
                ConflictSide::Ours => "Checkout ours",
                ConflictSide::Theirs => "Checkout theirs",
            },
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
        RepoCommandKind::CheckoutConflict { side, .. } => match side {
            ConflictSide::Ours => "Resolved using ours".to_string(),
            ConflictSide::Theirs => "Resolved using theirs".to_string(),
        },
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

fn refresh_effects(repo_id: RepoId, history_scope: gitgpui_core::domain::LogScope) -> Vec<Effect> {
    vec![
        Effect::LoadHeadBranch { repo_id },
        Effect::LoadUpstreamDivergence { repo_id },
        Effect::LoadBranches { repo_id },
        Effect::LoadTags { repo_id },
        Effect::LoadRemotes { repo_id },
        Effect::LoadRemoteBranches { repo_id },
        Effect::LoadStatus { repo_id },
        Effect::LoadStashes { repo_id, limit: 50 },
        Effect::LoadReflog {
            repo_id,
            limit: 200,
        },
        Effect::LoadLog {
            repo_id,
            scope: history_scope,
            limit: 200,
            cursor: None,
        },
    ]
}
