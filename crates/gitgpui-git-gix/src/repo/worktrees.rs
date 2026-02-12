use super::GixRepo;
use crate::util::{run_git_capture, run_git_with_output};
use gitgpui_core::domain::{CommitId, Worktree};
use gitgpui_core::services::{CommandOutput, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

impl GixRepo {
    pub(super) fn list_worktrees_impl(&self) -> Result<Vec<Worktree>> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("worktree")
            .arg("list")
            .arg("--porcelain");
        let output = run_git_capture(cmd, "git worktree list --porcelain")?;
        Ok(parse_git_worktree_list_porcelain(&output))
    }

    pub(super) fn add_worktree_with_output_impl(
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

    pub(super) fn remove_worktree_with_output_impl(&self, path: &Path) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("worktree")
            .arg("remove")
            .arg(path);
        run_git_with_output(cmd, &format!("git worktree remove {}", path.display()))
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
