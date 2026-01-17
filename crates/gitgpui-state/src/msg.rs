use crate::model::RepoId;
use gitgpui_core::domain::*;
use gitgpui_core::error::Error;
use gitgpui_core::services::GitRepository;
use gitgpui_core::services::PullMode;
use gitgpui_core::services::{CommandOutput, ConflictSide};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepoCommandKind {
    FetchAll,
    Pull { mode: PullMode },
    Push,
    CheckoutConflict { path: PathBuf, side: ConflictSide },
}

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
    SetHistoryScope {
        repo_id: RepoId,
        scope: LogScope,
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
    LoadStashes {
        repo_id: RepoId,
    },
    LoadReflog {
        repo_id: RepoId,
    },
    CheckoutBranch {
        repo_id: RepoId,
        name: String,
    },
    CheckoutCommit {
        repo_id: RepoId,
        commit_id: CommitId,
    },
    CherryPickCommit {
        repo_id: RepoId,
        commit_id: CommitId,
    },
    RevertCommit {
        repo_id: RepoId,
        commit_id: CommitId,
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
    CheckoutConflictSide {
        repo_id: RepoId,
        path: PathBuf,
        side: ConflictSide,
    },
    Stash {
        repo_id: RepoId,
        message: String,
        include_untracked: bool,
    },
    ApplyStash {
        repo_id: RepoId,
        index: usize,
    },
    DropStash {
        repo_id: RepoId,
        index: usize,
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
        scope: LogScope,
        result: Result<LogPage, Error>,
    },
    TagsLoaded {
        repo_id: RepoId,
        result: Result<Vec<Tag>, Error>,
    },
    StashesLoaded {
        repo_id: RepoId,
        result: Result<Vec<StashEntry>, Error>,
    },
    ReflogLoaded {
        repo_id: RepoId,
        result: Result<Vec<ReflogEntry>, Error>,
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
    DiffFileLoaded {
        repo_id: RepoId,
        target: DiffTarget,
        result: Result<Option<FileDiffText>, Error>,
    },

    RepoActionFinished {
        repo_id: RepoId,
        result: Result<(), Error>,
    },

    RepoCommandFinished {
        repo_id: RepoId,
        command: RepoCommandKind,
        result: Result<CommandOutput, Error>,
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
            Msg::SetHistoryScope { repo_id, scope } => f
                .debug_struct("SetHistoryScope")
                .field("repo_id", repo_id)
                .field("scope", scope)
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
            Msg::LoadStashes { repo_id } => f
                .debug_struct("LoadStashes")
                .field("repo_id", repo_id)
                .finish(),
            Msg::LoadReflog { repo_id } => f
                .debug_struct("LoadReflog")
                .field("repo_id", repo_id)
                .finish(),
            Msg::CheckoutBranch { repo_id, name } => f
                .debug_struct("CheckoutBranch")
                .field("repo_id", repo_id)
                .field("name", name)
                .finish(),
            Msg::CheckoutCommit { repo_id, commit_id } => f
                .debug_struct("CheckoutCommit")
                .field("repo_id", repo_id)
                .field("commit_id", commit_id)
                .finish(),
            Msg::CherryPickCommit { repo_id, commit_id } => f
                .debug_struct("CherryPickCommit")
                .field("repo_id", repo_id)
                .field("commit_id", commit_id)
                .finish(),
            Msg::RevertCommit { repo_id, commit_id } => f
                .debug_struct("RevertCommit")
                .field("repo_id", repo_id)
                .field("commit_id", commit_id)
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
            Msg::CheckoutConflictSide {
                repo_id,
                path,
                side,
            } => f
                .debug_struct("CheckoutConflictSide")
                .field("repo_id", repo_id)
                .field("path", path)
                .field("side", side)
                .finish(),
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
            Msg::ApplyStash { repo_id, index } => f
                .debug_struct("ApplyStash")
                .field("repo_id", repo_id)
                .field("index", index)
                .finish(),
            Msg::DropStash { repo_id, index } => f
                .debug_struct("DropStash")
                .field("repo_id", repo_id)
                .field("index", index)
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
            Msg::LogLoaded {
                repo_id,
                scope,
                result,
            } => f
                .debug_struct("LogLoaded")
                .field("repo_id", repo_id)
                .field("scope", scope)
                .field("result", result)
                .finish(),
            Msg::TagsLoaded { repo_id, result } => f
                .debug_struct("TagsLoaded")
                .field("repo_id", repo_id)
                .field("result", result)
                .finish(),
            Msg::StashesLoaded { repo_id, result } => f
                .debug_struct("StashesLoaded")
                .field("repo_id", repo_id)
                .field("result", result)
                .finish(),
            Msg::ReflogLoaded { repo_id, result } => f
                .debug_struct("ReflogLoaded")
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
            Msg::DiffFileLoaded {
                repo_id,
                target,
                result,
            } => f
                .debug_struct("DiffFileLoaded")
                .field("repo_id", repo_id)
                .field("target", target)
                .field("result", result)
                .finish(),
            Msg::RepoActionFinished { repo_id, result } => f
                .debug_struct("RepoActionFinished")
                .field("repo_id", repo_id)
                .field("result", result)
                .finish(),
            Msg::RepoCommandFinished {
                repo_id,
                command,
                result,
            } => f
                .debug_struct("RepoCommandFinished")
                .field("repo_id", repo_id)
                .field("command", command)
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
    LoadLog {
        repo_id: RepoId,
        scope: LogScope,
        limit: usize,
        cursor: Option<LogCursor>,
    },
    LoadTags {
        repo_id: RepoId,
    },
    LoadStashes {
        repo_id: RepoId,
        limit: usize,
    },
    LoadReflog {
        repo_id: RepoId,
        limit: usize,
    },
    LoadCommitDetails {
        repo_id: RepoId,
        commit_id: CommitId,
    },
    LoadDiff {
        repo_id: RepoId,
        target: DiffTarget,
    },
    LoadDiffFile {
        repo_id: RepoId,
        target: DiffTarget,
    },

    CheckoutBranch {
        repo_id: RepoId,
        name: String,
    },
    CheckoutCommit {
        repo_id: RepoId,
        commit_id: CommitId,
    },
    CherryPickCommit {
        repo_id: RepoId,
        commit_id: CommitId,
    },
    RevertCommit {
        repo_id: RepoId,
        commit_id: CommitId,
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
    CheckoutConflictSide {
        repo_id: RepoId,
        path: PathBuf,
        side: ConflictSide,
    },
    Stash {
        repo_id: RepoId,
        message: String,
        include_untracked: bool,
    },
    ApplyStash {
        repo_id: RepoId,
        index: usize,
    },
    DropStash {
        repo_id: RepoId,
        index: usize,
    },
}

#[derive(Debug)]
pub enum StoreEvent {
    StateChanged,
}
