use crate::domain::*;
use crate::error::{Error, ErrorKind};
use std::path::Path;
use std::path::PathBuf;
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
pub struct ConflictFileStages {
    pub path: PathBuf,
    pub base: Option<String>,
    pub ours: Option<String>,
    pub theirs: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteUrlKind {
    Fetch,
    Push,
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
    fn log_file_page(
        &self,
        _path: &Path,
        _limit: usize,
        _cursor: Option<&LogCursor>,
    ) -> Result<LogPage> {
        Err(Error::new(ErrorKind::Unsupported(
            "file history is not implemented for this backend",
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
    fn upstream_divergence(&self) -> Result<Option<UpstreamDivergence>> {
        Ok(None)
    }
    fn diff_unified(&self, target: &DiffTarget) -> Result<String>;
    fn diff_file_text(&self, _target: &DiffTarget) -> Result<Option<FileDiffText>> {
        Err(Error::new(ErrorKind::Unsupported(
            "file diff view is not implemented for this backend",
        )))
    }
    fn diff_file_image(&self, _target: &DiffTarget) -> Result<Option<FileDiffImage>> {
        Err(Error::new(ErrorKind::Unsupported(
            "image diff view is not implemented for this backend",
        )))
    }

    fn conflict_file_stages(&self, _path: &Path) -> Result<Option<ConflictFileStages>> {
        Err(Error::new(ErrorKind::Unsupported(
            "conflict stage reading is not implemented for this backend",
        )))
    }

    fn create_branch(&self, name: &str, target: &CommitId) -> Result<()>;
    fn delete_branch(&self, name: &str) -> Result<()>;
    fn checkout_branch(&self, name: &str) -> Result<()>;
    fn checkout_remote_branch(&self, _remote: &str, _branch: &str) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported(
            "remote branch checkout is not implemented for this backend",
        )))
    }
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
    fn commit_amend(&self, _message: &str) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported(
            "commit amend is not implemented for this backend",
        )))
    }

    fn rebase_with_output(&self, _onto: &str) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "git rebase is not implemented for this backend",
        )))
    }
    fn rebase_continue_with_output(&self) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "git rebase --continue is not implemented for this backend",
        )))
    }
    fn rebase_abort_with_output(&self) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "git rebase --abort is not implemented for this backend",
        )))
    }
    fn rebase_in_progress(&self) -> Result<bool> {
        Ok(false)
    }

    fn merge_commit_message(&self) -> Result<Option<String>> {
        Ok(None)
    }

    fn create_tag_with_output(&self, _name: &str, _target: &str) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "git tag creation is not implemented for this backend",
        )))
    }
    fn delete_tag_with_output(&self, _name: &str) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "git tag deletion is not implemented for this backend",
        )))
    }

    fn add_remote_with_output(&self, _name: &str, _url: &str) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "git remote add is not implemented for this backend",
        )))
    }
    fn remove_remote_with_output(&self, _name: &str) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "git remote remove is not implemented for this backend",
        )))
    }
    fn set_remote_url_with_output(
        &self,
        _name: &str,
        _url: &str,
        _kind: RemoteUrlKind,
    ) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "git remote set-url is not implemented for this backend",
        )))
    }

    fn fetch_all(&self) -> Result<()>;
    fn pull(&self, mode: PullMode) -> Result<()>;
    fn push(&self) -> Result<()>;
    fn push_force(&self) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported(
            "force push is not implemented for this backend",
        )))
    }
    fn push_set_upstream(&self, _remote: &str, _branch: &str) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported(
            "pushing with --set-upstream is not implemented for this backend",
        )))
    }

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

    fn push_force_with_output(&self) -> Result<CommandOutput> {
        self.push_force()?;
        Ok(CommandOutput::empty_success("git push --force-with-lease"))
    }

    fn push_set_upstream_with_output(&self, remote: &str, branch: &str) -> Result<CommandOutput> {
        self.push_set_upstream(remote, branch)?;
        Ok(CommandOutput::empty_success(format!(
            "git push --set-upstream {remote} HEAD:refs/heads/{branch}"
        )))
    }

    fn commit_amend_with_output(&self, message: &str) -> Result<CommandOutput> {
        self.commit_amend(message)?;
        Ok(CommandOutput::empty_success("git commit --amend"))
    }

    fn pull_branch_with_output(&self, _remote: &str, _branch: &str) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "pulling a specific remote branch is not implemented for this backend",
        )))
    }

    fn merge_ref_with_output(&self, _reference: &str) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "merging a specific ref is not implemented for this backend",
        )))
    }

    fn reset_with_output(&self, _target: &str, _mode: ResetMode) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "git reset is not implemented for this backend",
        )))
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

    fn export_patch_with_output(
        &self,
        _commit_id: &CommitId,
        _dest: &Path,
    ) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "patch export is not implemented for this backend",
        )))
    }

    fn apply_patch_with_output(&self, _patch: &Path) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "patch apply is not implemented for this backend",
        )))
    }

    fn apply_unified_patch_to_index_with_output(
        &self,
        _patch: &str,
        _reverse: bool,
    ) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "index patch apply is not implemented for this backend",
        )))
    }

    fn apply_unified_patch_to_worktree_with_output(
        &self,
        _patch: &str,
        _reverse: bool,
    ) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "worktree patch apply is not implemented for this backend",
        )))
    }

    fn list_worktrees(&self) -> Result<Vec<Worktree>> {
        Err(Error::new(ErrorKind::Unsupported(
            "worktree listing is not implemented for this backend",
        )))
    }

    fn add_worktree_with_output(
        &self,
        _path: &Path,
        _reference: Option<&str>,
    ) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "worktree add is not implemented for this backend",
        )))
    }

    fn remove_worktree_with_output(&self, _path: &Path) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "worktree remove is not implemented for this backend",
        )))
    }

    fn list_submodules(&self) -> Result<Vec<Submodule>> {
        Err(Error::new(ErrorKind::Unsupported(
            "submodule listing is not implemented for this backend",
        )))
    }

    fn add_submodule_with_output(&self, _url: &str, _path: &Path) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "submodule add is not implemented for this backend",
        )))
    }

    fn update_submodules_with_output(&self) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "submodule update is not implemented for this backend",
        )))
    }

    fn remove_submodule_with_output(&self, _path: &Path) -> Result<CommandOutput> {
        Err(Error::new(ErrorKind::Unsupported(
            "submodule remove is not implemented for this backend",
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
