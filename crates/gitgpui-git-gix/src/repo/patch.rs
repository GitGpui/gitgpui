use super::GixRepo;
use crate::util::{run_git_capture, run_git_with_output};
use gitgpui_core::domain::CommitId;
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::{CommandOutput, Result};
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

impl GixRepo {
    pub(super) fn export_patch_with_output_impl(
        &self,
        commit_id: &CommitId,
        dest: &Path,
    ) -> Result<CommandOutput> {
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

    pub(super) fn apply_patch_with_output_impl(&self, patch: &Path) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("am")
            .arg("--3way")
            .arg("--")
            .arg(patch);
        run_git_with_output(cmd, &format!("git am --3way {}", patch.display()))
    }

    pub(super) fn apply_unified_patch_to_index_with_output_impl(
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

    pub(super) fn apply_unified_patch_to_worktree_with_output_impl(
        &self,
        patch: &str,
        reverse: bool,
    ) -> Result<CommandOutput> {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let tmp_path = std::env::temp_dir().join(format!(
            "gitgpui-worktree-patch-{}-{nanos}.patch",
            std::process::id()
        ));
        std::fs::write(&tmp_path, patch.as_bytes())
            .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("apply")
            .arg("--recount")
            .arg("--whitespace=nowarn");
        if reverse {
            cmd.arg("--reverse");
        }
        cmd.arg(&tmp_path);

        let label = if reverse {
            format!("git apply --reverse {}", tmp_path.display())
        } else {
            format!("git apply {}", tmp_path.display())
        };

        let result = run_git_with_output(cmd, &label);
        let _ = std::fs::remove_file(&tmp_path);
        result
    }
}
