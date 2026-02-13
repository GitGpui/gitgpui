use super::*;

impl GitGpuiView {
    pub(super) fn apply_state_snapshot(
        &mut self,
        next: Arc<AppState>,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        let prev_error = self
            .active_repo()
            .and_then(|repo| repo.last_error.clone());
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

            let new_command_summaries = next_repo
                .command_log
                .iter()
                .skip(old_cmd_len.min(next_repo.command_log.len()))
                .map(|e| (e.ok, e.summary.clone()))
                .collect::<Vec<_>>();
            for (ok, summary) in new_command_summaries {
                self.push_toast(
                    if ok {
                        zed::ToastKind::Success
                    } else {
                        zed::ToastKind::Error
                    },
                    summary,
                    cx,
                );
            }
        }

        self.toast_host
            .update(cx, |host, cx| host.sync_clone_progress(next.clone.as_ref(), cx));

        self.state = next;

        prev_error != next_error
    }
}
