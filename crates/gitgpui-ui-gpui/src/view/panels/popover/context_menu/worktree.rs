use super::*;

pub(super) fn model(repo_id: RepoId, path: &std::path::PathBuf) -> ContextMenuModel {
    let mut items = vec![ContextMenuItem::Header("Worktree".into())];
    items.push(ContextMenuItem::Label(path.display().to_string().into()));
    items.push(ContextMenuItem::Separator);
    items.push(ContextMenuItem::Entry {
        label: "Open in new tab".into(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_includes_open_in_new_tab() {
        let repo_id = RepoId(1);
        let path = std::path::PathBuf::from("/tmp/worktree");
        let model = model(repo_id, &path);

        let open_action = model
            .items
            .iter()
            .find_map(|item| match item {
                ContextMenuItem::Entry { label, action, .. }
                    if label.to_string() == "Open in new tab" =>
                {
                    Some(action.clone())
                }
                _ => None,
            })
            .expect("expected Open in new tab entry");

        assert!(matches!(
            open_action,
            ContextMenuAction::OpenRepo { path: open_path } if open_path == path
        ));
    }
}
