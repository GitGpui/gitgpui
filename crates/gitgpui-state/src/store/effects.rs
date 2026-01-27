use crate::msg::{Effect, Msg};
use gitgpui_core::domain::{Diff, RepoSpec};
use gitgpui_core::services::CommandOutput;
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::{GitBackend, GitRepository};
use std::collections::HashMap;
use std::io::{BufRead as _, BufReader, Read as _};
use std::path::Path;
use std::process::{Command, Stdio};
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

        Effect::LoadConflictFile { repo_id, path } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let ours_theirs = repo.diff_file_text(&gitgpui_core::domain::DiffTarget::WorkingTree {
                        path: path.clone(),
                        area: gitgpui_core::domain::DiffArea::Unstaged,
                    });

                    let current = std::fs::read(repo.spec().workdir.join(&path))
                        .ok()
                        .and_then(|bytes| String::from_utf8(bytes).ok());

                    let result = ours_theirs.map(|opt| {
                        opt.map(|d| crate::model::ConflictFile {
                            path: d.path,
                            ours: d.old,
                            theirs: d.new,
                            current,
                        })
                    });

                    let _ = msg_tx.send(Msg::ConflictFileLoaded {
                        repo_id,
                        path,
                        result,
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

        Effect::SaveWorktreeFile {
            repo_id,
            path,
            contents,
            stage,
        } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let full = repo.spec().workdir.join(&path);
                    let result = (|| -> Result<CommandOutput, Error> {
                        if let Some(parent) = full.parent() {
                            std::fs::create_dir_all(parent)
                                .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;
                        }
                        std::fs::write(&full, contents.as_bytes())
                            .map_err(|e| Error::new(ErrorKind::Io(e.kind())))?;
                        if stage {
                            let path_ref: &Path = &path;
                            repo.stage(&[path_ref])?;
                        }
                        Ok(CommandOutput {
                            command: format!(
                                "Save {}{}",
                                path.display(),
                                if stage { " (staged)" } else { "" }
                            ),
                            stdout: String::new(),
                            stderr: String::new(),
                            exit_code: Some(0),
                        })
                    })();

                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::SaveWorktreeFile { path, stage },
                        result,
                    });
                });
            }
        }

        Effect::LoadFileHistory {
            repo_id,
            path,
            limit,
        } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::FileHistoryLoaded {
                        repo_id,
                        path: path.clone(),
                        result: repo.log_file_page(&path, limit, None),
                    });
                });
            }
        }

        Effect::LoadBlame {
            repo_id,
            path,
            rev,
        } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let result = repo.blame_file(&path, rev.as_deref());
                    let _ = msg_tx.send(Msg::BlameLoaded {
                        repo_id,
                        path: path.clone(),
                        rev: rev.clone(),
                        result,
                    });
                });
            }
        }

        Effect::LoadWorktrees { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::WorktreesLoaded {
                        repo_id,
                        result: repo.list_worktrees(),
                    });
                });
            }
        }

        Effect::LoadSubmodules { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::SubmodulesLoaded {
                        repo_id,
                        result: repo.list_submodules(),
                    });
                });
            }
        }

        Effect::LoadRebaseState { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RebaseStateLoaded {
                        repo_id,
                        result: repo.rebase_in_progress(),
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

        Effect::LoadDiffFileImage { repo_id, target } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let result = repo.diff_file_image(&target);
                    let _ = msg_tx.send(Msg::DiffFileImageLoaded {
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

        Effect::CheckoutRemoteBranch {
            repo_id,
            remote,
            name,
        } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.checkout_remote_branch(&remote, &name),
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

        Effect::CreateBranchAndCheckout { repo_id, name } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let target = gitgpui_core::domain::CommitId("HEAD".to_string());
                    let result = repo
                        .create_branch(&name, &target)
                        .and_then(|()| repo.checkout_branch(&name));
                    let _ = msg_tx.send(Msg::RepoActionFinished { repo_id, result });
                });
            }
        }

        Effect::DeleteBranch { repo_id, name } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.delete_branch(&name),
                    });
                });
            }
        }

        Effect::CloneRepo { url, dest } => {
            executor.spawn(move || {
                let mut cmd = Command::new("git");
                cmd.arg("-c")
                    .arg("color.ui=false")
                    .arg("clone")
                    .arg("--progress")
                    .arg(&url)
                    .arg(&dest)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                let command_str = format!(
                    "git clone --progress {} {}",
                    url,
                    dest.display()
                );

                let mut child = match cmd.spawn() {
                    Ok(child) => child,
                    Err(e) => {
                        let err = Error::new(ErrorKind::Io(e.kind()));
                        let _ = msg_tx.send(Msg::CloneRepoFinished {
                            url,
                            dest,
                            result: Err(err),
                        });
                        return;
                    }
                };

                let stdout = child.stdout.take();
                let stdout_handle = std::thread::spawn(move || {
                    let mut buf = Vec::new();
                    if let Some(mut stdout) = stdout {
                        let _ = stdout.read_to_end(&mut buf);
                    }
                    String::from_utf8_lossy(&buf).into_owned()
                });

                let stderr = child.stderr.take();
                let mut stderr_acc = String::new();
                if let Some(stderr) = stderr {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines().flatten() {
                        stderr_acc.push_str(&line);
                        stderr_acc.push('\n');
                        let _ = msg_tx.send(Msg::CloneRepoProgress {
                            dest: dest.clone(),
                            line,
                        });
                    }
                }

                let status = child.wait();
                let stdout_str = stdout_handle.join().unwrap_or_default();

                let result = match status {
                    Ok(status) => {
                        let mut out = CommandOutput::default();
                        out.command = command_str;
                        out.stdout = stdout_str;
                        out.stderr = stderr_acc;
                        out.exit_code = status.code();
                        if status.success() {
                            Ok(out)
                        } else {
                            Err(Error::new(ErrorKind::Backend(out.combined())))
                        }
                    }
                    Err(e) => Err(Error::new(ErrorKind::Io(e.kind()))),
                };

                let ok = result.is_ok();
                let _ = msg_tx.send(Msg::CloneRepoFinished {
                    url: url.clone(),
                    dest: dest.clone(),
                    result,
                });

                if ok {
                    let _ = msg_tx.send(Msg::OpenRepo(dest));
                }
            });
        }

        Effect::ExportPatch {
            repo_id,
            commit_id,
            dest,
        } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::ExportPatch {
                            commit_id: commit_id.clone(),
                            dest: dest.clone(),
                        },
                        result: repo.export_patch_with_output(&commit_id, &dest),
                    });
                });
            }
        }

        Effect::ApplyPatch { repo_id, patch } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::ApplyPatch {
                            patch: patch.clone(),
                        },
                        result: repo.apply_patch_with_output(&patch),
                    });
                });
            }
        }

        Effect::AddWorktree {
            repo_id,
            path,
            reference,
        } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::AddWorktree {
                            path: path.clone(),
                            reference: reference.clone(),
                        },
                        result: repo.add_worktree_with_output(&path, reference.as_deref()),
                    });
                });
            }
        }

        Effect::RemoveWorktree { repo_id, path } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::RemoveWorktree { path: path.clone() },
                        result: repo.remove_worktree_with_output(&path),
                    });
                });
            }
        }

        Effect::AddSubmodule { repo_id, url, path } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::AddSubmodule {
                            url: url.clone(),
                            path: path.clone(),
                        },
                        result: repo.add_submodule_with_output(&url, &path),
                    });
                });
            }
        }

        Effect::UpdateSubmodules { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::UpdateSubmodules,
                        result: repo.update_submodules_with_output(),
                    });
                });
            }
        }

        Effect::RemoveSubmodule { repo_id, path } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::RemoveSubmodule { path: path.clone() },
                        result: repo.remove_submodule_with_output(&path),
                    });
                });
            }
        }

        Effect::StageHunk { repo_id, patch } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::StageHunk,
                        result: repo.apply_unified_patch_to_index_with_output(&patch, false),
                    });
                });
            }
        }

        Effect::UnstageHunk { repo_id, patch } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::UnstageHunk,
                        result: repo.apply_unified_patch_to_index_with_output(&patch, true),
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

        Effect::DiscardWorktreeChangesPath { repo_id, path } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let path_ref: &Path = &path;
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.discard_worktree_changes(&[path_ref]),
                    });
                });
            }
        }

        Effect::DiscardWorktreeChangesPaths { repo_id, paths } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let mut unique = paths;
                    unique.sort();
                    unique.dedup();
                    let refs = unique.iter().map(|p| p.as_path()).collect::<Vec<_>>();
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.discard_worktree_changes(&refs),
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

        Effect::CommitAmend { repo_id, message } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::CommitAmendFinished {
                        repo_id,
                        result: repo.commit_amend(&message),
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

        Effect::ForcePush { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::ForcePush,
                        result: repo.push_force_with_output(),
                    });
                });
            }
        }

        Effect::PushSetUpstream {
            repo_id,
            remote,
            branch,
        } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::PushSetUpstream {
                            remote: remote.clone(),
                            branch: branch.clone(),
                        },
                        result: repo.push_set_upstream_with_output(&remote, &branch),
                    });
                });
            }
        }

        Effect::Reset {
            repo_id,
            target,
            mode,
        } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::Reset {
                            mode,
                            target: target.clone(),
                        },
                        result: repo.reset_with_output(&target, mode),
                    });
                });
            }
        }

        Effect::Rebase { repo_id, onto } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::Rebase { onto: onto.clone() },
                        result: repo.rebase_with_output(&onto),
                    });
                });
            }
        }

        Effect::RebaseContinue { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::RebaseContinue,
                        result: repo.rebase_continue_with_output(),
                    });
                });
            }
        }

        Effect::RebaseAbort { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::RebaseAbort,
                        result: repo.rebase_abort_with_output(),
                    });
                });
            }
        }

        Effect::CreateTag {
            repo_id,
            name,
            target,
        } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::CreateTag {
                            name: name.clone(),
                            target: target.clone(),
                        },
                        result: repo.create_tag_with_output(&name, &target),
                    });
                });
            }
        }

        Effect::DeleteTag { repo_id, name } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::DeleteTag { name: name.clone() },
                        result: repo.delete_tag_with_output(&name),
                    });
                });
            }
        }

        Effect::AddRemote { repo_id, name, url } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::AddRemote {
                            name: name.clone(),
                            url: url.clone(),
                        },
                        result: repo.add_remote_with_output(&name, &url),
                    });
                });
            }
        }

        Effect::RemoveRemote { repo_id, name } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::RemoveRemote { name: name.clone() },
                        result: repo.remove_remote_with_output(&name),
                    });
                });
            }
        }

        Effect::SetRemoteUrl {
            repo_id,
            name,
            url,
            kind,
        } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoCommandFinished {
                        repo_id,
                        command: crate::msg::RepoCommandKind::SetRemoteUrl {
                            name: name.clone(),
                            url: url.clone(),
                            kind,
                        },
                        result: repo.set_remote_url_with_output(&name, &url, kind),
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

        Effect::PopStash { repo_id, index } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let result = repo.stash_apply(index).and_then(|()| repo.stash_drop(index));
                    let _ = msg_tx.send(Msg::RepoActionFinished { repo_id, result });
                });
            }
        }
    }
}
