use super::*;
use crate::view::fingerprint as view_fingerprint;
use std::hash::{Hash, Hasher};

pub(super) fn notify_fingerprint(state: &AppState, popover: &PopoverKind) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hash_popover_kind(popover, &mut hasher);

    match popover {
        PopoverKind::CloneRepo => match &state.clone {
            None => 0u8.hash(&mut hasher),
            Some(clone) => {
                1u8.hash(&mut hasher);
                clone.seq.hash(&mut hasher);
                clone.url.hash(&mut hasher);
                clone.dest.hash(&mut hasher);
                match &clone.status {
                    CloneOpStatus::Running => 0u8.hash(&mut hasher),
                    CloneOpStatus::FinishedOk => 1u8.hash(&mut hasher),
                    CloneOpStatus::FinishedErr(err) => {
                        2u8.hash(&mut hasher);
                        err.hash(&mut hasher);
                    }
                }
            }
        },
        PopoverKind::RepoPicker => {
            state.active_repo.hash(&mut hasher);
            state.repos.len().hash(&mut hasher);
            // Repo picker list is usually small; hashing all ids+workdirs is fine and avoids stale lists.
            for repo in &state.repos {
                repo.id.hash(&mut hasher);
                repo.spec.workdir.hash(&mut hasher);
                view_fingerprint::hash_loadable_kind(&repo.open, &mut hasher);
            }
        }
        PopoverKind::Settings | PopoverKind::AppMenu => {
            // Mostly local UI state; depend only on whether a repo is active/open.
            state.active_repo.hash(&mut hasher);
            if let Some(repo) = repo_for_popover(state, popover) {
                view_fingerprint::hash_loadable_kind(&repo.open, &mut hasher);
            }
        }
        _ => {
            if let Some(repo) = repo_for_popover(state, popover) {
                hash_repo_for_popover(repo, popover, &mut hasher);
            } else {
                state.active_repo.hash(&mut hasher);
            }
        }
    }

    hasher.finish()
}

fn repo_for_popover<'a>(state: &'a AppState, popover: &PopoverKind) -> Option<&'a RepoState> {
    let repo_id = match popover {
        PopoverKind::RepoPicker | PopoverKind::CloneRepo | PopoverKind::Settings => None,

        // Popovers that implicitly use the currently active repo.
        PopoverKind::BranchPicker
        | PopoverKind::CreateBranch
        | PopoverKind::StashPrompt
        | PopoverKind::PullPicker
        | PopoverKind::PushPicker
        | PopoverKind::AppMenu
        | PopoverKind::DiffHunks
        | PopoverKind::HistoryColumnSettings => state.active_repo,

        // Popovers that carry an explicit repo id.
        PopoverKind::ResetPrompt { repo_id, .. }
        | PopoverKind::RebasePrompt { repo_id }
        | PopoverKind::CreateTagPrompt { repo_id, .. }
        | PopoverKind::RemoteAddPrompt { repo_id }
        | PopoverKind::RemoteUrlPicker { repo_id, .. }
        | PopoverKind::RemoteRemovePicker { repo_id }
        | PopoverKind::RemoteBranchDeletePicker { repo_id, .. }
        | PopoverKind::RemoteEditUrlPrompt { repo_id, .. }
        | PopoverKind::RemoteRemoveConfirm { repo_id, .. }
        | PopoverKind::RemoteMenu { repo_id, .. }
        | PopoverKind::WorktreeSectionMenu { repo_id }
        | PopoverKind::WorktreeMenu { repo_id, .. }
        | PopoverKind::SubmoduleSectionMenu { repo_id }
        | PopoverKind::SubmoduleMenu { repo_id, .. }
        | PopoverKind::WorktreeAddPrompt { repo_id }
        | PopoverKind::WorktreeOpenPicker { repo_id }
        | PopoverKind::WorktreeRemovePicker { repo_id }
        | PopoverKind::WorktreeRemoveConfirm { repo_id, .. }
        | PopoverKind::SubmoduleAddPrompt { repo_id }
        | PopoverKind::SubmoduleOpenPicker { repo_id }
        | PopoverKind::SubmoduleRemovePicker { repo_id }
        | PopoverKind::SubmoduleRemoveConfirm { repo_id, .. }
        | PopoverKind::FileHistory { repo_id, .. }
        | PopoverKind::Blame { repo_id, .. }
        | PopoverKind::PushSetUpstreamPrompt { repo_id, .. }
        | PopoverKind::ForcePushConfirm { repo_id }
        | PopoverKind::ForceDeleteBranchConfirm { repo_id, .. }
        | PopoverKind::DeleteRemoteBranchConfirm { repo_id, .. }
        | PopoverKind::DiscardChangesConfirm { repo_id, .. }
        | PopoverKind::PullReconcilePrompt { repo_id }
        | PopoverKind::DiffHunkMenu { repo_id, .. }
        | PopoverKind::DiffEditorMenu { repo_id, .. }
        | PopoverKind::CommitMenu { repo_id, .. }
        | PopoverKind::StatusFileMenu { repo_id, .. }
        | PopoverKind::BranchMenu { repo_id, .. }
        | PopoverKind::BranchSectionMenu { repo_id, .. }
        | PopoverKind::CommitFileMenu { repo_id, .. }
        | PopoverKind::TagMenu { repo_id, .. }
        | PopoverKind::HistoryBranchFilter { repo_id } => Some(*repo_id),
    }?;

    state.repos.iter().find(|r| r.id == repo_id)
}

fn hash_repo_for_popover<H: Hasher>(repo: &RepoState, popover: &PopoverKind, hasher: &mut H) {
    view_fingerprint::hash_loadable_kind(&repo.open, hasher);

    match popover {
        PopoverKind::BranchPicker
        | PopoverKind::CreateBranch
        | PopoverKind::BranchMenu { .. }
        | PopoverKind::BranchSectionMenu { .. }
        | PopoverKind::ForceDeleteBranchConfirm { .. }
        | PopoverKind::PushSetUpstreamPrompt { .. } => {
            repo.head_branch_rev.hash(hasher);
            repo.branches_rev.hash(hasher);
            repo.remote_branches_rev.hash(hasher);
            repo.tags_rev.hash(hasher);
        }

        PopoverKind::RemoteAddPrompt { .. }
        | PopoverKind::RemoteUrlPicker { .. }
        | PopoverKind::RemoteRemovePicker { .. }
        | PopoverKind::RemoteBranchDeletePicker { .. }
        | PopoverKind::RemoteEditUrlPrompt { .. }
        | PopoverKind::RemoteRemoveConfirm { .. }
        | PopoverKind::RemoteMenu { .. }
        | PopoverKind::DeleteRemoteBranchConfirm { .. } => {
            repo.remotes_rev.hash(hasher);
            repo.remote_branches_rev.hash(hasher);
        }

        PopoverKind::WorktreeSectionMenu { .. }
        | PopoverKind::WorktreeMenu { .. }
        | PopoverKind::WorktreeAddPrompt { .. }
        | PopoverKind::WorktreeOpenPicker { .. }
        | PopoverKind::WorktreeRemovePicker { .. }
        | PopoverKind::WorktreeRemoveConfirm { .. } => {
            repo.worktrees_rev.hash(hasher);
        }

        PopoverKind::SubmoduleSectionMenu { .. }
        | PopoverKind::SubmoduleMenu { .. }
        | PopoverKind::SubmoduleAddPrompt { .. }
        | PopoverKind::SubmoduleOpenPicker { .. }
        | PopoverKind::SubmoduleRemovePicker { .. }
        | PopoverKind::SubmoduleRemoveConfirm { .. } => {
            repo.submodules_rev.hash(hasher);
        }

        PopoverKind::StashPrompt => {
            repo.stashes_rev.hash(hasher);
            view_fingerprint::hash_loadable_arc(&repo.status, hasher);
        }

        PopoverKind::FileHistory { .. } => {
            repo.file_history_path.hash(hasher);
            view_fingerprint::hash_loadable_arc(&repo.file_history, hasher);
        }

        PopoverKind::Blame { .. } => {
            repo.blame_path.hash(hasher);
            repo.blame_rev.hash(hasher);
            view_fingerprint::hash_loadable_arc(&repo.blame, hasher);
        }

        PopoverKind::DiffHunks
        | PopoverKind::DiffHunkMenu { .. }
        | PopoverKind::DiffEditorMenu { .. }
        | PopoverKind::DiscardChangesConfirm { .. } => {
            repo.diff_rev.hash(hasher);
            repo.diff_target
                .as_ref()
                .map(|t| view_fingerprint::hash_diff_target(t, hasher));
            view_fingerprint::hash_loadable_arc(&repo.diff, hasher);
            repo.diff_file_rev.hash(hasher);
            view_fingerprint::hash_loadable_kind(&repo.diff_file, hasher);
            view_fingerprint::hash_loadable_kind(&repo.diff_file_image, hasher);

            // Working tree diff popovers need status for file-kind/conflict decisions.
            if matches!(repo.diff_target, Some(DiffTarget::WorkingTree { .. })) {
                view_fingerprint::hash_loadable_arc(&repo.status, hasher);
            }
        }

        PopoverKind::HistoryBranchFilter { .. } => {
            repo.history_scope.hash(hasher);
            repo.branches_rev.hash(hasher);
            repo.remote_branches_rev.hash(hasher);
            repo.tags_rev.hash(hasher);
        }

        PopoverKind::PullPicker
        | PopoverKind::PushPicker
        | PopoverKind::PullReconcilePrompt { .. }
        | PopoverKind::ForcePushConfirm { .. } => {
            repo.head_branch_rev.hash(hasher);
            repo.remotes_rev.hash(hasher);
            repo.remote_branches_rev.hash(hasher);
        }

        // Most prompt-style popovers don't require live state updates.
        PopoverKind::ResetPrompt { .. }
        | PopoverKind::RebasePrompt { .. }
        | PopoverKind::CreateTagPrompt { .. }
        | PopoverKind::CommitMenu { .. }
        | PopoverKind::CommitFileMenu { .. }
        | PopoverKind::StatusFileMenu { .. }
        | PopoverKind::TagMenu { .. }
        | PopoverKind::HistoryColumnSettings
        | PopoverKind::AppMenu
        | PopoverKind::Settings
        | PopoverKind::RepoPicker
        | PopoverKind::CloneRepo => {}
    }
}

fn hash_popover_kind<H: Hasher>(kind: &PopoverKind, hasher: &mut H) {
    match kind {
        PopoverKind::RepoPicker => 0u8.hash(hasher),
        PopoverKind::BranchPicker => 1u8.hash(hasher),
        PopoverKind::CreateBranch => 2u8.hash(hasher),
        PopoverKind::StashPrompt => 3u8.hash(hasher),
        PopoverKind::CloneRepo => 4u8.hash(hasher),
        PopoverKind::Settings => 5u8.hash(hasher),

        PopoverKind::ResetPrompt {
            repo_id,
            target,
            mode,
        } => {
            6u8.hash(hasher);
            repo_id.hash(hasher);
            target.hash(hasher);
            hash_reset_mode(*mode, hasher);
        }
        PopoverKind::RebasePrompt { repo_id } => {
            7u8.hash(hasher);
            repo_id.hash(hasher);
        }
        PopoverKind::CreateTagPrompt { repo_id, target } => {
            8u8.hash(hasher);
            repo_id.hash(hasher);
            target.hash(hasher);
        }
        PopoverKind::RemoteAddPrompt { repo_id } => {
            9u8.hash(hasher);
            repo_id.hash(hasher);
        }
        PopoverKind::RemoteUrlPicker { repo_id, kind } => {
            10u8.hash(hasher);
            repo_id.hash(hasher);
            hash_remote_url_kind(*kind, hasher);
        }
        PopoverKind::RemoteRemovePicker { repo_id } => {
            11u8.hash(hasher);
            repo_id.hash(hasher);
        }
        PopoverKind::RemoteBranchDeletePicker { repo_id, remote } => {
            12u8.hash(hasher);
            repo_id.hash(hasher);
            remote.hash(hasher);
        }
        PopoverKind::RemoteEditUrlPrompt {
            repo_id,
            name,
            kind,
        } => {
            13u8.hash(hasher);
            repo_id.hash(hasher);
            name.hash(hasher);
            hash_remote_url_kind(*kind, hasher);
        }
        PopoverKind::RemoteRemoveConfirm { repo_id, name } => {
            14u8.hash(hasher);
            repo_id.hash(hasher);
            name.hash(hasher);
        }
        PopoverKind::RemoteMenu { repo_id, name } => {
            15u8.hash(hasher);
            repo_id.hash(hasher);
            name.hash(hasher);
        }

        PopoverKind::WorktreeSectionMenu { repo_id } => {
            16u8.hash(hasher);
            repo_id.hash(hasher);
        }
        PopoverKind::WorktreeMenu { repo_id, path } => {
            17u8.hash(hasher);
            repo_id.hash(hasher);
            path.hash(hasher);
        }
        PopoverKind::SubmoduleSectionMenu { repo_id } => {
            18u8.hash(hasher);
            repo_id.hash(hasher);
        }
        PopoverKind::SubmoduleMenu { repo_id, path } => {
            19u8.hash(hasher);
            repo_id.hash(hasher);
            path.hash(hasher);
        }
        PopoverKind::WorktreeAddPrompt { repo_id } => {
            20u8.hash(hasher);
            repo_id.hash(hasher);
        }
        PopoverKind::WorktreeOpenPicker { repo_id } => {
            21u8.hash(hasher);
            repo_id.hash(hasher);
        }
        PopoverKind::WorktreeRemovePicker { repo_id } => {
            22u8.hash(hasher);
            repo_id.hash(hasher);
        }
        PopoverKind::WorktreeRemoveConfirm { repo_id, path } => {
            23u8.hash(hasher);
            repo_id.hash(hasher);
            path.hash(hasher);
        }
        PopoverKind::SubmoduleAddPrompt { repo_id } => {
            24u8.hash(hasher);
            repo_id.hash(hasher);
        }
        PopoverKind::SubmoduleOpenPicker { repo_id } => {
            25u8.hash(hasher);
            repo_id.hash(hasher);
        }
        PopoverKind::SubmoduleRemovePicker { repo_id } => {
            26u8.hash(hasher);
            repo_id.hash(hasher);
        }
        PopoverKind::SubmoduleRemoveConfirm { repo_id, path } => {
            27u8.hash(hasher);
            repo_id.hash(hasher);
            path.hash(hasher);
        }

        PopoverKind::FileHistory { repo_id, path } => {
            28u8.hash(hasher);
            repo_id.hash(hasher);
            path.hash(hasher);
        }
        PopoverKind::Blame { repo_id, path, rev } => {
            29u8.hash(hasher);
            repo_id.hash(hasher);
            path.hash(hasher);
            rev.hash(hasher);
        }

        PopoverKind::PushSetUpstreamPrompt { repo_id, remote } => {
            30u8.hash(hasher);
            repo_id.hash(hasher);
            remote.hash(hasher);
        }
        PopoverKind::ForcePushConfirm { repo_id } => {
            31u8.hash(hasher);
            repo_id.hash(hasher);
        }
        PopoverKind::ForceDeleteBranchConfirm { repo_id, name } => {
            32u8.hash(hasher);
            repo_id.hash(hasher);
            name.hash(hasher);
        }
        PopoverKind::DeleteRemoteBranchConfirm {
            repo_id,
            remote,
            branch,
        } => {
            33u8.hash(hasher);
            repo_id.hash(hasher);
            remote.hash(hasher);
            branch.hash(hasher);
        }
        PopoverKind::DiscardChangesConfirm {
            repo_id,
            area,
            path,
        } => {
            34u8.hash(hasher);
            repo_id.hash(hasher);
            hash_diff_area(*area, hasher);
            path.hash(hasher);
        }
        PopoverKind::PullReconcilePrompt { repo_id } => {
            35u8.hash(hasher);
            repo_id.hash(hasher);
        }
        PopoverKind::PullPicker => 36u8.hash(hasher),
        PopoverKind::PushPicker => 37u8.hash(hasher),
        PopoverKind::AppMenu => 38u8.hash(hasher),
        PopoverKind::DiffHunks => 39u8.hash(hasher),
        PopoverKind::DiffHunkMenu { repo_id, src_ix } => {
            40u8.hash(hasher);
            repo_id.hash(hasher);
            src_ix.hash(hasher);
        }
        PopoverKind::DiffEditorMenu {
            repo_id,
            area,
            path,
            hunks_count,
            lines_count,
            ..
        } => {
            41u8.hash(hasher);
            repo_id.hash(hasher);
            hash_diff_area(*area, hasher);
            path.hash(hasher);
            hunks_count.hash(hasher);
            lines_count.hash(hasher);
        }
        PopoverKind::CommitMenu { repo_id, commit_id } => {
            42u8.hash(hasher);
            repo_id.hash(hasher);
            commit_id.hash(hasher);
        }
        PopoverKind::StatusFileMenu {
            repo_id,
            area,
            path,
        } => {
            43u8.hash(hasher);
            repo_id.hash(hasher);
            hash_diff_area(*area, hasher);
            path.hash(hasher);
        }
        PopoverKind::BranchMenu {
            repo_id,
            section,
            name,
        } => {
            44u8.hash(hasher);
            repo_id.hash(hasher);
            hash_branch_section(*section, hasher);
            name.hash(hasher);
        }
        PopoverKind::BranchSectionMenu { repo_id, section } => {
            45u8.hash(hasher);
            repo_id.hash(hasher);
            hash_branch_section(*section, hasher);
        }
        PopoverKind::CommitFileMenu {
            repo_id,
            commit_id,
            path,
        } => {
            46u8.hash(hasher);
            repo_id.hash(hasher);
            commit_id.hash(hasher);
            path.hash(hasher);
        }
        PopoverKind::TagMenu { repo_id, commit_id } => {
            47u8.hash(hasher);
            repo_id.hash(hasher);
            commit_id.hash(hasher);
        }
        PopoverKind::HistoryBranchFilter { repo_id } => {
            48u8.hash(hasher);
            repo_id.hash(hasher);
        }
        PopoverKind::HistoryColumnSettings => 49u8.hash(hasher),
    }
}

fn hash_diff_area<H: Hasher>(area: DiffArea, hasher: &mut H) {
    match area {
        DiffArea::Staged => 0u8.hash(hasher),
        DiffArea::Unstaged => 1u8.hash(hasher),
    }
}

fn hash_branch_section<H: Hasher>(section: BranchSection, hasher: &mut H) {
    match section {
        BranchSection::Local => 0u8.hash(hasher),
        BranchSection::Remote => 1u8.hash(hasher),
    }
}

fn hash_remote_url_kind<H: Hasher>(kind: RemoteUrlKind, hasher: &mut H) {
    match kind {
        RemoteUrlKind::Fetch => 0u8.hash(hasher),
        RemoteUrlKind::Push => 1u8.hash(hasher),
    }
}

fn hash_reset_mode<H: Hasher>(mode: ResetMode, hasher: &mut H) {
    match mode {
        ResetMode::Soft => 0u8.hash(hasher),
        ResetMode::Mixed => 1u8.hash(hasher),
        ResetMode::Hard => 2u8.hash(hasher),
    }
}
