use crate::msg::Msg;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use super::super::{RepoId, executor::TaskExecutor};
use super::util::{RepoMap, spawn_with_repo};

pub(super) fn schedule_checkout_branch(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    name: String,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::RepoActionFinished {
            repo_id,
            result: repo.checkout_branch(&name),
        });
    });
}

pub(super) fn schedule_checkout_remote_branch(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    remote: String,
    name: String,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::RepoActionFinished {
            repo_id,
            result: repo.checkout_remote_branch(&remote, &name),
        });
    });
}

pub(super) fn schedule_checkout_commit(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    commit_id: gitgpui_core::domain::CommitId,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::RepoActionFinished {
            repo_id,
            result: repo.checkout_commit(&commit_id),
        });
    });
}

pub(super) fn schedule_cherry_pick_commit(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    commit_id: gitgpui_core::domain::CommitId,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::RepoActionFinished {
            repo_id,
            result: repo.cherry_pick(&commit_id),
        });
    });
}

pub(super) fn schedule_revert_commit(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    commit_id: gitgpui_core::domain::CommitId,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::RepoActionFinished {
            repo_id,
            result: repo.revert(&commit_id),
        });
    });
}

pub(super) fn schedule_create_branch(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    name: String,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let target = gitgpui_core::domain::CommitId("HEAD".to_string());
        let _ = msg_tx.send(Msg::RepoActionFinished {
            repo_id,
            result: repo.create_branch(&name, &target),
        });
    });
}

pub(super) fn schedule_create_branch_and_checkout(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    name: String,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let target = gitgpui_core::domain::CommitId("HEAD".to_string());
        let result = repo
            .create_branch(&name, &target)
            .and_then(|()| repo.checkout_branch(&name));
        let _ = msg_tx.send(Msg::RepoActionFinished { repo_id, result });
    });
}

pub(super) fn schedule_delete_branch(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    name: String,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::RepoActionFinished {
            repo_id,
            result: repo.delete_branch(&name),
        });
    });
}

pub(super) fn schedule_stage_path(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    path: PathBuf,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let path_ref: &Path = &path;
        let _ = msg_tx.send(Msg::RepoActionFinished {
            repo_id,
            result: repo.stage(&[path_ref]),
        });
    });
}

pub(super) fn schedule_stage_paths(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    paths: Vec<PathBuf>,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let mut unique = paths;
        unique.sort();
        unique.dedup();
        let refs = unique.iter().map(|p| p.as_path()).collect::<Vec<_>>();
        let _ = msg_tx.send(Msg::RepoActionFinished {
            repo_id,
            result: repo.stage(&refs),
        });
    });
}

pub(super) fn schedule_unstage_path(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    path: PathBuf,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let path_ref: &Path = &path;
        let _ = msg_tx.send(Msg::RepoActionFinished {
            repo_id,
            result: repo.unstage(&[path_ref]),
        });
    });
}

pub(super) fn schedule_unstage_paths(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    paths: Vec<PathBuf>,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let mut unique = paths;
        unique.sort();
        unique.dedup();
        let refs = unique.iter().map(|p| p.as_path()).collect::<Vec<_>>();
        let _ = msg_tx.send(Msg::RepoActionFinished {
            repo_id,
            result: repo.unstage(&refs),
        });
    });
}

pub(super) fn schedule_discard_worktree_changes_path(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    path: PathBuf,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let path_ref: &Path = &path;
        let _ = msg_tx.send(Msg::RepoActionFinished {
            repo_id,
            result: repo.discard_worktree_changes(&[path_ref]),
        });
    });
}

pub(super) fn schedule_discard_worktree_changes_paths(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    paths: Vec<PathBuf>,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let mut unique = paths;
        unique.sort();
        unique.dedup();
        let refs = unique.iter().map(|p| p.as_path()).collect::<Vec<_>>();
        let _ = msg_tx.send(Msg::RepoActionFinished {
            repo_id,
            result: repo.discard_worktree_changes(&refs),
        });
    });
}

pub(super) fn schedule_commit(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    message: String,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::CommitFinished {
            repo_id,
            result: repo.commit(&message),
        });
    });
}

pub(super) fn schedule_commit_amend(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    message: String,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::CommitAmendFinished {
            repo_id,
            result: repo.commit_amend(&message),
        });
    });
}

pub(super) fn schedule_stash(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    message: String,
    include_untracked: bool,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::RepoActionFinished {
            repo_id,
            result: repo.stash_create(&message, include_untracked),
        });
    });
}

pub(super) fn schedule_apply_stash(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    index: usize,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::RepoActionFinished {
            repo_id,
            result: repo.stash_apply(index),
        });
    });
}

pub(super) fn schedule_drop_stash(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    index: usize,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let _ = msg_tx.send(Msg::RepoActionFinished {
            repo_id,
            result: repo.stash_drop(index),
        });
    });
}

pub(super) fn schedule_pop_stash(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    index: usize,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let result = repo
            .stash_apply(index)
            .and_then(|()| repo.stash_drop(index));
        let _ = msg_tx.send(Msg::RepoActionFinished { repo_id, result });
    });
}
