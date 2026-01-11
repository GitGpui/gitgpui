use crate::model::RepoId;
use gitgpui_core::domain::*;
use gitgpui_core::error::Error;
use gitgpui_core::services::GitRepository;
use gitgpui_core::services::PullMode;
use std::path::PathBuf;
use std::sync::Arc;

pub enum Msg {
    OpenRepo(PathBuf),
    RestoreSession {
        open_repos: Vec<PathBuf>,
        active_repo: Option<PathBuf>,
    },
    CloseRepo {
        repo_id: RepoId,
    },
    SetActiveRepo {
        repo_id: RepoId,
    },
    ReloadRepo {
        repo_id: RepoId,
    },
    SelectCommit {
        repo_id: RepoId,
        commit_id: CommitId,
    },
    ClearCommitSelection {
        repo_id: RepoId,
    },
    SelectDiff {
        repo_id: RepoId,
        target: DiffTarget,
    },
    ClearDiffSelection {
        repo_id: RepoId,
    },
    CheckoutBranch {
        repo_id: RepoId,
        name: String,
    },
    CreateBranch {
        repo_id: RepoId,
        name: String,
    },
    StagePath {
        repo_id: RepoId,
        path: PathBuf,
    },
    UnstagePath {
        repo_id: RepoId,
        path: PathBuf,
    },
    Commit {
        repo_id: RepoId,
        message: String,
    },
    FetchAll {
        repo_id: RepoId,
    },
    Pull {
        repo_id: RepoId,
        mode: PullMode,
    },
    Push {
        repo_id: RepoId,
    },
    Stash {
        repo_id: RepoId,
        message: String,
        include_untracked: bool,
    },

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
    RemoteBranchesLoaded {
        repo_id: RepoId,
        result: Result<Vec<RemoteBranch>, Error>,
    },
    StatusLoaded {
        repo_id: RepoId,
        result: Result<RepoStatus, Error>,
    },
    HeadBranchLoaded {
        repo_id: RepoId,
        result: Result<String, Error>,
    },
    LogLoaded {
        repo_id: RepoId,
        result: Result<LogPage, Error>,
    },

    CommitDetailsLoaded {
        repo_id: RepoId,
        commit_id: CommitId,
        result: Result<CommitDetails, Error>,
    },

    DiffLoaded {
        repo_id: RepoId,
        target: DiffTarget,
        result: Result<Diff, Error>,
    },

    RepoActionFinished {
        repo_id: RepoId,
        result: Result<(), Error>,
    },
}

impl std::fmt::Debug for Msg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Msg::OpenRepo(path) => f.debug_tuple("OpenRepo").field(path).finish(),
            Msg::RestoreSession {
                open_repos,
                active_repo,
            } => f
                .debug_struct("RestoreSession")
                .field("open_repos", open_repos)
                .field("active_repo", active_repo)
                .finish(),
            Msg::CloseRepo { repo_id } => f
                .debug_struct("CloseRepo")
                .field("repo_id", repo_id)
                .finish(),
            Msg::SetActiveRepo { repo_id } => f
                .debug_struct("SetActiveRepo")
                .field("repo_id", repo_id)
                .finish(),
            Msg::ReloadRepo { repo_id } => f
                .debug_struct("ReloadRepo")
                .field("repo_id", repo_id)
                .finish(),
            Msg::SelectCommit { repo_id, commit_id } => f
                .debug_struct("SelectCommit")
                .field("repo_id", repo_id)
                .field("commit_id", commit_id)
                .finish(),
            Msg::ClearCommitSelection { repo_id } => f
                .debug_struct("ClearCommitSelection")
                .field("repo_id", repo_id)
                .finish(),
            Msg::SelectDiff { repo_id, target } => f
                .debug_struct("SelectDiff")
                .field("repo_id", repo_id)
                .field("target", target)
                .finish(),
            Msg::ClearDiffSelection { repo_id } => f
                .debug_struct("ClearDiffSelection")
                .field("repo_id", repo_id)
                .finish(),
            Msg::CheckoutBranch { repo_id, name } => f
                .debug_struct("CheckoutBranch")
                .field("repo_id", repo_id)
                .field("name", name)
                .finish(),
            Msg::CreateBranch { repo_id, name } => f
                .debug_struct("CreateBranch")
                .field("repo_id", repo_id)
                .field("name", name)
                .finish(),
            Msg::StagePath { repo_id, path } => f
                .debug_struct("StagePath")
                .field("repo_id", repo_id)
                .field("path", path)
                .finish(),
            Msg::UnstagePath { repo_id, path } => f
                .debug_struct("UnstagePath")
                .field("repo_id", repo_id)
                .field("path", path)
                .finish(),
            Msg::Commit { repo_id, message } => f
                .debug_struct("Commit")
                .field("repo_id", repo_id)
                .field("message", message)
                .finish(),
            Msg::FetchAll { repo_id } => f
                .debug_struct("FetchAll")
                .field("repo_id", repo_id)
                .finish(),
            Msg::Pull { repo_id, mode } => f
                .debug_struct("Pull")
                .field("repo_id", repo_id)
                .field("mode", mode)
                .finish(),
            Msg::Push { repo_id } => f.debug_struct("Push").field("repo_id", repo_id).finish(),
            Msg::Stash {
                repo_id,
                message,
                include_untracked,
            } => f
                .debug_struct("Stash")
                .field("repo_id", repo_id)
                .field("message", message)
                .field("include_untracked", include_untracked)
                .finish(),
            Msg::RepoOpenedOk { repo_id, spec, .. } => f
                .debug_struct("RepoOpenedOk")
                .field("repo_id", repo_id)
                .field("spec", spec)
                .finish_non_exhaustive(),
            Msg::RepoOpenedErr {
                repo_id,
                spec,
                error,
                ..
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
            Msg::RemoteBranchesLoaded { repo_id, result } => f
                .debug_struct("RemoteBranchesLoaded")
                .field("repo_id", repo_id)
                .field("result", result)
                .finish(),
            Msg::StatusLoaded { repo_id, result } => f
                .debug_struct("StatusLoaded")
                .field("repo_id", repo_id)
                .field("result", result)
                .finish(),
            Msg::HeadBranchLoaded { repo_id, result } => f
                .debug_struct("HeadBranchLoaded")
                .field("repo_id", repo_id)
                .field("result", result)
                .finish(),
            Msg::LogLoaded { repo_id, result } => f
                .debug_struct("LogLoaded")
                .field("repo_id", repo_id)
                .field("result", result)
                .finish(),
            Msg::CommitDetailsLoaded {
                repo_id,
                commit_id,
                result,
            } => f
                .debug_struct("CommitDetailsLoaded")
                .field("repo_id", repo_id)
                .field("commit_id", commit_id)
                .field("result", result)
                .finish(),
            Msg::DiffLoaded {
                repo_id,
                target,
                result,
            } => f
                .debug_struct("DiffLoaded")
                .field("repo_id", repo_id)
                .field("target", target)
                .field("result", result)
                .finish(),
            Msg::RepoActionFinished { repo_id, result } => f
                .debug_struct("RepoActionFinished")
                .field("repo_id", repo_id)
                .field("result", result)
                .finish(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Effect {
    OpenRepo {
        repo_id: RepoId,
        path: PathBuf,
    },
    LoadBranches {
        repo_id: RepoId,
    },
    LoadRemotes {
        repo_id: RepoId,
    },
    LoadRemoteBranches {
        repo_id: RepoId,
    },
    LoadStatus {
        repo_id: RepoId,
    },
    LoadHeadBranch {
        repo_id: RepoId,
    },
    LoadHeadLog {
        repo_id: RepoId,
        limit: usize,
        cursor: Option<LogCursor>,
    },
    LoadCommitDetails {
        repo_id: RepoId,
        commit_id: CommitId,
    },
    LoadDiff {
        repo_id: RepoId,
        target: DiffTarget,
    },

    CheckoutBranch {
        repo_id: RepoId,
        name: String,
    },
    CreateBranch {
        repo_id: RepoId,
        name: String,
    },
    StagePath {
        repo_id: RepoId,
        path: PathBuf,
    },
    UnstagePath {
        repo_id: RepoId,
        path: PathBuf,
    },
    Commit {
        repo_id: RepoId,
        message: String,
    },
    FetchAll {
        repo_id: RepoId,
    },
    Pull {
        repo_id: RepoId,
        mode: PullMode,
    },
    Push {
        repo_id: RepoId,
    },
    Stash {
        repo_id: RepoId,
        message: String,
        include_untracked: bool,
    },
}

#[derive(Debug)]
pub enum StoreEvent {
    StateChanged,
}
