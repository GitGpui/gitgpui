use super::*;

pub(super) fn model(this: &GitGpuiView) -> ContextMenuModel {
    let repo_id = this.active_repo_id();
    let disabled = repo_id.is_none();
    let repo_id = repo_id.unwrap_or(RepoId(0));

    ContextMenuModel::new(vec![
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
    ])
}
