use super::*;

impl GitGpuiView {
    pub(super) fn context_menu_model(&self, kind: &PopoverKind) -> Option<ContextMenuModel> {
        match kind {
            PopoverKind::PullPicker => {
                let repo_id = self.active_repo_id();
                let disabled = repo_id.is_none();
                let repo_id = repo_id.unwrap_or(RepoId(0));

                Some(ContextMenuModel::new(vec![
                    ContextMenuItem::Header("Pull".into()),
                    ContextMenuItem::Separator,
                    ContextMenuItem::Entry {
                        label: "Pull (default)".into(),
                        icon: Some("↓".into()),
                        shortcut: Some("Enter".into()),
                        disabled,
                        action: ContextMenuAction::Pull {
                            repo_id,
                            mode: PullMode::Default,
                        },
                    },
                    ContextMenuItem::Entry {
                        label: "Pull (fast-forward if possible)".into(),
                        icon: Some("↓".into()),
                        shortcut: Some("F".into()),
                        disabled,
                        action: ContextMenuAction::Pull {
                            repo_id,
                            mode: PullMode::FastForwardIfPossible,
                        },
                    },
                    ContextMenuItem::Entry {
                        label: "Pull (fast-forward only)".into(),
                        icon: Some("↓".into()),
                        shortcut: Some("O".into()),
                        disabled,
                        action: ContextMenuAction::Pull {
                            repo_id,
                            mode: PullMode::FastForwardOnly,
                        },
                    },
                    ContextMenuItem::Entry {
                        label: "Pull (rebase)".into(),
                        icon: Some("↓".into()),
                        shortcut: Some("R".into()),
                        disabled,
                        action: ContextMenuAction::Pull {
                            repo_id,
                            mode: PullMode::Rebase,
                        },
                    },
                    ContextMenuItem::Separator,
                    ContextMenuItem::Entry {
                        label: "Fetch all".into(),
                        icon: Some("↓".into()),
                        shortcut: Some("A".into()),
                        disabled,
                        action: ContextMenuAction::FetchAll { repo_id },
                    },
                ]))
            }
            PopoverKind::PushPicker => {
                let repo_id = self.active_repo_id();
                let disabled = repo_id.is_none();
                let repo_id = repo_id.unwrap_or(RepoId(0));

                Some(ContextMenuModel::new(vec![
                    ContextMenuItem::Header("Push".into()),
                    ContextMenuItem::Separator,
                    ContextMenuItem::Entry {
                        label: "Push".into(),
                        icon: Some("↑".into()),
                        shortcut: Some("Enter".into()),
                        disabled,
                        action: ContextMenuAction::Push { repo_id },
                    },
                    ContextMenuItem::Entry {
                        label: "Force push (with lease)…".into(),
                        icon: Some("⚠".into()),
                        shortcut: Some("F".into()),
                        disabled,
                        action: ContextMenuAction::OpenPopover {
                            kind: PopoverKind::ForcePushConfirm { repo_id },
                        },
                    },
                ]))
            }
            PopoverKind::CommitMenu { repo_id, commit_id } => {
                let sha = commit_id.as_ref().to_string();
                let short: SharedString = sha.get(0..8).unwrap_or(&sha).to_string().into();

                let commit_summary = self
                    .active_repo()
                    .and_then(|r| match &r.log {
                        Loadable::Ready(page) => page
                            .commits
                            .iter()
                            .find(|c| c.id == *commit_id)
                            .map(|c| format!("{} — {}", c.author, c.summary)),
                        _ => None,
                    })
                    .unwrap_or_default();

                let mut items = vec![ContextMenuItem::Header(format!("Commit {short}").into())];
                if !commit_summary.is_empty() {
                    items.push(ContextMenuItem::Label(commit_summary.into()));
                }
                items.push(ContextMenuItem::Separator);
                items.push(ContextMenuItem::Entry {
                    label: "Open diff".into(),
                    icon: Some("↗".into()),
                    shortcut: Some("Enter".into()),
                    disabled: false,
                    action: ContextMenuAction::SelectDiff {
                        repo_id: *repo_id,
                        target: DiffTarget::Commit {
                            commit_id: commit_id.clone(),
                            path: None,
                        },
                    },
                });
                items.push(ContextMenuItem::Entry {
                    label: "Add tag…".into(),
                    icon: Some("🏷".into()),
                    shortcut: Some("T".into()),
                    disabled: false,
                    action: ContextMenuAction::OpenPopover {
                        kind: PopoverKind::CreateTagPrompt {
                            repo_id: *repo_id,
                            target: sha.clone(),
                        },
                    },
                });
                items.push(ContextMenuItem::Entry {
                    label: "Checkout (detached)".into(),
                    icon: Some("⎇".into()),
                    shortcut: Some("D".into()),
                    disabled: false,
                    action: ContextMenuAction::CheckoutCommit {
                        repo_id: *repo_id,
                        commit_id: commit_id.clone(),
                    },
                });
                items.push(ContextMenuItem::Entry {
                    label: "Cherry-pick".into(),
                    icon: Some("⇡".into()),
                    shortcut: Some("P".into()),
                    disabled: false,
                    action: ContextMenuAction::CherryPickCommit {
                        repo_id: *repo_id,
                        commit_id: commit_id.clone(),
                    },
                });
                items.push(ContextMenuItem::Entry {
                    label: "Revert".into(),
                    icon: Some("↶".into()),
                    shortcut: Some("R".into()),
                    disabled: false,
                    action: ContextMenuAction::RevertCommit {
                        repo_id: *repo_id,
                        commit_id: commit_id.clone(),
                    },
                });

                items.push(ContextMenuItem::Separator);
                for (label, icon, mode) in [
                    ("Reset (--soft) to here", "↺", ResetMode::Soft),
                    ("Reset (--mixed) to here", "↺", ResetMode::Mixed),
                    ("Reset (--hard) to here", "↺", ResetMode::Hard),
                ] {
                    items.push(ContextMenuItem::Entry {
                        label: label.into(),
                        icon: Some(icon.into()),
                        shortcut: None,
                        disabled: false,
                        action: ContextMenuAction::OpenPopover {
                            kind: PopoverKind::ResetPrompt {
                                repo_id: *repo_id,
                                target: sha.clone(),
                                mode,
                            },
                        },
                    });
                }

                Some(ContextMenuModel::new(items))
            }
            PopoverKind::TagMenu { repo_id, commit_id } => {
                let sha = commit_id.as_ref().to_string();
                let short: SharedString = sha.get(0..8).unwrap_or(&sha).to_string().into();

                let tags = self
                    .state
                    .repos
                    .iter()
                    .find(|r| r.id == *repo_id)
                    .and_then(|r| match &r.tags {
                        Loadable::Ready(tags) => Some(tags.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();

                let mut items = vec![ContextMenuItem::Header(format!("Tags on {short}").into())];
                let mut tag_names = tags
                    .iter()
                    .filter(|t| t.target == *commit_id)
                    .map(|t| t.name.clone())
                    .collect::<Vec<_>>();
                tag_names.sort();

                if tag_names.is_empty() {
                    items.push(ContextMenuItem::Label("No tags".into()));
                    return Some(ContextMenuModel::new(items));
                }

                items.push(ContextMenuItem::Separator);
                for name in tag_names {
                    items.push(ContextMenuItem::Entry {
                        label: format!("Delete tag {name}").into(),
                        icon: Some("🗑".into()),
                        shortcut: None,
                        disabled: false,
                        action: ContextMenuAction::DeleteTag {
                            repo_id: *repo_id,
                            name,
                        },
                    });
                }

                Some(ContextMenuModel::new(items))
            }
            PopoverKind::StatusFileMenu {
                repo_id,
                area,
                path,
            } => {
                let can_discard_worktree_changes = self
                    .state
                    .repos
                    .iter()
                    .find(|r| r.id == *repo_id)
                    .and_then(|r| match &r.status {
                        Loadable::Ready(status) => Some(
                            status
                                .unstaged
                                .iter()
                                .any(|s| {
                                    s.path == path.as_path()
                                        && s.kind != gitgpui_core::domain::FileStatusKind::Untracked
                                }),
                        ),
                        _ => None,
                    })
                    .unwrap_or(false);

                let mut items = vec![ContextMenuItem::Header(
                    path.file_name()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.display().to_string())
                        .into(),
                )];
                items.push(ContextMenuItem::Label(path.display().to_string().into()));
                items.push(ContextMenuItem::Separator);

                items.push(ContextMenuItem::Entry {
                    label: "Open diff".into(),
                    icon: Some("↗".into()),
                    shortcut: Some("Enter".into()),
                    disabled: false,
                    action: ContextMenuAction::SelectDiff {
                        repo_id: *repo_id,
                        target: DiffTarget::WorkingTree {
                            path: path.clone(),
                            area: *area,
                        },
                    },
                });
                items.push(ContextMenuItem::Entry {
                    label: "File history".into(),
                    icon: Some("⟲".into()),
                    shortcut: Some("H".into()),
                    disabled: false,
                    action: ContextMenuAction::OpenPopover {
                        kind: PopoverKind::FileHistory {
                            repo_id: *repo_id,
                            path: path.clone(),
                        },
                    },
                });
                items.push(ContextMenuItem::Entry {
                    label: "Blame".into(),
                    icon: Some("≡".into()),
                    shortcut: Some("B".into()),
                    disabled: false,
                    action: ContextMenuAction::OpenPopover {
                        kind: PopoverKind::Blame {
                            repo_id: *repo_id,
                            path: path.clone(),
                            rev: None,
                        },
                    },
                });

                match area {
                    DiffArea::Unstaged => items.push(ContextMenuItem::Entry {
                        label: "Stage".into(),
                        icon: Some("+".into()),
                        shortcut: Some("S".into()),
                        disabled: false,
                        action: ContextMenuAction::StagePath {
                            repo_id: *repo_id,
                            path: path.clone(),
                        },
                    }),
                    DiffArea::Staged => items.push(ContextMenuItem::Entry {
                        label: "Unstage".into(),
                        icon: Some("−".into()),
                        shortcut: Some("U".into()),
                        disabled: false,
                        action: ContextMenuAction::UnstagePath {
                            repo_id: *repo_id,
                            path: path.clone(),
                        },
                    }),
                };

                items.push(ContextMenuItem::Entry {
                    label: "Discard changes".into(),
                    icon: Some("↺".into()),
                    shortcut: Some("D".into()),
                    disabled: !can_discard_worktree_changes,
                    action: ContextMenuAction::DiscardWorktreeChangesPath {
                        repo_id: *repo_id,
                        path: path.clone(),
                    },
                });

                items.push(ContextMenuItem::Separator);
                items.push(ContextMenuItem::Entry {
                    label: "Copy path".into(),
                    icon: Some("⧉".into()),
                    shortcut: Some("C".into()),
                    disabled: false,
                    action: ContextMenuAction::CopyText {
                        text: path.display().to_string(),
                    },
                });

                Some(ContextMenuModel::new(items))
            }
            PopoverKind::BranchMenu {
                repo_id,
                section,
                name,
            } => {
                let header: SharedString = match section {
                    BranchSection::Local => "Local branch".into(),
                    BranchSection::Remote => "Remote branch".into(),
                };
                let mut items = vec![ContextMenuItem::Header(header)];
                items.push(ContextMenuItem::Label(name.clone().into()));
                items.push(ContextMenuItem::Separator);

                let is_current_branch = self
                    .state
                    .repos
                    .iter()
                    .find(|r| r.id == *repo_id)
                    .and_then(|r| match &r.head_branch {
                        Loadable::Ready(b) => Some(b == name),
                        _ => None,
                    })
                    .unwrap_or(false);

                items.push(ContextMenuItem::Entry {
                    label: "Checkout".into(),
                    icon: Some("⎇".into()),
                    shortcut: Some("Enter".into()),
                    disabled: false,
                    action: match section {
                        BranchSection::Local => ContextMenuAction::CheckoutBranch {
                            repo_id: *repo_id,
                            name: name.clone(),
                        },
                        BranchSection::Remote => {
                            if let Some((remote, branch)) = name.split_once('/') {
                                ContextMenuAction::CheckoutRemoteBranch {
                                    repo_id: *repo_id,
                                    remote: remote.to_string(),
                                    name: branch.to_string(),
                                }
                            } else {
                                ContextMenuAction::CheckoutBranch {
                                    repo_id: *repo_id,
                                    name: name.clone(),
                                }
                            }
                        }
                    },
                });
                items.push(ContextMenuItem::Entry {
                    label: "Copy name".into(),
                    icon: Some("⧉".into()),
                    shortcut: Some("C".into()),
                    disabled: false,
                    action: ContextMenuAction::CopyText { text: name.clone() },
                });

                if *section == BranchSection::Local {
                    items.push(ContextMenuItem::Separator);
                    items.push(ContextMenuItem::Entry {
                        label: "Delete branch".into(),
                        icon: Some("🗑".into()),
                        shortcut: None,
                        disabled: is_current_branch,
                        action: ContextMenuAction::DeleteBranch {
                            repo_id: *repo_id,
                            name: name.clone(),
                        },
                    });
                }

                if *section == BranchSection::Remote {
                    items.push(ContextMenuItem::Separator);
                    if let Some((remote, branch)) = name.split_once('/') {
                        items.push(ContextMenuItem::Entry {
                            label: "Pull into current".into(),
                            icon: Some("↓".into()),
                            shortcut: Some("P".into()),
                            disabled: false,
                            action: ContextMenuAction::PullBranch {
                                repo_id: *repo_id,
                                remote: remote.to_string(),
                                branch: branch.to_string(),
                            },
                        });
                        items.push(ContextMenuItem::Entry {
                            label: "Merge into current".into(),
                            icon: Some("⇄".into()),
                            shortcut: Some("M".into()),
                            disabled: false,
                            action: ContextMenuAction::MergeRef {
                                repo_id: *repo_id,
                                reference: name.clone(),
                            },
                        });
                        items.push(ContextMenuItem::Separator);
                    }
                    items.push(ContextMenuItem::Entry {
                        label: "Fetch all".into(),
                        icon: Some("↓".into()),
                        shortcut: Some("F".into()),
                        disabled: false,
                        action: ContextMenuAction::FetchAll { repo_id: *repo_id },
                    });
                }

                Some(ContextMenuModel::new(items))
            }
            PopoverKind::BranchSectionMenu { repo_id, section } => {
                let header: SharedString = match section {
                    BranchSection::Local => "Local".into(),
                    BranchSection::Remote => "Remote".into(),
                };
                let mut items = vec![ContextMenuItem::Header(header)];
                items.push(ContextMenuItem::Separator);
                items.push(ContextMenuItem::Entry {
                    label: "Switch branch".into(),
                    icon: Some("⎇".into()),
                    shortcut: Some("Enter".into()),
                    disabled: false,
                    action: ContextMenuAction::OpenPopover {
                        kind: PopoverKind::BranchPicker,
                    },
                });

                if *section == BranchSection::Remote {
                    items.push(ContextMenuItem::Entry {
                        label: "Fetch all".into(),
                        icon: Some("↓".into()),
                        shortcut: Some("F".into()),
                        disabled: false,
                        action: ContextMenuAction::FetchAll { repo_id: *repo_id },
                    });
                }

                Some(ContextMenuModel::new(items))
            }
            PopoverKind::CommitFileMenu {
                repo_id,
                commit_id,
                path,
            } => {
                let mut items = vec![ContextMenuItem::Header(
                    path.file_name()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.display().to_string())
                        .into(),
                )];
                items.push(ContextMenuItem::Label(path.display().to_string().into()));
                items.push(ContextMenuItem::Separator);
                items.push(ContextMenuItem::Entry {
                    label: "Open diff".into(),
                    icon: Some("↗".into()),
                    shortcut: Some("Enter".into()),
                    disabled: false,
                    action: ContextMenuAction::SelectDiff {
                        repo_id: *repo_id,
                        target: DiffTarget::Commit {
                            commit_id: commit_id.clone(),
                            path: Some(path.clone()),
                        },
                    },
                });
                items.push(ContextMenuItem::Entry {
                    label: "File history".into(),
                    icon: Some("⟲".into()),
                    shortcut: Some("H".into()),
                    disabled: false,
                    action: ContextMenuAction::OpenPopover {
                        kind: PopoverKind::FileHistory {
                            repo_id: *repo_id,
                            path: path.clone(),
                        },
                    },
                });
                items.push(ContextMenuItem::Entry {
                    label: "Blame (this commit)".into(),
                    icon: Some("≡".into()),
                    shortcut: Some("B".into()),
                    disabled: false,
                    action: ContextMenuAction::OpenPopover {
                        kind: PopoverKind::Blame {
                            repo_id: *repo_id,
                            path: path.clone(),
                            rev: Some(commit_id.as_ref().to_string()),
                        },
                    },
                });
                items.push(ContextMenuItem::Entry {
                    label: "Copy path".into(),
                    icon: Some("⧉".into()),
                    shortcut: Some("C".into()),
                    disabled: false,
                    action: ContextMenuAction::CopyText {
                        text: path.display().to_string(),
                    },
                });
                Some(ContextMenuModel::new(items))
            }
            PopoverKind::DiffHunkMenu { repo_id, src_ix } => {
                let mut items = vec![ContextMenuItem::Header("Hunk".into())];
                items.push(ContextMenuItem::Separator);

                let (disabled, label, icon, shortcut) = match self
                    .state
                    .repos
                    .iter()
                    .find(|r| r.id == *repo_id)
                    .and_then(|r| r.diff_target.as_ref())
                {
                    Some(DiffTarget::WorkingTree { area, .. }) => match area {
                        DiffArea::Unstaged => (false, "Stage hunk", "+", Some("S")),
                        DiffArea::Staged => (false, "Unstage hunk", "−", Some("U")),
                    },
                    _ => (true, "Stage/Unstage hunk", "+", None::<&'static str>),
                };

                items.push(ContextMenuItem::Entry {
                    label: label.into(),
                    icon: Some(icon.into()),
                    shortcut: shortcut.map(Into::into),
                    disabled,
                    action: match self
                        .state
                        .repos
                        .iter()
                        .find(|r| r.id == *repo_id)
                        .and_then(|r| r.diff_target.as_ref())
                    {
                        Some(DiffTarget::WorkingTree { area: DiffArea::Staged, .. }) => {
                            ContextMenuAction::UnstageHunk {
                                repo_id: *repo_id,
                                src_ix: *src_ix,
                            }
                        }
                        _ => ContextMenuAction::StageHunk {
                            repo_id: *repo_id,
                            src_ix: *src_ix,
                        },
                    },
                });

                Some(ContextMenuModel::new(items))
            }
            PopoverKind::HistoryBranchFilter { repo_id } => Some(ContextMenuModel::new(vec![
                ContextMenuItem::Header("History scope".into()),
                ContextMenuItem::Separator,
                ContextMenuItem::Entry {
                    label: "Current branch".into(),
                    icon: Some("⎇".into()),
                    shortcut: Some("C".into()),
                    disabled: false,
                    action: ContextMenuAction::SetHistoryScope {
                        repo_id: *repo_id,
                        scope: gitgpui_core::domain::LogScope::CurrentBranch,
                    },
                },
                ContextMenuItem::Entry {
                    label: "All branches".into(),
                    icon: Some("∞".into()),
                    shortcut: Some("A".into()),
                    disabled: false,
                    action: ContextMenuAction::SetHistoryScope {
                        repo_id: *repo_id,
                        scope: gitgpui_core::domain::LogScope::AllBranches,
                    },
                },
            ])),
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
            ContextMenuAction::DiscardWorktreeChangesPath { repo_id, path } => {
                self.store.dispatch(Msg::SelectDiff {
                    repo_id,
                    target: DiffTarget::WorkingTree {
                        path: path.clone(),
                        area: DiffArea::Unstaged,
                    },
                });
                self.store
                    .dispatch(Msg::DiscardWorktreeChangesPath { repo_id, path });
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

    pub(super) fn build_unified_patch_for_hunk_src_ix(&self, hunk_src_ix: usize) -> Option<String> {
        if self.is_file_diff_view_active() {
            return None;
        }
        let lines = &self.diff_cache;
        let hunk = lines.get(hunk_src_ix)?;
        if !matches!(hunk.kind, gitgpui_core::domain::DiffLineKind::Hunk) {
            return None;
        }

        let file_start = (0..=hunk_src_ix)
            .rev()
            .find(|&ix| lines.get(ix).is_some_and(|l| l.text.starts_with("diff --git ")))?;

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
            .context_menu_model(&kind)
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
                .children(model.items.into_iter().enumerate().map(|(ix, item)| match item {
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
                                    this.context_menu_activate_action(action.clone(), window, cx);
                                },
                            ))
                        })
                        .into_any_element()
                    }
                }))
                .into_any_element(),
        )
    }
}
