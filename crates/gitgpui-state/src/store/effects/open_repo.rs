use crate::msg::Msg;
use gitgpui_core::domain::RepoSpec;
use gitgpui_core::services::GitBackend;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};

use super::super::{RepoId, executor::TaskExecutor};

pub(super) fn schedule_open_repo(
    executor: &TaskExecutor,
    backend: Arc<dyn GitBackend>,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    path: PathBuf,
) {
    executor.spawn(move || {
        let spec = RepoSpec { workdir: path };
        match backend.open(&spec.workdir) {
            Ok(repo) => {
                let _ = msg_tx.send(Msg::RepoOpenedOk {
                    repo_id,
                    spec,
                    repo,
                });
            }
            Err(error) => {
                let _ = msg_tx.send(Msg::RepoOpenedErr {
                    repo_id,
                    spec,
                    error,
                });
            }
        }
    });
}
