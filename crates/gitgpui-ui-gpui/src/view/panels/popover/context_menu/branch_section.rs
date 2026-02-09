use super::*;

pub(super) fn model(
    _this: &GitGpuiView,
    repo_id: RepoId,
    section: BranchSection,
) -> ContextMenuModel {
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
        action: ContextMenuAction::OpenPopover {
            kind: PopoverKind::BranchPicker,
        },
    });

    if section == BranchSection::Remote {
        items.push(ContextMenuItem::Entry {
            label: "Fetch all".into(),
            icon: Some("↓".into()),
            shortcut: Some("F".into()),
            disabled: false,
            action: ContextMenuAction::FetchAll { repo_id },
        });
    }

    ContextMenuModel::new(items)
}
