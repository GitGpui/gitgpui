use super::*;

pub(super) fn model(this: &GitGpuiView, repo_id: RepoId, commit_id: &CommitId) -> ContextMenuModel {
    let sha = commit_id.as_ref().to_string();
    let short: SharedString = sha.get(0..8).unwrap_or(&sha).to_string().into();

    let commit_summary = this
        .active_repo()
        .and_then(|r| match &r.log {
            Loadable::Ready(page) => page
                .commits
                .iter()
                .find(|c| c.id == *commit_id)
                .map(|c| format!("{} — {}", c.author, c.summary)),
            _ => None,
        })
        .unwrap_or_default();

    let mut items = vec![ContextMenuItem::Header(format!("Commit {short}").into())];
    if !commit_summary.is_empty() {
        items.push(ContextMenuItem::Label(commit_summary.into()));
    }
    items.push(ContextMenuItem::Separator);
    items.push(ContextMenuItem::Entry {
        label: "Open diff".into(),
        icon: Some("↗".into()),
        shortcut: Some("Enter".into()),
        disabled: false,
        action: ContextMenuAction::SelectDiff {
            repo_id,
            target: DiffTarget::Commit {
                commit_id: commit_id.clone(),
                path: None,
            },
        },
    });
    items.push(ContextMenuItem::Entry {
        label: "Add tag…".into(),
        icon: Some("🏷".into()),
        shortcut: Some("T".into()),
        disabled: false,
        action: ContextMenuAction::OpenPopover {
            kind: PopoverKind::CreateTagPrompt {
                repo_id,
                target: sha.clone(),
            },
        },
    });
    items.push(ContextMenuItem::Entry {
        label: "Checkout (detached)".into(),
        icon: Some("⎇".into()),
        shortcut: Some("D".into()),
        disabled: false,
        action: ContextMenuAction::CheckoutCommit {
            repo_id,
            commit_id: commit_id.clone(),
        },
    });
    items.push(ContextMenuItem::Entry {
        label: "Cherry-pick".into(),
        icon: Some("⇡".into()),
        shortcut: Some("P".into()),
        disabled: false,
        action: ContextMenuAction::CherryPickCommit {
            repo_id,
            commit_id: commit_id.clone(),
        },
    });
    items.push(ContextMenuItem::Entry {
        label: "Revert".into(),
        icon: Some("↶".into()),
        shortcut: Some("R".into()),
        disabled: false,
        action: ContextMenuAction::RevertCommit {
            repo_id,
            commit_id: commit_id.clone(),
        },
    });

    items.push(ContextMenuItem::Separator);
    for (label, icon, mode) in [
        ("Reset (--soft) to here", "↺", ResetMode::Soft),
        ("Reset (--mixed) to here", "↺", ResetMode::Mixed),
        ("Reset (--hard) to here", "↺", ResetMode::Hard),
    ] {
        items.push(ContextMenuItem::Entry {
            label: label.into(),
            icon: Some(icon.into()),
            shortcut: None,
            disabled: false,
            action: ContextMenuAction::OpenPopover {
                kind: PopoverKind::ResetPrompt {
                    repo_id,
                    target: sha.clone(),
                    mode,
                },
            },
        });
    }

    ContextMenuModel::new(items)
}
