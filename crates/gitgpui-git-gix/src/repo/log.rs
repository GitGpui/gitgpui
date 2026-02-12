use super::GixRepo;
use crate::util::{
    parse_git_log_pretty_records, parse_name_status_line, parse_reflog_index, run_git_capture,
    unix_seconds_to_system_time, unix_seconds_to_system_time_or_epoch,
};
use gitgpui_core::domain::{
    Commit, CommitDetails, CommitFileChange, CommitId, LogCursor, LogPage, ReflogEntry,
};
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::Result;
use gix::bstr::ByteSlice as _;
use gix::traverse::commit::simple::CommitTimeOrder;
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use std::str;

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

impl GixRepo {
    pub(super) fn log_head_page_impl(
        &self,
        limit: usize,
        cursor: Option<&LogCursor>,
    ) -> Result<LogPage> {
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

    pub(super) fn log_all_branches_page_impl(
        &self,
        limit: usize,
        cursor: Option<&LogCursor>,
    ) -> Result<LogPage> {
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

    pub(super) fn log_file_page_impl(
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

    pub(super) fn commit_details_impl(&self, id: &CommitId) -> Result<CommitDetails> {
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
                .collect::<Vec<CommitFileChange>>()
        };

        Ok(CommitDetails {
            id: id.clone(),
            message,
            committed_at,
            parent_ids,
            files,
        })
    }

    pub(super) fn reflog_head_impl(&self, limit: usize) -> Result<Vec<ReflogEntry>> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
