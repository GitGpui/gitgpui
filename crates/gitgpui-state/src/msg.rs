use crate::model::RepoId;
use gitgpui_core::domain::*;
use gitgpui_core::error::Error;
use gitgpui_core::services::GitRepository;
use gitgpui_core::services::{CommandOutput, ConflictSide, PullMode, RemoteUrlKind, ResetMode};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepoCommandKind {
    FetchAll,
    Pull {
        mode: PullMode,
    },
    PullBranch {
        remote: String,
        branch: String,
    },
    MergeRef {
        reference: String,
    },
    Push,
    ForcePush,
    PushSetUpstream {
        remote: String,
        branch: String,
    },
    Reset {
        mode: ResetMode,
        target: String,
    },
    Rebase {
        onto: String,
    },
    RebaseContinue,
    RebaseAbort,
    CreateTag {
        name: String,
        target: String,
    },
    DeleteTag {
        name: String,
    },
    AddRemote {
        name: String,
        url: String,
    },
    RemoveRemote {
        name: String,
    },
    SetRemoteUrl {
        name: String,
        url: String,
        kind: RemoteUrlKind,
    },
    CheckoutConflict {
        path: PathBuf,
        side: ConflictSide,
    },
    SaveWorktreeFile {
        path: PathBuf,
        stage: bool,
    },
    ExportPatch {
        commit_id: CommitId,
        dest: PathBuf,
    },
    ApplyPatch {
        patch: PathBuf,
    },
    AddWorktree {
        path: PathBuf,
        reference: Option<String>,
    },
    RemoveWorktree {
        path: PathBuf,
    },
    AddSubmodule {
        url: String,
        path: PathBuf,
    },
    UpdateSubmodules,
    RemoveSubmodule {
        path: PathBuf,
    },
    StageHunk,
    UnstageHunk,
    ApplyWorktreePatch {
        reverse: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepoExternalChange {
    Worktree,
    GitState,
    Both,
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
    RepoExternallyChanged {
        repo_id: RepoId,
        change: RepoExternalChange,
    },
    SetHistoryScope {
        repo_id: RepoId,
        scope: LogScope,
    },
    LoadMoreHistory {
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
    LoadStashes {
        repo_id: RepoId,
    },
    LoadConflictFile {
        repo_id: RepoId,
        path: PathBuf,
    },
    LoadReflog {
        repo_id: RepoId,
    },
    LoadFileHistory {
        repo_id: RepoId,
        path: PathBuf,
        limit: usize,
    },
    LoadBlame {
        repo_id: RepoId,
        path: PathBuf,
        rev: Option<String>,
    },
    LoadWorktrees {
        repo_id: RepoId,
    },
    LoadSubmodules {
        repo_id: RepoId,
    },
    StageHunk {
        repo_id: RepoId,
        patch: String,
    },
    UnstageHunk {
        repo_id: RepoId,
        patch: String,
    },
    ApplyWorktreePatch {
        repo_id: RepoId,
        patch: String,
        reverse: bool,
    },
    CheckoutBranch {
        repo_id: RepoId,
        name: String,
    },
    CheckoutRemoteBranch {
        repo_id: RepoId,
        remote: String,
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
    CreateBranchAndCheckout {
        repo_id: RepoId,
        name: String,
    },
    DeleteBranch {
        repo_id: RepoId,
        name: String,
    },
    CloneRepo {
        url: String,
        dest: PathBuf,
    },
    CloneRepoProgress {
        dest: PathBuf,
        line: String,
    },
    CloneRepoFinished {
        url: String,
        dest: PathBuf,
        result: Result<CommandOutput, Error>,
    },
    ExportPatch {
        repo_id: RepoId,
        commit_id: CommitId,
        dest: PathBuf,
    },
    ApplyPatch {
        repo_id: RepoId,
        patch: PathBuf,
    },
    AddWorktree {
        repo_id: RepoId,
        path: PathBuf,
        reference: Option<String>,
    },
    RemoveWorktree {
        repo_id: RepoId,
        path: PathBuf,
    },
    AddSubmodule {
        repo_id: RepoId,
        url: String,
        path: PathBuf,
    },
    UpdateSubmodules {
        repo_id: RepoId,
    },
    RemoveSubmodule {
        repo_id: RepoId,
        path: PathBuf,
    },
    StagePath {
        repo_id: RepoId,
        path: PathBuf,
    },
    StagePaths {
        repo_id: RepoId,
        paths: Vec<PathBuf>,
    },
    UnstagePath {
        repo_id: RepoId,
        path: PathBuf,
    },
    UnstagePaths {
        repo_id: RepoId,
        paths: Vec<PathBuf>,
    },
    DiscardWorktreeChangesPath {
        repo_id: RepoId,
        path: PathBuf,
    },
    DiscardWorktreeChangesPaths {
        repo_id: RepoId,
        paths: Vec<PathBuf>,
    },
    SaveWorktreeFile {
        repo_id: RepoId,
        path: PathBuf,
        contents: String,
        stage: bool,
    },
    Commit {
        repo_id: RepoId,
        message: String,
    },
    CommitAmend {
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
    PullBranch {
        repo_id: RepoId,
        remote: String,
        branch: String,
    },
    MergeRef {
        repo_id: RepoId,
        reference: String,
    },
    Push {
        repo_id: RepoId,
    },
    ForcePush {
        repo_id: RepoId,
    },
    PushSetUpstream {
        repo_id: RepoId,
        remote: String,
        branch: String,
    },
    Reset {
        repo_id: RepoId,
        target: String,
        mode: ResetMode,
    },
    Rebase {
        repo_id: RepoId,
        onto: String,
    },
    RebaseContinue {
        repo_id: RepoId,
    },
    RebaseAbort {
        repo_id: RepoId,
    },
    CreateTag {
        repo_id: RepoId,
        name: String,
        target: String,
    },
    DeleteTag {
        repo_id: RepoId,
        name: String,
    },
    AddRemote {
        repo_id: RepoId,
        name: String,
        url: String,
    },
    RemoveRemote {
        repo_id: RepoId,
        name: String,
    },
    SetRemoteUrl {
        repo_id: RepoId,
        name: String,
        url: String,
        kind: RemoteUrlKind,
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
    PopStash {
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
    UpstreamDivergenceLoaded {
        repo_id: RepoId,
        result: Result<Option<UpstreamDivergence>, Error>,
    },
    LogLoaded {
        repo_id: RepoId,
        scope: LogScope,
        cursor: Option<LogCursor>,
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
    RebaseStateLoaded {
        repo_id: RepoId,
        result: Result<bool, Error>,
    },
    MergeCommitMessageLoaded {
        repo_id: RepoId,
        result: Result<Option<String>, Error>,
    },
    FileHistoryLoaded {
        repo_id: RepoId,
        path: PathBuf,
        result: Result<LogPage, Error>,
    },
    BlameLoaded {
        repo_id: RepoId,
        path: PathBuf,
        rev: Option<String>,
        result: Result<Vec<gitgpui_core::services::BlameLine>, Error>,
    },
    ConflictFileLoaded {
        repo_id: RepoId,
        path: PathBuf,
        result: Result<Option<crate::model::ConflictFile>, Error>,
    },
    WorktreesLoaded {
        repo_id: RepoId,
        result: Result<Vec<Worktree>, Error>,
    },
    SubmodulesLoaded {
        repo_id: RepoId,
        result: Result<Vec<Submodule>, Error>,
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
    DiffFileImageLoaded {
        repo_id: RepoId,
        target: DiffTarget,
        result: Result<Option<FileDiffImage>, Error>,
    },

    RepoActionFinished {
        repo_id: RepoId,
        result: Result<(), Error>,
    },
    CommitFinished {
        repo_id: RepoId,
        result: Result<(), Error>,
    },
    CommitAmendFinished {
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
            Msg::RepoExternallyChanged { repo_id, change } => f
                .debug_struct("RepoExternallyChanged")
                .field("repo_id", repo_id)
                .field("change", change)
                .finish(),
            Msg::SetHistoryScope { repo_id, scope } => f
                .debug_struct("SetHistoryScope")
                .field("repo_id", repo_id)
                .field("scope", scope)
                .finish(),
            Msg::LoadMoreHistory { repo_id } => f
                .debug_struct("LoadMoreHistory")
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
            Msg::LoadStashes { repo_id } => f
                .debug_struct("LoadStashes")
                .field("repo_id", repo_id)
                .finish(),
            Msg::LoadConflictFile { repo_id, path } => f
                .debug_struct("LoadConflictFile")
                .field("repo_id", repo_id)
                .field("path", path)
                .finish(),
            Msg::LoadReflog { repo_id } => f
                .debug_struct("LoadReflog")
                .field("repo_id", repo_id)
                .finish(),
            Msg::LoadFileHistory {
                repo_id,
                path,
                limit,
            } => f
                .debug_struct("LoadFileHistory")
                .field("repo_id", repo_id)
                .field("path", path)
                .field("limit", limit)
                .finish(),
            Msg::LoadBlame { repo_id, path, rev } => f
                .debug_struct("LoadBlame")
                .field("repo_id", repo_id)
                .field("path", path)
                .field("rev", rev)
                .finish(),
            Msg::LoadWorktrees { repo_id } => f
                .debug_struct("LoadWorktrees")
                .field("repo_id", repo_id)
                .finish(),
            Msg::LoadSubmodules { repo_id } => f
                .debug_struct("LoadSubmodules")
                .field("repo_id", repo_id)
                .finish(),
            Msg::StageHunk { repo_id, patch } => f
                .debug_struct("StageHunk")
                .field("repo_id", repo_id)
                .field("patch_len", &patch.len())
                .finish(),
            Msg::UnstageHunk { repo_id, patch } => f
                .debug_struct("UnstageHunk")
                .field("repo_id", repo_id)
                .field("patch_len", &patch.len())
                .finish(),
            Msg::ApplyWorktreePatch {
                repo_id,
                patch,
                reverse,
            } => f
                .debug_struct("ApplyWorktreePatch")
                .field("repo_id", repo_id)
                .field("reverse", reverse)
                .field("patch_len", &patch.len())
                .finish(),
            Msg::CheckoutBranch { repo_id, name } => f
                .debug_struct("CheckoutBranch")
                .field("repo_id", repo_id)
                .field("name", name)
                .finish(),
            Msg::CheckoutRemoteBranch {
                repo_id,
                remote,
                name,
            } => f
                .debug_struct("CheckoutRemoteBranch")
                .field("repo_id", repo_id)
                .field("remote", remote)
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
            Msg::CreateBranchAndCheckout { repo_id, name } => f
                .debug_struct("CreateBranchAndCheckout")
                .field("repo_id", repo_id)
                .field("name", name)
                .finish(),
            Msg::DeleteBranch { repo_id, name } => f
                .debug_struct("DeleteBranch")
                .field("repo_id", repo_id)
                .field("name", name)
                .finish(),
            Msg::CloneRepo { url, dest } => f
                .debug_struct("CloneRepo")
                .field("url", url)
                .field("dest", dest)
                .finish(),
            Msg::CloneRepoProgress { dest, line } => f
                .debug_struct("CloneRepoProgress")
                .field("dest", dest)
                .field("line", line)
                .finish(),
            Msg::CloneRepoFinished { url, dest, result } => f
                .debug_struct("CloneRepoFinished")
                .field("url", url)
                .field("dest", dest)
                .field("ok", &result.is_ok())
                .finish(),
            Msg::ExportPatch {
                repo_id,
                commit_id,
                dest,
            } => f
                .debug_struct("ExportPatch")
                .field("repo_id", repo_id)
                .field("commit_id", commit_id)
                .field("dest", dest)
                .finish(),
            Msg::ApplyPatch { repo_id, patch } => f
                .debug_struct("ApplyPatch")
                .field("repo_id", repo_id)
                .field("patch", patch)
                .finish(),
            Msg::AddWorktree {
                repo_id,
                path,
                reference,
            } => f
                .debug_struct("AddWorktree")
                .field("repo_id", repo_id)
                .field("path", path)
                .field("reference", reference)
                .finish(),
            Msg::RemoveWorktree { repo_id, path } => f
                .debug_struct("RemoveWorktree")
                .field("repo_id", repo_id)
                .field("path", path)
                .finish(),
            Msg::AddSubmodule { repo_id, url, path } => f
                .debug_struct("AddSubmodule")
                .field("repo_id", repo_id)
                .field("url", url)
                .field("path", path)
                .finish(),
            Msg::UpdateSubmodules { repo_id } => f
                .debug_struct("UpdateSubmodules")
                .field("repo_id", repo_id)
                .finish(),
            Msg::RemoveSubmodule { repo_id, path } => f
                .debug_struct("RemoveSubmodule")
                .field("repo_id", repo_id)
                .field("path", path)
                .finish(),
            Msg::StagePath { repo_id, path } => f
                .debug_struct("StagePath")
                .field("repo_id", repo_id)
                .field("path", path)
                .finish(),
            Msg::StagePaths { repo_id, paths } => f
                .debug_struct("StagePaths")
                .field("repo_id", repo_id)
                .field("paths_len", &paths.len())
                .finish(),
            Msg::UnstagePath { repo_id, path } => f
                .debug_struct("UnstagePath")
                .field("repo_id", repo_id)
                .field("path", path)
                .finish(),
            Msg::UnstagePaths { repo_id, paths } => f
                .debug_struct("UnstagePaths")
                .field("repo_id", repo_id)
                .field("paths_len", &paths.len())
                .finish(),
            Msg::DiscardWorktreeChangesPath { repo_id, path } => f
                .debug_struct("DiscardWorktreeChangesPath")
                .field("repo_id", repo_id)
                .field("path", path)
                .finish(),
            Msg::DiscardWorktreeChangesPaths { repo_id, paths } => f
                .debug_struct("DiscardWorktreeChangesPaths")
                .field("repo_id", repo_id)
                .field("paths_len", &paths.len())
                .finish(),
            Msg::SaveWorktreeFile {
                repo_id,
                path,
                contents,
                stage,
            } => f
                .debug_struct("SaveWorktreeFile")
                .field("repo_id", repo_id)
                .field("path", path)
                .field("contents_len", &contents.len())
                .field("stage", stage)
                .finish(),
            Msg::Commit { repo_id, message } => f
                .debug_struct("Commit")
                .field("repo_id", repo_id)
                .field("message", message)
                .finish(),
            Msg::CommitAmend { repo_id, message } => f
                .debug_struct("CommitAmend")
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
            Msg::PullBranch {
                repo_id,
                remote,
                branch,
            } => f
                .debug_struct("PullBranch")
                .field("repo_id", repo_id)
                .field("remote", remote)
                .field("branch", branch)
                .finish(),
            Msg::MergeRef { repo_id, reference } => f
                .debug_struct("MergeRef")
                .field("repo_id", repo_id)
                .field("reference", reference)
                .finish(),
            Msg::Push { repo_id } => f.debug_struct("Push").field("repo_id", repo_id).finish(),
            Msg::ForcePush { repo_id } => f
                .debug_struct("ForcePush")
                .field("repo_id", repo_id)
                .finish(),
            Msg::PushSetUpstream {
                repo_id,
                remote,
                branch,
            } => f
                .debug_struct("PushSetUpstream")
                .field("repo_id", repo_id)
                .field("remote", remote)
                .field("branch", branch)
                .finish(),
            Msg::Reset {
                repo_id,
                target,
                mode,
            } => f
                .debug_struct("Reset")
                .field("repo_id", repo_id)
                .field("target", target)
                .field("mode", mode)
                .finish(),
            Msg::Rebase { repo_id, onto } => f
                .debug_struct("Rebase")
                .field("repo_id", repo_id)
                .field("onto", onto)
                .finish(),
            Msg::RebaseContinue { repo_id } => f
                .debug_struct("RebaseContinue")
                .field("repo_id", repo_id)
                .finish(),
            Msg::RebaseAbort { repo_id } => f
                .debug_struct("RebaseAbort")
                .field("repo_id", repo_id)
                .finish(),
            Msg::CreateTag {
                repo_id,
                name,
                target,
            } => f
                .debug_struct("CreateTag")
                .field("repo_id", repo_id)
                .field("name", name)
                .field("target", target)
                .finish(),
            Msg::DeleteTag { repo_id, name } => f
                .debug_struct("DeleteTag")
                .field("repo_id", repo_id)
                .field("name", name)
                .finish(),
            Msg::AddRemote { repo_id, name, url } => f
                .debug_struct("AddRemote")
                .field("repo_id", repo_id)
                .field("name", name)
                .field("url", url)
                .finish(),
            Msg::RemoveRemote { repo_id, name } => f
                .debug_struct("RemoveRemote")
                .field("repo_id", repo_id)
                .field("name", name)
                .finish(),
            Msg::SetRemoteUrl {
                repo_id,
                name,
                url,
                kind,
            } => f
                .debug_struct("SetRemoteUrl")
                .field("repo_id", repo_id)
                .field("name", name)
                .field("url", url)
                .field("kind", kind)
                .finish(),
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
            Msg::PopStash { repo_id, index } => f
                .debug_struct("PopStash")
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
            Msg::UpstreamDivergenceLoaded { repo_id, result } => f
                .debug_struct("UpstreamDivergenceLoaded")
                .field("repo_id", repo_id)
                .field("result", result)
                .finish(),
            Msg::LogLoaded {
                repo_id,
                scope,
                cursor,
                result,
            } => f
                .debug_struct("LogLoaded")
                .field("repo_id", repo_id)
                .field("scope", scope)
                .field("cursor", cursor)
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
            Msg::RebaseStateLoaded { repo_id, result } => f
                .debug_struct("RebaseStateLoaded")
                .field("repo_id", repo_id)
                .field("result", result)
                .finish(),
            Msg::MergeCommitMessageLoaded { repo_id, result } => f
                .debug_struct("MergeCommitMessageLoaded")
                .field("repo_id", repo_id)
                .field("result", result)
                .finish(),
            Msg::FileHistoryLoaded {
                repo_id,
                path,
                result,
            } => f
                .debug_struct("FileHistoryLoaded")
                .field("repo_id", repo_id)
                .field("path", path)
                .field("result", result)
                .finish(),
            Msg::BlameLoaded {
                repo_id,
                path,
                rev,
                result,
            } => f
                .debug_struct("BlameLoaded")
                .field("repo_id", repo_id)
                .field("path", path)
                .field("rev", rev)
                .field("result", result)
                .finish(),
            Msg::ConflictFileLoaded {
                repo_id,
                path,
                result,
            } => f
                .debug_struct("ConflictFileLoaded")
                .field("repo_id", repo_id)
                .field("path", path)
                .field("result", result)
                .finish(),
            Msg::WorktreesLoaded { repo_id, result } => f
                .debug_struct("WorktreesLoaded")
                .field("repo_id", repo_id)
                .field("result", result)
                .finish(),
            Msg::SubmodulesLoaded { repo_id, result } => f
                .debug_struct("SubmodulesLoaded")
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
            Msg::DiffFileImageLoaded {
                repo_id,
                target,
                result,
            } => f
                .debug_struct("DiffFileImageLoaded")
                .field("repo_id", repo_id)
                .field("target", target)
                .field("result", result)
                .finish(),
            Msg::RepoActionFinished { repo_id, result } => f
                .debug_struct("RepoActionFinished")
                .field("repo_id", repo_id)
                .field("result", result)
                .finish(),
            Msg::CommitFinished { repo_id, result } => f
                .debug_struct("CommitFinished")
                .field("repo_id", repo_id)
                .field("result", result)
                .finish(),
            Msg::CommitAmendFinished { repo_id, result } => f
                .debug_struct("CommitAmendFinished")
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
    LoadUpstreamDivergence {
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
    LoadFileHistory {
        repo_id: RepoId,
        path: PathBuf,
        limit: usize,
    },
    LoadBlame {
        repo_id: RepoId,
        path: PathBuf,
        rev: Option<String>,
    },
    LoadWorktrees {
        repo_id: RepoId,
    },
    LoadSubmodules {
        repo_id: RepoId,
    },
    LoadRebaseState {
        repo_id: RepoId,
    },
    LoadMergeCommitMessage {
        repo_id: RepoId,
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
    LoadDiffFileImage {
        repo_id: RepoId,
        target: DiffTarget,
    },
    LoadConflictFile {
        repo_id: RepoId,
        path: PathBuf,
    },
    SaveWorktreeFile {
        repo_id: RepoId,
        path: PathBuf,
        contents: String,
        stage: bool,
    },

    CheckoutBranch {
        repo_id: RepoId,
        name: String,
    },
    CheckoutRemoteBranch {
        repo_id: RepoId,
        remote: String,
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
    CreateBranchAndCheckout {
        repo_id: RepoId,
        name: String,
    },
    DeleteBranch {
        repo_id: RepoId,
        name: String,
    },
    CloneRepo {
        url: String,
        dest: PathBuf,
    },
    ExportPatch {
        repo_id: RepoId,
        commit_id: CommitId,
        dest: PathBuf,
    },
    ApplyPatch {
        repo_id: RepoId,
        patch: PathBuf,
    },
    AddWorktree {
        repo_id: RepoId,
        path: PathBuf,
        reference: Option<String>,
    },
    RemoveWorktree {
        repo_id: RepoId,
        path: PathBuf,
    },
    AddSubmodule {
        repo_id: RepoId,
        url: String,
        path: PathBuf,
    },
    UpdateSubmodules {
        repo_id: RepoId,
    },
    RemoveSubmodule {
        repo_id: RepoId,
        path: PathBuf,
    },
    StageHunk {
        repo_id: RepoId,
        patch: String,
    },
    UnstageHunk {
        repo_id: RepoId,
        patch: String,
    },
    ApplyWorktreePatch {
        repo_id: RepoId,
        patch: String,
        reverse: bool,
    },
    StagePath {
        repo_id: RepoId,
        path: PathBuf,
    },
    StagePaths {
        repo_id: RepoId,
        paths: Vec<PathBuf>,
    },
    UnstagePath {
        repo_id: RepoId,
        path: PathBuf,
    },
    UnstagePaths {
        repo_id: RepoId,
        paths: Vec<PathBuf>,
    },
    DiscardWorktreeChangesPath {
        repo_id: RepoId,
        path: PathBuf,
    },
    DiscardWorktreeChangesPaths {
        repo_id: RepoId,
        paths: Vec<PathBuf>,
    },
    Commit {
        repo_id: RepoId,
        message: String,
    },
    CommitAmend {
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
    PullBranch {
        repo_id: RepoId,
        remote: String,
        branch: String,
    },
    MergeRef {
        repo_id: RepoId,
        reference: String,
    },
    Push {
        repo_id: RepoId,
    },
    ForcePush {
        repo_id: RepoId,
    },
    PushSetUpstream {
        repo_id: RepoId,
        remote: String,
        branch: String,
    },
    Reset {
        repo_id: RepoId,
        target: String,
        mode: ResetMode,
    },
    Rebase {
        repo_id: RepoId,
        onto: String,
    },
    RebaseContinue {
        repo_id: RepoId,
    },
    RebaseAbort {
        repo_id: RepoId,
    },
    CreateTag {
        repo_id: RepoId,
        name: String,
        target: String,
    },
    DeleteTag {
        repo_id: RepoId,
        name: String,
    },
    AddRemote {
        repo_id: RepoId,
        name: String,
        url: String,
    },
    RemoveRemote {
        repo_id: RepoId,
        name: String,
    },
    SetRemoteUrl {
        repo_id: RepoId,
        name: String,
        url: String,
        kind: RemoteUrlKind,
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
    PopStash {
        repo_id: RepoId,
        index: usize,
    },
}

#[derive(Debug)]
pub enum StoreEvent {
    StateChanged,
}
