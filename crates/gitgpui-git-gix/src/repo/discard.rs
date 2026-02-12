use super::GixRepo;
use crate::util::run_git_simple;
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::Result;
use std::path::Path;
use std::process::Command;
use std::str;

impl GixRepo {
    pub(super) fn discard_worktree_changes_impl(&self, paths: &[&Path]) -> Result<()> {
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
