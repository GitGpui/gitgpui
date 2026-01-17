use crate::domain::*;
use crate::error::{Error, ErrorKind};
use std::path::Path;
use std::sync::Arc;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommandOutput {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

impl CommandOutput {
    pub fn empty_success(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            stdout: String::new(),
            stderr: String::new(),
            exit_code: Some(0),
        }
    }

    pub fn combined(&self) -> String {
        let mut out = String::new();
        if !self.stdout.trim().is_empty() {
            out.push_str(self.stdout.trim_end());
            out.push('\n');
        }
        if !self.stderr.trim().is_empty() {
            out.push_str(self.stderr.trim_end());
            out.push('\n');
        }
        out.trim_end().to_string()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConflictSide {
    Ours,
    Theirs,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlameLine {
    pub commit_id: String,
    pub author: String,
    pub author_time_unix: Option<i64>,
    pub summary: String,
    pub line: String,
}

pub trait GitRepository: Send + Sync {
    fn spec(&self) -> &RepoSpec;

    fn log_head_page(&self, limit: usize, cursor: Option<&LogCursor>) -> Result<LogPage>;
    fn log_all_branches_page(&self, _limit: usize, _cursor: Option<&LogCursor>) -> Result<LogPage> {
        Err(Error::new(ErrorKind::Unsupported(
            "all-branches history is not implemented for this backend",
        )))
    }
    fn commit_details(&self, id: &CommitId) -> Result<CommitDetails>;
    fn reflog_head(&self, limit: usize) -> Result<Vec<ReflogEntry>>;
    fn current_branch(&self) -> Result<String>;
    fn list_branches(&self) -> Result<Vec<Branch>>;
    fn list_tags(&self) -> Result<Vec<Tag>> {
        Err(Error::new(ErrorKind::Unsupported(
            "tag listing is not implemented for this backend",
        )))
    }
    fn list_remotes(&self) -> Result<Vec<Remote>>;
    fn list_remote_branches(&self) -> Result<Vec<RemoteBranch>>;
    fn status(&self) -> Result<RepoStatus>;
    fn diff_unified(&self, target: &DiffTarget) -> Result<String>;
    fn diff_file_text(&self, _target: &DiffTarget) -> Result<Option<FileDiffText>> {
        Err(Error::new(ErrorKind::Unsupported(
            "file diff view is not implemented for this backend",
        )))
    }

    fn create_branch(&self, name: &str, target: &CommitId) -> Result<()>;
    fn delete_branch(&self, name: &str) -> Result<()>;
    fn checkout_branch(&self, name: &str) -> Result<()>;
    fn checkout_commit(&self, id: &CommitId) -> Result<()>;
    fn cherry_pick(&self, id: &CommitId) -> Result<()>;
    fn revert(&self, id: &CommitId) -> Result<()>;

    fn stash_create(&self, message: &str, include_untracked: bool) -> Result<()>;
    fn stash_list(&self) -> Result<Vec<StashEntry>>;
    fn stash_apply(&self, index: usize) -> Result<()>;
    fn stash_drop(&self, index: usize) -> Result<()>;

    fn stage(&self, paths: &[&Path]) -> Result<()>;
    fn unstage(&self, paths: &[&Path]) -> Result<()>;
    fn commit(&self, message: &str) -> Result<()>;
    fn fetch_all(&self) -> Result<()>;
    fn pull(&self, mode: PullMode) -> Result<()>;
    fn push(&self) -> Result<()>;

    fn fetch_all_with_output(&self) -> Result<CommandOutput> {
        self.fetch_all()?;
        Ok(CommandOutput::empty_success("git fetch --all"))
    }

    fn pull_with_output(&self, mode: PullMode) -> Result<CommandOutput> {
        self.pull(mode)?;
        Ok(CommandOutput::empty_success("git pull"))
    }

    fn push_with_output(&self) -> Result<CommandOutput> {
        self.push()?;
        Ok(CommandOutput::empty_success("git push"))
    }

    fn blame_file(&self, _path: &Path, _rev: Option<&str>) -> Result<Vec<BlameLine>> {
        Err(Error::new(ErrorKind::Unsupported(
            "git blame is not implemented for this backend",
        )))
    }

    fn checkout_conflict_side(&self, _path: &Path, _side: ConflictSide) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "conflict resolution is not implemented for this backend",
        )))
    }

    fn discard_worktree_changes(&self, paths: &[&Path]) -> Result<()>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PullMode {
    Default,
    FastForwardIfPossible,
    FastForwardOnly,
    Rebase,
}

pub trait GitBackend: Send + Sync {
    fn open(&self, workdir: &Path) -> Result<Arc<dyn GitRepository>>;
}
