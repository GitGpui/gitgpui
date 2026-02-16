use super::*;

pub(super) fn model(repo_id: RepoId, path: &std::path::PathBuf) -> ContextMenuModel {
    let mut items = vec![ContextMenuItem::Header("Worktree".into())];
    items.push(ContextMenuItem::Label(path.display().to_string().into()));
    items.push(ContextMenuItem::Separator);
    items.push(ContextMenuItem::Entry {
        label: "Open".into(),
        icon: Some("↗".into()),
        shortcut: None,
        disabled: false,
        action: ContextMenuAction::OpenRepo { path: path.clone() },
    });
    items.push(ContextMenuItem::Separator);
    items.push(ContextMenuItem::Entry {
        label: "Remove…".into(),
        icon: Some("🗑".into()),
        shortcut: None,
        disabled: false,
        action: ContextMenuAction::OpenPopover {
            kind: PopoverKind::WorktreeRemoveConfirm {
                repo_id,
                path: path.clone(),
            },
        },
    });

    ContextMenuModel::new(items)
}
