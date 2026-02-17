use super::*;

impl GitGpuiView {
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
                AppNotificationKind::Error => zed::ToastKind::Error,
                AppNotificationKind::Warning => zed::ToastKind::Warning,
                AppNotificationKind::Info | AppNotificationKind::Success => zed::ToastKind::Success,
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
                self.push_toast(zed::ToastKind::Error, msg, cx);
            }

            let new_command_entries = next_repo
                .command_log
                .iter()
                .skip(old_cmd_len.min(next_repo.command_log.len()))
                .collect::<Vec<_>>();
            for entry in &new_command_entries {
                self.push_toast(
                    if entry.ok {
                        zed::ToastKind::Success
                    } else {
                        zed::ToastKind::Error
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

        prev_error != next_error
    }
}
