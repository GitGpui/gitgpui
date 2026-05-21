use super::*;

fn merge_active(repo: &RepoState) -> bool {
    matches!(&repo.merge_commit_message, Loadable::Ready(Some(_)))
}

fn repo_has_head_commit(repo: &RepoState) -> bool {
    match &repo.log {
        Loadable::Ready(page) => !page.commits.is_empty(),
        _ => true,
    }
}

fn can_amend(repo: Option<&RepoState>) -> bool {
    let Some(repo) = repo else {
        return false;
    };

    repo.commit_in_flight == 0
        && !merge_active(repo)
        && !matches!(repo.rebase_in_progress, Loadable::Ready(true))
        && repo_has_head_commit(repo)
}

pub(super) fn model(this: &PopoverHost, repo_id: RepoId) -> ContextMenuModel {
    let repo = this.state.repos.iter().find(|repo| repo.id == repo_id);
    model_for_state(
        repo,
        this.commit_amend_enabled,
        this.commit_push_after_enabled,
    )
}

fn model_for_state(
    repo: Option<&RepoState>,
    commit_amend_enabled: bool,
    commit_push_after_enabled: bool,
) -> ContextMenuModel {
    let check = |enabled: bool| enabled.then_some("icons/check.svg".into());

    ContextMenuModel::new(vec![
        ContextMenuItem::Header("Commit options".into()),
        ContextMenuItem::Separator,
        ContextMenuItem::Entry {
            label: "Amend previous commit".into(),
            icon: check(commit_amend_enabled),
            shortcut: Some("A".into()),
            disabled: !commit_amend_enabled && !can_amend(repo),
            action: Box::new(ContextMenuAction::SetCommitAmendEnabled {
                enabled: !commit_amend_enabled,
            }),
        },
        ContextMenuItem::Entry {
            label: "Push after commit".into(),
            icon: check(commit_push_after_enabled),
            shortcut: Some("P".into()),
            disabled: repo.is_none(),
            action: Box::new(ContextMenuAction::SetCommitPushAfterEnabled {
                enabled: !commit_push_after_enabled,
            }),
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_does_not_include_previous_commit_messages() {
        let model = model_for_state(None, false, false);

        assert!(!model.items.iter().any(|item| matches!(
            item,
            ContextMenuItem::Entry {
                action,
                ..
            } if matches!(action.as_ref(), ContextMenuAction::UseCommitMessage { .. })
        )));
        assert_eq!(model.items.len(), 4);
    }
}
