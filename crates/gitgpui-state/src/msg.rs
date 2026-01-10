use crate::model::RepoId;
use gitgpui_core::domain::*;
use gitgpui_core::error::Error;
use gitgpui_core::services::GitRepository;
use std::path::PathBuf;
use std::sync::Arc;

pub enum Msg {
    OpenRepo(PathBuf),

    RepoOpenedOk {
        repo_id: RepoId,
        spec: RepoSpec,
        repo: Arc<dyn GitRepository>,
    },
    RepoOpenedErr {
        repo_id: RepoId,
        spec: RepoSpec,
        error: Error,
    },

    BranchesLoaded {
        repo_id: RepoId,
        result: Result<Vec<Branch>, Error>,
    },
    RemotesLoaded {
        repo_id: RepoId,
        result: Result<Vec<Remote>, Error>,
    },
    StatusLoaded {
        repo_id: RepoId,
        result: Result<Vec<FileStatus>, Error>,
    },
    LogLoaded {
        repo_id: RepoId,
        result: Result<LogPage, Error>,
    },
}

impl std::fmt::Debug for Msg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Msg::OpenRepo(path) => f.debug_tuple("OpenRepo").field(path).finish(),
            Msg::RepoOpenedOk { repo_id, spec, .. } => f
                .debug_struct("RepoOpenedOk")
                .field("repo_id", repo_id)
                .field("spec", spec)
                .finish_non_exhaustive(),
            Msg::RepoOpenedErr {
                repo_id, spec, error, ..
            } => f
                .debug_struct("RepoOpenedErr")
                .field("repo_id", repo_id)
                .field("spec", spec)
                .field("error", error)
                .finish(),
            Msg::BranchesLoaded { repo_id, result } => f
                .debug_struct("BranchesLoaded")
                .field("repo_id", repo_id)
                .field("result", result)
                .finish(),
            Msg::RemotesLoaded { repo_id, result } => f
                .debug_struct("RemotesLoaded")
                .field("repo_id", repo_id)
                .field("result", result)
                .finish(),
            Msg::StatusLoaded { repo_id, result } => f
                .debug_struct("StatusLoaded")
                .field("repo_id", repo_id)
                .field("result", result)
                .finish(),
            Msg::LogLoaded { repo_id, result } => f
                .debug_struct("LogLoaded")
                .field("repo_id", repo_id)
                .field("result", result)
                .finish(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Effect {
    OpenRepo { repo_id: RepoId, path: PathBuf },
    LoadBranches { repo_id: RepoId },
    LoadRemotes { repo_id: RepoId },
    LoadStatus { repo_id: RepoId },
    LoadHeadLog { repo_id: RepoId, limit: usize, cursor: Option<LogCursor> },
}

#[derive(Debug)]
pub enum StoreEvent {
    StateChanged,
}
