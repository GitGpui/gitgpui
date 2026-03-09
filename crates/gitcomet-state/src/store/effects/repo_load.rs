use crate::msg::Msg;
use gitcomet_core::domain::{Diff, DiffArea, DiffTarget, LogCursor, LogScope};
use gitcomet_core::error::{Error, ErrorKind};
use gitcomet_core::services::decode_utf8_optional;
use std::path::PathBuf;
use std::sync::mpsc;

use super::super::{RepoId, executor::TaskExecutor};
use super::util::{RepoMap, send_or_log, spawn_with_repo, spawn_with_repo_or_else};

fn missing_repo_error(repo_id: RepoId) -> Error {
    Error::new(ErrorKind::Backend(format!(
        "Repository handle not found for repo_id {}",
        repo_id.0
    )))
}

pub(super) fn schedule_load_branches(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo_or_else(
        executor,
        repos,
        repo_id,
        msg_tx,
        move |repo, msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::BranchesLoaded {
                    repo_id,
                    result: repo.list_branches(),
                }),
            );
        },
        move |msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::BranchesLoaded {
                    repo_id,
                    result: Err(missing_repo_error(repo_id)),
                }),
            );
        },
    );
}

pub(super) fn schedule_load_remotes(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo_or_else(
        executor,
        repos,
        repo_id,
        msg_tx,
        move |repo, msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::RemotesLoaded {
                    repo_id,
                    result: repo.list_remotes(),
                }),
            );
        },
        move |msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::RemotesLoaded {
                    repo_id,
                    result: Err(missing_repo_error(repo_id)),
                }),
            );
        },
    );
}

pub(super) fn schedule_load_remote_branches(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo_or_else(
        executor,
        repos,
        repo_id,
        msg_tx,
        move |repo, msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::RemoteBranchesLoaded {
                    repo_id,
                    result: repo.list_remote_branches(),
                }),
            );
        },
        move |msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::RemoteBranchesLoaded {
                    repo_id,
                    result: Err(missing_repo_error(repo_id)),
                }),
            );
        },
    );
}

pub(super) fn schedule_load_status(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo_or_else(
        executor,
        repos,
        repo_id,
        msg_tx,
        move |repo, msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::StatusLoaded {
                    repo_id,
                    result: repo.status(),
                }),
            );
        },
        move |msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::StatusLoaded {
                    repo_id,
                    result: Err(missing_repo_error(repo_id)),
                }),
            );
        },
    );
}

pub(super) fn schedule_load_head_branch(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo_or_else(
        executor,
        repos,
        repo_id,
        msg_tx,
        move |repo, msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::HeadBranchLoaded {
                    repo_id,
                    result: repo.current_branch(),
                }),
            );
        },
        move |msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::HeadBranchLoaded {
                    repo_id,
                    result: Err(missing_repo_error(repo_id)),
                }),
            );
        },
    );
}

pub(super) fn schedule_load_upstream_divergence(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo_or_else(
        executor,
        repos,
        repo_id,
        msg_tx,
        move |repo, msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::UpstreamDivergenceLoaded {
                    repo_id,
                    result: repo.upstream_divergence(),
                }),
            );
        },
        move |msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::UpstreamDivergenceLoaded {
                    repo_id,
                    result: Err(missing_repo_error(repo_id)),
                }),
            );
        },
    );
}

pub(super) fn schedule_load_log(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    scope: LogScope,
    limit: usize,
    cursor: Option<LogCursor>,
) {
    let cursor_on_missing = cursor.clone();
    spawn_with_repo_or_else(
        executor,
        repos,
        repo_id,
        msg_tx,
        move |repo, msg_tx| {
            let result = {
                let cursor_ref = cursor.as_ref();
                match scope {
                    LogScope::CurrentBranch => repo.log_head_page(limit, cursor_ref),
                    LogScope::AllBranches => repo.log_all_branches_page(limit, cursor_ref),
                }
            };
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::LogLoaded {
                    repo_id,
                    scope,
                    cursor,
                    result,
                }),
            );
        },
        move |msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::LogLoaded {
                    repo_id,
                    scope,
                    cursor: cursor_on_missing,
                    result: Err(missing_repo_error(repo_id)),
                }),
            );
        },
    );
}

pub(super) fn schedule_load_tags(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo_or_else(
        executor,
        repos,
        repo_id,
        msg_tx,
        move |repo, msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::TagsLoaded {
                    repo_id,
                    result: repo.list_tags(),
                }),
            );
        },
        move |msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::TagsLoaded {
                    repo_id,
                    result: Err(missing_repo_error(repo_id)),
                }),
            );
        },
    );
}

pub(super) fn schedule_load_remote_tags(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo_or_else(
        executor,
        repos,
        repo_id,
        msg_tx,
        move |repo, msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::RemoteTagsLoaded {
                    repo_id,
                    result: repo.list_remote_tags(),
                }),
            );
        },
        move |msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::RemoteTagsLoaded {
                    repo_id,
                    result: Err(missing_repo_error(repo_id)),
                }),
            );
        },
    );
}

pub(super) fn schedule_load_stashes(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    limit: usize,
) {
    spawn_with_repo_or_else(
        executor,
        repos,
        repo_id,
        msg_tx,
        move |repo, msg_tx| {
            let mut entries = repo.stash_list();
            if let Ok(v) = &mut entries {
                v.truncate(limit);
            }
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::StashesLoaded {
                    repo_id,
                    result: entries,
                }),
            );
        },
        move |msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::StashesLoaded {
                    repo_id,
                    result: Err(missing_repo_error(repo_id)),
                }),
            );
        },
    );
}

pub(super) fn schedule_load_conflict_file(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    path: PathBuf,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let conflict_session = repo.conflict_session(&path).ok().flatten();

        let stages = match repo.conflict_file_stages(&path) {
            Ok(v) => Ok(v),
            Err(e) if matches!(e.kind(), ErrorKind::Unsupported(_)) => repo
                .diff_file_text(&DiffTarget::WorkingTree {
                    path: path.clone(),
                    area: DiffArea::Unstaged,
                })
                .map(|opt| {
                    opt.map(|d| {
                        let ours_bytes = d.old.as_ref().map(|text| text.as_bytes().to_vec());
                        let theirs_bytes = d.new.as_ref().map(|text| text.as_bytes().to_vec());
                        gitcomet_core::services::ConflictFileStages {
                            path: d.path,
                            base_bytes: None,
                            ours_bytes,
                            theirs_bytes,
                            base: None,
                            ours: d.old,
                            theirs: d.new,
                        }
                    })
                }),
            Err(e) => Err(e),
        };

        let current_bytes = std::fs::read(repo.spec().workdir.join(&path)).ok();
        let current = decode_utf8_optional(current_bytes.as_deref());

        let result = stages.map(|opt| {
            opt.map(|d| {
                let gitcomet_core::services::ConflictFileStages {
                    path,
                    base_bytes,
                    ours_bytes,
                    theirs_bytes,
                    base,
                    ours,
                    theirs,
                } = d;

                crate::model::ConflictFile {
                    path,
                    base: base.or_else(|| decode_utf8_optional(base_bytes.as_deref())),
                    ours: ours.or_else(|| decode_utf8_optional(ours_bytes.as_deref())),
                    theirs: theirs.or_else(|| decode_utf8_optional(theirs_bytes.as_deref())),
                    base_bytes,
                    ours_bytes,
                    theirs_bytes,
                    current_bytes,
                    current,
                }
            })
        });

        send_or_log(
            &msg_tx,
            Msg::Internal(crate::msg::InternalMsg::ConflictFileLoaded {
                repo_id,
                path,
                result: Box::new(result),
                conflict_session,
            }),
        );
    });
}

pub(super) fn schedule_load_reflog(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    limit: usize,
) {
    spawn_with_repo_or_else(
        executor,
        repos,
        repo_id,
        msg_tx,
        move |repo, msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::ReflogLoaded {
                    repo_id,
                    result: repo.reflog_head(limit),
                }),
            );
        },
        move |msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::ReflogLoaded {
                    repo_id,
                    result: Err(missing_repo_error(repo_id)),
                }),
            );
        },
    );
}

pub(super) fn schedule_load_file_history(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    path: PathBuf,
    limit: usize,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        send_or_log(
            &msg_tx,
            Msg::Internal(crate::msg::InternalMsg::FileHistoryLoaded {
                repo_id,
                path: path.clone(),
                result: repo.log_file_page(&path, limit, None),
            }),
        );
    });
}

pub(super) fn schedule_load_blame(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    path: PathBuf,
    rev: Option<String>,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let result = repo.blame_file(&path, rev.as_deref());
        send_or_log(
            &msg_tx,
            Msg::Internal(crate::msg::InternalMsg::BlameLoaded {
                repo_id,
                path: path.clone(),
                rev: rev.clone(),
                result,
            }),
        );
    });
}

pub(super) fn schedule_load_worktrees(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        send_or_log(
            &msg_tx,
            Msg::Internal(crate::msg::InternalMsg::WorktreesLoaded {
                repo_id,
                result: repo.list_worktrees(),
            }),
        );
    });
}

pub(super) fn schedule_load_submodules(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        send_or_log(
            &msg_tx,
            Msg::Internal(crate::msg::InternalMsg::SubmodulesLoaded {
                repo_id,
                result: repo.list_submodules(),
            }),
        );
    });
}

pub(super) fn schedule_load_rebase_state(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo_or_else(
        executor,
        repos,
        repo_id,
        msg_tx,
        move |repo, msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::RebaseStateLoaded {
                    repo_id,
                    result: repo.rebase_in_progress(),
                }),
            );
        },
        move |msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::RebaseStateLoaded {
                    repo_id,
                    result: Err(missing_repo_error(repo_id)),
                }),
            );
        },
    );
}

pub(super) fn schedule_load_merge_commit_message(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
) {
    spawn_with_repo_or_else(
        executor,
        repos,
        repo_id,
        msg_tx,
        move |repo, msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::MergeCommitMessageLoaded {
                    repo_id,
                    result: repo.merge_commit_message(),
                }),
            );
        },
        move |msg_tx| {
            send_or_log(
                &msg_tx,
                Msg::Internal(crate::msg::InternalMsg::MergeCommitMessageLoaded {
                    repo_id,
                    result: Err(missing_repo_error(repo_id)),
                }),
            );
        },
    );
}

pub(super) fn schedule_load_commit_details(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    commit_id: gitcomet_core::domain::CommitId,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        send_or_log(
            &msg_tx,
            Msg::Internal(crate::msg::InternalMsg::CommitDetailsLoaded {
                repo_id,
                commit_id: commit_id.clone(),
                result: repo.commit_details(&commit_id),
            }),
        );
    });
}

pub(super) fn schedule_load_diff(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    target: DiffTarget,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let result = repo
            .diff_unified(&target)
            .map(|text| Diff::from_unified(target.clone(), &text));
        send_or_log(
            &msg_tx,
            Msg::Internal(crate::msg::InternalMsg::DiffLoaded {
                repo_id,
                target,
                result,
            }),
        );
    });
}

pub(super) fn schedule_load_diff_file(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    target: DiffTarget,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let result = repo.diff_file_text(&target);
        send_or_log(
            &msg_tx,
            Msg::Internal(crate::msg::InternalMsg::DiffFileLoaded {
                repo_id,
                target,
                result,
            }),
        );
    });
}

pub(super) fn schedule_load_diff_file_image(
    executor: &TaskExecutor,
    repos: &RepoMap,
    msg_tx: mpsc::Sender<Msg>,
    repo_id: RepoId,
    target: DiffTarget,
) {
    spawn_with_repo(executor, repos, repo_id, msg_tx, move |repo, msg_tx| {
        let result = repo.diff_file_image(&target);
        send_or_log(
            &msg_tx,
            Msg::Internal(crate::msg::InternalMsg::DiffFileImageLoaded {
                repo_id,
                target,
                result,
            }),
        );
    });
}
