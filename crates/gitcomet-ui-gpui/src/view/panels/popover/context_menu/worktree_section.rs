use super::*;

pub(super) fn model(repo_id: RepoId) -> ContextMenuModel {
    let mut items = vec![ContextMenuItem::Header("Worktrees".into())];
    items.push(ContextMenuItem::Separator);
    items.push(ContextMenuItem::Entry {
        label: "Add worktree…".into(),
        icon: Some("+".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::OpenPopover {
            kind: PopoverKind::WorktreeAddPrompt { repo_id },
        }),
    });
    items.push(ContextMenuItem::Entry {
        label: "Refresh worktrees".into(),
        icon: Some("↻".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::LoadWorktrees { repo_id }),
    });
    items.push(ContextMenuItem::Entry {
        label: "Open worktree…".into(),
        icon: Some("↗".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::OpenPopover {
            kind: PopoverKind::WorktreeOpenPicker { repo_id },
        }),
    });
    items.push(ContextMenuItem::Entry {
        label: "Remove worktree…".into(),
        icon: Some("🗑".into()),
        shortcut: None,
        disabled: false,
        action: Box::new(ContextMenuAction::OpenPopover {
            kind: PopoverKind::WorktreeRemovePicker { repo_id },
        }),
    });

    ContextMenuModel::new(items)
}
