use super::GixRepo;
use crate::util::run_git_with_output;
use gitgpui_core::domain::{CommitId, Submodule};
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::{CommandOutput, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

impl GixRepo {
    pub(super) fn list_submodules_impl(&self) -> Result<Vec<Submodule>> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("submodule")
            .arg("status")
            .arg("--recursive");
        let output = cmd
            .output()
            .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let parsed = parse_git_submodule_status(&stdout);
        if output.status.success() || !parsed.is_empty() {
            return Ok(parsed);
        }

        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("<non-utf8 stderr>");
        // Some repositories may contain gitlinks without corresponding .gitmodules entries.
        // `git submodule status` treats this as fatal; for UI purposes we just show an empty list.
        if stderr.contains("no submodule mapping found in .gitmodules for path") {
            return Ok(Vec::new());
        }

        Err(Error::new(ErrorKind::Backend(format!(
            "git submodule status --recursive failed: {stderr}"
        ))))
    }

    pub(super) fn add_submodule_with_output_impl(
        &self,
        url: &str,
        path: &Path,
    ) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("submodule")
            .arg("add")
            .arg(url)
            .arg(path);
        run_git_with_output(cmd, &format!("git submodule add {url} {}", path.display()))
    }

    pub(super) fn update_submodules_with_output_impl(&self) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("submodule")
            .arg("update")
            .arg("--init")
            .arg("--recursive");
        run_git_with_output(cmd, "git submodule update --init --recursive")
    }

    pub(super) fn remove_submodule_with_output_impl(&self, path: &Path) -> Result<CommandOutput> {
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
