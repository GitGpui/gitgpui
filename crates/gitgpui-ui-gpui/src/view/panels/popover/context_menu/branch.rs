use super::*;

pub(super) fn model(
    this: &PopoverHost,
    repo_id: RepoId,
    section: BranchSection,
    name: &String,
) -> ContextMenuModel {
    let header: SharedString = match section {
        BranchSection::Local => "Local branch".into(),
        BranchSection::Remote => "Remote branch".into(),
    };
    let mut items = vec![ContextMenuItem::Header(header)];
    items.push(ContextMenuItem::Label(name.clone().into()));
    items.push(ContextMenuItem::Separator);

    let is_current_branch = this
        .state
        .repos
        .iter()
        .find(|r| r.id == repo_id)
        .and_then(|r| match &r.head_branch {
            Loadable::Ready(b) => Some(b == name),
            _ => None,
        })
        .unwrap_or(false);

    items.push(ContextMenuItem::Entry {
        label: "Checkout".into(),
        icon: Some("⎇".into()),
        shortcut: Some("Enter".into()),
        disabled: false,
        action: match section {
            BranchSection::Local => ContextMenuAction::CheckoutBranch {
                repo_id,
                name: name.clone(),
            },
            BranchSection::Remote => {
                if let Some((remote, branch)) = name.split_once('/') {
                    ContextMenuAction::CheckoutRemoteBranch {
                        repo_id,
                        remote: remote.to_string(),
                        name: branch.to_string(),
                    }
                } else {
                    ContextMenuAction::CheckoutBranch {
                        repo_id,
                        name: name.clone(),
                    }
                }
            }
        },
    });
    items.push(ContextMenuItem::Entry {
        label: "Copy name".into(),
        icon: Some("⧉".into()),
        shortcut: Some("C".into()),
        disabled: false,
        action: ContextMenuAction::CopyText { text: name.clone() },
    });

    if section == BranchSection::Local {
        items.push(ContextMenuItem::Separator);
        items.push(ContextMenuItem::Entry {
            label: "Delete branch".into(),
            icon: Some("🗑".into()),
            shortcut: None,
            disabled: is_current_branch,
            action: ContextMenuAction::DeleteBranch {
                repo_id,
                name: name.clone(),
            },
        });
    }

    if section == BranchSection::Remote {
        items.push(ContextMenuItem::Separator);
        if let Some((remote, branch)) = name.split_once('/') {
            items.push(ContextMenuItem::Entry {
                label: "Pull into current".into(),
                icon: Some("↓".into()),
                shortcut: Some("P".into()),
                disabled: false,
                action: ContextMenuAction::PullBranch {
                    repo_id,
                    remote: remote.to_string(),
                    branch: branch.to_string(),
                },
            });
            items.push(ContextMenuItem::Entry {
                label: "Merge into current".into(),
                icon: Some("⇄".into()),
                shortcut: Some("M".into()),
                disabled: false,
                action: ContextMenuAction::MergeRef {
                    repo_id,
                    reference: name.clone(),
                },
            });
            items.push(ContextMenuItem::Separator);
            items.push(ContextMenuItem::Entry {
                label: "Delete remote branch…".into(),
                icon: Some("🗑".into()),
                shortcut: None,
                disabled: false,
                action: ContextMenuAction::OpenPopover {
                    kind: PopoverKind::DeleteRemoteBranchConfirm {
                        repo_id,
                        remote: remote.to_string(),
                        branch: branch.to_string(),
                    },
                },
            });
            items.push(ContextMenuItem::Separator);
        }
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
