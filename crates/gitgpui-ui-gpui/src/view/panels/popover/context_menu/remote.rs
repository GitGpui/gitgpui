use super::*;

pub(super) fn model(_this: &PopoverHost, repo_id: RepoId, name: &String) -> ContextMenuModel {
    let mut items = vec![ContextMenuItem::Header("Remote".into())];
    items.push(ContextMenuItem::Label(name.clone().into()));
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
            action: ContextMenuAction::OpenPopover {
                kind: PopoverKind::RemoteEditUrlPrompt {
                    repo_id,
                    name: name.clone(),
                    kind,
                },
            },
        });
    }

    items.push(ContextMenuItem::Separator);
    items.push(ContextMenuItem::Entry {
        label: "Remove remote…".into(),
        icon: Some("🗑".into()),
        shortcut: None,
        disabled: false,
        action: ContextMenuAction::OpenPopover {
            kind: PopoverKind::RemoteRemoveConfirm {
                repo_id,
                name: name.clone(),
            },
        },
    });

    ContextMenuModel::new(items)
}

