use super::GixRepo;
use crate::util::{
    parse_reflog_index, run_git_capture, run_git_simple, unix_seconds_to_system_time,
};
use gitgpui_core::domain::{CommitId, StashEntry};
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::Result;
use std::path::Path;
use std::process::Command;

impl GixRepo {
    pub(super) fn create_branch_impl(&self, name: &str, target: &CommitId) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("branch")
            .arg(name)
            .arg(target.as_ref());
        run_git_simple(cmd, "git branch")
    }

    pub(super) fn delete_branch_impl(&self, name: &str) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("branch")
            .arg("-d")
            .arg(name);
        run_git_simple(cmd, "git branch -d")
    }

    pub(super) fn checkout_branch_impl(&self, name: &str) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("checkout")
            .arg(name);
        run_git_simple(cmd, "git checkout")
    }

    pub(super) fn checkout_remote_branch_impl(&self, remote: &str, branch: &str) -> Result<()> {
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

    pub(super) fn checkout_commit_impl(&self, id: &CommitId) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("checkout")
            .arg(id.as_ref());
        run_git_simple(cmd, "git checkout <commit>")
    }

    pub(super) fn cherry_pick_impl(&self, id: &CommitId) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("cherry-pick")
            .arg(id.as_ref());
        run_git_simple(cmd, "git cherry-pick")
    }

    pub(super) fn revert_impl(&self, id: &CommitId) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("revert")
            .arg("--no-edit")
            .arg(id.as_ref());
        run_git_simple(cmd, "git revert")
    }

    pub(super) fn stash_create_impl(&self, message: &str, include_untracked: bool) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("stash")
            .arg("push");
        if include_untracked {
            cmd.arg("-u");
        }
        if !message.is_empty() {
            cmd.arg("-m").arg(message);
        }
        run_git_simple(cmd, "git stash push")
    }

    pub(super) fn stash_list_impl(&self) -> Result<Vec<StashEntry>> {
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
            entries.push(StashEntry {
                index,
                id: CommitId(id.to_string()),
                message,
                created_at,
            });
        }
        Ok(entries)
    }

    pub(super) fn stash_apply_impl(&self, index: usize) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("stash")
            .arg("apply")
            .arg(format!("stash@{{{index}}}"));
        run_git_simple(cmd, "git stash apply")
    }

    pub(super) fn stash_drop_impl(&self, index: usize) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("stash")
            .arg("drop")
            .arg(format!("stash@{{{index}}}"));
        run_git_simple(cmd, "git stash drop")
    }

    pub(super) fn stage_impl(&self, paths: &[&Path]) -> Result<()> {
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

    pub(super) fn unstage_impl(&self, paths: &[&Path]) -> Result<()> {
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

    pub(super) fn commit_impl(&self, message: &str) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("commit")
            .arg("-m")
            .arg(message);
        run_git_simple(cmd, "git commit")
    }

    pub(super) fn commit_amend_impl(&self, message: &str) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("commit")
            .arg("--amend")
            .arg("-m")
            .arg(message);
        run_git_simple(cmd, "git commit --amend")
    }
}
