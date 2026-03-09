use super::*;

pub(super) fn model(
    _this: &PopoverHost,
    repo_id: RepoId,
    section: BranchSection,
) -> ContextMenuModel {
    model_for_section(repo_id, section)
}

fn model_for_section(repo_id: RepoId, section: BranchSection) -> ContextMenuModel {
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
        action: Box::new(ContextMenuAction::OpenPopover {
            kind: PopoverKind::BranchPicker,
        }),
    });

    if section == BranchSection::Remote {
        items.push(ContextMenuItem::Entry {
            label: "Add remote…".into(),
            icon: Some("+".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::OpenPopover {
                kind: PopoverKind::remote(repo_id, RemotePopoverKind::AddPrompt),
            }),
        });
        items.push(ContextMenuItem::Entry {
            label: "Fetch all".into(),
            icon: Some("↓".into()),
            shortcut: Some("F".into()),
            disabled: false,
            action: Box::new(ContextMenuAction::FetchAll { repo_id }),
        });
        items.push(ContextMenuItem::Entry {
            label: "Prune merged branches".into(),
            icon: Some("🧹".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::PruneMergedBranches { repo_id }),
        });
        items.push(ContextMenuItem::Entry {
            label: "Prune local tags".into(),
            icon: Some("🏷".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::PruneLocalTags { repo_id }),
        });
        items.push(ContextMenuItem::Separator);
        for (label, kind) in [
            ("Edit fetch URL…", RemoteUrlKind::Fetch),
            ("Edit push URL…", RemoteUrlKind::Push),
        ] {
            items.push(ContextMenuItem::Entry {
                label: label.into(),
                icon: Some("✎".into()),
                shortcut: None,
                disabled: false,
                action: Box::new(ContextMenuAction::OpenPopover {
                    kind: PopoverKind::remote(repo_id, RemotePopoverKind::UrlPicker { kind }),
                }),
            });
        }
        items.push(ContextMenuItem::Entry {
            label: "Remove remote…".into(),
            icon: Some("🗑".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::OpenPopover {
                kind: PopoverKind::remote(repo_id, RemotePopoverKind::RemovePicker),
            }),
        });
        items.push(ContextMenuItem::Separator);
        items.push(ContextMenuItem::Entry {
            label: "Delete remote branch…".into(),
            icon: Some("🗑".into()),
            shortcut: None,
            disabled: false,
            action: Box::new(ContextMenuAction::OpenPopover {
                kind: PopoverKind::remote(
                    repo_id,
                    RemotePopoverKind::BranchDeletePicker { remote: None },
                ),
            }),
        });
    }

    ContextMenuModel::new(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action_for_label<'a>(model: &'a ContextMenuModel, label: &str) -> &'a ContextMenuAction {
        model
            .items
            .iter()
            .find_map(|item| match item {
                ContextMenuItem::Entry {
                    label: entry_label,
                    action,
                    ..
                } if entry_label.as_ref() == label => Some(action.as_ref()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected entry with label {label}"))
    }

    #[test]
    fn remote_section_routes_url_edits_through_picker_popovers() {
        let repo_id = RepoId(11);
        let model = super::model_for_section(repo_id, BranchSection::Remote);

        match action_for_label(&model, "Edit fetch URL…") {
            ContextMenuAction::OpenPopover {
                kind:
                    PopoverKind::Repo {
                        repo_id: action_repo_id,
                        kind:
                            RepoPopoverKind::Remote(RemotePopoverKind::UrlPicker { kind: action_kind }),
                    },
            } => {
                assert_eq!(*action_repo_id, repo_id);
                assert_eq!(action_kind, &RemoteUrlKind::Fetch);
            }
            _ => panic!("expected fetch URL picker popover action"),
        }

        match action_for_label(&model, "Edit push URL…") {
            ContextMenuAction::OpenPopover {
                kind:
                    PopoverKind::Repo {
                        repo_id: action_repo_id,
                        kind:
                            RepoPopoverKind::Remote(RemotePopoverKind::UrlPicker { kind: action_kind }),
                    },
            } => {
                assert_eq!(*action_repo_id, repo_id);
                assert_eq!(action_kind, &RemoteUrlKind::Push);
            }
            _ => panic!("expected push URL picker popover action"),
        }
    }

    #[test]
    fn remote_section_routes_remove_remote_through_picker_popover() {
        let repo_id = RepoId(7);
        let model = super::model_for_section(repo_id, BranchSection::Remote);

        match action_for_label(&model, "Remove remote…") {
            ContextMenuAction::OpenPopover {
                kind:
                    PopoverKind::Repo {
                        repo_id: action_repo_id,
                        kind: RepoPopoverKind::Remote(RemotePopoverKind::RemovePicker),
                    },
            } => {
                assert_eq!(*action_repo_id, repo_id);
            }
            _ => panic!("expected remove remote picker popover action"),
        }
    }
}
