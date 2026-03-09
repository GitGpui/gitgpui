use super::GixRepo;
use crate::util::path_buf_from_git_bytes;
use gitcomet_core::domain::{
    FileConflictKind, FileStatus, FileStatusKind, RepoStatus, UpstreamDivergence,
};
use gitcomet_core::error::{Error, ErrorKind};
use gitcomet_core::services::Result;
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
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
                        let path = path_buf_from_git_bytes(
                            rela_path.as_ref(),
                            "gix status index/worktree modification path",
                        )?;
                        let (kind, conflict) = map_entry_status(status);
                        unstaged.push(FileStatus {
                            path,
                            kind,
                            conflict,
                        });
                    }
                    gix::status::index_worktree::Item::DirectoryContents { entry, .. } => {
                        let Some(kind) = map_directory_entry_status(entry.status) else {
                            continue;
                        };

                        let path = path_buf_from_git_bytes(
                            entry.rela_path.as_ref(),
                            "gix status directory entry path",
                        )?;
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

                        let path = path_buf_from_git_bytes(
                            dirwalk_entry.rela_path.as_ref(),
                            "gix status rewrite path",
                        )?;
                        unstaged.push(FileStatus {
                            path,
                            kind,
                            conflict: None,
                        });
                    }
                },

                gix::status::Item::TreeIndex(change) => {
                    use gix::diff::index::ChangeRef;

                    let (path, kind) = match change {
                        ChangeRef::Addition { location, .. } => (
                            path_buf_from_git_bytes(
                                location.as_ref(),
                                "gix status staged addition path",
                            )?,
                            FileStatusKind::Added,
                        ),
                        ChangeRef::Deletion { location, .. } => (
                            path_buf_from_git_bytes(
                                location.as_ref(),
                                "gix status staged deletion path",
                            )?,
                            FileStatusKind::Deleted,
                        ),
                        ChangeRef::Modification { location, .. } => (
                            path_buf_from_git_bytes(
                                location.as_ref(),
                                "gix status staged modification path",
                            )?,
                            FileStatusKind::Modified,
                        ),
                        ChangeRef::Rewrite { location, copy, .. } => (
                            path_buf_from_git_bytes(
                                location.as_ref(),
                                "gix status staged rewrite path",
                            )?,
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
        for (path, conflict_kind) in gix_unmerged_conflicts(&repo)? {
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

fn gix_unmerged_conflicts(repo: &gix::Repository) -> Result<Vec<(PathBuf, FileConflictKind)>> {
    let index = repo
        .index_or_load_from_head_or_empty()
        .map_err(|e| Error::new(ErrorKind::Backend(format!("gix index: {e}"))))?;
    let path_backing = index.path_backing();
    let mut stage_entries = Vec::new();

    for entry in index.entries() {
        let stage = entry.stage_raw() as u8;
        if !(1..=3).contains(&stage) {
            continue;
        }

        let path = path_buf_from_git_bytes(
            entry.path_in(path_backing).as_ref(),
            "gix index unmerged conflict path",
        )?;
        stage_entries.push((path, stage));
    }

    Ok(collect_unmerged_conflicts(stage_entries))
}

fn collect_unmerged_conflicts(
    stage_entries: impl IntoIterator<Item = (PathBuf, u8)>,
) -> Vec<(PathBuf, FileConflictKind)> {
    let mut stage_masks: HashMap<PathBuf, u8> = HashMap::default();

    for (path, stage) in stage_entries {
        let Some(shift) = stage.checked_sub(1) else {
            continue;
        };
        if shift > 2 {
            continue;
        }

        let bit = 1u8 << shift;
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

fn map_directory_entry_status(status: gix::dir::entry::Status) -> Option<FileStatusKind> {
    match status {
        // Directory-walk entries represent an unstaged change only when they are
        // genuinely untracked. `Tracked` entries are traversal metadata and must
        // not become synthetic "modified" files.
        gix::dir::entry::Status::Untracked => Some(FileStatusKind::Untracked),
        gix::dir::entry::Status::Ignored(_)
        | gix::dir::entry::Status::Tracked
        | gix::dir::entry::Status::Pruned => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        collect_unmerged_conflicts, conflict_kind_from_stage_mask, map_directory_entry_status,
    };
    use gitcomet_core::domain::{FileConflictKind, FileStatusKind};
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
    fn collect_unmerged_conflicts_groups_stage_entries_by_path() {
        let stages = vec![
            (PathBuf::from("dd.txt"), 1),
            (PathBuf::from("au.txt"), 2),
            (PathBuf::from("ud.txt"), 1),
            (PathBuf::from("ud.txt"), 2),
            (PathBuf::from("ua.txt"), 3),
            (PathBuf::from("du.txt"), 1),
            (PathBuf::from("du.txt"), 3),
            (PathBuf::from("aa.txt"), 2),
            (PathBuf::from("aa.txt"), 3),
            (PathBuf::from("uu.txt"), 1),
            (PathBuf::from("uu.txt"), 2),
            (PathBuf::from("uu.txt"), 3),
        ];

        let parsed = collect_unmerged_conflicts(stages);
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

    #[test]
    fn collect_unmerged_conflicts_ignores_unconflicted_and_unknown_stages() {
        let stages = vec![
            (PathBuf::from("clean.txt"), 0),
            (PathBuf::from("ignored.txt"), 4),
            (PathBuf::from("conflicted.txt"), 2),
            (PathBuf::from("conflicted.txt"), 3),
        ];

        let parsed = collect_unmerged_conflicts(stages);
        assert_eq!(
            parsed,
            vec![(PathBuf::from("conflicted.txt"), FileConflictKind::BothAdded)]
        );
    }

    #[test]
    fn map_directory_entry_status_only_reports_untracked_entries() {
        use gix::dir::entry::Status;

        assert_eq!(
            map_directory_entry_status(Status::Untracked),
            Some(FileStatusKind::Untracked)
        );
        assert_eq!(map_directory_entry_status(Status::Tracked), None);
        assert_eq!(
            map_directory_entry_status(Status::Ignored(gix::ignore::Kind::Expendable)),
            None
        );
        assert_eq!(
            map_directory_entry_status(Status::Ignored(gix::ignore::Kind::Precious)),
            None
        );
        assert_eq!(map_directory_entry_status(Status::Pruned), None);
    }
}
