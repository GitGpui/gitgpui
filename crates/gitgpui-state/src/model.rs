use gitgpui_core::domain::*;

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

    pub diff_target: Option<DiffTarget>,
    pub diff: Loadable<Diff>,

    pub last_error: Option<String>,
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
            diff_target: None,
            diff: Loadable::NotLoaded,
            last_error: None,
        }
    }
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
