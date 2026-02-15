use super::*;

pub(super) fn model(
    this: &PopoverHost,
    repo_id: RepoId,
    path: &std::path::PathBuf,
) -> ContextMenuModel {
    let mut items = vec![ContextMenuItem::Header("Submodule".into())];
    items.push(ContextMenuItem::Label(path.display().to_string().into()));
    items.push(ContextMenuItem::Separator);

    let open_path = this
        .state
        .repos
        .iter()
        .find(|r| r.id == repo_id)
        .map(|r| r.spec.workdir.join(path));
    let open_disabled = open_path.is_none();
    items.push(ContextMenuItem::Entry {
        label: "Open".into(),
        icon: Some("↗".into()),
        shortcut: None,
        disabled: open_disabled,
        action: ContextMenuAction::OpenRepo {
            path: open_path.unwrap_or_default(),
        },
    });

    items.push(ContextMenuItem::Separator);
    items.push(ContextMenuItem::Entry {
        label: "Remove…".into(),
        icon: Some("🗑".into()),
        shortcut: None,
        disabled: false,
        action: ContextMenuAction::OpenPopover {
            kind: PopoverKind::SubmoduleRemoveConfirm {
                repo_id,
                path: path.clone(),
            },
        },
    });

    ContextMenuModel::new(items)
}

