use gitgpui_core::domain::{
    Branch, Commit, CommitDetails, CommitFileChange, CommitId, DiffArea, DiffTarget, FileStatus,
    FileStatusKind, LogCursor, LogPage, ReflogEntry, Remote, RemoteBranch, RepoSpec, RepoStatus,
};
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::{GitBackend, GitRepository, PullMode, Result};
use gix::bstr::ByteSlice as _;
use gix::traverse::commit::simple::CommitTimeOrder;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::{process::Command, str};

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

    fn log_head_page(&self, limit: usize, cursor: Option<&LogCursor>) -> Result<LogPage> {
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
            let info =
                info.map_err(|e| Error::new(ErrorKind::Backend(format!("gix walk: {e}"))))?;
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

    fn commit_details(&self, id: &CommitId) -> Result<CommitDetails> {
        let sha = id.as_ref();

        let message = {
            let mut cmd = Command::new("git");
            cmd.arg("-C")
                .arg(&self.spec.workdir)
                .arg("show")
                .arg("-s")
                .arg("--format=%B")
                .arg(sha);
            run_git_capture(cmd, "git show --format=%B")?
                .trim_end()
                .to_string()
        };

        let committed_at = {
            let mut cmd = Command::new("git");
            cmd.arg("-C")
                .arg(&self.spec.workdir)
                .arg("show")
                .arg("-s")
                .arg("--format=%cI")
                .arg(sha);
            run_git_capture(cmd, "git show --format=%cI")?
                .trim()
                .to_string()
        };

        let parent_ids = {
            let mut cmd = Command::new("git");
            cmd.arg("-C")
                .arg(&self.spec.workdir)
                .arg("show")
                .arg("-s")
                .arg("--format=%P")
                .arg(sha);
            run_git_capture(cmd, "git show --format=%P")?
                .split_whitespace()
                .map(|p| CommitId(p.to_string()))
                .collect::<Vec<_>>()
        };

        let files = {
            let mut cmd = Command::new("git");
            cmd.arg("-C")
                .arg(&self.spec.workdir)
                .arg("show")
                .arg("--name-status")
                .arg("--pretty=format:")
                .arg(sha);
            let output = run_git_capture(cmd, "git show --name-status")?;
            output
                .lines()
                .filter_map(parse_name_status_line)
                .collect::<Vec<_>>()
        };

        Ok(CommitDetails {
            id: id.clone(),
            message,
            committed_at,
            parent_ids,
            files,
        })
    }

    fn reflog_head(&self, limit: usize) -> Result<Vec<ReflogEntry>> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("-c")
            .arg("color.ui=false")
            .arg("--no-pager")
            .arg("reflog")
            .arg("show")
            .arg("--date=unix")
            .arg(format!("-n{limit}"))
            .arg("--format=%H%x00%gd%x00%gs%x00%ct")
            .arg("HEAD");

        let output = run_git_capture(cmd, "git reflog")?;
        let mut entries = Vec::new();
        for (ix, line) in output.lines().enumerate() {
            let mut parts = line.split('\0');
            let Some(new_id) = parts.next().filter(|s| !s.is_empty()) else {
                continue;
            };
            let selector = parts.next().unwrap_or_default().to_string();
            let message = parts.next().unwrap_or_default().to_string();
            let time = parts
                .next()
                .and_then(|s| s.parse::<i64>().ok())
                .and_then(unix_seconds_to_system_time);

            let index = parse_reflog_index(&selector).unwrap_or(ix);

            entries.push(ReflogEntry {
                index,
                new_id: CommitId(new_id.to_string()),
                message,
                time,
                selector,
            });
        }
        Ok(entries)
    }

    fn current_branch(&self) -> Result<String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.spec.workdir)
            .arg("rev-parse")
            .arg("--abbrev-ref")
            .arg("HEAD")
            .output()
            .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

        if !output.status.success() {
            let stderr = str::from_utf8(&output.stderr).unwrap_or("<non-utf8 stderr>");
            return Err(Error::new(ErrorKind::Backend(format!(
                "git rev-parse failed: {stderr}"
            ))));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
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
            let reference = reference
                .map_err(|e| Error::new(ErrorKind::Backend(format!("gix ref iter: {e}"))))?;
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

    fn list_remote_branches(&self) -> Result<Vec<RemoteBranch>> {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.spec.workdir)
            .arg("for-each-ref")
            .arg("--format=%(refname:strip=2)")
            .arg("refs/remotes")
            .output()
            .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

        if !output.status.success() {
            let stderr = str::from_utf8(&output.stderr).unwrap_or("<non-utf8 stderr>");
            return Err(Error::new(ErrorKind::Backend(format!(
                "git for-each-ref refs/remotes failed: {stderr}"
            ))));
        }

        Ok(parse_remote_branches(&String::from_utf8_lossy(
            &output.stdout,
        )))
    }

    fn status(&self) -> Result<RepoStatus> {
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
                        let kind = map_entry_status(status);
                        unstaged.push(FileStatus { path, kind });
                    }
                    gix::status::index_worktree::Item::DirectoryContents { entry, .. } => {
                        let kind = match entry.status {
                            gix::dir::entry::Status::Untracked => FileStatusKind::Untracked,
                            gix::dir::entry::Status::Ignored(_) => continue,
                            gix::dir::entry::Status::Tracked => FileStatusKind::Modified,
                            gix::dir::entry::Status::Pruned => continue,
                        };

                        let path = PathBuf::from(entry.rela_path.to_str_lossy().into_owned());
                        unstaged.push(FileStatus { path, kind });
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
                        unstaged.push(FileStatus { path, kind });
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

                    staged.push(FileStatus { path, kind });
                }
            }
        }

        staged.sort_by(|a, b| a.path.cmp(&b.path));
        unstaged.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(RepoStatus { staged, unstaged })
    }

    fn diff_unified(&self, target: &DiffTarget) -> Result<String> {
        match target {
            DiffTarget::WorkingTree { path, area } => {
                let mut cmd = Command::new("git");
                cmd.arg("-C")
                    .arg(&self.spec.workdir)
                    .arg("-c")
                    .arg("color.ui=false")
                    .arg("--no-pager")
                    .arg("diff")
                    .arg("--no-ext-diff");

                if matches!(area, DiffArea::Staged) {
                    cmd.arg("--cached");
                }

                cmd.arg("--").arg(path);

                let output = cmd
                    .output()
                    .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

                let ok_exit = output.status.success() || output.status.code() == Some(1);
                if !ok_exit {
                    let stderr = str::from_utf8(&output.stderr).unwrap_or("<non-utf8 stderr>");
                    return Err(Error::new(ErrorKind::Backend(format!(
                        "git diff failed: {stderr}"
                    ))));
                }

                Ok(String::from_utf8_lossy(&output.stdout).into_owned())
            }
            DiffTarget::Commit { commit_id, path } => {
                let mut cmd = Command::new("git");
                cmd.arg("-C")
                    .arg(&self.spec.workdir)
                    .arg("-c")
                    .arg("color.ui=false")
                    .arg("--no-pager")
                    .arg("show")
                    .arg("--no-ext-diff")
                    .arg("--pretty=format:")
                    .arg(commit_id.as_ref());

                if let Some(path) = path {
                    cmd.arg("--").arg(path);
                }

                run_git_capture(cmd, "git show --pretty=format:")
            }
        }
    }

    fn create_branch(&self, _name: &str, _target: &gitgpui_core::domain::CommitId) -> Result<()> {
        let _ = _target;
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("branch")
            .arg(_name);
        run_git_simple(cmd, "git branch")
    }

    fn delete_branch(&self, _name: &str) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported(
            "gix backend skeleton: delete_branch not implemented yet",
        )))
    }

    fn checkout_branch(&self, _name: &str) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("checkout")
            .arg(_name);
        run_git_simple(cmd, "git checkout")
    }

    fn checkout_commit(&self, id: &CommitId) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("checkout")
            .arg(id.as_ref());
        run_git_simple(cmd, "git checkout <commit>")
    }

    fn cherry_pick(&self, id: &CommitId) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("cherry-pick")
            .arg(id.as_ref());
        run_git_simple(cmd, "git cherry-pick")
    }

    fn revert(&self, id: &CommitId) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("revert")
            .arg("--no-edit")
            .arg(id.as_ref());
        run_git_simple(cmd, "git revert")
    }

    fn stash_create(&self, _message: &str, _include_untracked: bool) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("stash")
            .arg("push");
        if _include_untracked {
            cmd.arg("-u");
        }
        if !_message.is_empty() {
            cmd.arg("-m").arg(_message);
        }
        run_git_simple(cmd, "git stash push")
    }

    fn stash_list(&self) -> Result<Vec<gitgpui_core::domain::StashEntry>> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("-c")
            .arg("color.ui=false")
            .arg("--no-pager")
            .arg("stash")
            .arg("list")
            .arg("--date=unix")
            .arg("--format=%gd%x00%ct%x00%gs");

        let output = run_git_capture(cmd, "git stash list")?;
        let mut entries = Vec::new();
        for (ix, line) in output.lines().enumerate() {
            let mut parts = line.split('\0');
            let Some(selector) = parts.next().filter(|s| !s.is_empty()) else {
                continue;
            };
            let created_at = parts
                .next()
                .and_then(|s| s.parse::<i64>().ok())
                .and_then(unix_seconds_to_system_time);
            let message = parts.next().unwrap_or_default().to_string();
            let index = parse_reflog_index(selector).unwrap_or(ix);
            entries.push(gitgpui_core::domain::StashEntry {
                index,
                message,
                created_at,
            });
        }
        Ok(entries)
    }

    fn stash_apply(&self, _index: usize) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("stash")
            .arg("apply")
            .arg(format!("stash@{{{_index}}}"));
        run_git_simple(cmd, "git stash apply")
    }

    fn stash_drop(&self, _index: usize) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("stash")
            .arg("drop")
            .arg(format!("stash@{{{_index}}}"));
        run_git_simple(cmd, "git stash drop")
    }

    fn stage(&self, paths: &[&Path]) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.spec.workdir).arg("add").arg("--");
        for path in paths {
            cmd.arg(path);
        }
        run_git_simple(cmd, "git add")
    }

    fn unstage(&self, paths: &[&Path]) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("restore")
            .arg("--staged")
            .arg("--");
        for path in paths {
            cmd.arg(path);
        }
        run_git_simple(cmd, "git restore --staged")
    }

    fn commit(&self, message: &str) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("commit")
            .arg("-m")
            .arg(message);
        run_git_simple(cmd, "git commit")
    }

    fn fetch_all(&self) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("fetch")
            .arg("--all");
        run_git_simple(cmd, "git fetch --all")
    }

    fn pull(&self, mode: PullMode) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.spec.workdir).arg("pull");
        match mode {
            PullMode::Default => {}
            PullMode::FastForwardIfPossible => {
                cmd.arg("--ff");
            }
            PullMode::FastForwardOnly => {
                cmd.arg("--ff-only");
            }
            PullMode::Rebase => {
                cmd.arg("--rebase");
            }
        }
        run_git_simple(cmd, "git pull")
    }

    fn push(&self) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.spec.workdir).arg("push");
        run_git_simple(cmd, "git push")
    }

    fn discard_worktree_changes(&self, _paths: &[&Path]) -> Result<()> {
        Err(Error::new(ErrorKind::Unsupported(
            "gix backend skeleton: discard not implemented yet",
        )))
    }
}

fn run_git_simple(mut cmd: Command, label: &str) -> Result<()> {
    let output = cmd
        .output()
        .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

    let ok_exit = output.status.success() || output.status.code() == Some(1);
    if !ok_exit {
        let stderr = str::from_utf8(&output.stderr).unwrap_or("<non-utf8 stderr>");
        return Err(Error::new(ErrorKind::Backend(format!(
            "{label} failed: {stderr}"
        ))));
    }

    Ok(())
}

fn run_git_capture(mut cmd: Command, label: &str) -> Result<String> {
    let output = cmd
        .output()
        .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

    if !output.status.success() {
        let stderr = str::from_utf8(&output.stderr).unwrap_or("<non-utf8 stderr>");
        return Err(Error::new(ErrorKind::Backend(format!(
            "{label} failed: {stderr}"
        ))));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_name_status_line(line: &str) -> Option<CommitFileChange> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let mut parts = line.split('\t');
    let status = parts.next()?.trim();
    if status.is_empty() {
        return None;
    }

    let status_kind = status.chars().next()?;
    let kind = match status_kind {
        'A' => FileStatusKind::Added,
        'M' => FileStatusKind::Modified,
        'D' => FileStatusKind::Deleted,
        'R' => FileStatusKind::Renamed,
        'C' => FileStatusKind::Added,
        _ => FileStatusKind::Modified,
    };

    let path = match status_kind {
        'R' | 'C' => {
            let _old = parts.next()?;
            parts.next().unwrap_or_default()
        }
        _ => parts.next().unwrap_or_default(),
    };
    let path = path.trim();
    if path.is_empty() {
        return None;
    }

    Some(CommitFileChange {
        path: PathBuf::from(path),
        kind,
    })
}

fn unix_seconds_to_system_time(seconds: i64) -> Option<SystemTime> {
    if seconds >= 0 {
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds as u64))
    } else {
        None
    }
}

fn parse_reflog_index(selector: &str) -> Option<usize> {
    let start = selector.rfind("@{")? + 2;
    let end = selector[start..].find('}')? + start;
    selector[start..end].parse::<usize>().ok()
}

fn parse_remote_branches(output: &str) -> Vec<RemoteBranch> {
    let mut branches = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line.ends_with("/HEAD") {
            continue;
        }
        let Some((remote, name)) = line.split_once('/') else {
            continue;
        };
        branches.push(RemoteBranch {
            remote: remote.to_string(),
            name: name.to_string(),
        });
    }
    branches.sort_by(|a, b| a.remote.cmp(&b.remote).then_with(|| a.name.cmp(&b.name)));
    branches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_remote_branches_splits_and_skips_head() {
        let output = "origin/HEAD\norigin/main\nupstream/feature/foo\n\n";
        let branches = parse_remote_branches(output);
        assert_eq!(
            branches,
            vec![
                RemoteBranch {
                    remote: "origin".to_string(),
                    name: "main".to_string()
                },
                RemoteBranch {
                    remote: "upstream".to_string(),
                    name: "feature/foo".to_string()
                },
            ]
        );
    }
}

fn map_entry_status<T, U>(
    status: gix::status::plumbing::index_as_worktree::EntryStatus<T, U>,
) -> FileStatusKind {
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
