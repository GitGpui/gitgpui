use super::*;

pub(super) fn model(repo_id: RepoId) -> ContextMenuModel {
    let mut items = vec![ContextMenuItem::Header("Submodules".into())];
    items.push(ContextMenuItem::Separator);
    items.push(ContextMenuItem::Entry {
        label: "Add submodule…".into(),
        icon: Some("+".into()),
        shortcut: None,
        disabled: false,
        action: ContextMenuAction::OpenPopover {
            kind: PopoverKind::SubmoduleAddPrompt { repo_id },
        },
    });
    items.push(ContextMenuItem::Entry {
        label: "Update submodules".into(),
        icon: Some("↻".into()),
        shortcut: None,
        disabled: false,
        action: ContextMenuAction::UpdateSubmodules { repo_id },
    });
    items.push(ContextMenuItem::Entry {
        label: "Open submodule…".into(),
        icon: Some("↗".into()),
        shortcut: None,
        disabled: false,
        action: ContextMenuAction::OpenPopover {
            kind: PopoverKind::SubmoduleOpenPicker { repo_id },
        },
    });
    items.push(ContextMenuItem::Entry {
        label: "Remove submodule…".into(),
        icon: Some("🗑".into()),
        shortcut: None,
        disabled: false,
        action: ContextMenuAction::OpenPopover {
            kind: PopoverKind::SubmoduleRemovePicker { repo_id },
        },
    });

    ContextMenuModel::new(items)
}
