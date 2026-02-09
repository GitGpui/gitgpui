use super::*;

pub(super) fn model(this: &GitGpuiView, repo_id: RepoId, src_ix: usize) -> ContextMenuModel {
    let mut items = vec![ContextMenuItem::Header("Hunk".into())];
    items.push(ContextMenuItem::Separator);

    let (disabled, label, icon, shortcut) = match this
        .state
        .repos
        .iter()
        .find(|r| r.id == repo_id)
        .and_then(|r| r.diff_target.as_ref())
    {
        Some(DiffTarget::WorkingTree { area, .. }) => match area {
            DiffArea::Unstaged => (false, "Stage hunk", "+", Some("S")),
            DiffArea::Staged => (false, "Unstage hunk", "−", Some("U")),
        },
        _ => (true, "Stage/Unstage hunk", "+", None::<&'static str>),
    };

    items.push(ContextMenuItem::Entry {
        label: label.into(),
        icon: Some(icon.into()),
        shortcut: shortcut.map(Into::into),
        disabled,
        action: match this
            .state
            .repos
            .iter()
            .find(|r| r.id == repo_id)
            .and_then(|r| r.diff_target.as_ref())
        {
            Some(DiffTarget::WorkingTree {
                area: DiffArea::Staged,
                ..
            }) => ContextMenuAction::UnstageHunk { repo_id, src_ix },
            _ => ContextMenuAction::StageHunk { repo_id, src_ix },
        },
    });

    ContextMenuModel::new(items)
}
