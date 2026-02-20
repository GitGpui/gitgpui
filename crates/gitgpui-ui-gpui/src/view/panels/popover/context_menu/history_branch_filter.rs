use super::*;

pub(super) fn model(repo_id: RepoId) -> ContextMenuModel {
    ContextMenuModel::new(vec![
        ContextMenuItem::Header("History scope".into()),
        ContextMenuItem::Separator,
        ContextMenuItem::Entry {
            label: "Current branch".into(),
            icon: Some("⎇".into()),
            shortcut: Some("C".into()),
            disabled: false,
            action: Box::new(ContextMenuAction::SetHistoryScope {
                repo_id,
                scope: gitgpui_core::domain::LogScope::CurrentBranch,
            }),
        },
        ContextMenuItem::Entry {
            label: "All branches".into(),
            icon: Some("∞".into()),
            shortcut: Some("A".into()),
            disabled: false,
            action: Box::new(ContextMenuAction::SetHistoryScope {
                repo_id,
                scope: gitgpui_core::domain::LogScope::AllBranches,
            }),
        },
    ])
}
