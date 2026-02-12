use crate::model::{
    AppNotification, AppNotificationKind, AppState, CommandLogEntry, DiagnosticEntry,
    DiagnosticKind, RepoId, RepoLoadsInFlight, RepoState,
};
use crate::msg::{Effect, RepoCommandKind};
use gitgpui_core::domain::DiffTarget;
use gitgpui_core::error::Error;
use gitgpui_core::services::CommandOutput;
use std::path::{Path, PathBuf};
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

pub(super) fn diff_target_wants_image_preview(target: &DiffTarget) -> bool {
    match target {
        DiffTarget::WorkingTree { path, .. } => is_supported_image_path(path),
        DiffTarget::Commit {
            path: Some(path), ..
        } => is_supported_image_path(path),
        _ => false,
    }
}

pub(super) fn diff_reload_effects(repo_id: RepoId, target: DiffTarget) -> Vec<Effect> {
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

pub(super) fn refresh_primary_effects(repo_state: &mut RepoState) -> Vec<Effect> {
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
        .request(RepoLoadsInFlight::MERGE_COMMIT_MESSAGE)
    {
        effects.push(Effect::LoadMergeCommitMessage { repo_id });
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

pub(super) fn refresh_full_effects(repo_state: &mut RepoState) -> Vec<Effect> {
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
        .request(RepoLoadsInFlight::MERGE_COMMIT_MESSAGE)
    {
        effects.push(Effect::LoadMergeCommitMessage { repo_id });
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

pub(super) fn dedup_paths_in_order(paths: Vec<PathBuf>) -> Vec<PathBuf> {
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

pub(super) fn push_command_log(
    repo_state: &mut RepoState,
    ok: bool,
    command: &RepoCommandKind,
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

pub(super) fn push_action_log(
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
    command: &RepoCommandKind,
    output: &CommandOutput,
    ok: bool,
    error: Option<&Error>,
) -> (String, String) {
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
            RepoCommandKind::ApplyWorktreePatch { reverse } => {
                if *reverse {
                    "Discard"
                } else {
                    "Patch"
                }
            }
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
        RepoCommandKind::ApplyWorktreePatch { reverse } => {
            if *reverse {
                "Changes discarded".to_string()
            } else {
                "Patch applied".to_string()
            }
        }
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
