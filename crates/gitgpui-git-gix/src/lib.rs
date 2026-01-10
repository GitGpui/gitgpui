use gitgpui_core::domain::{
    Branch, Commit, CommitId, FileStatus, FileStatusKind, LogCursor, LogPage, Remote, RepoSpec,
};
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::{GitBackend, GitRepository, Result};
use gix::bstr::ByteSlice as _;
use gix::traverse::commit::simple::CommitTimeOrder;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

pub struct GixBackend;

impl Default for GixBackend {
    fn default() -> Self {
        Self
    }
}

impl GitBackend for GixBackend {
    fn open(&self, workdir: &Path) -> Result<Arc<dyn GitRepository>> {
        let workdir = workdir
            .canonicalize()
            .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

        let repo = gix::open(&workdir)
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix open: {e}"))))?;

        Ok(Arc::new(GixRepo {
            spec: RepoSpec {
                workdir: workdir.clone(),
            },
            _workdir: workdir,
            _repo: repo.into_sync(),
        }))
    }
}

pub struct GixRepo {
    spec: RepoSpec,
    _workdir: PathBuf,
    _repo: gix::ThreadSafeRepository,
}

impl GitRepository for GixRepo {
    fn spec(&self) -> &RepoSpec {
        &self.spec
    }

    fn log_head_page(
        &self,
        limit: usize,
        cursor: Option<&LogCursor>,
    ) -> Result<LogPage> {
        let repo = self._repo.to_thread_local();
        let head_id = repo
            .head_id()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix head_id: {e}"))))?
            .detach();

        let mut walk = repo
            .rev_walk([head_id])
            .sorting(gix::revision::walk::Sorting::ByCommitTime(
                CommitTimeOrder::NewestFirst,
            ))
            .all()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix rev_walk: {e}"))))?;

        let mut started = cursor.is_none();
        let mut commits = Vec::with_capacity(limit.min(2048));
        let mut next_cursor: Option<LogCursor> = None;

        while let Some(info) = walk.next() {
            let info = info.map_err(|e| Error::new(ErrorKind::Backend(format!("gix walk: {e}"))))?;
            let id = info.id().detach().to_string();

            if !started {
                if let Some(c) = cursor {
                    if c.last_seen.0 == id {
                        started = true;
                    }
                }
                continue;
            }

            let commit_obj = info
                .object()
                .map_err(|e| Error::new(ErrorKind::Backend(format!("gix commit object: {e}"))))?;

            let summary = commit_obj
                .message_raw_sloppy()
                .lines()
                .next()
                .unwrap_or_default()
                .to_str_lossy()
                .into_owned();

            let author = commit_obj
                .author()
                .map(|s| s.name.to_str_lossy().into_owned())
                .unwrap_or_else(|_| "unknown".to_string());

            let seconds = commit_obj.time().map(|t| t.seconds).unwrap_or(0);
            let time = if seconds >= 0 {
                SystemTime::UNIX_EPOCH + Duration::from_secs(seconds as u64)
            } else {
                SystemTime::UNIX_EPOCH
            };

            let parent_ids = info
                .parent_ids()
                .map(|p| CommitId(p.detach().to_string()))
                .collect::<Vec<_>>();

            commits.push(Commit {
                id: CommitId(id),
                parent_ids,
                summary,
                author,
                time,
            });

            if commits.len() >= limit {
                next_cursor = commits.last().map(|c| LogCursor {
                    last_seen: c.id.clone(),
                });
                break;
            }
        }

        Ok(LogPage {
            commits,
            next_cursor,
        })
    }

    fn list_branches(&self) -> Result<Vec<Branch>> {
        let repo = self._repo.to_thread_local();

        let refs = repo
            .references()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix references: {e}"))))?;

        let iter = refs
            .local_branches()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix local_branches: {e}"))))?
            .peeled()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix peel refs: {e}"))))?;

        let mut branches = Vec::new();
        for reference in iter {
            let reference = reference.map_err(|e| {
                Error::new(ErrorKind::Backend(format!("gix ref iter: {e}")))
            })?;
            let name = reference.name().shorten().to_str_lossy().into_owned();
            let target = CommitId(reference.id().detach().to_string());

            branches.push(Branch {
                name,
                target,
                upstream: None,
            });
        }

        branches.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(branches)
    }

    fn list_remotes(&self) -> Result<Vec<Remote>> {
        let repo = self._repo.to_thread_local();
        let mut remotes = Vec::new();

        for name in repo.remote_names() {
            let remote = repo
                .find_remote(name.as_ref())
                .map_err(|e| Error::new(ErrorKind::Backend(format!("gix find_remote: {e}"))))?;

            let url = remote
                .url(gix::remote::Direction::Fetch)
                .map(|u| u.to_string());

            remotes.push(Remote {
                name: name.to_str_lossy().into_owned(),
                url,
            });
        }

        remotes.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(remotes)
    }

    fn status(&self) -> Result<Vec<gitgpui_core::domain::FileStatus>> {
        let repo = self._repo.to_thread_local();
        let platform = repo
            .status(gix::progress::Discard)
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix status platform: {e}"))))?
            .untracked_files(gix::status::UntrackedFiles::Files);

        let mut out = Vec::new();
        let iter = platform
            .into_iter(std::iter::empty::<gix::bstr::BString>())
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix status iter: {e}"))))?;

        for item in iter {
            let item = item
                .map_err(|e| Error::new(ErrorKind::Backend(format!("gix status item: {e}"))))?;

            match item {
                gix::status::Item::IndexWorktree(item) => match item {
                    gix::status::index_worktree::Item::Modification {
                        rela_path, status, ..
                    } => {
                        let path = PathBuf::from(rela_path.to_str_lossy().into_owned());
                        let kind = map_entry_status(status);
                        out.push(FileStatus { path, kind });
                    }
                    gix::status::index_worktree::Item::DirectoryContents { entry, .. } => {
                        let kind = match entry.status {
                            gix::dir::entry::Status::Untracked => FileStatusKind::Untracked,
                            gix::dir::entry::Status::Ignored(_) => continue,
                            gix::dir::entry::Status::Tracked => FileStatusKind::Modified,
                            gix::dir::entry::Status::Pruned => continue,
                        };

                        let path = PathBuf::from(entry.rela_path.to_str_lossy().into_owned());
                        out.push(FileStatus { path, kind });
                    }
                    gix::status::index_worktree::Item::Rewrite {
                        dirwalk_entry, copy, ..
                    } => {
                        let kind = if copy {
                            FileStatusKind::Added
                        } else {
                            FileStatusKind::Renamed
                        };

                        let path = PathBuf::from(dirwalk_entry.rela_path.to_str_lossy().into_owned());
                        out.push(FileStatus { path, kind });
                    }
                },

                // Staged changes will be handled later when we split status into
                // "index" vs "worktree" sections in the domain model.
                gix::status::Item::TreeIndex(_) => {}
            }
        }

        Ok(out)
    }

    fn create_branch(&self, _name: &str, _target: &gitgpui_core::domain::CommitId) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported(
            "gix backend skeleton: create_branch not implemented yet",
        )))
    }

    fn delete_branch(&self, _name: &str) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported(
            "gix backend skeleton: delete_branch not implemented yet",
        )))
    }

    fn checkout_branch(&self, _name: &str) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported(
            "gix backend skeleton: checkout_branch not implemented yet",
        )))
    }

    fn stash_create(&self, _message: &str, _include_untracked: bool) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported(
            "gix backend skeleton: stash_create not implemented yet",
        )))
    }

    fn stash_list(&self) -> Result<Vec<gitgpui_core::domain::StashEntry>> {
        Err(Error::new(ErrorKind::Unsupported(
            "gix backend skeleton: stash_list not implemented yet",
        )))
    }

    fn stash_apply(&self, _index: usize) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported(
            "gix backend skeleton: stash_apply not implemented yet",
        )))
    }

    fn stash_drop(&self, _index: usize) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported(
            "gix backend skeleton: stash_drop not implemented yet",
        )))
    }

    fn discard_worktree_changes(&self, _paths: &[&Path]) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported(
            "gix backend skeleton: discard not implemented yet",
        )))
    }
}

fn map_entry_status<T, U>(status: gix::status::plumbing::index_as_worktree::EntryStatus<T, U>) -> FileStatusKind {
    use gix::status::plumbing::index_as_worktree::{Change, EntryStatus};

    match status {
        EntryStatus::Conflict(_) => FileStatusKind::Conflicted,
        EntryStatus::IntentToAdd => FileStatusKind::Added,
        EntryStatus::NeedsUpdate(_) => FileStatusKind::Modified,
        EntryStatus::Change(change) => match change {
            Change::Removed => FileStatusKind::Deleted,
            Change::Type { .. } => FileStatusKind::Modified,
            Change::Modification { .. } => FileStatusKind::Modified,
            Change::SubmoduleModification(_) => FileStatusKind::Modified,
        },
    }
}
