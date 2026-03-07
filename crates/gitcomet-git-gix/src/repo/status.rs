use super::GixRepo;
use gitcomet_core::domain::{
    FileConflictKind, FileStatus, FileStatusKind, RepoStatus, UpstreamDivergence,
};
use gitcomet_core::error::{Error, ErrorKind};
use gitcomet_core::services::Result;
use gix::bstr::ByteSlice as _;
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::path::{Path, PathBuf};
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

        // Some platforms may omit certain unmerged shapes (notably stage-1-only
        // both-deleted conflicts) from gix status output. Supplement conflict
        // entries from the index's unmerged stages for complete parity.
        for (path, conflict_kind) in git_unmerged_conflicts(&self.spec.workdir)? {
            if let Some(entry) = unstaged.iter_mut().find(|entry| entry.path == path) {
                entry.kind = FileStatusKind::Conflicted;
                entry.conflict = Some(conflict_kind);
            } else {
                unstaged.push(FileStatus {
                    path,
                    kind: FileStatusKind::Conflicted,
                    conflict: Some(conflict_kind),
                });
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
        let conflicted: HashSet<std::path::PathBuf> = unstaged
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

fn git_unmerged_conflicts(workdir: &Path) -> Result<Vec<(PathBuf, FileConflictKind)>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workdir)
        .arg("ls-files")
        .arg("-u")
        .arg("-z")
        .output()
        .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::new(ErrorKind::Backend(format!(
            "git ls-files -u failed: {}",
            stderr.trim()
        ))));
    }

    Ok(parse_unmerged_conflicts(&output.stdout))
}

fn parse_unmerged_conflicts(stdout: &[u8]) -> Vec<(PathBuf, FileConflictKind)> {
    let mut stage_masks: HashMap<PathBuf, u8> = HashMap::default();

    for record in stdout
        .split(|b| *b == b'\0')
        .filter(|record| !record.is_empty())
    {
        let Some(tab_index) = record.iter().rposition(|b| *b == b'\t') else {
            continue;
        };
        let metadata = &record[..tab_index];
        let path_bytes = &record[tab_index + 1..];
        if path_bytes.is_empty() {
            continue;
        }

        let stage = metadata
            .split(|b| *b == b' ')
            .filter(|part| !part.is_empty())
            .next_back()
            .and_then(|part| std::str::from_utf8(part).ok())
            .and_then(|part| part.parse::<u8>().ok());

        let Some(stage @ 1..=3) = stage else {
            continue;
        };

        let path = PathBuf::from(String::from_utf8_lossy(path_bytes).into_owned());
        let bit = 1u8 << (stage - 1);
        stage_masks
            .entry(path)
            .and_modify(|mask| *mask |= bit)
            .or_insert(bit);
    }

    let mut conflicts = stage_masks
        .into_iter()
        .filter_map(|(path, mask)| conflict_kind_from_stage_mask(mask).map(|kind| (path, kind)))
        .collect::<Vec<_>>();
    conflicts.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    conflicts
}

fn conflict_kind_from_stage_mask(mask: u8) -> Option<FileConflictKind> {
    Some(match mask {
        0b001 => FileConflictKind::BothDeleted,
        0b010 => FileConflictKind::AddedByUs,
        0b011 => FileConflictKind::DeletedByThem,
        0b100 => FileConflictKind::AddedByThem,
        0b101 => FileConflictKind::DeletedByUs,
        0b110 => FileConflictKind::BothAdded,
        0b111 => FileConflictKind::BothModified,
        _ => return None,
    })
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

#[cfg(test)]
mod tests {
    use super::{conflict_kind_from_stage_mask, parse_unmerged_conflicts};
    use gitcomet_core::domain::FileConflictKind;
    use rustc_hash::FxHashMap as HashMap;
    use std::path::PathBuf;

    #[test]
    fn conflict_kind_from_stage_mask_covers_all_shapes() {
        assert_eq!(
            conflict_kind_from_stage_mask(0b001),
            Some(FileConflictKind::BothDeleted)
        );
        assert_eq!(
            conflict_kind_from_stage_mask(0b010),
            Some(FileConflictKind::AddedByUs)
        );
        assert_eq!(
            conflict_kind_from_stage_mask(0b011),
            Some(FileConflictKind::DeletedByThem)
        );
        assert_eq!(
            conflict_kind_from_stage_mask(0b100),
            Some(FileConflictKind::AddedByThem)
        );
        assert_eq!(
            conflict_kind_from_stage_mask(0b101),
            Some(FileConflictKind::DeletedByUs)
        );
        assert_eq!(
            conflict_kind_from_stage_mask(0b110),
            Some(FileConflictKind::BothAdded)
        );
        assert_eq!(
            conflict_kind_from_stage_mask(0b111),
            Some(FileConflictKind::BothModified)
        );
        assert_eq!(conflict_kind_from_stage_mask(0), None);
    }

    #[test]
    fn parse_unmerged_conflicts_groups_stage_entries_by_path() {
        let stdout = concat!(
            "100644 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1\tdd.txt\0",
            "100644 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 2\tau.txt\0",
            "100644 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1\tud.txt\0",
            "100644 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 2\tud.txt\0",
            "100644 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 3\tua.txt\0",
            "100644 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1\tdu.txt\0",
            "100644 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 3\tdu.txt\0",
            "100644 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 2\taa.txt\0",
            "100644 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 3\taa.txt\0",
            "100644 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1\tuu.txt\0",
            "100644 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 2\tuu.txt\0",
            "100644 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 3\tuu.txt\0"
        )
        .as_bytes();

        let parsed = parse_unmerged_conflicts(stdout);
        let by_path = parsed
            .into_iter()
            .collect::<HashMap<PathBuf, FileConflictKind>>();

        assert_eq!(
            by_path.get(&PathBuf::from("dd.txt")),
            Some(&FileConflictKind::BothDeleted)
        );
        assert_eq!(
            by_path.get(&PathBuf::from("au.txt")),
            Some(&FileConflictKind::AddedByUs)
        );
        assert_eq!(
            by_path.get(&PathBuf::from("ud.txt")),
            Some(&FileConflictKind::DeletedByThem)
        );
        assert_eq!(
            by_path.get(&PathBuf::from("ua.txt")),
            Some(&FileConflictKind::AddedByThem)
        );
        assert_eq!(
            by_path.get(&PathBuf::from("du.txt")),
            Some(&FileConflictKind::DeletedByUs)
        );
        assert_eq!(
            by_path.get(&PathBuf::from("aa.txt")),
            Some(&FileConflictKind::BothAdded)
        );
        assert_eq!(
            by_path.get(&PathBuf::from("uu.txt")),
            Some(&FileConflictKind::BothModified)
        );
    }
}
