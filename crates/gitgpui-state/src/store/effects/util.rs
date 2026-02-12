use crate::msg::Msg;
use gitgpui_core::services::GitRepository;
use std::collections::HashMap;
use std::sync::{Arc, mpsc};

use super::super::{RepoId, executor::TaskExecutor};

pub(super) type RepoMap = HashMap<RepoId, Arc<dyn GitRepository>>;

pub(super) fn spawn_with_repo(
    executor: &TaskExecutor,
    repos: &RepoMap,
    repo_id: RepoId,
    msg_tx: mpsc::Sender<Msg>,
    task: impl FnOnce(Arc<dyn GitRepository>, mpsc::Sender<Msg>) + Send + 'static,
) {
    if let Some(repo) = repos.get(&repo_id).cloned() {
        executor.spawn(move || task(repo, msg_tx));
    }
}
