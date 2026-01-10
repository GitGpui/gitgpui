use gitgpui_core::domain::*;
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::{GitBackend, GitRepository, Result};
use std::path::Path;
use std::sync::Arc;

#[derive(Default)]
pub struct NoopBackend;

impl GitBackend for NoopBackend {
    fn open(&self, workdir: &Path) -> Result<Arc<dyn GitRepository>> {
        let _ = workdir;
        Err(Error::new(ErrorKind::Unsupported(
            "No Git backend enabled. Build with `--features gix`.",
        )))
    }
}

pub(crate) struct NoopRepo {
    spec: RepoSpec,
}

impl NoopRepo {
    #[allow(dead_code)]
    pub fn new(spec: RepoSpec) -> Self {
        Self { spec }
    }
}

impl GitRepository for NoopRepo {
    fn spec(&self) -> &RepoSpec {
        &self.spec
    }

    fn log_head_page(&self, _limit: usize, _cursor: Option<&LogCursor>) -> Result<LogPage> {
        Err(Error::new(ErrorKind::Unsupported("No Git backend enabled")))
    }

    fn list_branches(&self) -> Result<Vec<Branch>> {
        Err(Error::new(ErrorKind::Unsupported("No Git backend enabled")))
    }

    fn list_remotes(&self) -> Result<Vec<Remote>> {
        Err(Error::new(ErrorKind::Unsupported("No Git backend enabled")))
    }

    fn status(&self) -> Result<Vec<FileStatus>> {
        Err(Error::new(ErrorKind::Unsupported("No Git backend enabled")))
    }

    fn create_branch(&self, _name: &str, _target: &CommitId) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported("No Git backend enabled")))
    }

    fn delete_branch(&self, _name: &str) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported("No Git backend enabled")))
    }

    fn checkout_branch(&self, _name: &str) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported("No Git backend enabled")))
    }

    fn stash_create(&self, _message: &str, _include_untracked: bool) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported("No Git backend enabled")))
    }

    fn stash_list(&self) -> Result<Vec<StashEntry>> {
        Err(Error::new(ErrorKind::Unsupported("No Git backend enabled")))
    }

    fn stash_apply(&self, _index: usize) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported("No Git backend enabled")))
    }

    fn stash_drop(&self, _index: usize) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported("No Git backend enabled")))
    }

    fn discard_worktree_changes(&self, _paths: &[&Path]) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported("No Git backend enabled")))
    }
}

