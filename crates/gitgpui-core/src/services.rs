use crate::domain::*;
use crate::error::Error;
use std::path::Path;
use std::sync::Arc;

pub type Result<T> = std::result::Result<T, Error>;

pub trait GitRepository: Send + Sync {
    fn spec(&self) -> &RepoSpec;

    fn log_head_page(&self, limit: usize, cursor: Option<&LogCursor>) -> Result<LogPage>;
    fn list_branches(&self) -> Result<Vec<Branch>>;
    fn list_remotes(&self) -> Result<Vec<Remote>>;
    fn status(&self) -> Result<Vec<FileStatus>>;

    fn create_branch(&self, name: &str, target: &CommitId) -> Result<()>;
    fn delete_branch(&self, name: &str) -> Result<()>;
    fn checkout_branch(&self, name: &str) -> Result<()>;

    fn stash_create(&self, message: &str, include_untracked: bool) -> Result<()>;
    fn stash_list(&self) -> Result<Vec<StashEntry>>;
    fn stash_apply(&self, index: usize) -> Result<()>;
    fn stash_drop(&self, index: usize) -> Result<()>;

    fn discard_worktree_changes(&self, paths: &[&Path]) -> Result<()>;
}

pub trait GitBackend: Send + Sync {
    fn open(&self, workdir: &Path) -> Result<Arc<dyn GitRepository>>;
}

