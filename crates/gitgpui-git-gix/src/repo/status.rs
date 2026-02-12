use super::GixRepo;
use gitgpui_core::domain::{
    FileConflictKind, FileStatus, FileStatusKind, RepoStatus, UpstreamDivergence,
};
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::Result;
use gix::bstr::ByteSlice as _;
use std::path::PathBuf;
use std::process::Command;

impl GixRepo {
    pub(super) fn status_impl(&self) -> Result<RepoStatus> {
        let repo = self._repo.to_thread_local();
        let platform = repo
            .status(gix::progress::Discard)
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix status platform: {e}"))))?
            .untracked_files(gix::status::UntrackedFiles::Files);

        let mut unstaged = Vec::new();
        let mut staged = Vec::new();
        let iter = platform
            .into_iter(std::iter::empty::<gix::bstr::BString>())
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix status iter: {e}"))))?;

        for item in iter {
            let item =
                item.map_err(|e| Error::new(ErrorKind::Backend(format!("gix status item: {e}"))))?;

            match item {
                gix::status::Item::IndexWorktree(item) => match item {
                    gix::status::index_worktree::Item::Modification {
                        rela_path, status, ..
                    } => {
                        let path = PathBuf::from(rela_path.to_str_lossy().into_owned());
                        let (kind, conflict) = map_entry_status(status);
                        unstaged.push(FileStatus {
                            path,
                            kind,
                            conflict,
                        });
                    }
                    gix::status::index_worktree::Item::DirectoryContents { entry, .. } => {
                        let kind = match entry.status {
                            gix::dir::entry::Status::Untracked => FileStatusKind::Untracked,
                            gix::dir::entry::Status::Ignored(_) => continue,
                            gix::dir::entry::Status::Tracked => FileStatusKind::Modified,
                            gix::dir::entry::Status::Pruned => continue,
                        };

                        let path = PathBuf::from(entry.rela_path.to_str_lossy().into_owned());
                        unstaged.push(FileStatus {
                            path,
                            kind,
                            conflict: None,
                        });
                    }
                    gix::status::index_worktree::Item::Rewrite {
                        dirwalk_entry,
                        copy,
                        ..
                    } => {
                        let kind = if copy {
                            FileStatusKind::Added
                        } else {
                            FileStatusKind::Renamed
                        };

                        let path =
                            PathBuf::from(dirwalk_entry.rela_path.to_str_lossy().into_owned());
                        unstaged.push(FileStatus {
                            path,
                            kind,
                            conflict: None,
                        });
                    }
                },

                gix::status::Item::TreeIndex(change) => {
                    use gix_diff::index::ChangeRef;

                    let (path, kind) = match change {
                        ChangeRef::Addition { location, .. } => (
                            PathBuf::from(location.to_str_lossy().into_owned()),
                            FileStatusKind::Added,
                        ),
                        ChangeRef::Deletion { location, .. } => (
                            PathBuf::from(location.to_str_lossy().into_owned()),
                            FileStatusKind::Deleted,
                        ),
                        ChangeRef::Modification { location, .. } => (
                            PathBuf::from(location.to_str_lossy().into_owned()),
                            FileStatusKind::Modified,
                        ),
                        ChangeRef::Rewrite { location, copy, .. } => (
                            PathBuf::from(location.to_str_lossy().into_owned()),
                            if copy {
                                FileStatusKind::Added
                            } else {
                                FileStatusKind::Renamed
                            },
                        ),
                    };

                    staged.push(FileStatus {
                        path,
                        kind,
                        conflict: None,
                    });
                }
            }
        }

        fn kind_priority(kind: FileStatusKind) -> u8 {
            match kind {
                FileStatusKind::Conflicted => 5,
                FileStatusKind::Renamed => 4,
                FileStatusKind::Deleted => 3,
                FileStatusKind::Added => 2,
                FileStatusKind::Modified => 1,
                FileStatusKind::Untracked => 0,
            }
        }

        fn sort_and_dedup(entries: &mut Vec<FileStatus>) {
            entries.sort_unstable_by(|a, b| {
                a.path
                    .cmp(&b.path)
                    .then_with(|| kind_priority(b.kind).cmp(&kind_priority(a.kind)))
            });
            entries.dedup_by(|a, b| a.path == b.path);
        }

        sort_and_dedup(&mut staged);
        sort_and_dedup(&mut unstaged);

        // gix may report unmerged entries (conflicts) as both Index/Worktree and Tree/Index
        // changes, which causes the same path to show up in both sections in the UI. Mirror
        // `git status` behavior by showing conflicted paths only once.
        let conflicted: std::collections::HashSet<std::path::PathBuf> = unstaged
            .iter()
            .filter(|e| e.kind == FileStatusKind::Conflicted)
            .map(|e| e.path.clone())
            .collect();
        if !conflicted.is_empty() {
            staged.retain(|e| !conflicted.contains(&e.path));
        }

        Ok(RepoStatus { staged, unstaged })
    }

    pub(super) fn upstream_divergence_impl(&self) -> Result<Option<UpstreamDivergence>> {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.spec.workdir)
            .arg("rev-list")
            .arg("--left-right")
            .arg("--count")
            .arg("@{upstream}...HEAD")
            .output()
            .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

        if !output.status.success() {
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut parts = stdout.split_whitespace();
        let behind = parts.next().and_then(|s| s.parse::<usize>().ok());
        let ahead = parts.next().and_then(|s| s.parse::<usize>().ok());
        Ok(match (ahead, behind) {
            (Some(ahead), Some(behind)) => Some(UpstreamDivergence { ahead, behind }),
            _ => None,
        })
    }
}

fn map_entry_status<T, U>(
    status: gix::status::plumbing::index_as_worktree::EntryStatus<T, U>,
) -> (FileStatusKind, Option<FileConflictKind>) {
    use gix::status::plumbing::index_as_worktree::{Change, Conflict, EntryStatus};

    match status {
        EntryStatus::Conflict { summary, .. } => (
            FileStatusKind::Conflicted,
            Some(match summary {
                Conflict::BothDeleted => FileConflictKind::BothDeleted,
                Conflict::AddedByUs => FileConflictKind::AddedByUs,
                Conflict::DeletedByThem => FileConflictKind::DeletedByThem,
                Conflict::AddedByThem => FileConflictKind::AddedByThem,
                Conflict::DeletedByUs => FileConflictKind::DeletedByUs,
                Conflict::BothAdded => FileConflictKind::BothAdded,
                Conflict::BothModified => FileConflictKind::BothModified,
            }),
        ),
        EntryStatus::IntentToAdd => (FileStatusKind::Added, None),
        EntryStatus::NeedsUpdate(_) => (FileStatusKind::Modified, None),
        EntryStatus::Change(change) => (
            match change {
                Change::Removed => FileStatusKind::Deleted,
                Change::Type { .. } => FileStatusKind::Modified,
                Change::Modification { .. } => FileStatusKind::Modified,
                Change::SubmoduleModification(_) => FileStatusKind::Modified,
            },
            None,
        ),
    }
}
