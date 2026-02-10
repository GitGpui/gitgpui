use gitgpui_core::domain::{
    Branch, Commit, CommitDetails, CommitFileChange, CommitId, DiffArea, DiffTarget, FileDiffText,
    FileStatus, FileStatusKind, LogCursor, LogPage, ReflogEntry, Remote, RemoteBranch, RepoSpec,
    RepoStatus, Submodule, Tag, Upstream, UpstreamDivergence, Worktree,
};
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::{
    BlameLine, CommandOutput, ConflictSide, GitBackend, GitRepository, PullMode, RemoteUrlKind,
    ResetMode, Result,
};
use gix::bstr::ByteSlice as _;
use gix::traverse::commit::simple::CommitTimeOrder;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::{process::Command, str};

struct CursorGate<'a> {
    cursor: Option<&'a LogCursor>,
    started: bool,
}

impl<'a> CursorGate<'a> {
    fn new(cursor: Option<&'a LogCursor>) -> Self {
        Self {
            cursor,
            started: cursor.is_none(),
        }
    }

    fn should_skip(&mut self, id: &str) -> bool {
        if self.started {
            return false;
        }

        let Some(cursor) = self.cursor else {
            self.started = true;
            return false;
        };

        if cursor.last_seen.0 == id {
            self.started = true;
        }

        true
    }
}

fn commit_from_walk_info<'repo>(
    info: &gix::revision::walk::Info<'repo>,
    id: String,
) -> Result<Commit> {
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
    let time = unix_seconds_to_system_time_or_epoch(seconds);

    let parent_ids = info
        .parent_ids()
        .map(|parent_id| CommitId(parent_id.detach().to_string()))
        .collect::<Vec<_>>();

    Ok(Commit {
        id: CommitId(id),
        parent_ids,
        summary,
        author,
        time,
    })
}

fn log_page_from_walk<'repo, E>(
    walk: impl Iterator<Item = std::result::Result<gix::revision::walk::Info<'repo>, E>>,
    limit: usize,
    cursor: Option<&LogCursor>,
) -> Result<LogPage>
where
    E: std::fmt::Display,
{
    let mut cursor_gate = CursorGate::new(cursor);
    let mut commits = Vec::with_capacity(limit.min(2048));
    let mut next_cursor: Option<LogCursor> = None;

    for info in walk {
        let info = info.map_err(|e| Error::new(ErrorKind::Backend(format!("gix walk: {e}"))))?;
        let id = info.id().detach().to_string();

        if cursor_gate.should_skip(&id) {
            continue;
        }

        commits.push(commit_from_walk_info(&info, id)?);

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

        let repo = gix::open(&workdir).map_err(|e| match e {
            gix::open::Error::NotARepository { .. } => Error::new(ErrorKind::NotARepository),
            gix::open::Error::Io(io) => Error::new(ErrorKind::Io(io.kind())),
            e => Error::new(ErrorKind::Backend(format!("gix open: {e}"))),
        })?;

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

        let walk = repo
            .rev_walk([head_id])
            .sorting(gix::revision::walk::Sorting::ByCommitTime(
                CommitTimeOrder::NewestFirst,
            ))
            .first_parent_only()
            .all()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix rev_walk: {e}"))))?;
        log_page_from_walk(walk, limit, cursor)
    }

    fn log_all_branches_page(&self, limit: usize, cursor: Option<&LogCursor>) -> Result<LogPage> {
        let repo = self._repo.to_thread_local();
        let head_id = repo
            .head_id()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix head_id: {e}"))))?
            .detach();

        let refs = repo
            .references()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix references: {e}"))))?;

        // Emulate `git log --all`: include all refs under `refs/`, not just `refs/heads` and
        // `refs/remotes`. Some repositories (e.g. Chromium) use additional namespaces like
        // `refs/branch-heads/*`.
        let mut tips = Vec::new();
        let mut seen = HashSet::new();
        tips.push(head_id);
        seen.insert(head_id);

        let iter = refs
            .all()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix references(all): {e}"))))?
            .peeled()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix peel refs: {e}"))))?;
        for reference in iter {
            let reference = reference
                .map_err(|e| Error::new(ErrorKind::Backend(format!("gix ref iter: {e}"))))?;
            if matches!(
                reference.name().category(),
                Some(gix::reference::Category::Tag)
            ) {
                continue;
            }
            let id = reference.id().detach();
            if seen.insert(id) {
                tips.push(id);
            }
        }

        let walk = repo
            .rev_walk(tips)
            .sorting(gix::revision::walk::Sorting::ByCommitTime(
                CommitTimeOrder::NewestFirst,
            ))
            .all()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix rev_walk: {e}"))))?;
        log_page_from_walk(walk, limit, cursor)
    }

    fn log_file_page(
        &self,
        path: &Path,
        limit: usize,
        _cursor: Option<&LogCursor>,
    ) -> Result<LogPage> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("log")
            .arg("--follow")
            .arg(format!("-n{limit}"))
            .arg("--date=unix")
            .arg("--pretty=format:%H%x1f%P%x1f%an%x1f%ct%x1f%s%x1e")
            .arg("--")
            .arg(path);

        let output = run_git_capture(cmd, "git log --follow")?;
        Ok(parse_git_log_pretty_records(&output))
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
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("rev-parse")
            .arg("--abbrev-ref")
            .arg("HEAD");
        Ok(run_git_capture(cmd, "git rev-parse --abbrev-ref HEAD")?
            .trim()
            .to_string())
    }

    fn list_branches(&self) -> Result<Vec<Branch>> {
        fn parse_upstream_short(s: &str) -> Option<Upstream> {
            let s = s.trim();
            if s.is_empty() {
                return None;
            }
            let (remote, branch) = s.split_once('/')?;
            Some(Upstream {
                remote: remote.to_string(),
                branch: branch.to_string(),
            })
        }

        fn parse_upstream_track(s: &str) -> Option<UpstreamDivergence> {
            let s = s.trim();
            if s.is_empty() {
                return None;
            }
            let s = s.trim_start_matches('[').trim_end_matches(']');
            if s.trim().is_empty() || s.contains("gone") {
                return None;
            }

            let mut ahead: Option<usize> = None;
            let mut behind: Option<usize> = None;

            for part in s.split(',') {
                let mut it = part.trim().split_whitespace();
                let Some(kind) = it.next() else { continue };
                let Some(n) = it.next().and_then(|x| x.parse::<usize>().ok()) else {
                    continue;
                };
                match kind {
                    "ahead" => ahead = Some(n),
                    "behind" => behind = Some(n),
                    _ => {}
                }
            }

            let ahead = ahead.unwrap_or(0);
            let behind = behind.unwrap_or(0);
            Some(UpstreamDivergence { ahead, behind })
        }

        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("for-each-ref")
            .arg("--format=%(refname:short)\t%(objectname)\t%(upstream:short)\t%(upstream:track)")
            .arg("refs/heads");
        let stdout = run_git_capture(cmd, "git for-each-ref refs/heads")?;

        let mut branches = Vec::new();
        for line in stdout.lines() {
            let mut parts = line.split('\t');
            let Some(name) = parts.next().map(str::trim).filter(|s| !s.is_empty()) else {
                continue;
            };
            let Some(sha) = parts.next().map(str::trim).filter(|s| !s.is_empty()) else {
                continue;
            };
            let upstream_short = parts.next().unwrap_or("").trim();
            let track = parts.next().unwrap_or("").trim();

            let upstream = parse_upstream_short(upstream_short);
            let divergence = upstream.as_ref().and_then(|_| parse_upstream_track(track));

            branches.push(Branch {
                name: name.to_string(),
                target: CommitId(sha.to_string()),
                upstream,
                divergence,
            });
        }

        branches.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(branches)
    }

    fn list_tags(&self) -> Result<Vec<Tag>> {
        let repo = self._repo.to_thread_local();

        let refs = repo
            .references()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix references: {e}"))))?;

        let iter = refs
            .tags()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix tags: {e}"))))?
            .peeled()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix peel refs: {e}"))))?;

        let mut tags = Vec::new();
        for reference in iter {
            let reference = reference
                .map_err(|e| Error::new(ErrorKind::Backend(format!("gix ref iter: {e}"))))?;
            let name = reference.name().shorten().to_str_lossy().into_owned();
            let target = CommitId(reference.id().detach().to_string());
            tags.push(Tag { name, target });
        }

        tags.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(tags)
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
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("for-each-ref")
            .arg("--format=%(refname:strip=2)")
            .arg("refs/remotes");
        let output = run_git_capture(cmd, "git for-each-ref refs/remotes")?;
        Ok(parse_remote_branches(&output))
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

        Ok(RepoStatus { staged, unstaged })
    }

    fn upstream_divergence(&self) -> Result<Option<UpstreamDivergence>> {
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

    fn pull_branch_with_output(&self, remote: &str, branch: &str) -> Result<CommandOutput> {
        let command_str = format!("git pull --no-rebase {remote} {branch}");
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("-c")
            .arg("color.ui=false")
            .arg("--no-pager")
            .arg("pull")
            .arg("--no-rebase")
            .arg(remote)
            .arg(branch);
        run_git_with_output(cmd, &command_str)
    }

    fn merge_ref_with_output(&self, reference: &str) -> Result<CommandOutput> {
        let command_str = format!("git merge --no-edit {reference}");
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("-c")
            .arg("color.ui=false")
            .arg("--no-pager")
            .arg("merge")
            .arg("--no-edit")
            .arg(reference);
        run_git_with_output(cmd, &command_str)
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

    fn diff_file_text(&self, target: &DiffTarget) -> Result<Option<FileDiffText>> {
        match target {
            DiffTarget::WorkingTree { path, area } => {
                let path_str = path.to_string_lossy();
                let (old, new) = match area {
                    DiffArea::Unstaged => {
                        let old = match git_show_path_utf8_optional(
                            &self.spec.workdir,
                            ":",
                            path_str.as_ref(),
                        ) {
                            Ok(old) => old,
                            Err(e) if matches!(e.kind(), ErrorKind::Backend(s) if git_show_unmerged_stage0(s)) =>
                            {
                                let ours = git_show_path_utf8_optional_unmerged_stage(
                                    &self.spec.workdir,
                                    ":2:",
                                    path_str.as_ref(),
                                    2,
                                )?;
                                let theirs = git_show_path_utf8_optional_unmerged_stage(
                                    &self.spec.workdir,
                                    ":3:",
                                    path_str.as_ref(),
                                    3,
                                )?;
                                return Ok(Some(FileDiffText {
                                    path: path.clone(),
                                    old: ours,
                                    new: theirs,
                                }));
                            }
                            Err(e) => return Err(e),
                        };
                        let new = read_worktree_file_utf8_optional(&self.spec.workdir, path)?;
                        (old, new)
                    }
                    DiffArea::Staged => {
                        let old = git_show_path_utf8_optional(
                            &self.spec.workdir,
                            "HEAD:",
                            path_str.as_ref(),
                        )?;
                        let new = match git_show_path_utf8_optional(
                            &self.spec.workdir,
                            ":",
                            path_str.as_ref(),
                        ) {
                            Ok(new) => new,
                            Err(e) if matches!(e.kind(), ErrorKind::Backend(s) if git_show_unmerged_stage0(s)) => {
                                git_show_path_utf8_optional_unmerged_stage(
                                    &self.spec.workdir,
                                    ":2:",
                                    path_str.as_ref(),
                                    2,
                                )?
                                .or_else(|| {
                                    git_show_path_utf8_optional_unmerged_stage(
                                        &self.spec.workdir,
                                        ":3:",
                                        path_str.as_ref(),
                                        3,
                                    )
                                    .ok()
                                    .flatten()
                                })
                            }
                            Err(e) => return Err(e),
                        };
                        (old, new)
                    }
                };

                Ok(Some(FileDiffText {
                    path: path.clone(),
                    old,
                    new,
                }))
            }
            DiffTarget::Commit { commit_id, path } => {
                let Some(path) = path else {
                    return Ok(None);
                };

                let path_str = path.to_string_lossy();
                let parent = git_first_parent_optional(&self.spec.workdir, commit_id.as_ref())?;

                let old = match parent {
                    Some(parent) => {
                        let spec = format!("{parent}:");
                        git_show_path_utf8_optional(&self.spec.workdir, &spec, path_str.as_ref())?
                    }
                    None => None,
                };
                let new = {
                    let spec = format!("{}:", commit_id.as_ref());
                    git_show_path_utf8_optional(&self.spec.workdir, &spec, path_str.as_ref())?
                };

                Ok(Some(FileDiffText {
                    path: path.clone(),
                    old,
                    new,
                }))
            }
        }
    }

    fn diff_file_image(
        &self,
        target: &DiffTarget,
    ) -> Result<Option<gitgpui_core::domain::FileDiffImage>> {
        use gitgpui_core::domain::FileDiffImage;

        match target {
            DiffTarget::WorkingTree { path, area } => {
                let path_str = path.to_string_lossy();
                let (old, new) = match area {
                    DiffArea::Unstaged => {
                        let old = match git_show_path_bytes_optional(
                            &self.spec.workdir,
                            ":",
                            path_str.as_ref(),
                        ) {
                            Ok(old) => old,
                            Err(e) if matches!(e.kind(), ErrorKind::Backend(s) if git_show_unmerged_stage0(s)) =>
                            {
                                let ours = git_show_path_bytes_optional_unmerged_stage(
                                    &self.spec.workdir,
                                    ":2:",
                                    path_str.as_ref(),
                                    2,
                                )?;
                                let theirs = git_show_path_bytes_optional_unmerged_stage(
                                    &self.spec.workdir,
                                    ":3:",
                                    path_str.as_ref(),
                                    3,
                                )?;
                                return Ok(Some(FileDiffImage {
                                    path: path.clone(),
                                    old: ours,
                                    new: theirs,
                                }));
                            }
                            Err(e) => return Err(e),
                        };
                        let new = read_worktree_file_bytes_optional(&self.spec.workdir, path)?;
                        (old, new)
                    }
                    DiffArea::Staged => {
                        let old = git_show_path_bytes_optional(
                            &self.spec.workdir,
                            "HEAD:",
                            path_str.as_ref(),
                        )?;
                        let new = match git_show_path_bytes_optional(
                            &self.spec.workdir,
                            ":",
                            path_str.as_ref(),
                        ) {
                            Ok(new) => new,
                            Err(e) if matches!(e.kind(), ErrorKind::Backend(s) if git_show_unmerged_stage0(s)) => {
                                git_show_path_bytes_optional_unmerged_stage(
                                    &self.spec.workdir,
                                    ":2:",
                                    path_str.as_ref(),
                                    2,
                                )?
                                .or_else(|| {
                                    git_show_path_bytes_optional_unmerged_stage(
                                        &self.spec.workdir,
                                        ":3:",
                                        path_str.as_ref(),
                                        3,
                                    )
                                    .ok()
                                    .flatten()
                                })
                            }
                            Err(e) => return Err(e),
                        };
                        (old, new)
                    }
                };

                Ok(Some(FileDiffImage {
                    path: path.clone(),
                    old,
                    new,
                }))
            }
            DiffTarget::Commit { commit_id, path } => {
                let Some(path) = path else {
                    return Ok(None);
                };

                let path_str = path.to_string_lossy();
                let parent = git_first_parent_optional(&self.spec.workdir, commit_id.as_ref())?;

                let old = match parent {
                    Some(parent) => {
                        let spec = format!("{parent}:");
                        git_show_path_bytes_optional(&self.spec.workdir, &spec, path_str.as_ref())?
                    }
                    None => None,
                };
                let new = {
                    let spec = format!("{}:", commit_id.as_ref());
                    git_show_path_bytes_optional(&self.spec.workdir, &spec, path_str.as_ref())?
                };

                Ok(Some(FileDiffImage {
                    path: path.clone(),
                    old,
                    new,
                }))
            }
        }
    }

    fn create_branch(&self, _name: &str, _target: &gitgpui_core::domain::CommitId) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("branch")
            .arg(_name)
            .arg(_target.as_ref());
        run_git_simple(cmd, "git branch")
    }

    fn delete_branch(&self, _name: &str) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("branch")
            .arg("-d")
            .arg(_name);
        run_git_simple(cmd, "git branch -d")
    }

    fn checkout_branch(&self, _name: &str) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("checkout")
            .arg(_name);
        run_git_simple(cmd, "git checkout")
    }

    fn checkout_remote_branch(&self, remote: &str, branch: &str) -> Result<()> {
        let upstream = format!("{remote}/{branch}");

        let output = Command::new("git")
            .arg("-C")
            .arg(&self.spec.workdir)
            .arg("checkout")
            .arg("--track")
            .arg("-b")
            .arg(branch)
            .arg(&upstream)
            .output()
            .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let already_exists =
            stderr.contains("already exists") || stderr.contains("fatal: a branch named");

        if !already_exists {
            return Err(Error::new(ErrorKind::Backend(format!(
                "git checkout --track failed: {}",
                stderr.trim()
            ))));
        }

        // If the local branch already exists, check it out and update its upstream.
        let mut checkout = Command::new("git");
        checkout
            .arg("-C")
            .arg(&self.spec.workdir)
            .arg("checkout")
            .arg(branch);
        run_git_simple(checkout, "git checkout")?;

        let mut set_upstream = Command::new("git");
        set_upstream
            .arg("-C")
            .arg(&self.spec.workdir)
            .arg("branch")
            .arg(format!("--set-upstream-to={upstream}"))
            .arg(branch);
        run_git_simple(set_upstream, "git branch --set-upstream-to")
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
            .arg("--format=%gd%x00%H%x00%ct%x00%gs");

        let output = run_git_capture(cmd, "git stash list")?;
        let mut entries = Vec::new();
        for (ix, line) in output.lines().enumerate() {
            let mut parts = line.split('\0');
            let Some(selector) = parts.next().filter(|s| !s.is_empty()) else {
                continue;
            };
            let Some(id) = parts.next().filter(|s| !s.is_empty()) else {
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
                id: CommitId(id.to_string()),
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
        cmd.arg("-C").arg(&self.spec.workdir).arg("add").arg("-A");
        if !paths.is_empty() {
            cmd.arg("--");
            for path in paths {
                cmd.arg(path);
            }
        }
        run_git_simple(cmd, "git add")
    }

    fn unstage(&self, paths: &[&Path]) -> Result<()> {
        if paths.is_empty() {
            let head = Command::new("git")
                .arg("-C")
                .arg(&self.spec.workdir)
                .arg("rev-parse")
                .arg("--verify")
                .arg("HEAD")
                .output()
                .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

            if head.status.success() {
                let mut cmd = Command::new("git");
                cmd.arg("-C").arg(&self.spec.workdir).arg("reset");
                return run_git_simple(cmd, "git reset");
            }

            let mut cmd = Command::new("git");
            cmd.arg("-C")
                .arg(&self.spec.workdir)
                .arg("rm")
                .arg("--cached")
                .arg("-r")
                .arg("--")
                .arg(".");
            return run_git_simple(cmd, "git rm --cached -r");
        }

        let head = Command::new("git")
            .arg("-C")
            .arg(&self.spec.workdir)
            .arg("rev-parse")
            .arg("--verify")
            .arg("HEAD")
            .output()
            .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.spec.workdir);
        if head.status.success() {
            cmd.arg("reset").arg("HEAD").arg("--");
        } else {
            cmd.arg("rm").arg("--cached").arg("--");
        }
        for path in paths {
            cmd.arg(path);
        }

        if head.status.success() {
            run_git_simple(cmd, "git reset HEAD")
        } else {
            run_git_simple(cmd, "git rm --cached")
        }
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

    fn commit_amend(&self, message: &str) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("commit")
            .arg("--amend")
            .arg("-m")
            .arg(message);
        run_git_simple(cmd, "git commit --amend")
    }

    fn fetch_all(&self) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("fetch")
            .arg("--all");
        run_git_simple(cmd, "git fetch --all")
    }

    fn fetch_all_with_output(&self) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("fetch")
            .arg("--all");
        run_git_with_output(cmd, "git fetch --all")
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

    fn pull_with_output(&self, mode: PullMode) -> Result<CommandOutput> {
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
        run_git_with_output(cmd, "git pull")
    }

    fn push(&self) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.spec.workdir).arg("push");
        run_git_simple(cmd, "git push")
    }

    fn push_with_output(&self) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.spec.workdir).arg("push");
        run_git_with_output(cmd, "git push")
    }

    fn push_force(&self) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("push")
            .arg("--force-with-lease");
        run_git_simple(cmd, "git push --force-with-lease")
    }

    fn push_force_with_output(&self) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("push")
            .arg("--force-with-lease");
        run_git_with_output(cmd, "git push --force-with-lease")
    }

    fn reset_with_output(&self, target: &str, mode: ResetMode) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.spec.workdir).arg("reset");
        let mode_flag = match mode {
            ResetMode::Soft => "--soft",
            ResetMode::Mixed => "--mixed",
            ResetMode::Hard => "--hard",
        };
        cmd.arg(mode_flag).arg(target);
        let label = format!("git reset {mode_flag} {target}");
        run_git_with_output(cmd, &label)
    }

    fn rebase_with_output(&self, onto: &str) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("rebase")
            .arg(onto);
        run_git_with_output(cmd, &format!("git rebase {onto}"))
    }

    fn rebase_continue_with_output(&self) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("rebase")
            .arg("--continue");
        run_git_with_output(cmd, "git rebase --continue")
    }

    fn rebase_abort_with_output(&self) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("rebase")
            .arg("--abort");
        run_git_with_output(cmd, "git rebase --abort")
    }

    fn rebase_in_progress(&self) -> Result<bool> {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.spec.workdir)
            .arg("rev-parse")
            .arg("--verify")
            .arg("REBASE_HEAD")
            .output()
            .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;
        Ok(output.status.success())
    }

    fn create_tag_with_output(&self, name: &str, target: &str) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("-c")
            .arg("alias.tag=")
            .arg("-c")
            .arg("tag.gpgsign=false")
            .arg("-c")
            .arg("tag.forcesignannotated=false")
            .arg("tag")
            .arg("-m")
            .arg(name)
            .arg(name)
            .arg(target);
        run_git_with_output(cmd, &format!("git tag -m {name} {name} {target}"))
    }

    fn delete_tag_with_output(&self, name: &str) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("-c")
            .arg("alias.tag=")
            .arg("tag")
            .arg("-d")
            .arg(name);
        run_git_with_output(cmd, &format!("git tag -d {name}"))
    }

    fn add_remote_with_output(&self, name: &str, url: &str) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("remote")
            .arg("add")
            .arg(name)
            .arg(url);
        run_git_with_output(cmd, &format!("git remote add {name} {url}"))
    }

    fn remove_remote_with_output(&self, name: &str) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("remote")
            .arg("remove")
            .arg(name);
        run_git_with_output(cmd, &format!("git remote remove {name}"))
    }

    fn set_remote_url_with_output(
        &self,
        name: &str,
        url: &str,
        kind: RemoteUrlKind,
    ) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("remote")
            .arg("set-url");
        match kind {
            RemoteUrlKind::Fetch => {}
            RemoteUrlKind::Push => {
                cmd.arg("--push");
            }
        }
        cmd.arg(name).arg(url);
        let label = match kind {
            RemoteUrlKind::Fetch => format!("git remote set-url {name} {url}"),
            RemoteUrlKind::Push => format!("git remote set-url --push {name} {url}"),
        };
        run_git_with_output(cmd, &label)
    }

    fn push_set_upstream(&self, remote: &str, branch: &str) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("push")
            .arg("--set-upstream")
            .arg(remote)
            .arg(format!("HEAD:refs/heads/{branch}"));
        run_git_simple(
            cmd,
            &format!("git push --set-upstream {remote} HEAD:refs/heads/{branch}"),
        )
    }

    fn push_set_upstream_with_output(&self, remote: &str, branch: &str) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("push")
            .arg("--set-upstream")
            .arg(remote)
            .arg(format!("HEAD:refs/heads/{branch}"));
        run_git_with_output(
            cmd,
            &format!("git push --set-upstream {remote} HEAD:refs/heads/{branch}"),
        )
    }

    fn blame_file(&self, path: &Path, rev: Option<&str>) -> Result<Vec<BlameLine>> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("blame")
            .arg("--line-porcelain");
        if let Some(rev) = rev {
            cmd.arg(rev);
        }
        cmd.arg("--").arg(path);

        let output = run_git_capture(cmd, "git blame --line-porcelain")?;
        Ok(parse_git_blame_porcelain(&output))
    }

    fn checkout_conflict_side(&self, path: &Path, side: ConflictSide) -> Result<CommandOutput> {
        let desired_stage = match side {
            ConflictSide::Ours => 2,
            ConflictSide::Theirs => 3,
        };

        let mut ls = Command::new("git");
        ls.arg("-C")
            .arg(&self.spec.workdir)
            .arg("ls-files")
            .arg("-u")
            .arg("--")
            .arg(path);
        let output = ls
            .output()
            .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::new(ErrorKind::Backend(format!(
                "git ls-files -u failed: {}",
                stderr.trim()
            ))));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stage_exists = stdout.lines().any(|line| {
            let mut parts = line.split_whitespace();
            let _mode = parts.next();
            let _sha = parts.next();
            let stage = parts.next().and_then(|s| s.parse::<u8>().ok());
            stage == Some(desired_stage)
        });

        if !stage_exists {
            let mut rm = Command::new("git");
            rm.arg("-C")
                .arg(&self.spec.workdir)
                .arg("rm")
                .arg("--")
                .arg(path);
            return run_git_with_output(rm, "git rm --");
        }

        let mut checkout = Command::new("git");
        checkout.arg("-C").arg(&self.spec.workdir).arg("checkout");
        match side {
            ConflictSide::Ours => {
                checkout.arg("--ours");
            }
            ConflictSide::Theirs => {
                checkout.arg("--theirs");
            }
        }
        checkout.arg("--").arg(path);
        let checkout_out = run_git_with_output(checkout, "git checkout --ours/--theirs")?;

        let mut add = Command::new("git");
        add.arg("-C")
            .arg(&self.spec.workdir)
            .arg("add")
            .arg("--")
            .arg(path);
        let add_out = run_git_with_output(add, "git add --")?;

        Ok(CommandOutput {
            command: checkout_out.command,
            stdout: [checkout_out.stdout, add_out.stdout]
                .into_iter()
                .filter(|s| !s.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n"),
            stderr: [checkout_out.stderr, add_out.stderr]
                .into_iter()
                .filter(|s| !s.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n"),
            exit_code: add_out.exit_code.or(checkout_out.exit_code),
        })
    }

    fn export_patch_with_output(&self, commit_id: &CommitId, dest: &Path) -> Result<CommandOutput> {
        let sha = commit_id.as_ref();
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("format-patch")
            .arg("-1")
            .arg(sha)
            .arg("--stdout")
            .arg("--binary");
        let patch = run_git_capture(cmd, &format!("git format-patch -1 {sha} --stdout"))?;
        std::fs::write(dest, patch.as_bytes()).map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;
        Ok(CommandOutput {
            command: format!("Export patch {sha}"),
            stdout: format!("Saved patch to {}", dest.display()),
            stderr: String::new(),
            exit_code: Some(0),
        })
    }

    fn apply_patch_with_output(&self, patch: &Path) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("am")
            .arg("--3way")
            .arg("--")
            .arg(patch);
        run_git_with_output(cmd, &format!("git am --3way {}", patch.display()))
    }

    fn apply_unified_patch_to_index_with_output(
        &self,
        patch: &str,
        reverse: bool,
    ) -> Result<CommandOutput> {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let tmp_path = std::env::temp_dir().join(format!(
            "gitgpui-index-patch-{}-{nanos}.patch",
            std::process::id()
        ));
        std::fs::write(&tmp_path, patch.as_bytes())
            .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("apply")
            .arg("--cached")
            .arg("--recount")
            .arg("--whitespace=nowarn");
        if reverse {
            cmd.arg("--reverse");
        }
        cmd.arg(&tmp_path);

        let label = if reverse {
            format!("git apply --cached --reverse {}", tmp_path.display())
        } else {
            format!("git apply --cached {}", tmp_path.display())
        };

        let result = run_git_with_output(cmd, &label);
        let _ = std::fs::remove_file(&tmp_path);
        result
    }

    fn list_worktrees(&self) -> Result<Vec<Worktree>> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("worktree")
            .arg("list")
            .arg("--porcelain");
        let output = run_git_capture(cmd, "git worktree list --porcelain")?;
        Ok(parse_git_worktree_list_porcelain(&output))
    }

    fn add_worktree_with_output(
        &self,
        path: &Path,
        reference: Option<&str>,
    ) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("worktree")
            .arg("add")
            .arg(path);
        let label = if let Some(reference) = reference {
            cmd.arg(reference);
            format!("git worktree add {} {}", path.display(), reference)
        } else {
            format!("git worktree add {}", path.display())
        };
        run_git_with_output(cmd, &label)
    }

    fn remove_worktree_with_output(&self, path: &Path) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("worktree")
            .arg("remove")
            .arg(path);
        run_git_with_output(cmd, &format!("git worktree remove {}", path.display()))
    }

    fn list_submodules(&self) -> Result<Vec<Submodule>> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("submodule")
            .arg("status")
            .arg("--recursive");
        let output = run_git_capture(cmd, "git submodule status --recursive")?;
        Ok(parse_git_submodule_status(&output))
    }

    fn add_submodule_with_output(&self, url: &str, path: &Path) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("submodule")
            .arg("add")
            .arg(url)
            .arg(path);
        run_git_with_output(cmd, &format!("git submodule add {url} {}", path.display()))
    }

    fn update_submodules_with_output(&self) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("submodule")
            .arg("update")
            .arg("--init")
            .arg("--recursive");
        run_git_with_output(cmd, "git submodule update --init --recursive")
    }

    fn remove_submodule_with_output(&self, path: &Path) -> Result<CommandOutput> {
        let mut cmd1 = Command::new("git");
        cmd1.arg("-C")
            .arg(&self.spec.workdir)
            .arg("submodule")
            .arg("deinit")
            .arg("-f")
            .arg("--")
            .arg(path);
        let out1 =
            run_git_with_output(cmd1, &format!("git submodule deinit -f {}", path.display()))?;

        let mut cmd2 = Command::new("git");
        cmd2.arg("-C")
            .arg(&self.spec.workdir)
            .arg("rm")
            .arg("-f")
            .arg("--")
            .arg(path);
        let out2 = run_git_with_output(cmd2, &format!("git rm -f {}", path.display()))?;

        Ok(CommandOutput {
            command: format!("Remove submodule {}", path.display()),
            stdout: [out1.stdout.trim_end(), out2.stdout.trim_end()]
                .iter()
                .filter(|s| !s.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join("\n"),
            stderr: [out1.stderr.trim_end(), out2.stderr.trim_end()]
                .iter()
                .filter(|s| !s.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join("\n"),
            exit_code: Some(0),
        })
    }

    fn discard_worktree_changes(&self, paths: &[&Path]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }

        let mut checkout_paths = Vec::new();
        let mut remove_paths = Vec::new();
        let mut clean_paths = Vec::new();

        for &path in paths {
            if worktree_differs_from_index(&self.spec.workdir, path)? {
                checkout_paths.push(path);
                continue;
            }

            if path_exists_in_index(&self.spec.workdir, path)? {
                if !path_exists_in_head(&self.spec.workdir, path)? {
                    remove_paths.push(path);
                }
            } else {
                clean_paths.push(path);
            }
        }

        if !remove_paths.is_empty() {
            let mut cmd = Command::new("git");
            cmd.arg("-C")
                .arg(&self.spec.workdir)
                .arg("rm")
                .arg("-f")
                .arg("--");
            for path in remove_paths {
                cmd.arg(path);
            }
            run_git_simple(cmd, "git rm -f")?;
        }

        if !clean_paths.is_empty() {
            let mut cmd = Command::new("git");
            cmd.arg("-C")
                .arg(&self.spec.workdir)
                .arg("clean")
                .arg("-fd")
                .arg("--");
            for path in clean_paths {
                cmd.arg(path);
            }
            run_git_simple(cmd, "git clean -fd")?;
        }

        if !checkout_paths.is_empty() {
            let mut cmd = Command::new("git");
            cmd.arg("-C")
                .arg(&self.spec.workdir)
                .arg("checkout")
                .arg("--");
            for path in checkout_paths {
                cmd.arg(path);
            }
            run_git_simple(cmd, "git checkout --")?;
        }

        Ok(())
    }
}

fn worktree_differs_from_index(workdir: &Path, path: &Path) -> Result<bool> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(workdir)
        .arg("diff")
        .arg("--quiet")
        .arg("--")
        .arg(path);

    let output = cmd
        .output()
        .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

    match output.status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => {
            let stderr = str::from_utf8(&output.stderr).unwrap_or("<non-utf8 stderr>");
            Err(Error::new(ErrorKind::Backend(format!(
                "git diff --quiet failed: {}",
                stderr.trim()
            ))))
        }
    }
}

fn path_exists_in_head(workdir: &Path, path: &Path) -> Result<bool> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(workdir)
        .arg("ls-tree")
        .arg("--name-only")
        .arg("HEAD")
        .arg("--")
        .arg(path);

    let output = cmd
        .output()
        .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

    if output.status.success() {
        return Ok(!output.stdout.is_empty());
    }

    let stderr = str::from_utf8(&output.stderr).unwrap_or("<non-utf8 stderr>");
    if stderr.contains("Not a valid object name")
        || stderr.contains("unknown revision")
        || stderr.contains("bad revision")
        || stderr.contains("bad object")
    {
        return Ok(false);
    }

    Err(Error::new(ErrorKind::Backend(format!(
        "git ls-tree --name-only failed: {}",
        stderr.trim()
    ))))
}

fn path_exists_in_index(workdir: &Path, path: &Path) -> Result<bool> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(workdir)
        .arg("ls-files")
        .arg("--error-unmatch")
        .arg("--")
        .arg(path);

    let output = cmd
        .output()
        .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => {
            let stderr = str::from_utf8(&output.stderr).unwrap_or("<non-utf8 stderr>");
            Err(Error::new(ErrorKind::Backend(format!(
                "git ls-files --error-unmatch failed: {}",
                stderr.trim()
            ))))
        }
    }
}

fn read_worktree_file_utf8_optional(workdir: &Path, path: &Path) -> Result<Option<String>> {
    let full = workdir.join(path);
    match std::fs::read(&full) {
        Ok(bytes) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| Error::new(ErrorKind::Unsupported("file is not valid UTF-8"))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::new(ErrorKind::Io(e.kind()))),
    }
}

fn read_worktree_file_bytes_optional(workdir: &Path, path: &Path) -> Result<Option<Vec<u8>>> {
    let full = workdir.join(path);
    match std::fs::read(&full) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::new(ErrorKind::Io(e.kind()))),
    }
}

fn git_show_path_utf8_optional(
    workdir: &Path,
    rev_prefix: &str,
    path: &str,
) -> Result<Option<String>> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(workdir)
        .arg("-c")
        .arg("color.ui=false")
        .arg("--no-pager")
        .arg("show")
        .arg("--no-ext-diff")
        .arg("--pretty=format:")
        .arg(format!("{rev_prefix}{path}"));

    let output = cmd
        .output()
        .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

    if output.status.success() {
        return String::from_utf8(output.stdout)
            .map(Some)
            .map_err(|_| Error::new(ErrorKind::Unsupported("file is not valid UTF-8")));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.to_string();
    if git_blob_missing_for_show(&stderr) {
        return Ok(None);
    }

    Err(Error::new(ErrorKind::Backend(format!(
        "git show failed: {}",
        stderr.trim()
    ))))
}

fn git_show_unmerged_stage0(stderr: &str) -> bool {
    let s = stderr;
    s.contains("is in the index, but not at stage 0")
        || (s.contains("Did you mean ':1:") && s.contains("is in the index"))
}

fn git_show_unmerged_stage_missing(stderr: &str, stage: u8) -> bool {
    let s = stderr;
    match stage {
        2 => s.contains("is in the index, but not at stage 2"),
        3 => s.contains("is in the index, but not at stage 3"),
        _ => false,
    }
}

fn git_show_path_utf8_optional_unmerged_stage(
    workdir: &Path,
    rev_prefix: &str,
    path: &str,
    stage: u8,
) -> Result<Option<String>> {
    match git_show_path_utf8_optional(workdir, rev_prefix, path) {
        Ok(value) => Ok(value),
        Err(e)
            if matches!(e.kind(), ErrorKind::Backend(s) if git_show_unmerged_stage_missing(s, stage)) =>
        {
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

fn git_show_path_bytes_optional(
    workdir: &Path,
    rev_prefix: &str,
    path: &str,
) -> Result<Option<Vec<u8>>> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(workdir)
        .arg("-c")
        .arg("color.ui=false")
        .arg("--no-pager")
        .arg("show")
        .arg("--no-ext-diff")
        .arg("--pretty=format:")
        .arg(format!("{rev_prefix}{path}"));

    let output = cmd
        .output()
        .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

    if output.status.success() {
        return Ok(Some(output.stdout));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.to_string();
    if git_blob_missing_for_show(&stderr) {
        return Ok(None);
    }

    Err(Error::new(ErrorKind::Backend(format!(
        "git show failed: {}",
        stderr.trim()
    ))))
}

fn git_show_path_bytes_optional_unmerged_stage(
    workdir: &Path,
    rev_prefix: &str,
    path: &str,
    stage: u8,
) -> Result<Option<Vec<u8>>> {
    match git_show_path_bytes_optional(workdir, rev_prefix, path) {
        Ok(value) => Ok(value),
        Err(e)
            if matches!(e.kind(), ErrorKind::Backend(s) if git_show_unmerged_stage_missing(s, stage)) =>
        {
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

fn git_first_parent_optional(workdir: &Path, commit: &str) -> Result<Option<String>> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(workdir)
        .arg("--no-pager")
        .arg("rev-parse")
        .arg(format!("{commit}^"));

    let output = cmd
        .output()
        .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Ok(Some(stdout.trim().to_string()));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.to_string();
    if stderr.contains("unknown revision")
        || stderr.contains("bad revision")
        || stderr.contains("bad object")
    {
        return Ok(None);
    }

    Err(Error::new(ErrorKind::Backend(format!(
        "git rev-parse failed: {}",
        stderr.trim()
    ))))
}

fn git_blob_missing_for_show(stderr: &str) -> bool {
    let s = stderr;
    s.contains("does not exist in") // `Path 'x' does not exist in 'REV'`
        || s.contains("exists on disk, but not in") // common suggestion text
        || s.contains("Path '") && s.contains("' does not exist")
        || s.contains("fatal: invalid object name")
        || s.contains("bad object")
        || s.contains("unknown revision")
        || s.contains("bad revision")
}

fn run_git_simple(mut cmd: Command, label: &str) -> Result<()> {
    let output = cmd
        .output()
        .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

    if !output.status.success() {
        let stderr = str::from_utf8(&output.stderr).unwrap_or("<non-utf8 stderr>");
        return Err(Error::new(ErrorKind::Backend(format!(
            "{label} failed: {stderr}"
        ))));
    }

    Ok(())
}

fn run_git_with_output(mut cmd: Command, label: &str) -> Result<CommandOutput> {
    let output = cmd
        .output()
        .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

    let exit_code = output.status.code();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        let stderr_trimmed = stderr.trim();
        return Err(Error::new(ErrorKind::Backend(format!(
            "{}",
            if stderr_trimmed.is_empty() {
                format!("{label} failed")
            } else {
                format!("{label} failed: {stderr_trimmed}")
            }
        ))));
    }

    Ok(CommandOutput {
        command: label.to_string(),
        stdout,
        stderr,
        exit_code,
    })
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

fn parse_git_blame_porcelain(output: &str) -> Vec<BlameLine> {
    let mut out = Vec::new();
    let mut cached_by_commit: std::collections::HashMap<String, (String, Option<i64>, String)> =
        std::collections::HashMap::new();

    let mut current_commit: Option<String> = None;
    let mut author: Option<String> = None;
    let mut author_time: Option<i64> = None;
    let mut summary: Option<String> = None;

    for line in output.lines() {
        if line.starts_with('\t') {
            let commit = current_commit
                .clone()
                .unwrap_or_else(|| "0000000".to_string());
            let line_text = line.strip_prefix('\t').unwrap_or("").to_string();

            let (author_filled, author_time_filled, summary_filled) = if author.is_none()
                && author_time.is_none()
                && summary.is_none()
                && cached_by_commit.contains_key(&commit)
            {
                cached_by_commit.get(&commit).cloned().unwrap_or_default()
            } else {
                (
                    author.clone().unwrap_or_default(),
                    author_time,
                    summary.clone().unwrap_or_default(),
                )
            };

            cached_by_commit.insert(
                commit.clone(),
                (
                    author_filled.clone(),
                    author_time_filled,
                    summary_filled.clone(),
                ),
            );

            out.push(BlameLine {
                commit_id: commit,
                author: author_filled,
                author_time_unix: author_time_filled,
                summary: summary_filled,
                line: line_text,
            });

            author = None;
            author_time = None;
            summary = None;
            continue;
        }

        let mut parts = line.split_whitespace();
        if let Some(first) = parts.next() {
            let is_header = first.len() >= 8 && first.chars().all(|c| c.is_ascii_hexdigit());
            if is_header && parts.next().is_some() && parts.next().is_some() {
                current_commit = Some(first.to_string());
                continue;
            }
        }

        if let Some(rest) = line.strip_prefix("author ") {
            author = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("author-time ") {
            author_time = rest.trim().parse::<i64>().ok();
        } else if let Some(rest) = line.strip_prefix("summary ") {
            summary = Some(rest.to_string());
        }
    }

    out
}

fn parse_git_log_pretty_records(output: &str) -> LogPage {
    let mut commits = Vec::new();
    for record in output.split('\u{001e}') {
        let record = record.trim();
        if record.is_empty() {
            continue;
        }
        let mut parts = record.split('\u{001f}');
        let Some(id) = parts
            .next()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let parents = parts.next().unwrap_or_default();
        let author = parts.next().unwrap_or_default().to_string();
        let time_secs = parts
            .next()
            .and_then(|s| s.trim().parse::<i64>().ok())
            .unwrap_or(0);
        let summary = parts.next().unwrap_or_default().to_string();

        let time = if time_secs >= 0 {
            SystemTime::UNIX_EPOCH + Duration::from_secs(time_secs as u64)
        } else {
            SystemTime::UNIX_EPOCH
        };

        let parent_ids = parents
            .split_whitespace()
            .filter(|p| !p.trim().is_empty())
            .map(|p| CommitId(p.to_string()))
            .collect::<Vec<_>>();

        commits.push(Commit {
            id: CommitId(id),
            parent_ids,
            summary,
            author,
            time,
        });
    }

    LogPage {
        commits,
        next_cursor: None,
    }
}

fn parse_git_worktree_list_porcelain(output: &str) -> Vec<Worktree> {
    let mut out = Vec::new();
    let mut current: Option<Worktree> = None;

    for raw in output.lines() {
        let line = raw.trim();
        if line.is_empty() {
            if let Some(wt) = current.take() {
                out.push(wt);
            }
            continue;
        }

        if let Some(rest) = line.strip_prefix("worktree ") {
            if let Some(wt) = current.take() {
                out.push(wt);
            }
            current = Some(Worktree {
                path: PathBuf::from(rest.trim()),
                head: None,
                branch: None,
                detached: false,
            });
            continue;
        }

        let Some(wt) = current.as_mut() else {
            continue;
        };

        if let Some(rest) = line.strip_prefix("HEAD ") {
            let sha = rest.trim();
            if !sha.is_empty() {
                wt.head = Some(CommitId(sha.to_string()));
            }
        } else if let Some(rest) = line.strip_prefix("branch ") {
            let b = rest.trim();
            if let Some(stripped) = b.strip_prefix("refs/heads/") {
                wt.branch = Some(stripped.to_string());
            } else if !b.is_empty() {
                wt.branch = Some(b.to_string());
            }
        } else if line == "detached" {
            wt.detached = true;
            wt.branch = None;
        }
    }

    if let Some(wt) = current.take() {
        out.push(wt);
    }

    out
}

fn parse_git_submodule_status(output: &str) -> Vec<Submodule> {
    let mut out = Vec::new();
    for raw in output.lines() {
        let line = raw.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        let mut chars = line.chars();
        let status = chars.next().unwrap_or(' ');
        let rest: String = chars.collect();
        let rest = rest.trim();
        let mut parts = rest.split_whitespace();
        let Some(sha) = parts.next() else {
            continue;
        };
        let Some(path) = parts.next() else {
            continue;
        };
        out.push(Submodule {
            path: PathBuf::from(path),
            head: CommitId(sha.to_string()),
            status,
        });
    }
    out
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

fn unix_seconds_to_system_time_or_epoch(seconds: i64) -> SystemTime {
    unix_seconds_to_system_time(seconds).unwrap_or(SystemTime::UNIX_EPOCH)
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

    #[test]
    fn cursor_gate_skips_until_after_last_seen() {
        let cursor = LogCursor {
            last_seen: CommitId("c2".to_string()),
        };
        let mut gate = CursorGate::new(Some(&cursor));

        assert!(gate.should_skip("c1"));
        assert!(gate.should_skip("c2"));
        assert!(!gate.should_skip("c3"));
        assert!(!gate.should_skip("c4"));
    }

    #[test]
    fn unix_seconds_to_system_time_clamps_negative_to_epoch() {
        assert_eq!(
            unix_seconds_to_system_time_or_epoch(-1),
            SystemTime::UNIX_EPOCH
        );
        assert_eq!(
            unix_seconds_to_system_time_or_epoch(1),
            SystemTime::UNIX_EPOCH + Duration::from_secs(1)
        );
    }
}

fn map_entry_status<T, U>(
    status: gix::status::plumbing::index_as_worktree::EntryStatus<T, U>,
) -> FileStatusKind {
    use gix::status::plumbing::index_as_worktree::{Change, EntryStatus};

    match status {
        EntryStatus::Conflict { .. } => FileStatusKind::Conflicted,
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
