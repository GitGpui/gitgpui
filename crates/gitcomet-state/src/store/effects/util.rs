use crate::msg::Msg;
use gitcomet_core::services::GitRepository;
use rustc_hash::FxHashMap as HashMap;
use std::sync::Arc;

use super::super::{RepoId, executor::TaskExecutor, worker_channel::StoreWorkerSender};

pub(super) type RepoMap = HashMap<RepoId, Arc<dyn GitRepository>>;

pub(super) fn spawn_with_repo(
    executor: &TaskExecutor,
    repos: &RepoMap,
    repo_id: RepoId,
    msg_tx: StoreWorkerSender,
    task: impl FnOnce(Arc<dyn GitRepository>, StoreWorkerSender) + Send + 'static,
) -> bool {
    spawn_with_repo_or_else(executor, repos, repo_id, msg_tx, task, |_| {})
}

pub(super) fn spawn_with_repo_or_else(
    executor: &TaskExecutor,
    repos: &RepoMap,
    repo_id: RepoId,
    msg_tx: StoreWorkerSender,
    task: impl FnOnce(Arc<dyn GitRepository>, StoreWorkerSender) + Send + 'static,
    on_missing: impl FnOnce(StoreWorkerSender) + Send + 'static,
) -> bool {
    if let Some(repo) = repos.get(&repo_id).cloned() {
        executor.spawn(move || task(repo, msg_tx));
        true
    } else {
        on_missing(msg_tx);
        false
    }
}

pub(super) fn send_or_log(msg_tx: &StoreWorkerSender, msg: Msg) {
    msg_tx.send_effect_or_log(msg, "store effect pipeline");
}
