use super::*;

pub(super) fn model(repo_id: RepoId) -> ContextMenuModel {
    let mut items = vec![ContextMenuItem::Header("Subtrees".into())];
    items.push(ContextMenuItem::Separator);
    items.push(ContextMenuItem::Entry {
        label: "Add subtree…".into(),
        icon: Some("icons/plus.svg".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::OpenPopover {
            kind: PopoverKind::subtree(repo_id, SubtreePopoverKind::AddPrompt),
        }),
    });
    items.push(ContextMenuItem::Entry {
        label: "Pull subtree…".into(),
        icon: Some("icons/pull.svg".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::OpenPopover {
            kind: PopoverKind::subtree(repo_id, SubtreePopoverKind::PullPicker),
        }),
    });
    items.push(ContextMenuItem::Entry {
        label: "Push subtree…".into(),
        icon: Some("icons/push.svg".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::OpenPopover {
            kind: PopoverKind::subtree(repo_id, SubtreePopoverKind::PushPicker),
        }),
    });
    items.push(ContextMenuItem::Entry {
        label: "Split subtree…".into(),
        icon: Some("icons/branch.svg".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::OpenPopover {
            kind: PopoverKind::subtree(repo_id, SubtreePopoverKind::SplitPicker),
        }),
    });
    items.push(ContextMenuItem::Entry {
        label: "Open subtree source repo…".into(),
        icon: Some("icons/open_external.svg".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::OpenPopover {
            kind: PopoverKind::subtree(repo_id, SubtreePopoverKind::OpenPicker),
        }),
    });
    items.push(ContextMenuItem::Entry {
        label: "Remove subtree…".into(),
        icon: Some("icons/trash.svg".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::OpenPopover {
            kind: PopoverKind::subtree(repo_id, SubtreePopoverKind::RemovePicker),
        }),
    });

    ContextMenuModel::new(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_does_not_include_reveal_subtree_entry() {
        let model = model(RepoId(1));

        assert!(model.items.iter().all(|item| {
            !matches!(
                item,
                ContextMenuItem::Entry { label, .. } if label.as_ref() == "Reveal subtree…"
            )
        }));
    }
}
