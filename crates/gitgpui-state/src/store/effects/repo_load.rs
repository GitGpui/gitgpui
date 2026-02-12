use crate::msg::Msg;
use gitgpui_core::domain::{Diff, DiffArea, DiffTarget, LogCursor, LogScope};
use std::path::PathBuf;
use std::sync::mpsc;

use super::super::{RepoId, executor::TaskExecutor};
use super::util::{RepoMap, spawn_with_repo};

pub(super) fn schedule_load_branches(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::BranchesLoaded {
            repo_id,
            result: repo.list_branches(),
        });
    });
}

pub(super) fn schedule_load_remotes(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::RemotesLoaded {
            repo_id,
            result: repo.list_remotes(),
        });
    });
}

pub(super) fn schedule_load_remote_branches(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::RemoteBranchesLoaded {
            repo_id,
            result: repo.list_remote_branches(),
        });
    });
}

pub(super) fn schedule_load_status(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::StatusLoaded {
            repo_id,
            result: repo.status(),
        });
    });
}

pub(super) fn schedule_load_head_branch(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::HeadBranchLoaded {
            repo_id,
            result: repo.current_branch(),
        });
    });
}

pub(super) fn schedule_load_upstream_divergence(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::UpstreamDivergenceLoaded {
            repo_id,
            result: repo.upstream_divergence(),
        });
    });
}

pub(super) fn schedule_load_log(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    scope: LogScope,
    limit: usize,
    cursor: Option<LogCursor>,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let result = {
            let cursor_ref = cursor.as_ref();
            match scope {
                LogScope::CurrentBranch => repo.log_head_page(limit, cursor_ref),
                LogScope::AllBranches => repo.log_all_branches_page(limit, cursor_ref),
            }
        };
        let _ = msg_tx.send(Msg::LogLoaded {
            repo_id,
            scope,
            cursor,
            result,
        });
    });
}

pub(super) fn schedule_load_tags(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::TagsLoaded {
            repo_id,
            result: repo.list_tags(),
        });
    });
}

pub(super) fn schedule_load_stashes(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    limit: usize,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let mut entries = repo.stash_list();
        if let Ok(v) = &mut entries {
            v.truncate(limit);
        }
        let _ = msg_tx.send(Msg::StashesLoaded {
            repo_id,
            result: entries,
        });
    });
}

pub(super) fn schedule_load_conflict_file(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    path: PathBuf,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let ours_theirs = repo.diff_file_text(&DiffTarget::WorkingTree {
            path: path.clone(),
            area: DiffArea::Unstaged,
        });

        let current = std::fs::read(repo.spec().workdir.join(&path))
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok());

        let result = ours_theirs.map(|opt| {
            opt.map(|d| crate::model::ConflictFile {
                path: d.path,
                ours: d.old,
                theirs: d.new,
                current,
            })
        });

        let _ = msg_tx.send(Msg::ConflictFileLoaded {
            repo_id,
            path,
            result,
        });
    });
}

pub(super) fn schedule_load_reflog(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    limit: usize,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::ReflogLoaded {
            repo_id,
            result: repo.reflog_head(limit),
        });
    });
}

pub(super) fn schedule_load_file_history(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    path: PathBuf,
    limit: usize,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::FileHistoryLoaded {
            repo_id,
            path: path.clone(),
            result: repo.log_file_page(&path, limit, None),
        });
    });
}

pub(super) fn schedule_load_blame(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    path: PathBuf,
    rev: Option<String>,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let result = repo.blame_file(&path, rev.as_deref());
        let _ = msg_tx.send(Msg::BlameLoaded {
            repo_id,
            path: path.clone(),
            rev: rev.clone(),
            result,
        });
    });
}

pub(super) fn schedule_load_worktrees(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::WorktreesLoaded {
            repo_id,
            result: repo.list_worktrees(),
        });
    });
}

pub(super) fn schedule_load_submodules(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::SubmodulesLoaded {
            repo_id,
            result: repo.list_submodules(),
        });
    });
}

pub(super) fn schedule_load_rebase_state(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::RebaseStateLoaded {
            repo_id,
            result: repo.rebase_in_progress(),
        });
    });
}

pub(super) fn schedule_load_merge_commit_message(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::MergeCommitMessageLoaded {
            repo_id,
            result: repo.merge_commit_message(),
        });
    });
}

pub(super) fn schedule_load_commit_details(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    commit_id: gitgpui_core::domain::CommitId,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::CommitDetailsLoaded {
            repo_id,
            commit_id: commit_id.clone(),
            result: repo.commit_details(&commit_id),
        });
    });
}

pub(super) fn schedule_load_diff(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    target: DiffTarget,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let result = repo
            .diff_unified(&target)
            .map(|text| Diff::from_unified(target.clone(), &text));
        let _ = msg_tx.send(Msg::DiffLoaded {
            repo_id,
            target,
            result,
        });
    });
}

pub(super) fn schedule_load_diff_file(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    target: DiffTarget,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let result = repo.diff_file_text(&target);
        let _ = msg_tx.send(Msg::DiffFileLoaded {
            repo_id,
            target,
            result,
        });
    });
}

pub(super) fn schedule_load_diff_file_image(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    target: DiffTarget,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let result = repo.diff_file_image(&target);
        let _ = msg_tx.send(Msg::DiffFileImageLoaded {
            repo_id,
            target,
            result,
        });
    });
}
