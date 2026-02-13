use super::*;

pub(super) fn model(this: &PopoverHost) -> ContextMenuModel {
    let repo_id = this.active_repo_id();
    let disabled = repo_id.is_none();
    let repo_id = repo_id.unwrap_or(RepoId(0));

    ContextMenuModel::new(vec![
        ContextMenuItem::Header("Pull".into()),
        ContextMenuItem::Separator,
        ContextMenuItem::Entry {
            label: "Pull (default)".into(),
            icon: Some("↓".into()),
            shortcut: Some("Enter".into()),
            disabled,
            action: ContextMenuAction::Pull {
                repo_id,
                mode: PullMode::Default,
            },
        },
        ContextMenuItem::Entry {
            label: "Pull (fast-forward if possible)".into(),
            icon: Some("↓".into()),
            shortcut: Some("F".into()),
            disabled,
            action: ContextMenuAction::Pull {
                repo_id,
                mode: PullMode::FastForwardIfPossible,
            },
        },
        ContextMenuItem::Entry {
            label: "Pull (fast-forward only)".into(),
            icon: Some("↓".into()),
            shortcut: Some("O".into()),
            disabled,
            action: ContextMenuAction::Pull {
                repo_id,
                mode: PullMode::FastForwardOnly,
            },
        },
        ContextMenuItem::Entry {
            label: "Pull (rebase)".into(),
            icon: Some("↓".into()),
            shortcut: Some("R".into()),
            disabled,
            action: ContextMenuAction::Pull {
                repo_id,
                mode: PullMode::Rebase,
            },
        },
        ContextMenuItem::Separator,
        ContextMenuItem::Entry {
            label: "Fetch all".into(),
            icon: Some("↓".into()),
            shortcut: Some("A".into()),
            disabled,
            action: ContextMenuAction::FetchAll { repo_id },
        },
    ])
}
