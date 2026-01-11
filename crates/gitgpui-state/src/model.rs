use gitgpui_core::domain::*;
use std::time::SystemTime;

#[derive(Clone, Debug, Default)]
pub struct AppState {
    pub repos: Vec<RepoState>,
    pub active_repo: Option<RepoId>,
}

#[derive(Clone, Debug)]
pub struct RepoState {
    pub id: RepoId,
    pub spec: RepoSpec,

    pub open: Loadable<()>,
    pub head_branch: Loadable<String>,
    pub branches: Loadable<Vec<Branch>>,
    pub remotes: Loadable<Vec<Remote>>,
    pub remote_branches: Loadable<Vec<RemoteBranch>>,
    pub status: Loadable<RepoStatus>,
    pub log: Loadable<LogPage>,
    pub stashes: Loadable<Vec<StashEntry>>,
    pub reflog: Loadable<Vec<ReflogEntry>>,

    pub selected_commit: Option<CommitId>,
    pub commit_details: Loadable<CommitDetails>,
    pub diff_target: Option<DiffTarget>,
    pub diff: Loadable<Diff>,

    pub last_error: Option<String>,
    pub diagnostics: Vec<DiagnosticEntry>,
}

impl RepoState {
    pub fn new_opening(id: RepoId, spec: RepoSpec) -> Self {
        Self {
            id,
            spec,
            open: Loadable::Loading,
            head_branch: Loadable::NotLoaded,
            branches: Loadable::NotLoaded,
            remotes: Loadable::NotLoaded,
            remote_branches: Loadable::NotLoaded,
            status: Loadable::NotLoaded,
            log: Loadable::NotLoaded,
            stashes: Loadable::NotLoaded,
            reflog: Loadable::NotLoaded,
            selected_commit: None,
            commit_details: Loadable::NotLoaded,
            diff_target: None,
            diff: Loadable::NotLoaded,
            last_error: None,
            diagnostics: Vec::new(),
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
