use super::GixRepo;
use crate::util::run_git_capture;
use gitcomet_core::conflict_session::{ConflictPayload, ConflictSession};
use gitcomet_core::domain::{DiffArea, DiffTarget, FileDiffImage, FileDiffText, FileStatusKind};
use gitcomet_core::error::{Error, ErrorKind};
use gitcomet_core::services::{ConflictFileStages, Result, decode_utf8_optional};
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

                let repo = self._repo.to_thread_local();
                let (old, new) = match area {
                    DiffArea::Unstaged => {
                        let old = match gix_index_unconflicted_blob_bytes_optional(&repo, path)? {
                            IndexUnconflictedBlob::Present(bytes) => {
                                Some(decode_utf8_bytes(bytes)?)
                            }
                            IndexUnconflictedBlob::Missing => None,
                            IndexUnconflictedBlob::Unmerged => {
                                let ours = decode_utf8_bytes_optional(
                                    gix_index_stage_blob_bytes_optional(&repo, path, 2)?,
                                )?;
                                let theirs = decode_utf8_bytes_optional(
                                    gix_index_stage_blob_bytes_optional(&repo, path, 3)?,
                                )?;
                                return Ok(Some(FileDiffText {
                                    path: path.clone(),
                                    old: ours,
                                    new: theirs,
                                }));
                            }
                        };
                        let new = read_worktree_file_utf8_optional(&self.spec.workdir, path)?;
                        (old, new)
                    }
                    DiffArea::Staged => {
                        let old = decode_utf8_bytes_optional(
                            gix_revision_path_blob_bytes_optional(&repo, "HEAD", path)?,
                        )?;
                        let new = match gix_index_unconflicted_blob_bytes_optional(&repo, path)? {
                            IndexUnconflictedBlob::Present(bytes) => {
                                Some(decode_utf8_bytes(bytes)?)
                            }
                            IndexUnconflictedBlob::Missing => None,
                            IndexUnconflictedBlob::Unmerged => decode_utf8_bytes_optional(
                                gix_index_stage_blob_bytes_optional(&repo, path, 2)?,
                            )?
                            .or(decode_utf8_bytes_optional(
                                gix_index_stage_blob_bytes_optional(&repo, path, 3)?,
                            )?),
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

                let repo = self._repo.to_thread_local();
                let parent = gix_first_parent_optional(&repo, commit_id.as_ref())?;

                let old = match parent {
                    Some(parent) => decode_utf8_bytes_optional(
                        gix_revision_path_blob_bytes_optional(&repo, &parent, path)?,
                    )?,
                    None => None,
                };
                let new = decode_utf8_bytes_optional(gix_revision_path_blob_bytes_optional(
                    &repo,
                    commit_id.as_ref(),
                    path,
                )?)?;

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

                let repo = self._repo.to_thread_local();
                let (old, new) = match area {
                    DiffArea::Unstaged => {
                        let old = match gix_index_unconflicted_blob_bytes_optional(&repo, path)? {
                            IndexUnconflictedBlob::Present(bytes) => Some(bytes),
                            IndexUnconflictedBlob::Missing => None,
                            IndexUnconflictedBlob::Unmerged => {
                                let ours = gix_index_stage_blob_bytes_optional(&repo, path, 2)?;
                                let theirs = gix_index_stage_blob_bytes_optional(&repo, path, 3)?;
                                return Ok(Some(FileDiffImage {
                                    path: path.clone(),
                                    old: ours,
                                    new: theirs,
                                }));
                            }
                        };
                        let new = read_worktree_file_bytes_optional(&self.spec.workdir, path)?;
                        (old, new)
                    }
                    DiffArea::Staged => {
                        let old = gix_revision_path_blob_bytes_optional(&repo, "HEAD", path)?;
                        let new = match gix_index_unconflicted_blob_bytes_optional(&repo, path)? {
                            IndexUnconflictedBlob::Present(bytes) => Some(bytes),
                            IndexUnconflictedBlob::Missing => None,
                            IndexUnconflictedBlob::Unmerged => {
                                gix_index_stage_blob_bytes_optional(&repo, path, 2)?
                                    .or(gix_index_stage_blob_bytes_optional(&repo, path, 3)?)
                            }
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

                let repo = self._repo.to_thread_local();
                let parent = gix_first_parent_optional(&repo, commit_id.as_ref())?;

                let old = match parent {
                    Some(parent) => gix_revision_path_blob_bytes_optional(&repo, &parent, path)?,
                    None => None,
                };
                let new = gix_revision_path_blob_bytes_optional(&repo, commit_id.as_ref(), path)?;

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

        let repo = self._repo.to_thread_local();
        let base_bytes = gix_index_stage_blob_bytes_optional(&repo, path, 1)?;
        let ours_bytes = gix_index_stage_blob_bytes_optional(&repo, path, 2)?;
        let theirs_bytes = gix_index_stage_blob_bytes_optional(&repo, path, 3)?;
        let base = decode_utf8_optional(base_bytes.as_deref());
        let ours = decode_utf8_optional(ours_bytes.as_deref());
        let theirs = decode_utf8_optional(theirs_bytes.as_deref());

        Ok(Some(ConflictFileStages {
            path: path.to_path_buf(),
            base_bytes,
            ours_bytes,
            theirs_bytes,
            base,
            ours,
            theirs,
        }))
    }

    pub(super) fn conflict_session_impl(&self, path: &Path) -> Result<Option<ConflictSession>> {
        let repo_path = to_repo_path(path, &self.spec.workdir)?;
        let status = self.status_impl()?;
        let Some(conflict_kind) = status
            .unstaged
            .iter()
            .find(|entry| entry.path == repo_path && entry.kind == FileStatusKind::Conflicted)
            .and_then(|entry| entry.conflict)
        else {
            return Ok(None);
        };

        let Some(stages) = self.conflict_file_stages_impl(&repo_path)? else {
            return Ok(None);
        };
        let current_bytes = std::fs::read(self.spec.workdir.join(&repo_path)).ok();
        let current = decode_utf8_optional(current_bytes.as_deref());

        let payload_from = |bytes: Option<Vec<u8>>, text: Option<String>| -> ConflictPayload {
            if let Some(text) = text {
                ConflictPayload::Text(text)
            } else if let Some(bytes) = bytes {
                ConflictPayload::from_bytes(bytes)
            } else {
                ConflictPayload::Absent
            }
        };

        let base = payload_from(stages.base_bytes, stages.base);
        let ours = payload_from(stages.ours_bytes, stages.ours);
        let theirs = payload_from(stages.theirs_bytes, stages.theirs);

        let session = if let Some(current) = current {
            ConflictSession::from_merged_text(
                repo_path,
                conflict_kind,
                base,
                ours,
                theirs,
                &current,
            )
        } else {
            ConflictSession::new(repo_path, conflict_kind, base, ours, theirs)
        };
        Ok(Some(session))
    }
}

fn to_repo_path(path: &Path, workdir: &Path) -> Result<std::path::PathBuf> {
    if path.is_absolute() {
        let relative = path.strip_prefix(workdir).map_err(|_| {
            Error::new(ErrorKind::Backend(format!(
                "path '{}' is outside repository workdir '{}'",
                path.display(),
                workdir.display()
            )))
        })?;
        Ok(relative.to_path_buf())
    } else {
        Ok(path.to_path_buf())
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

enum IndexUnconflictedBlob {
    Present(Vec<u8>),
    Missing,
    Unmerged,
}

fn decode_utf8_bytes(bytes: Vec<u8>) -> Result<String> {
    String::from_utf8(bytes)
        .map_err(|_| Error::new(ErrorKind::Unsupported("file is not valid UTF-8")))
}

fn decode_utf8_bytes_optional(bytes: Option<Vec<u8>>) -> Result<Option<String>> {
    bytes.map(decode_utf8_bytes).transpose()
}

fn gix_blob_bytes_from_object_id_optional(
    repo: &gix::Repository,
    object_id: gix::ObjectId,
) -> Result<Option<Vec<u8>>> {
    let Some(object) = repo
        .try_find_object(object_id)
        .map_err(|e| Error::new(ErrorKind::Backend(format!("gix try_find_object: {e}"))))?
    else {
        return Ok(None);
    };

    Ok(match object.try_into_blob() {
        Ok(mut blob) => Some(blob.take_data()),
        Err(_) => None,
    })
}

fn gix_revision_id_optional(
    repo: &gix::Repository,
    revision: &str,
) -> Result<Option<gix::ObjectId>> {
    if revision == "HEAD" {
        return match repo.head_id() {
            Ok(id) => Ok(Some(id.detach())),
            Err(_) => Ok(None),
        };
    }

    if let Ok(id) = gix::ObjectId::from_hex(revision.as_bytes()) {
        return Ok(Some(id));
    }

    let Some(mut reference) = repo
        .try_find_reference(revision)
        .map_err(|e| Error::new(ErrorKind::Backend(format!("gix try_find_reference: {e}"))))?
    else {
        return Ok(None);
    };

    let id = match reference.try_id() {
        Some(id) => id.detach(),
        None => match reference.peel_to_id() {
            Ok(id) => id.detach(),
            Err(_) => return Ok(None),
        },
    };
    Ok(Some(id))
}

fn gix_revision_path_blob_bytes_optional(
    repo: &gix::Repository,
    revision: &str,
    path: &Path,
) -> Result<Option<Vec<u8>>> {
    let Some(object_id) = gix_revision_id_optional(repo, revision)? else {
        return Ok(None);
    };

    let object = match repo.find_object(object_id) {
        Ok(object) => object,
        Err(_) => return Ok(None),
    };
    let tree = match object.peel_to_tree() {
        Ok(tree) => tree,
        Err(_) => return Ok(None),
    };

    let Some(entry) = tree
        .lookup_entry_by_path(path)
        .map_err(|e| Error::new(ErrorKind::Backend(format!("gix lookup_entry_by_path: {e}"))))?
    else {
        return Ok(None);
    };

    gix_blob_bytes_from_object_id_optional(repo, entry.object_id())
}

fn gix_index_stage_from_u8(stage: u8) -> Option<gix::index::entry::Stage> {
    match stage {
        0 => Some(gix::index::entry::Stage::Unconflicted),
        1 => Some(gix::index::entry::Stage::Base),
        2 => Some(gix::index::entry::Stage::Ours),
        3 => Some(gix::index::entry::Stage::Theirs),
        _ => None,
    }
}

fn gix_index_stage_blob_bytes_optional(
    repo: &gix::Repository,
    path: &Path,
    stage: u8,
) -> Result<Option<Vec<u8>>> {
    let Some(stage) = gix_index_stage_from_u8(stage) else {
        return Err(Error::new(ErrorKind::Backend(format!(
            "invalid conflict stage: {stage}"
        ))));
    };

    let index = repo
        .index_or_load_from_head_or_empty()
        .map_err(|e| Error::new(ErrorKind::Backend(format!("gix index: {e}"))))?;

    let path = gix::path::os_str_into_bstr(path.as_os_str())
        .map_err(|_| Error::new(ErrorKind::Unsupported("path is not valid UTF-8")))?;
    let Some(entry) = index.entry_by_path_and_stage(path, stage) else {
        return Ok(None);
    };

    gix_blob_bytes_from_object_id_optional(repo, entry.id)
}

fn gix_index_unconflicted_blob_bytes_optional(
    repo: &gix::Repository,
    path: &Path,
) -> Result<IndexUnconflictedBlob> {
    let index = repo
        .index_or_load_from_head_or_empty()
        .map_err(|e| Error::new(ErrorKind::Backend(format!("gix index: {e}"))))?;

    let path = gix::path::os_str_into_bstr(path.as_os_str())
        .map_err(|_| Error::new(ErrorKind::Unsupported("path is not valid UTF-8")))?;

    if let Some(entry) = index.entry_by_path_and_stage(path, gix::index::entry::Stage::Unconflicted)
    {
        return Ok(
            match gix_blob_bytes_from_object_id_optional(repo, entry.id)? {
                Some(bytes) => IndexUnconflictedBlob::Present(bytes),
                None => IndexUnconflictedBlob::Missing,
            },
        );
    }

    if index.entry_range(path).is_some() {
        return Ok(IndexUnconflictedBlob::Unmerged);
    }

    Ok(IndexUnconflictedBlob::Missing)
}

fn gix_first_parent_optional(repo: &gix::Repository, commit: &str) -> Result<Option<String>> {
    let Some(commit_id) = gix_revision_id_optional(repo, commit)? else {
        return Ok(None);
    };

    let commit = match repo.find_commit(commit_id) {
        Ok(commit) => commit,
        Err(_) => return Ok(None),
    };
    Ok(commit.parent_ids().next().map(|id| id.detach().to_string()))
}
