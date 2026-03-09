use super::*;

impl GitCometView {
    pub(super) fn apply_state_snapshot(
        &mut self,
        next: Arc<AppState>,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        let prev_error = self.active_repo().and_then(|repo| repo.last_error.clone());
        let next_error = next
            .active_repo
            .and_then(|repo_id| next.repos.iter().find(|repo| repo.id == repo_id))
            .and_then(|repo| repo.last_error.clone());

        let old_notification_len = self.state.notifications.len();
        let new_notifications = next
            .notifications
            .iter()
            .skip(old_notification_len.min(next.notifications.len()))
            .cloned()
            .collect::<Vec<_>>();
        for notification in new_notifications {
            let kind = match notification.kind {
                AppNotificationKind::Error => components::ToastKind::Error,
                AppNotificationKind::Warning => components::ToastKind::Warning,
                AppNotificationKind::Info | AppNotificationKind::Success => {
                    components::ToastKind::Success
                }
            };
            self.push_toast(kind, notification.message, cx);
        }

        for next_repo in &next.repos {
            let (old_diag_len, old_cmd_len) = self
                .state
                .repos
                .iter()
                .find(|r| r.id == next_repo.id)
                .map(|r| (r.diagnostics.len(), r.command_log.len()))
                .unwrap_or((0, 0));

            let new_diag_messages = next_repo
                .diagnostics
                .iter()
                .skip(old_diag_len.min(next_repo.diagnostics.len()))
                .filter(|d| d.kind == DiagnosticKind::Error)
                .map(|d| d.message.clone())
                .collect::<Vec<_>>();
            for msg in new_diag_messages {
                if self.pending_force_delete_branch_prompt.is_none()
                    && let Some(name) = parse_force_delete_branch_name(&msg)
                {
                    self.pending_force_delete_branch_prompt = Some((next_repo.id, name));
                }
                self.push_toast(components::ToastKind::Error, msg, cx);
            }

            let new_command_entries = next_repo
                .command_log
                .iter()
                .skip(old_cmd_len.min(next_repo.command_log.len()))
                .collect::<Vec<_>>();
            for entry in &new_command_entries {
                if entry.command.starts_with("telemetry.") {
                    continue;
                }

                let force_remove_worktree_path = if entry.ok {
                    None
                } else {
                    parse_force_remove_worktree_path(&entry.command, &entry.stderr)
                };
                if self.pending_force_remove_worktree_prompt.is_none()
                    && let Some(path) = force_remove_worktree_path.clone()
                {
                    self.pending_force_remove_worktree_prompt = Some((next_repo.id, path));
                }
                if force_remove_worktree_path.is_some() {
                    continue;
                }

                self.push_toast(
                    if entry.ok {
                        components::ToastKind::Success
                    } else {
                        components::ToastKind::Error
                    },
                    entry.summary.clone(),
                    cx,
                );
            }

            if self.pending_pull_reconcile_prompt.is_none()
                && next.active_repo == Some(next_repo.id)
                && new_command_entries.iter().any(|entry| {
                    if entry.ok {
                        return false;
                    }
                    if !entry.command.trim_start().starts_with("git pull") {
                        return false;
                    }

                    let stderr = entry.stderr.as_str();
                    stderr.contains("Need to specify how to reconcile divergent branches")
                        || stderr.contains(
                            "divergent branches and need to specify how to reconcile them",
                        )
                        || stderr.contains("Not possible to fast-forward")
                })
            {
                self.pending_pull_reconcile_prompt = Some(next_repo.id);
            }
        }

        self.toast_host.update(cx, |host, cx| {
            host.sync_clone_progress(next.clone.as_ref(), cx)
        });

        self.state = next;
        self.drive_focused_mergetool_bootstrap();

        prev_error != next_error
    }
}

fn parse_force_delete_branch_name(message: &str) -> Option<String> {
    if !message.contains("git branch -d failed:") {
        return None;
    }
    let needle = "run 'git branch -D ";
    let start = message.find(needle)? + needle.len();
    let rest = &message[start..];
    let end = rest.find('\'')?;
    let name = rest[..end].trim();
    (!name.is_empty()).then(|| name.to_string())
}

fn parse_force_remove_worktree_path(command: &str, stderr: &str) -> Option<std::path::PathBuf> {
    if !is_force_remove_worktree_required_error(command, stderr) {
        return None;
    }
    parse_worktree_path_from_fatal(stderr).or_else(|| parse_worktree_path_from_command(command))
}

fn is_force_remove_worktree_required_error(command: &str, stderr: &str) -> bool {
    let command = command.trim();
    let is_worktree_remove = command.starts_with("git worktree remove ")
        && !command.starts_with("git worktree remove --force ");
    is_worktree_remove
        && stderr.contains("contains modified or untracked files")
        && stderr.contains("use --force to delete it")
}

fn parse_worktree_path_from_fatal(stderr: &str) -> Option<std::path::PathBuf> {
    let needle = "fatal: '";
    let start = stderr.find(needle)? + needle.len();
    let rest = &stderr[start..];
    let end = rest.find("' contains modified or untracked files")?;
    let path = rest[..end].trim();
    (!path.is_empty()).then(|| std::path::PathBuf::from(path))
}

fn parse_worktree_path_from_command(command: &str) -> Option<std::path::PathBuf> {
    let command = command.trim();
    let rest = command.strip_prefix("git worktree remove ")?;
    let path = rest.trim();
    if path.is_empty() || path.starts_with('-') {
        return None;
    }
    Some(std::path::PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parse_force_remove_worktree_path_prefers_fatal_path() {
        let command = "git worktree remove /tmp/from-command";
        let stderr = "git worktree remove /tmp/from-command failed: fatal: '/tmp/from-stderr' contains modified or untracked files, use --force to delete it.";
        assert_eq!(
            parse_force_remove_worktree_path(command, stderr),
            Some(PathBuf::from("/tmp/from-stderr"))
        );
    }

    #[test]
    fn parse_force_remove_worktree_path_falls_back_to_command_path() {
        let command = "git worktree remove /tmp/worktree";
        let stderr = "contains modified or untracked files, use --force to delete it";
        assert_eq!(
            parse_force_remove_worktree_path(command, stderr),
            Some(PathBuf::from("/tmp/worktree"))
        );
    }

    #[test]
    fn parse_force_remove_worktree_path_ignores_non_matching_errors() {
        let command = "git worktree remove /tmp/worktree";
        let stderr = "fatal: '/tmp/worktree' is not a working tree";
        assert_eq!(parse_force_remove_worktree_path(command, stderr), None);
    }

    #[test]
    fn parse_force_remove_worktree_path_ignores_already_forced_command() {
        let command = "git worktree remove --force /tmp/worktree";
        let stderr = "contains modified or untracked files, use --force to delete it";
        assert_eq!(parse_force_remove_worktree_path(command, stderr), None);
    }
}
