use super::*;

impl MainPaneView {
    pub(in super::super::super) fn is_file_diff_target(target: Option<&DiffTarget>) -> bool {
        matches!(
            target,
            Some(DiffTarget::WorkingTree { .. } | DiffTarget::Commit { path: Some(_), .. })
        )
    }

    pub(super) fn is_file_preview_active(&self) -> bool {
        let preview_path = self
            .untracked_worktree_preview_path()
            .or_else(|| self.added_file_preview_abs_path())
            .or_else(|| self.deleted_file_preview_abs_path());

        let Some(path) = preview_path else {
            return false;
        };
        !crate::view::should_bypass_text_file_preview_for_path(&path)
    }

    pub(super) fn worktree_preview_line_count(&self) -> Option<usize> {
        match &self.worktree_preview {
            Loadable::Ready(lines) => Some(lines.len()),
            _ => None,
        }
    }

    pub(in super::super::super) fn untracked_worktree_preview_path(
        &self,
    ) -> Option<std::path::PathBuf> {
        let repo = self.active_repo()?;
        let status = match &repo.status {
            Loadable::Ready(s) => s,
            _ => return None,
        };
        let workdir = repo.spec.workdir.clone();
        let DiffTarget::WorkingTree { path, area } = repo.diff_target.as_ref()? else {
            return None;
        };
        if *area != DiffArea::Unstaged {
            return None;
        }
        let is_untracked = status
            .unstaged
            .iter()
            .any(|e| e.kind == FileStatusKind::Untracked && &e.path == path);
        is_untracked.then(|| {
            if path.is_absolute() {
                path.clone()
            } else {
                workdir.join(path)
            }
        })
    }

    pub(in super::super::super) fn added_file_preview_abs_path(
        &self,
    ) -> Option<std::path::PathBuf> {
        let repo = self.active_repo()?;
        let workdir = repo.spec.workdir.clone();
        let target = repo.diff_target.as_ref()?;

        match target {
            DiffTarget::WorkingTree { path, area } => {
                if *area != DiffArea::Staged {
                    return None;
                }
                let status = match &repo.status {
                    Loadable::Ready(s) => s,
                    _ => return None,
                };
                let is_added = status
                    .staged
                    .iter()
                    .any(|e| e.kind == FileStatusKind::Added && &e.path == path);
                if !is_added {
                    return None;
                }
                Some(if path.is_absolute() {
                    path.clone()
                } else {
                    workdir.join(path)
                })
            }
            DiffTarget::Commit {
                commit_id,
                path: Some(path),
            } => {
                let details = match &repo.commit_details {
                    Loadable::Ready(d) => d,
                    _ => return None,
                };
                if &details.id != commit_id {
                    return None;
                }
                let is_added = details
                    .files
                    .iter()
                    .any(|f| f.kind == FileStatusKind::Added && &f.path == path);
                if !is_added {
                    return None;
                }
                Some(workdir.join(path))
            }
            _ => None,
        }
    }

    pub(in super::super::super) fn deleted_file_preview_abs_path(
        &self,
    ) -> Option<std::path::PathBuf> {
        let repo = self.active_repo()?;
        let workdir = repo.spec.workdir.clone();
        let target = repo.diff_target.as_ref()?;

        match target {
            DiffTarget::WorkingTree { path, area } => {
                let status = match &repo.status {
                    Loadable::Ready(s) => s,
                    _ => return None,
                };
                let entries = match area {
                    DiffArea::Unstaged => status.unstaged.as_slice(),
                    DiffArea::Staged => status.staged.as_slice(),
                };
                let is_deleted = entries
                    .iter()
                    .any(|e| e.kind == FileStatusKind::Deleted && &e.path == path);
                if !is_deleted {
                    return None;
                }
                Some(if path.is_absolute() {
                    path.clone()
                } else {
                    workdir.join(path)
                })
            }
            DiffTarget::Commit {
                commit_id,
                path: Some(path),
            } => {
                let details = match &repo.commit_details {
                    Loadable::Ready(d) => d,
                    _ => return None,
                };
                if &details.id != commit_id {
                    return None;
                }
                let is_deleted = details
                    .files
                    .iter()
                    .any(|f| f.kind == FileStatusKind::Deleted && &f.path == path);
                if !is_deleted {
                    return None;
                }
                Some(workdir.join(path))
            }
            _ => None,
        }
    }

    pub(in super::super::super) fn ensure_preview_loading(&mut self, path: std::path::PathBuf) {
        let should_reset = match self.worktree_preview_path.as_ref() {
            Some(p) => p != &path,
            None => true,
        };
        if should_reset {
            self.worktree_preview_scroll
                .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
            self.worktree_preview_path = Some(path);
            self.worktree_preview = Loadable::Loading;
            self.worktree_preview_segments_cache_path = None;
            self.worktree_preview_segments_cache.clear();
        } else if matches!(
            self.worktree_preview,
            Loadable::NotLoaded | Loadable::Error(_)
        ) {
            self.worktree_preview = Loadable::Loading;
        }
    }

    pub(in super::super::super) fn ensure_worktree_preview_loaded(
        &mut self,
        path: std::path::PathBuf,
        cx: &mut gpui::Context<Self>,
    ) {
        let should_reload = match self.worktree_preview_path.as_ref() {
            Some(p) => p != &path,
            None => true,
        } || matches!(
            self.worktree_preview,
            Loadable::Error(_) | Loadable::NotLoaded
        );
        if !should_reload {
            return;
        }

        self.worktree_preview_path = Some(path.clone());
        self.worktree_preview = Loadable::Loading;
        self.worktree_preview_segments_cache_path = None;
        self.worktree_preview_segments_cache.clear();
        self.worktree_preview_scroll
            .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);

        cx.spawn(async move |view, cx| {
            const MAX_BYTES: u64 = 2 * 1024 * 1024;
            let result = smol::unblock({
                let path_for_task = path.clone();
                move || {
                let meta = std::fs::metadata(&path_for_task).map_err(|e| e.to_string())?;
                if meta.is_dir() {
                    return Err("Selected path is a directory. Select a file inside to preview, or stage the directory to add its contents.".to_string());
                }
                if meta.len() > MAX_BYTES {
                    return Err(format!(
                        "File is too large to preview ({} bytes).",
                        meta.len()
                    ));
                }

                let bytes = std::fs::read(&path_for_task).map_err(|e| e.to_string())?;
                let text = String::from_utf8(bytes).map_err(|_| {
                    "File is not valid UTF-8; binary preview is not supported.".to_string()
                })?;

                let lines = text.lines().map(|s| s.to_string()).collect::<Vec<_>>();
                Ok::<Arc<Vec<String>>, String>(Arc::new(lines))
                }
            })
            .await;
            let _ = view.update(cx, |this, cx| {
                if this.worktree_preview_path.as_ref() != Some(&path) {
                    return;
                }
                this.worktree_preview_scroll
                    .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
                this.worktree_preview = match result {
                    Ok(lines) => Loadable::Ready(lines),
                    Err(e) => Loadable::Error(e),
                };
                cx.notify();
            });
        })
        .detach();
    }

    pub(in super::super::super) fn try_populate_worktree_preview_from_diff_file(&mut self) {
        let Some((abs_path, preview_result)) = (|| {
            let repo = self.active_repo()?;
            let path_from_target = match repo.diff_target.as_ref()? {
                DiffTarget::WorkingTree { path, .. } => Some(path),
                DiffTarget::Commit {
                    path: Some(path), ..
                } => Some(path),
                _ => None,
            }?;

            let abs_path = if path_from_target.is_absolute() {
                path_from_target.clone()
            } else {
                repo.spec.workdir.join(path_from_target)
            };

            let prefer_old = match repo.diff_target.as_ref()? {
                DiffTarget::WorkingTree { path, area } => match &repo.status {
                    Loadable::Ready(status) => {
                        let entries = match area {
                            DiffArea::Unstaged => status.unstaged.as_slice(),
                            DiffArea::Staged => status.staged.as_slice(),
                        };
                        entries
                            .iter()
                            .any(|e| e.kind == FileStatusKind::Deleted && &e.path == path)
                    }
                    _ => false,
                },
                DiffTarget::Commit {
                    commit_id,
                    path: Some(path),
                } => match &repo.commit_details {
                    Loadable::Ready(details) if &details.id == commit_id => details
                        .files
                        .iter()
                        .any(|f| f.kind == FileStatusKind::Deleted && &f.path == path),
                    _ => false,
                },
                _ => false,
            };

            let mut diff_file_error: Option<String> = None;
            let mut preview_result: Option<Result<Arc<Vec<String>>, String>> = match &repo.diff_file
            {
                Loadable::NotLoaded | Loadable::Loading => None,
                Loadable::Error(e) => {
                    diff_file_error = Some(e.clone());
                    None
                }
                Loadable::Ready(file) => file.as_ref().and_then(|file| {
                    let text = if prefer_old {
                        file.old.as_deref()
                    } else {
                        file.new.as_deref()
                    };
                    text.map(|text| {
                        let lines = text.lines().map(|s| s.to_string()).collect::<Vec<_>>();
                        Ok(Arc::new(lines))
                    })
                }),
            };

            if preview_result.is_none() {
                match &repo.diff {
                    Loadable::Ready(diff) => {
                        let annotated = annotate_unified(diff);
                        if prefer_old {
                            if let Some((_abs_path, lines)) = build_deleted_file_preview_from_diff(
                                &annotated,
                                &repo.spec.workdir,
                                repo.diff_target.as_ref(),
                            ) {
                                preview_result = Some(Ok(Arc::new(lines)));
                            }
                        } else if let Some((_abs_path, lines)) = build_new_file_preview_from_diff(
                            &annotated,
                            &repo.spec.workdir,
                            repo.diff_target.as_ref(),
                        ) {
                            preview_result = Some(Ok(Arc::new(lines)));
                        } else if let Some(e) = diff_file_error {
                            preview_result = Some(Err(e));
                        } else {
                            preview_result =
                                Some(Err("No text preview available for this file.".to_string()));
                        }
                    }
                    Loadable::Error(e) => preview_result = Some(Err(e.clone())),
                    Loadable::NotLoaded | Loadable::Loading => {}
                }
            }

            Some((abs_path, preview_result))
        })() else {
            return;
        };

        if matches!(self.worktree_preview, Loadable::Ready(_))
            && self.worktree_preview_path.as_ref() == Some(&abs_path)
        {
            return;
        }

        let Some(preview_result) = preview_result else {
            return;
        };

        match preview_result {
            Ok(lines) => {
                self.worktree_preview_scroll
                    .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
                self.worktree_preview_path = Some(abs_path);
                self.worktree_preview = Loadable::Ready(lines);
                self.worktree_preview_segments_cache_path = None;
                self.worktree_preview_segments_cache.clear();
            }
            Err(e) => {
                if self.worktree_preview_path.as_ref() != Some(&abs_path)
                    || matches!(
                        self.worktree_preview,
                        Loadable::NotLoaded | Loadable::Loading
                    )
                {
                    self.worktree_preview_scroll
                        .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
                    self.worktree_preview_path = Some(abs_path);
                    self.worktree_preview = Loadable::Error(e);
                    self.worktree_preview_segments_cache_path = None;
                    self.worktree_preview_segments_cache.clear();
                }
            }
        }
    }
}
