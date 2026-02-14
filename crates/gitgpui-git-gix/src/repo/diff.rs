use super::GixRepo;
use crate::util::run_git_capture;
use gitgpui_core::domain::{DiffArea, DiffTarget, FileDiffImage, FileDiffText};
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::{ConflictFileStages, Result};
use std::path::Path;
use std::process::Command;
use std::str;

impl GixRepo {
    pub(super) fn diff_unified_impl(&self, target: &DiffTarget) -> Result<String> {
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

    pub(super) fn diff_file_text_impl(&self, target: &DiffTarget) -> Result<Option<FileDiffText>> {
        match target {
            DiffTarget::WorkingTree { path, area } => {
                if matches!(area, DiffArea::Unstaged) {
                    let full_path = if path.is_absolute() {
                        path.clone()
                    } else {
                        self.spec.workdir.join(path)
                    };
                    if std::fs::metadata(&full_path).is_ok_and(|m| m.is_dir()) {
                        return Ok(None);
                    }
                }

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

    pub(super) fn diff_file_image_impl(
        &self,
        target: &DiffTarget,
    ) -> Result<Option<FileDiffImage>> {
        match target {
            DiffTarget::WorkingTree { path, area } => {
                if matches!(area, DiffArea::Unstaged) {
                    let full_path = if path.is_absolute() {
                        path.clone()
                    } else {
                        self.spec.workdir.join(path)
                    };
                    if std::fs::metadata(&full_path).is_ok_and(|m| m.is_dir()) {
                        return Ok(None);
                    }
                }

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

    pub(super) fn conflict_file_stages_impl(
        &self,
        path: &Path,
    ) -> Result<Option<ConflictFileStages>> {
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.spec.workdir.join(path)
        };
        if std::fs::metadata(&full_path).is_ok_and(|m| m.is_dir()) {
            return Ok(None);
        }

        let path_str = path.to_string_lossy();
        let base = git_show_path_utf8_optional_unmerged_stage(
            &self.spec.workdir,
            ":1:",
            path_str.as_ref(),
            1,
        )?;
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

        Ok(Some(ConflictFileStages {
            path: path.to_path_buf(),
            base,
            ours,
            theirs,
        }))
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
        1 => s.contains("is in the index, but not at stage 1"),
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
        Err(e) if matches!(e.kind(), ErrorKind::Backend(s) if git_show_unmerged_stage_missing(s, stage)) => {
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
        Err(e) if matches!(e.kind(), ErrorKind::Backend(s) if git_show_unmerged_stage_missing(s, stage)) => {
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
    let s = stderr.to_ascii_lowercase();
    s.contains("does not exist in") // `Path 'x' does not exist in 'REV'`
        || s.contains("exists on disk, but not in") // common suggestion text
        || (s.contains("path '") && s.contains("' does not exist"))
        || s.contains("neither on disk nor in the index")
        || s.contains("fatal: invalid object name")
        || s.contains("bad object")
        || s.contains("unknown revision")
        || s.contains("bad revision")
}
