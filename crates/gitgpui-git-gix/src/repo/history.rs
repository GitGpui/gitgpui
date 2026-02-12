use super::GixRepo;
use crate::util::run_git_with_output;
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::{CommandOutput, ResetMode, Result};
use std::process::Command;
use std::str;

impl GixRepo {
    pub(super) fn reset_with_output_impl(
        &self,
        target: &str,
        mode: ResetMode,
    ) -> Result<CommandOutput> {
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

    pub(super) fn rebase_with_output_impl(&self, onto: &str) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("rebase")
            .arg(onto);
        run_git_with_output(cmd, &format!("git rebase {onto}"))
    }

    pub(super) fn rebase_continue_with_output_impl(&self) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("rebase")
            .arg("--continue");
        run_git_with_output(cmd, "git rebase --continue")
    }

    pub(super) fn rebase_abort_with_output_impl(&self) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("rebase")
            .arg("--abort");
        run_git_with_output(cmd, "git rebase --abort")
    }

    pub(super) fn rebase_in_progress_impl(&self) -> Result<bool> {
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

    pub(super) fn merge_commit_message_impl(&self) -> Result<Option<String>> {
        let merge_head = Command::new("git")
            .arg("-C")
            .arg(&self.spec.workdir)
            .arg("rev-parse")
            .arg("--verify")
            .arg("MERGE_HEAD")
            .output()
            .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

        if !merge_head.status.success() {
            return Ok(None);
        }

        let merge_msg_path = Command::new("git")
            .arg("-C")
            .arg(&self.spec.workdir)
            .arg("rev-parse")
            .arg("--git-path")
            .arg("MERGE_MSG")
            .output()
            .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

        if !merge_msg_path.status.success() {
            let stderr = str::from_utf8(&merge_msg_path.stderr).unwrap_or("<non-utf8 stderr>");
            return Err(Error::new(ErrorKind::Backend(format!(
                "git rev-parse --git-path MERGE_MSG failed: {}",
                stderr.trim()
            ))));
        }

        let merge_msg_path = String::from_utf8_lossy(&merge_msg_path.stdout);
        let merge_msg_path = merge_msg_path.trim();
        if merge_msg_path.is_empty() {
            return Ok(None);
        }

        let merge_msg_path = std::path::PathBuf::from(merge_msg_path);
        let merge_msg_path = if merge_msg_path.is_absolute() {
            merge_msg_path
        } else {
            self.spec.workdir.join(merge_msg_path)
        };

        let contents = match std::fs::read_to_string(&merge_msg_path) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(Error::new(ErrorKind::Io(e.kind()))),
        };

        let mut lines: Vec<&str> = Vec::new();
        for line in contents.lines() {
            let line = line.trim_end();
            if line.trim_start().starts_with('#') {
                continue;
            }
            lines.push(line);
        }

        let Some(start) = lines.iter().position(|l| !l.trim().is_empty()) else {
            return Ok(None);
        };
        let end = lines
            .iter()
            .rposition(|l| !l.trim().is_empty())
            .map(|ix| ix + 1)
            .unwrap_or(start);

        let message = lines[start..end].join("\n");
        if message.trim().is_empty() {
            Ok(None)
        } else {
            Ok(Some(message))
        }
    }
}
