use super::*;

pub(super) fn model(
    this: &GitGpuiView,
    repo_id: RepoId,
    area: DiffArea,
    path: &std::path::PathBuf,
) -> ContextMenuModel {
    let is_conflicted = this
        .state
        .repos
        .iter()
        .find(|r| r.id == repo_id)
        .and_then(|r| match &r.status {
            Loadable::Ready(status) => status
                .unstaged
                .iter()
                .chain(status.staged.iter())
                .find(|s| &s.path == path)
                .map(|s| s.kind == gitgpui_core::domain::FileStatusKind::Conflicted),
            _ => None,
        })
        .unwrap_or(false);

    let selection = this
        .status_multi_selection
        .get(&repo_id)
        .map(|s| match area {
            DiffArea::Unstaged => s.unstaged.as_slice(),
            DiffArea::Staged => s.staged.as_slice(),
        })
        .unwrap_or(&[]);
    let use_selection = selection.len() > 1 && selection.iter().any(|p| p == path);
    let selected_paths = if use_selection {
        selection.to_vec()
    } else {
        vec![path.clone()]
    };

    let can_discard_worktree_changes = this
        .state
        .repos
        .iter()
        .find(|r| r.id == repo_id)
        .and_then(|r| match &r.status {
            Loadable::Ready(status) => Some(selected_paths.iter().all(|p| {
                let path = p.as_path();
                if let Some(unstaged) = status.unstaged.iter().find(|s| s.path == path) {
                    return unstaged.kind != gitgpui_core::domain::FileStatusKind::Untracked
                        && unstaged.kind != gitgpui_core::domain::FileStatusKind::Conflicted;
                }
                status.staged.iter().any(|s| {
                    s.path == path && s.kind == gitgpui_core::domain::FileStatusKind::Added
                })
            })),
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
            repo_id,
            target: DiffTarget::WorkingTree {
                path: path.clone(),
                area,
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
                repo_id,
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
                repo_id,
                path: path.clone(),
                rev: None,
            },
        },
    });

    if is_conflicted {
        items.push(ContextMenuItem::Separator);
        let n = selected_paths.len();
        items.push(ContextMenuItem::Entry {
            label: if use_selection {
                format!("Resolve selected using ours ({n})").into()
            } else {
                "Resolve using ours".into()
            },
            icon: Some("⇤".into()),
            shortcut: Some("O".into()),
            disabled: false,
            action: ContextMenuAction::CheckoutConflictSide {
                repo_id,
                paths: selected_paths.clone(),
                side: gitgpui_core::services::ConflictSide::Ours,
            },
        });
        items.push(ContextMenuItem::Entry {
            label: if use_selection {
                format!("Resolve selected using theirs ({n})").into()
            } else {
                "Resolve using theirs".into()
            },
            icon: Some("⇥".into()),
            shortcut: Some("T".into()),
            disabled: false,
            action: ContextMenuAction::CheckoutConflictSide {
                repo_id,
                paths: selected_paths.clone(),
                side: gitgpui_core::services::ConflictSide::Theirs,
            },
        });

        let can_manual = !use_selection || selected_paths.len() == 1;
        let manual_path = if use_selection {
            selected_paths
                .first()
                .cloned()
                .unwrap_or_else(|| path.clone())
        } else {
            path.clone()
        };
        items.push(ContextMenuItem::Entry {
            label: if can_manual {
                "Resolve manually…".into()
            } else {
                "Resolve manually… (select 1 file)".into()
            },
            icon: Some("✎".into()),
            shortcut: Some("M".into()),
            disabled: !can_manual,
            action: ContextMenuAction::SelectDiff {
                repo_id,
                target: DiffTarget::WorkingTree {
                    path: manual_path,
                    area: DiffArea::Unstaged,
                },
            },
        });
    } else {
        match area {
            DiffArea::Unstaged => items.push(ContextMenuItem::Entry {
                label: if use_selection {
                    format!("Stage ({})", selected_paths.len()).into()
                } else {
                    "Stage".into()
                },
                icon: Some("+".into()),
                shortcut: Some("S".into()),
                disabled: false,
                action: if use_selection {
                    ContextMenuAction::StagePaths {
                        repo_id,
                        paths: selected_paths.clone(),
                    }
                } else {
                    ContextMenuAction::StagePath {
                        repo_id,
                        path: path.clone(),
                    }
                },
            }),
            DiffArea::Staged => items.push(ContextMenuItem::Entry {
                label: if use_selection {
                    format!("Unstage ({})", selected_paths.len()).into()
                } else {
                    "Unstage".into()
                },
                icon: Some("−".into()),
                shortcut: Some("U".into()),
                disabled: false,
                action: if use_selection {
                    ContextMenuAction::UnstagePaths {
                        repo_id,
                        paths: selected_paths.clone(),
                    }
                } else {
                    ContextMenuAction::UnstagePath {
                        repo_id,
                        path: path.clone(),
                    }
                },
            }),
        };
    }

    items.push(ContextMenuItem::Entry {
        label: if use_selection {
            format!("Discard ({})", selected_paths.len()).into()
        } else {
            "Discard changes".into()
        },
        icon: Some("↺".into()),
        shortcut: Some("D".into()),
        disabled: !can_discard_worktree_changes,
        action: if use_selection {
            ContextMenuAction::DiscardWorktreeChangesPaths {
                repo_id,
                paths: selected_paths.clone(),
            }
        } else {
            ContextMenuAction::DiscardWorktreeChangesPath {
                repo_id,
                path: path.clone(),
            }
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

    ContextMenuModel::new(items)
}
