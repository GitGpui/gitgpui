use super::*;

pub(super) fn model(this: &GitGpuiView, repo_id: RepoId, commit_id: &CommitId) -> ContextMenuModel {
    let sha = commit_id.as_ref().to_string();
    let short: SharedString = sha.get(0..8).unwrap_or(&sha).to_string().into();

    let tags = this
        .state
        .repos
        .iter()
        .find(|r| r.id == repo_id)
        .and_then(|r| match &r.tags {
            Loadable::Ready(tags) => Some(tags.as_slice()),
            _ => None,
        })
        .unwrap_or(&[]);

    let mut items = vec![ContextMenuItem::Header(format!("Tags on {short}").into())];
    let mut tag_names = tags
        .iter()
        .filter(|t| t.target == *commit_id)
        .map(|t| t.name.clone())
        .collect::<Vec<_>>();
    tag_names.sort();

    if tag_names.is_empty() {
        items.push(ContextMenuItem::Label("No tags".into()));
        return ContextMenuModel::new(items);
    }

    items.push(ContextMenuItem::Separator);
    for name in tag_names {
        items.push(ContextMenuItem::Entry {
            label: format!("Delete tag {name}").into(),
            icon: Some("🗑".into()),
            shortcut: None,
            disabled: false,
            action: ContextMenuAction::DeleteTag { repo_id, name },
        });
    }

    ContextMenuModel::new(items)
}
