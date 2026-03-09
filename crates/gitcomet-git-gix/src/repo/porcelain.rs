use super::GixRepo;
use crate::util::{
    git_stash_untracked_blob_spec, parse_reflog_index, run_git_capture, run_git_simple,
    run_git_simple_with_paths, unix_seconds_to_system_time, validate_hex_commit_id,
    validate_ref_like_arg,
};
use gitcomet_core::domain::{CommitId, StashEntry};
use gitcomet_core::error::{Error, ErrorKind};
use gitcomet_core::services::Result;
use rustc_hash::FxHashSet as HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;

impl GixRepo {
    pub(super) fn create_branch_impl(&self, name: &str, target: &CommitId) -> Result<()> {
        validate_ref_like_arg(name, "branch name")?;
        validate_hex_commit_id(target)?;

        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("branch")
            .arg("--")
            .arg(name)
            .arg(target.as_ref());
        run_git_simple(cmd, "git branch")
    }

    pub(super) fn delete_branch_impl(&self, name: &str) -> Result<()> {
        validate_ref_like_arg(name, "branch name")?;

        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("branch")
            .arg("-d")
            .arg("--")
            .arg(name);
        run_git_simple(cmd, "git branch -d")
    }

    pub(super) fn delete_branch_force_impl(&self, name: &str) -> Result<()> {
        validate_ref_like_arg(name, "branch name")?;

        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("branch")
            .arg("-D")
            .arg("--")
            .arg(name);
        run_git_simple(cmd, "git branch -D")
    }

    pub(super) fn checkout_branch_impl(&self, name: &str) -> Result<()> {
        validate_ref_like_arg(name, "branch name")?;

        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("checkout")
            .arg(name);
        run_git_simple(cmd, "git checkout")
    }

    pub(super) fn checkout_remote_branch_impl(
        &self,
        remote: &str,
        branch: &str,
        local_branch: &str,
    ) -> Result<()> {
        validate_ref_like_arg(remote, "remote name")?;
        validate_ref_like_arg(branch, "branch name")?;
        validate_ref_like_arg(local_branch, "branch name")?;

        let upstream = format!("{remote}/{branch}");

        let output = Command::new("git")
            .arg("-C")
            .arg(&self.spec.workdir)
            .arg("checkout")
            .arg("--track")
            .arg("-b")
            .arg(local_branch)
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
            .arg(local_branch);
        run_git_simple(checkout, "git checkout")?;

        let mut set_upstream = Command::new("git");
        set_upstream
            .arg("-C")
            .arg(&self.spec.workdir)
            .arg("branch")
            .arg("--set-upstream-to")
            .arg(&upstream)
            .arg("--")
            .arg(local_branch);
        run_git_simple(set_upstream, "git branch --set-upstream-to")
    }

    pub(super) fn checkout_commit_impl(&self, id: &CommitId) -> Result<()> {
        validate_hex_commit_id(id)?;

        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("checkout")
            .arg(id.as_ref());
        run_git_simple(cmd, "git checkout <commit>")
    }

    pub(super) fn cherry_pick_impl(&self, id: &CommitId) -> Result<()> {
        validate_hex_commit_id(id)?;

        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("cherry-pick")
            .arg("--")
            .arg(id.as_ref());
        run_git_simple(cmd, "git cherry-pick")
    }

    pub(super) fn revert_impl(&self, id: &CommitId) -> Result<()> {
        validate_hex_commit_id(id)?;

        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("revert")
            .arg("--no-edit")
            .arg("--")
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
            .arg("-c")
            .arg("core.quotePath=false")
            .arg("stash")
            .arg("apply")
            .arg(format!("stash@{{{index}}}"));
        let output = cmd
            .output()
            .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;

        if output.status.success() {
            return Ok(());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stash_apply_reports_untracked_restore_failure(&stdout, &stderr)
            && !stash_apply_blocked_before_merge(&stdout, &stderr)
        {
            // Keep best-effort merge markers for collided untracked files, but still
            // propagate the original apply failure to surface the error to the user.
            let _ = self.merge_untracked_restore_conflicts_from_stash(index, &stdout, &stderr)?;
        }

        let details = if !stderr.trim().is_empty() {
            stderr.trim()
        } else {
            stdout.trim()
        };
        if details.is_empty() {
            Err(Error::new(ErrorKind::Backend(
                "git stash apply failed".to_string(),
            )))
        } else {
            Err(Error::new(ErrorKind::Backend(format!(
                "git stash apply failed: {details}"
            ))))
        }
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
        run_git_simple_with_paths(&self.spec.workdir, "git add", &["add", "-A"], paths)
    }

    fn merge_untracked_restore_conflicts_from_stash(
        &self,
        index: usize,
        stdout: &str,
        stderr: &str,
    ) -> Result<usize> {
        let mut merged = 0usize;
        for path in untracked_restore_conflict_paths(stdout, stderr) {
            let ours_path = self.spec.workdir.join(&path);
            if !ours_path.exists() {
                continue;
            }

            let ours_bytes =
                fs::read(&ours_path).map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;
            let theirs_bytes = self.stash_untracked_blob_bytes(index, &path)?;
            if ours_bytes == theirs_bytes {
                merged = merged.saturating_add(1);
                continue;
            }

            let ours_text = std::str::from_utf8(&ours_bytes).map_err(|_| {
                Error::new(ErrorKind::Backend(format!(
                    "git stash apply failed: cannot merge binary/local non-utf8 untracked file {}",
                    path.display()
                )))
            })?;
            let theirs_text = std::str::from_utf8(&theirs_bytes).map_err(|_| {
                Error::new(ErrorKind::Backend(format!(
                    "git stash apply failed: cannot merge binary/stashed non-utf8 untracked file {}",
                    path.display()
                )))
            })?;

            let merged_text = build_untracked_conflict_markers(ours_text, theirs_text);
            fs::write(&ours_path, merged_text).map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;
            merged = merged.saturating_add(1);
        }

        Ok(merged)
    }

    fn stash_untracked_blob_bytes(&self, index: usize, path: &Path) -> Result<Vec<u8>> {
        let blob_rev = git_stash_untracked_blob_spec(index, path)?;
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("show")
            .arg(blob_rev);
        let output = cmd
            .output()
            .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;
        if output.status.success() {
            return Ok(output.stdout);
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(Error::new(ErrorKind::Backend(format!(
            "git stash apply failed: could not read stashed untracked file {}: {}",
            path.display(),
            stderr.trim()
        ))))
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

        if head.status.success() {
            run_git_simple_with_paths(
                &self.spec.workdir,
                "git reset HEAD",
                &["reset", "HEAD"],
                paths,
            )
        } else {
            run_git_simple_with_paths(
                &self.spec.workdir,
                "git rm --cached",
                &["rm", "--cached"],
                paths,
            )
        }
    }

    pub(super) fn commit_impl(&self, message: &str) -> Result<()> {
        let merge_in_progress = self.merge_in_progress_for_commit()?;
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.spec.workdir).arg("commit");
        if merge_in_progress {
            cmd.arg("--allow-empty");
        }
        cmd.arg("-m").arg(message);
        let label = if merge_in_progress {
            "git commit --allow-empty"
        } else {
            "git commit"
        };
        run_git_simple(cmd, label)
    }

    fn merge_in_progress_for_commit(&self) -> Result<bool> {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.spec.workdir)
            .arg("rev-parse")
            .arg("--verify")
            .arg("MERGE_HEAD")
            .output()
            .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;
        Ok(output.status.success())
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

fn stash_apply_reports_untracked_restore_failure(stdout: &str, stderr: &str) -> bool {
    let combined = format!("{stdout}\n{stderr}").to_ascii_lowercase();
    combined.contains("could not restore untracked files from stash")
        || combined.contains("already exists, no checkout")
}

fn stash_apply_blocked_before_merge(stdout: &str, stderr: &str) -> bool {
    let combined = format!("{stdout}\n{stderr}").to_ascii_lowercase();
    combined.contains("would be overwritten by merge")
        || combined.contains("please commit your changes or stash them before you merge")
}

fn untracked_restore_conflict_paths(stdout: &str, stderr: &str) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut seen = HashSet::default();
    let suffix = " already exists, no checkout";
    for line in stderr.lines().chain(stdout.lines()) {
        let Some(mut path) = line.trim().strip_suffix(suffix) else {
            continue;
        };
        path = path.trim();
        if let Some(stripped) = path.strip_prefix("error: ") {
            path = stripped.trim();
        } else if let Some(stripped) = path.strip_prefix("fatal: ") {
            path = stripped.trim();
        }
        if let Some(stripped) = path.strip_prefix('"').and_then(|p| p.strip_suffix('"')) {
            path = stripped;
        } else if let Some(stripped) = path.strip_prefix('\'').and_then(|p| p.strip_suffix('\'')) {
            path = stripped;
        }
        if path.is_empty() {
            continue;
        }
        if seen.insert(path.to_string()) {
            out.push(std::path::PathBuf::from(path));
        }
    }
    out
}

fn build_untracked_conflict_markers(current: &str, stashed: &str) -> String {
    let mut out = String::new();
    out.push_str("<<<<<<< Current file\n");
    out.push_str(current);
    if !current.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("=======\n");
    out.push_str(stashed);
    if !stashed.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(">>>>>>> Stashed file\n");
    out
}

#[cfg(test)]
mod tests {
    use super::{
        build_untracked_conflict_markers, stash_apply_blocked_before_merge,
        stash_apply_reports_untracked_restore_failure, untracked_restore_conflict_paths,
    };
    use std::path::Path;

    #[test]
    fn parses_untracked_restore_conflict_paths_with_optional_error_prefixes() {
        let stderr = "error: Cargo.toml.orig already exists, no checkout\n";
        let paths = untracked_restore_conflict_paths("", stderr);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], Path::new("Cargo.toml.orig"));
    }

    #[test]
    fn parses_untracked_restore_conflict_paths_with_quoted_paths() {
        let stderr = "\"docs/a file.txt\" already exists, no checkout\n";
        let paths = untracked_restore_conflict_paths("", stderr);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], Path::new("docs/a file.txt"));
    }

    #[test]
    fn stash_apply_reports_untracked_restore_failure_detects_known_messages() {
        assert!(stash_apply_reports_untracked_restore_failure(
            "Could not restore untracked files from stash",
            "",
        ));
        assert!(stash_apply_reports_untracked_restore_failure(
            "",
            "fatal: docs/a.txt already exists, no checkout",
        ));
        assert!(!stash_apply_reports_untracked_restore_failure(
            "fatal: merge conflict",
            "fatal: merge conflict",
        ));
    }

    #[test]
    fn stash_apply_blocked_before_merge_detects_known_messages() {
        assert!(stash_apply_blocked_before_merge(
            "error: local changes would be overwritten by merge",
            "",
        ));
        assert!(stash_apply_blocked_before_merge(
            "",
            "Please commit your changes or stash them before you merge.",
        ));
        assert!(!stash_apply_blocked_before_merge(
            "could not restore untracked files from stash",
            "",
        ));
    }

    #[test]
    fn untracked_restore_conflict_paths_dedups_and_skips_empty_entries() {
        let stderr = concat!(
            "fatal: 'docs/a file.txt' already exists, no checkout\n",
            "error: 'docs/a file.txt' already exists, no checkout\n",
            "\"\" already exists, no checkout\n",
        );
        let stdout = "docs/b file.txt already exists, no checkout\n";
        let paths = untracked_restore_conflict_paths(stdout, stderr);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], Path::new("docs/a file.txt"));
        assert_eq!(paths[1], Path::new("docs/b file.txt"));
    }

    #[test]
    fn build_untracked_conflict_markers_appends_missing_newlines() {
        let merged = build_untracked_conflict_markers("ours", "theirs");
        assert_eq!(
            merged,
            concat!(
                "<<<<<<< Current file\n",
                "ours\n",
                "=======\n",
                "theirs\n",
                ">>>>>>> Stashed file\n"
            )
        );
    }
}
