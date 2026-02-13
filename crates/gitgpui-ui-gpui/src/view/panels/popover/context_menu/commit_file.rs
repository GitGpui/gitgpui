use super::*;

pub(super) fn model(
    _this: &PopoverHost,
    repo_id: RepoId,
    commit_id: &CommitId,
    path: &std::path::PathBuf,
) -> ContextMenuModel {
    let mut items = vec![ContextMenuItem::Header(
        path.file_name()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string())
            .into(),
    )];
    items.push(ContextMenuItem::Label(path.display().to_string().into()));
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
                path: Some(path.clone()),
            },
        },
    });
    items.push(ContextMenuItem::Entry {
        label: "Open file".into(),
        icon: Some("🗎".into()),
        shortcut: None,
        disabled: false,
        action: ContextMenuAction::OpenFile {
            repo_id,
            path: path.clone(),
        },
    });
    items.push(ContextMenuItem::Entry {
        label: "Open file location".into(),
        icon: Some("📂".into()),
        shortcut: None,
        disabled: false,
        action: ContextMenuAction::OpenFileLocation {
            repo_id,
            path: path.clone(),
        },
    });
    items.push(ContextMenuItem::Entry {
        label: "File history".into(),
        icon: Some("⟲".into()),
        shortcut: Some("H".into()),
        disabled: false,
        action: ContextMenuAction::OpenPopover {
            kind: PopoverKind::FileHistory {
                repo_id,
                path: path.clone(),
            },
        },
    });
    items.push(ContextMenuItem::Entry {
        label: "Blame (this commit)".into(),
        icon: Some("≡".into()),
        shortcut: Some("B".into()),
        disabled: false,
        action: ContextMenuAction::OpenPopover {
            kind: PopoverKind::Blame {
                repo_id,
                path: path.clone(),
                rev: Some(commit_id.as_ref().to_string()),
            },
        },
    });
    items.push(ContextMenuItem::Entry {
        label: "Copy path".into(),
        icon: Some("⧉".into()),
        shortcut: Some("C".into()),
        disabled: false,
        action: ContextMenuAction::CopyText {
            text: path.display().to_string(),
        },
    });

    ContextMenuModel::new(items)
}
