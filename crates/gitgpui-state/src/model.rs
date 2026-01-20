use gitgpui_core::domain::*;
use std::time::SystemTime;

#[derive(Clone, Debug, Default)]
pub struct AppState {
    pub repos: Vec<RepoState>,
    pub active_repo: Option<RepoId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandLogEntry {
    pub time: SystemTime,
    pub ok: bool,
    pub command: String,
    pub summary: String,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Clone, Debug)]
pub struct RepoState {
    pub id: RepoId,
    pub spec: RepoSpec,

    pub open: Loadable<()>,
    pub history_scope: LogScope,
    pub head_branch: Loadable<String>,
    pub upstream_divergence: Loadable<Option<UpstreamDivergence>>,
    pub branches: Loadable<Vec<Branch>>,
    pub tags: Loadable<Vec<Tag>>,
    pub remotes: Loadable<Vec<Remote>>,
    pub remote_branches: Loadable<Vec<RemoteBranch>>,
    pub status: Loadable<RepoStatus>,
    pub log: Loadable<LogPage>,
    pub stashes: Loadable<Vec<StashEntry>>,
    pub reflog: Loadable<Vec<ReflogEntry>>,

    pub selected_commit: Option<CommitId>,
    pub commit_details: Loadable<CommitDetails>,
    pub diff_target: Option<DiffTarget>,
    pub diff_rev: u64,
    pub diff: Loadable<Diff>,
    pub diff_file_rev: u64,
    pub diff_file: Loadable<Option<FileDiffText>>,

    pub last_error: Option<String>,
    pub diagnostics: Vec<DiagnosticEntry>,

    pub command_log: Vec<CommandLogEntry>,
}

impl RepoState {
    pub fn new_opening(id: RepoId, spec: RepoSpec) -> Self {
        Self {
            id,
            spec,
            open: Loadable::Loading,
            history_scope: LogScope::CurrentBranch,
            head_branch: Loadable::NotLoaded,
            upstream_divergence: Loadable::NotLoaded,
            branches: Loadable::NotLoaded,
            tags: Loadable::NotLoaded,
            remotes: Loadable::NotLoaded,
            remote_branches: Loadable::NotLoaded,
            status: Loadable::NotLoaded,
            log: Loadable::NotLoaded,
            stashes: Loadable::NotLoaded,
            reflog: Loadable::NotLoaded,
            selected_commit: None,
            commit_details: Loadable::NotLoaded,
            diff_target: None,
            diff_rev: 0,
            diff: Loadable::NotLoaded,
            diff_file_rev: 0,
            diff_file: Loadable::NotLoaded,
            last_error: None,
            diagnostics: Vec::new(),
            command_log: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticEntry {
    pub time: SystemTime,
    pub kind: DiagnosticKind,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticKind {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct RepoId(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Loadable<T> {
    NotLoaded,
    Loading,
    Ready(T),
    Error(String),
}

impl<T> Loadable<T> {
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }
}
