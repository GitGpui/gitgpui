use super::*;

mod branch;
mod branch_section;
mod commit;
mod commit_file;
mod diff_editor;
mod diff_hunk;
mod history_branch_filter;
mod pull;
mod push;
mod status_file;
mod tag;

impl GitGpuiView {
    pub(super) fn context_menu_model(
        &self,
        kind: &PopoverKind,
        _cx: &gpui::Context<Self>,
    ) -> Option<ContextMenuModel> {
        match kind {
            PopoverKind::PullPicker => Some(pull::model(self)),
            PopoverKind::PushPicker => Some(push::model(self)),
            PopoverKind::CommitMenu { repo_id, commit_id } => {
                Some(commit::model(self, *repo_id, commit_id))
            }
            PopoverKind::TagMenu { repo_id, commit_id } => {
                Some(tag::model(self, *repo_id, commit_id))
            }
            PopoverKind::StatusFileMenu {
                repo_id,
                area,
                path,
                selection,
            } => Some(status_file::model(
                self,
                selection.as_slice(),
                *repo_id,
                *area,
                path,
            )),
            PopoverKind::BranchMenu {
                repo_id,
                section,
                name,
            } => Some(branch::model(self, *repo_id, *section, name)),
            PopoverKind::BranchSectionMenu { repo_id, section } => {
                Some(branch_section::model(self, *repo_id, *section))
            }
            PopoverKind::CommitFileMenu {
                repo_id,
                commit_id,
                path,
            } => Some(commit_file::model(self, *repo_id, commit_id, path)),
            PopoverKind::DiffHunkMenu { repo_id, src_ix } => {
                Some(diff_hunk::model(self, *repo_id, *src_ix))
            }
            PopoverKind::DiffEditorMenu {
                repo_id,
                area,
                path,
                hunk_patch,
                hunks_count,
                lines_patch,
                lines_count,
                copy_text,
            } => Some(diff_editor::model(
                *repo_id,
                *area,
                path,
                hunk_patch,
                *hunks_count,
                lines_patch,
                *lines_count,
                copy_text,
            )),
            PopoverKind::HistoryBranchFilter { repo_id } => {
                Some(history_branch_filter::model(*repo_id))
            }
            _ => None,
        }
    }

    pub(super) fn context_menu_activate_action(
        &mut self,
        action: ContextMenuAction,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        match action {
            ContextMenuAction::SelectDiff { repo_id, target } => {
                self.store.dispatch(Msg::SelectDiff { repo_id, target });
                self.rebuild_diff_cache();
            }
            ContextMenuAction::CheckoutCommit { repo_id, commit_id } => {
                self.store
                    .dispatch(Msg::CheckoutCommit { repo_id, commit_id });
            }
            ContextMenuAction::CherryPickCommit { repo_id, commit_id } => {
                self.store
                    .dispatch(Msg::CherryPickCommit { repo_id, commit_id });
            }
            ContextMenuAction::RevertCommit { repo_id, commit_id } => {
                self.store
                    .dispatch(Msg::RevertCommit { repo_id, commit_id });
            }
            ContextMenuAction::CheckoutBranch { repo_id, name } => {
                self.store.dispatch(Msg::CheckoutBranch { repo_id, name });
                self.rebuild_diff_cache();
            }
            ContextMenuAction::CheckoutRemoteBranch {
                repo_id,
                remote,
                name,
            } => {
                self.store.dispatch(Msg::CheckoutRemoteBranch {
                    repo_id,
                    remote,
                    name,
                });
                self.rebuild_diff_cache();
            }
            ContextMenuAction::DeleteBranch { repo_id, name } => {
                self.store.dispatch(Msg::DeleteBranch { repo_id, name });
                self.rebuild_diff_cache();
            }
            ContextMenuAction::SetHistoryScope { repo_id, scope } => {
                self.store.dispatch(Msg::SetHistoryScope { repo_id, scope });
            }
            ContextMenuAction::StagePath { repo_id, path } => {
                self.store.dispatch(Msg::SelectDiff {
                    repo_id,
                    target: DiffTarget::WorkingTree {
                        path: path.clone(),
                        area: DiffArea::Unstaged,
                    },
                });
                self.store.dispatch(Msg::StagePath { repo_id, path });
                self.rebuild_diff_cache();
            }
            ContextMenuAction::StagePaths { repo_id, paths } => {
                self.details_pane.update(cx, |pane, cx| {
                    pane.status_multi_selection.remove(&repo_id);
                    cx.notify();
                });
                self.store.dispatch(Msg::ClearDiffSelection { repo_id });
                self.store.dispatch(Msg::StagePaths { repo_id, paths });
                self.rebuild_diff_cache();
            }
            ContextMenuAction::UnstagePath { repo_id, path } => {
                self.store.dispatch(Msg::SelectDiff {
                    repo_id,
                    target: DiffTarget::WorkingTree {
                        path: path.clone(),
                        area: DiffArea::Staged,
                    },
                });
                self.store.dispatch(Msg::UnstagePath { repo_id, path });
                self.rebuild_diff_cache();
            }
            ContextMenuAction::UnstagePaths { repo_id, paths } => {
                self.details_pane.update(cx, |pane, cx| {
                    pane.status_multi_selection.remove(&repo_id);
                    cx.notify();
                });
                self.store.dispatch(Msg::ClearDiffSelection { repo_id });
                self.store.dispatch(Msg::UnstagePaths { repo_id, paths });
                self.rebuild_diff_cache();
            }
            ContextMenuAction::DiscardWorktreeChangesPath { repo_id, path } => {
                let anchor = self
                    .popover_anchor
                    .unwrap_or_else(|| point(px(64.0), px(64.0)));
                self.open_popover_at(
                    PopoverKind::DiscardChangesConfirm {
                        repo_id,
                        paths: vec![path],
                    },
                    anchor,
                    window,
                    cx,
                );
                return;
            }
            ContextMenuAction::DiscardWorktreeChangesPaths { repo_id, paths } => {
                let anchor = self
                    .popover_anchor
                    .unwrap_or_else(|| point(px(64.0), px(64.0)));
                self.open_popover_at(
                    PopoverKind::DiscardChangesConfirm { repo_id, paths },
                    anchor,
                    window,
                    cx,
                );
                return;
            }
            ContextMenuAction::CheckoutConflictSide {
                repo_id,
                paths,
                side,
            } => {
                self.details_pane.update(cx, |pane, cx| {
                    pane.status_multi_selection.remove(&repo_id);
                    cx.notify();
                });
                self.store.dispatch(Msg::ClearDiffSelection { repo_id });
                for path in paths {
                    self.store.dispatch(Msg::CheckoutConflictSide {
                        repo_id,
                        path,
                        side,
                    });
                }
                self.rebuild_diff_cache();
            }
            ContextMenuAction::FetchAll { repo_id } => {
                self.store.dispatch(Msg::FetchAll { repo_id });
            }
            ContextMenuAction::Pull { repo_id, mode } => {
                self.store.dispatch(Msg::Pull { repo_id, mode });
            }
            ContextMenuAction::PullBranch {
                repo_id,
                remote,
                branch,
            } => {
                self.store.dispatch(Msg::PullBranch {
                    repo_id,
                    remote,
                    branch,
                });
            }
            ContextMenuAction::MergeRef { repo_id, reference } => {
                self.store.dispatch(Msg::MergeRef { repo_id, reference });
            }
            ContextMenuAction::Push { repo_id } => {
                self.store.dispatch(Msg::Push { repo_id });
            }
            ContextMenuAction::OpenPopover { kind } => {
                let anchor = self
                    .popover_anchor
                    .unwrap_or_else(|| point(px(64.0), px(64.0)));
                self.open_popover_at(kind, anchor, window, cx);
                return;
            }
            ContextMenuAction::CopyText { text } => {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
            }
            ContextMenuAction::ApplyIndexPatch {
                repo_id,
                patch,
                reverse,
            } => {
                if patch.trim().is_empty() {
                    self.push_toast(zed::ToastKind::Error, "Patch is empty".to_string(), cx);
                } else if reverse {
                    self.store.dispatch(Msg::UnstageHunk { repo_id, patch });
                    self.rebuild_diff_cache();
                } else {
                    self.store.dispatch(Msg::StageHunk { repo_id, patch });
                    self.rebuild_diff_cache();
                }
            }
            ContextMenuAction::ApplyWorktreePatch {
                repo_id,
                patch,
                reverse,
            } => {
                if patch.trim().is_empty() {
                    self.push_toast(zed::ToastKind::Error, "Patch is empty".to_string(), cx);
                } else {
                    self.store.dispatch(Msg::ApplyWorktreePatch {
                        repo_id,
                        patch,
                        reverse,
                    });
                    self.rebuild_diff_cache();
                }
            }
            ContextMenuAction::StageHunk { repo_id, src_ix } => {
                if let Some(patch) = self.build_unified_patch_for_hunk_src_ix(src_ix) {
                    self.store.dispatch(Msg::StageHunk { repo_id, patch });
                } else {
                    self.push_toast(
                        zed::ToastKind::Error,
                        "Couldn't build patch for this hunk".to_string(),
                        cx,
                    );
                }
            }
            ContextMenuAction::UnstageHunk { repo_id, src_ix } => {
                if let Some(patch) = self.build_unified_patch_for_hunk_src_ix(src_ix) {
                    self.store.dispatch(Msg::UnstageHunk { repo_id, patch });
                } else {
                    self.push_toast(
                        zed::ToastKind::Error,
                        "Couldn't build patch for this hunk".to_string(),
                        cx,
                    );
                }
            }
            ContextMenuAction::DeleteTag { repo_id, name } => {
                self.store.dispatch(Msg::DeleteTag { repo_id, name });
            }
        }
        self.close_popover(cx);
    }

    pub(super) fn discard_worktree_changes_confirmed(
        &mut self,
        repo_id: RepoId,
        mut paths: Vec<std::path::PathBuf>,
        cx: &mut gpui::Context<Self>,
    ) {
        if paths.is_empty() {
            return;
        }
        if paths.len() > 1 {
            let mut unique: Vec<std::path::PathBuf> = Vec::with_capacity(paths.len());
            for p in paths {
                if !unique.iter().any(|existing| existing == &p) {
                    unique.push(p);
                }
            }
            paths = unique;

            self.details_pane.update(cx, |pane, cx| {
                pane.status_multi_selection.remove(&repo_id);
                cx.notify();
            });
            self.store.dispatch(Msg::ClearDiffSelection { repo_id });
            self.store
                .dispatch(Msg::DiscardWorktreeChangesPaths { repo_id, paths });
            self.rebuild_diff_cache();
            return;
        }

        let Some(path) = paths.into_iter().next() else {
            return;
        };

        let is_added_file = self
            .state
            .repos
            .iter()
            .find(|r| r.id == repo_id)
            .and_then(|r| match &r.status {
                Loadable::Ready(status) => status
                    .unstaged
                    .iter()
                    .chain(status.staged.iter())
                    .find(|s| s.path == path)
                    .map(|s| s.kind),
                _ => None,
            })
            .is_some_and(|kind| matches!(kind, FileStatusKind::Untracked | FileStatusKind::Added));

        if is_added_file {
            let path_is_selected = self
                .active_repo()
                .filter(|r| r.id == repo_id)
                .and_then(|r| r.diff_target.as_ref())
                .is_some_and(|target| {
                    matches!(target, DiffTarget::WorkingTree { path: selected, .. } if *selected == path)
                });
            if path_is_selected {
                self.store.dispatch(Msg::ClearDiffSelection { repo_id });
            }
        } else {
            self.store.dispatch(Msg::SelectDiff {
                repo_id,
                target: DiffTarget::WorkingTree {
                    path: path.clone(),
                    area: DiffArea::Unstaged,
                },
            });
        }
        self.store
            .dispatch(Msg::DiscardWorktreeChangesPath { repo_id, path });
        self.rebuild_diff_cache();
    }

    pub(super) fn build_unified_patch_for_hunk_src_ix(&self, hunk_src_ix: usize) -> Option<String> {
        if self.is_file_diff_view_active() {
            return None;
        }
        let lines = &self.diff_cache;
        let hunk = lines.get(hunk_src_ix)?;
        if !matches!(hunk.kind, gitgpui_core::domain::DiffLineKind::Hunk) {
            return None;
        }

        let file_start = (0..=hunk_src_ix).rev().find(|&ix| {
            lines
                .get(ix)
                .is_some_and(|l| l.text.starts_with("diff --git "))
        })?;

        let first_hunk = (file_start + 1..lines.len())
            .find(|&ix| {
                let Some(line) = lines.get(ix) else {
                    return false;
                };
                matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk)
                    || line.text.starts_with("diff --git ")
            })
            .unwrap_or(lines.len());

        let header_end = first_hunk.min(hunk_src_ix);
        let hunk_end = (hunk_src_ix + 1..lines.len())
            .find(|&ix| {
                let Some(line) = lines.get(ix) else {
                    return false;
                };
                matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk)
                    || line.text.starts_with("diff --git ")
            })
            .unwrap_or(lines.len());

        let mut out = String::new();
        for line in &lines[file_start..header_end] {
            out.push_str(&line.text);
            out.push('\n');
        }
        for line in &lines[hunk_src_ix..hunk_end] {
            out.push_str(&line.text);
            out.push('\n');
        }
        (!out.trim().is_empty()).then_some(out)
    }

    pub(super) fn context_menu_view(
        &mut self,
        kind: PopoverKind,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::Div {
        let theme = self.theme;
        let model = self
            .context_menu_model(&kind, cx)
            .unwrap_or_else(|| ContextMenuModel::new(vec![]));
        let model_for_keys = model.clone();

        let focus = self.context_menu_focus_handle.clone();
        let current_selected = self.context_menu_selected_ix;
        let selected_for_render = current_selected
            .filter(|&ix| model.is_selectable(ix))
            .or_else(|| model.first_selectable());

        zed::context_menu(
            theme,
            div()
                .track_focus(&focus)
                .key_context("ContextMenu")
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _e: &MouseDownEvent, window, _cx| {
                        window.focus(&this.context_menu_focus_handle);
                    }),
                )
                .on_key_down(
                    cx.listener(move |this, e: &gpui::KeyDownEvent, window, cx| {
                        let key = e.keystroke.key.as_str();
                        let mods = e.keystroke.modifiers;
                        if mods.control || mods.platform || mods.alt || mods.function {
                            return;
                        }

                        match key {
                            "escape" => {
                                this.close_popover(cx);
                            }
                            "up" => {
                                let next = model_for_keys
                                    .next_selectable(this.context_menu_selected_ix, -1);
                                this.context_menu_selected_ix = next;
                                cx.notify();
                            }
                            "down" => {
                                let next = model_for_keys
                                    .next_selectable(this.context_menu_selected_ix, 1);
                                this.context_menu_selected_ix = next;
                                cx.notify();
                            }
                            "home" => {
                                this.context_menu_selected_ix = model_for_keys.first_selectable();
                                cx.notify();
                            }
                            "end" => {
                                this.context_menu_selected_ix = model_for_keys.last_selectable();
                                cx.notify();
                            }
                            "enter" => {
                                let Some(ix) = this
                                    .context_menu_selected_ix
                                    .filter(|&ix| model_for_keys.is_selectable(ix))
                                    .or_else(|| model_for_keys.first_selectable())
                                else {
                                    return;
                                };
                                if let Some(ContextMenuItem::Entry { action, .. }) =
                                    model_for_keys.items.get(ix).cloned()
                                {
                                    this.context_menu_activate_action(action, window, cx);
                                }
                            }
                            _ => {
                                if key.chars().count() == 1 {
                                    let needle = key.to_ascii_uppercase();
                                    let hit = model_for_keys.items.iter().enumerate().find_map(
                                        |(ix, item)| {
                                            let ContextMenuItem::Entry {
                                                shortcut, disabled, ..
                                            } = item
                                            else {
                                                return None;
                                            };
                                            if *disabled {
                                                return None;
                                            }
                                            let shortcut =
                                                shortcut.as_ref()?.as_ref().to_ascii_uppercase();
                                            (shortcut == needle).then_some(ix)
                                        },
                                    );

                                    if let Some(ix) = hit
                                        && let Some(ContextMenuItem::Entry { action, .. }) =
                                            model_for_keys.items.get(ix).cloned()
                                    {
                                        this.context_menu_activate_action(action, window, cx);
                                    }
                                }
                            }
                        }
                    }),
                )
                .children(model.items.into_iter().enumerate().map(|(ix, item)| {
                    match item {
                        ContextMenuItem::Separator => zed::context_menu_separator(theme)
                            .id(("context_menu_sep", ix))
                            .into_any_element(),
                        ContextMenuItem::Header(title) => zed::context_menu_header(theme, title)
                            .id(("context_menu_header", ix))
                            .into_any_element(),
                        ContextMenuItem::Label(text) => zed::context_menu_label(theme, text)
                            .id(("context_menu_label", ix))
                            .into_any_element(),
                        ContextMenuItem::Entry {
                            label,
                            icon,
                            shortcut,
                            disabled,
                            action,
                        } => {
                            let selected = selected_for_render == Some(ix);
                            zed::context_menu_entry(
                                ("context_menu_entry", ix),
                                theme,
                                selected,
                                disabled,
                                icon,
                                label,
                                shortcut,
                                false,
                            )
                            .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                                if *hovering {
                                    this.context_menu_selected_ix = Some(ix);
                                    cx.notify();
                                }
                            }))
                            .when(!disabled, |row| {
                                row.on_click(cx.listener(
                                    move |this, _e: &ClickEvent, window, cx| {
                                        this.context_menu_activate_action(
                                            action.clone(),
                                            window,
                                            cx,
                                        );
                                    },
                                ))
                            })
                            .into_any_element()
                        }
                    }
                }))
                .into_any_element(),
        )
    }
}
