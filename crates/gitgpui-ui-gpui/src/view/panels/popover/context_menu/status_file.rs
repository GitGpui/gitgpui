use super::*;
use std::collections::HashSet;

pub(super) fn model(
    this: &PopoverHost,
    repo_id: RepoId,
    area: DiffArea,
    path: &std::path::PathBuf,
    cx: &gpui::Context<PopoverHost>,
) -> ContextMenuModel {
    let (use_selection, selected_count) = {
        let pane = this.details_pane.read(cx);
        let selection = pane
            .status_multi_selection
            .get(&repo_id)
            .map(|sel| match area {
                DiffArea::Unstaged => sel.unstaged.as_slice(),
                DiffArea::Staged => sel.staged.as_slice(),
            })
            .unwrap_or(&[]);

        let use_selection = selection.len() > 1 && selection.iter().any(|p| p == path);
        let selected_count = if use_selection { selection.len() } else { 1 };
        (use_selection, selected_count)
    };

    let is_conflicted = this
        .state
        .repos
        .iter()
        .find(|r| r.id == repo_id)
        .and_then(|r| match &r.status {
            Loadable::Ready(status) => {
                Some(status.unstaged.iter().chain(status.staged.iter()).any(|s| {
                    &s.path == path && s.kind == gitgpui_core::domain::FileStatusKind::Conflicted
                }))
            }
            _ => None,
        })
        .unwrap_or(false);

    let can_discard_worktree_changes = this
        .state
        .repos
        .iter()
        .find(|r| r.id == repo_id)
        .and_then(|r| match &r.status {
            Loadable::Ready(status) => {
                let selected = {
                    let pane = this.details_pane.read(cx);
                    let selection = pane
                        .status_multi_selection
                        .get(&repo_id)
                        .map(|sel| match area {
                            DiffArea::Unstaged => sel.unstaged.as_slice(),
                            DiffArea::Staged => sel.staged.as_slice(),
                        })
                        .unwrap_or(&[]);

                    if use_selection {
                        selection
                    } else {
                        std::slice::from_ref(path)
                    }
                };

                if selected.is_empty() {
                    return Some(false);
                }

                let mut conflicted: HashSet<&std::path::Path> = HashSet::new();
                for entry in status.unstaged.iter().chain(status.staged.iter()) {
                    if entry.kind == gitgpui_core::domain::FileStatusKind::Conflicted {
                        conflicted.insert(entry.path.as_path());
                    }
                }

                if area == DiffArea::Unstaged {
                    if conflicted.is_empty() {
                        return Some(true);
                    }
                    return Some(selected.iter().all(|p| !conflicted.contains(p.as_path())));
                }

                let mut unstaged_paths: HashSet<&std::path::Path> =
                    HashSet::with_capacity(status.unstaged.len());
                for entry in &status.unstaged {
                    unstaged_paths.insert(entry.path.as_path());
                }

                let mut staged_added_paths: HashSet<&std::path::Path> = HashSet::new();
                for entry in &status.staged {
                    if entry.kind == gitgpui_core::domain::FileStatusKind::Added {
                        staged_added_paths.insert(entry.path.as_path());
                    }
                }

                Some(selected.iter().all(|p| {
                    let p = p.as_path();
                    !conflicted.contains(p)
                        && (unstaged_paths.contains(p) || staged_added_paths.contains(p))
                }))
            }
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
        let n = selected_count;
        items.push(ContextMenuItem::Entry {
            label: if use_selection {
                format!("Resolve selected using ours ({n})").into()
            } else {
                "Resolve using ours".into()
            },
            icon: Some("⇤".into()),
            shortcut: Some("O".into()),
            disabled: false,
            action: ContextMenuAction::CheckoutConflictSideSelectionOrPath {
                repo_id,
                area,
                path: path.clone(),
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
            action: ContextMenuAction::CheckoutConflictSideSelectionOrPath {
                repo_id,
                area,
                path: path.clone(),
                side: gitgpui_core::services::ConflictSide::Theirs,
            },
        });

        let can_manual = !use_selection;
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
                    path: path.clone(),
                    area: DiffArea::Unstaged,
                },
            },
        });
    } else {
        match area {
            DiffArea::Unstaged => items.push(ContextMenuItem::Entry {
                label: if use_selection {
                    format!("Stage ({})", selected_count).into()
                } else {
                    "Stage".into()
                },
                icon: Some("+".into()),
                shortcut: Some("S".into()),
                disabled: false,
                action: ContextMenuAction::StageSelectionOrPath {
                    repo_id,
                    area,
                    path: path.clone(),
                },
            }),
            DiffArea::Staged => items.push(ContextMenuItem::Entry {
                label: if use_selection {
                    format!("Unstage ({})", selected_count).into()
                } else {
                    "Unstage".into()
                },
                icon: Some("−".into()),
                shortcut: Some("U".into()),
                disabled: false,
                action: ContextMenuAction::UnstageSelectionOrPath {
                    repo_id,
                    area,
                    path: path.clone(),
                },
            }),
        };
    }

    items.push(ContextMenuItem::Entry {
        label: if use_selection {
            format!("Discard ({})", selected_count).into()
        } else {
            "Discard changes".into()
        },
        icon: Some("↺".into()),
        shortcut: Some("D".into()),
        disabled: !can_discard_worktree_changes,
        action: ContextMenuAction::DiscardWorktreeChangesSelectionOrPath {
            repo_id,
            area,
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

    ContextMenuModel::new(items)
}
