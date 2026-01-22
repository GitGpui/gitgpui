use crate::msg::{Effect, Msg};
use gitgpui_core::domain::{Diff, RepoSpec};
use gitgpui_core::services::{GitBackend, GitRepository};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, mpsc};

use super::RepoId;
use super::executor::TaskExecutor;

pub(super) fn schedule_effect(
    executor: &TaskExecutor,
    backend: &Arc<dyn GitBackend>,
    repos: &HashMap<RepoId, Arc<dyn GitRepository>>,
    msg_tx: mpsc::Sender<Msg>,
    effect: Effect,
) {
    match effect {
        Effect::OpenRepo { repo_id, path } => {
            let backend = Arc::clone(backend);
            executor.spawn(move || {
                let spec = RepoSpec { workdir: path };
                match backend.open(&spec.workdir) {
                    Ok(repo) => {
                        let _ = msg_tx.send(Msg::RepoOpenedOk {
                            repo_id,
                            spec,
                            repo,
                        });
                    }
                    Err(error) => {
                        let _ = msg_tx.send(Msg::RepoOpenedErr {
                            repo_id,
                            spec,
                            error,
                        });
                    }
                }
            });
        }

        Effect::LoadBranches { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::BranchesLoaded {
                        repo_id,
                        result: repo.list_branches(),
                    });
                });
            }
        }

        Effect::LoadRemotes { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RemotesLoaded {
                        repo_id,
                        result: repo.list_remotes(),
                    });
                });
            }
        }

        Effect::LoadRemoteBranches { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RemoteBranchesLoaded {
                        repo_id,
                        result: repo.list_remote_branches(),
                    });
                });
            }
        }

        Effect::LoadStatus { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::StatusLoaded {
                        repo_id,
                        result: repo.status(),
                    });
                });
            }
        }

        Effect::LoadHeadBranch { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::HeadBranchLoaded {
                        repo_id,
                        result: repo.current_branch(),
                    });
                });
            }
        }

        Effect::LoadUpstreamDivergence { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::UpstreamDivergenceLoaded {
                        repo_id,
                        result: repo.upstream_divergence(),
                    });
                });
            }
        }

        Effect::LoadLog {
            repo_id,
            scope,
            limit,
            cursor,
        } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let cursor_ref = cursor.as_ref();
                    let _ = msg_tx.send(Msg::LogLoaded {
                        repo_id,
                        scope,
                        result: match scope {
                            gitgpui_core::domain::LogScope::CurrentBranch => {
                                repo.log_head_page(limit, cursor_ref)
                            }
                            gitgpui_core::domain::LogScope::AllBranches => {
                                repo.log_all_branches_page(limit, cursor_ref)
                            }
                        },
                    });
                });
            }
        }

        Effect::LoadTags { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::TagsLoaded {
                        repo_id,
                        result: repo.list_tags(),
                    });
                });
            }
        }

        Effect::LoadStashes { repo_id, limit } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let mut entries = repo.stash_list();
                    if let Ok(v) = &mut entries {
                        v.truncate(limit);
                    }
                    let _ = msg_tx.send(Msg::StashesLoaded {
                        repo_id,
                        result: entries,
                    });
                });
            }
        }

        Effect::LoadReflog { repo_id, limit } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::ReflogLoaded {
                        repo_id,
                        result: repo.reflog_head(limit),
                    });
                });
            }
        }

        Effect::LoadCommitDetails { repo_id, commit_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::CommitDetailsLoaded {
                        repo_id,
                        commit_id: commit_id.clone(),
                        result: repo.commit_details(&commit_id),
                    });
                });
            }
        }

        Effect::LoadDiff { repo_id, target } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let result = repo
                        .diff_unified(&target)
                        .map(|text| Diff::from_unified(target.clone(), &text));
                    let _ = msg_tx.send(Msg::DiffLoaded {
                        repo_id,
                        target,
                        result,
                    });
                });
            }
        }

        Effect::LoadDiffFile { repo_id, target } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let result = repo.diff_file_text(&target);
                    let _ = msg_tx.send(Msg::DiffFileLoaded {
                        repo_id,
                        target,
                        result,
                    });
                });
            }
        }

        Effect::CheckoutBranch { repo_id, name } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.checkout_branch(&name),
                    });
                });
            }
        }

        Effect::CheckoutCommit { repo_id, commit_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.checkout_commit(&commit_id),
                    });
                });
            }
        }

        Effect::CherryPickCommit { repo_id, commit_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.cherry_pick(&commit_id),
                    });
                });
            }
        }

        Effect::RevertCommit { repo_id, commit_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.revert(&commit_id),
                    });
                });
            }
        }

        Effect::CreateBranch { repo_id, name } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let target = gitgpui_core::domain::CommitId("HEAD".to_string());
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.create_branch(&name, &target),
                    });
                });
            }
        }

        Effect::StagePath { repo_id, path } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let path_ref: &Path = &path;
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.stage(&[path_ref]),
                    });
                });
            }
        }

        Effect::StagePaths { repo_id, paths } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let mut unique = paths;
                    unique.sort();
                    unique.dedup();
                    let refs = unique.iter().map(|p| p.as_path()).collect::<Vec<_>>();
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.stage(&refs),
                    });
                });
            }
        }

        Effect::UnstagePath { repo_id, path } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let path_ref: &Path = &path;
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.unstage(&[path_ref]),
                    });
                });
            }
        }

        Effect::UnstagePaths { repo_id, paths } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let mut unique = paths;
                    unique.sort();
                    unique.dedup();
                    let refs = unique.iter().map(|p| p.as_path()).collect::<Vec<_>>();
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.unstage(&refs),
                    });
                });
            }
        }

        Effect::Commit { repo_id, message } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::CommitFinished {
                        repo_id,
                        result: repo.commit(&message),
                    });
                });
            }
        }

        Effect::FetchAll { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::FetchAll,
                        result: repo.fetch_all_with_output(),
                    });
                });
            }
        }

        Effect::Pull { repo_id, mode } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::Pull { mode },
                        result: repo.pull_with_output(mode),
                    });
                });
            }
        }

        Effect::PullBranch {
            repo_id,
            remote,
            branch,
        } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::PullBranch {
                            remote: remote.clone(),
                            branch: branch.clone(),
                        },
                        result: repo.pull_branch_with_output(&remote, &branch),
                    });
                });
            }
        }

        Effect::MergeRef { repo_id, reference } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::MergeRef {
                            reference: reference.clone(),
                        },
                        result: repo.merge_ref_with_output(&reference),
                    });
                });
            }
        }

        Effect::Push { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::Push,
                        result: repo.push_with_output(),
                    });
                });
            }
        }

        Effect::CheckoutConflictSide {
            repo_id,
            path,
            side,
        } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let result = repo.checkout_conflict_side(&path, side);
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::CheckoutConflict {
                            path: path.clone(),
                            side,
                        },
                        result,
                    });
                });
            }
        }

        Effect::Stash {
            repo_id,
            message,
            include_untracked,
        } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.stash_create(&message, include_untracked),
                    });
                });
            }
        }

        Effect::ApplyStash { repo_id, index } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.stash_apply(index),
                    });
                });
            }
        }

        Effect::DropStash { repo_id, index } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.stash_drop(index),
                    });
                });
            }
        }
    }
}
