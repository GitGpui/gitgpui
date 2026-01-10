use gitgpui_core::domain::*;

#[derive(Clone, Debug, Default)]
pub struct AppState {
    pub current_repo: Option<RepoState>,
}

#[derive(Clone, Debug)]
pub struct RepoState {
    pub id: RepoId,
    pub spec: RepoSpec,

    pub open: Loadable<()>,
    pub branches: Loadable<Vec<Branch>>,
    pub remotes: Loadable<Vec<Remote>>,
    pub status: Loadable<Vec<FileStatus>>,
    pub log: Loadable<LogPage>,
}

impl RepoState {
    pub fn new_opening(id: RepoId, spec: RepoSpec) -> Self {
        Self {
            id,
            spec,
            open: Loadable::Loading,
            branches: Loadable::NotLoaded,
            remotes: Loadable::NotLoaded,
            status: Loadable::NotLoaded,
            log: Loadable::NotLoaded,
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

