use super::*;

impl GitGpuiView {
    pub(super) fn apply_state_snapshot(
        &mut self,
        next: Arc<AppState>,
        cx: &mut gpui::Context<Self>,
    ) {
        let prev_active_repo_id = self.state.active_repo;
        let prev_selected_commit = prev_active_repo_id.and_then(|repo_id| {
            self.state
                .repos
                .iter()
                .find(|r| r.id == repo_id)
                .and_then(|r| r.selected_commit.clone())
        });

        let next_repo_id = next.active_repo;
        let next_repo = next_repo_id.and_then(|id| next.repos.iter().find(|r| r.id == id));
        let next_diff_target = next_repo.and_then(|r| r.diff_target.as_ref()).cloned();
        let next_selected_commit = next_repo.and_then(|r| r.selected_commit.clone());

        let prev_diff_target = self
            .active_repo()
            .and_then(|r| r.diff_target.as_ref())
            .cloned();

        let next_clone = next.clone.clone();

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

        match next_clone.as_ref() {
            Some(op) => match &op.status {
                CloneOpStatus::Running => {
                    let needs_reset = self.clone_progress_toast_id.is_none()
                        || self.clone_progress_dest.as_ref() != Some(&op.dest);
                    if needs_reset {
                        if let Some(id) = self.clone_progress_toast_id.take() {
                            self.remove_toast(id, cx);
                        }
                        self.clone_progress_last_seq = 0;
                        self.clone_progress_dest = Some(op.dest.clone());

                        let id = self.push_persistent_toast(
                            zed::ToastKind::Success,
                            format!("Cloning repository…\n{}\n→ {}", op.url, op.dest.display()),
                            cx,
                        );
                        self.clone_progress_toast_id = Some(id);
                    }

                    if let Some(id) = self.clone_progress_toast_id
                        && self.clone_progress_last_seq != op.seq
                    {
                        self.clone_progress_last_seq = op.seq;
                        let tail_lines = op.output_tail.iter().rev().take(12).rev().cloned();
                        let tail = tail_lines.collect::<Vec<_>>().join("\n");
                        let message = if tail.is_empty() {
                            format!("Cloning repository…\n{}\n→ {}", op.url, op.dest.display())
                        } else {
                            format!(
                                "Cloning repository…\n{}\n→ {}\n\n{}",
                                op.url,
                                op.dest.display(),
                                tail
                            )
                        };
                        self.update_toast_text(id, message, cx);
                    }
                }
                CloneOpStatus::FinishedOk => {
                    if self.clone_progress_last_seq != op.seq {
                        if let Some(id) = self.clone_progress_toast_id.take() {
                            self.remove_toast(id, cx);
                        }
                        self.clone_progress_dest = None;
                        self.clone_progress_last_seq = op.seq;
                        self.push_toast(
                            zed::ToastKind::Success,
                            format!("Clone finished: {}", op.dest.display()),
                            cx,
                        );
                    }
                }
                CloneOpStatus::FinishedErr(err) => {
                    if self.clone_progress_last_seq != op.seq {
                        if let Some(id) = self.clone_progress_toast_id.take() {
                            self.remove_toast(id, cx);
                        }
                        self.clone_progress_dest = None;
                        self.clone_progress_last_seq = op.seq;
                        self.push_toast(zed::ToastKind::Error, format!("Clone failed: {err}"), cx);
                    }
                }
            },
            None => {
                if let Some(id) = self.clone_progress_toast_id.take() {
                    self.remove_toast(id, cx);
                }
                self.clone_progress_last_seq = 0;
                self.clone_progress_dest = None;
            }
        }

        if prev_diff_target != next_diff_target {
            self.diff_selection_anchor = None;
            self.diff_selection_range = None;
            self.diff_autoscroll_pending = next_diff_target.is_some();
        }

        self.state = next;

        self.sync_conflict_resolver(cx);

        let repos = &self.state.repos;
        let last_status = &mut self.status_multi_selection_last_status;
        self.status_multi_selection.retain(|repo_id, selection| {
            let Some(repo) = repos.iter().find(|r| r.id == *repo_id) else {
                last_status.remove(repo_id);
                return false;
            };

            if selection.unstaged.is_empty() && selection.staged.is_empty() {
                last_status.remove(repo_id);
                return false;
            }

            let Loadable::Ready(status) = &repo.status else {
                return true;
            };

            let status_changed = match last_status.get(repo_id) {
                Some(prev) => !Arc::ptr_eq(prev, status),
                None => true,
            };
            if status_changed {
                last_status.insert(*repo_id, Arc::clone(status));
                reconcile_status_multi_selection(selection, status);
            }

            if selection.unstaged.is_empty() && selection.staged.is_empty() {
                last_status.remove(repo_id);
                return false;
            }

            true
        });

        if prev_active_repo_id != next_repo_id {
            self.history_scroll
                .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
            self.commit_scroll.set_offset(point(px(0.0), px(0.0)));
            self.commit_files_scroll
                .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
        } else if prev_selected_commit != next_selected_commit {
            self.commit_scroll.set_offset(point(px(0.0), px(0.0)));
            self.commit_files_scroll
                .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
        }

        self.update_commit_details_delay(cx);
    }

    fn update_commit_details_delay(&mut self, cx: &mut gpui::Context<Self>) {
        let Some((repo_id, selected_id, ready_for_selected, is_error)) = (|| {
            let repo = self.active_repo()?;
            let selected_id = repo.selected_commit.clone()?;
            let ready_for_selected = matches!(
                &repo.commit_details,
                Loadable::Ready(details) if details.id == selected_id
            );
            let is_error = matches!(&repo.commit_details, Loadable::Error(_));
            Some((repo.id, selected_id, ready_for_selected, is_error))
        })() else {
            self.commit_details_delay = None;
            return;
        };

        if ready_for_selected || is_error {
            self.commit_details_delay = None;
            return;
        }

        let same_selection = self
            .commit_details_delay
            .as_ref()
            .is_some_and(|s| s.repo_id == repo_id && s.commit_id == selected_id);
        if same_selection {
            return;
        }

        self.commit_details_delay_seq = self.commit_details_delay_seq.wrapping_add(1);
        let seq = self.commit_details_delay_seq;
        self.commit_details_delay = Some(CommitDetailsDelayState {
            repo_id,
            commit_id: selected_id.clone(),
            show_loading: false,
        });

        cx.spawn(
            async move |view: WeakEntity<GitGpuiView>, cx: &mut gpui::AsyncApp| {
                Timer::after(Duration::from_millis(100)).await;
                let _ = view.update(cx, |this, cx| {
                    if this.commit_details_delay_seq != seq {
                        return;
                    }
                    let Some(repo) = this.active_repo() else {
                        return;
                    };
                    let Some(selected_id) = repo.selected_commit.clone() else {
                        return;
                    };
                    if repo.id != repo_id {
                        return;
                    }

                    let ready_for_selected = matches!(
                        &repo.commit_details,
                        Loadable::Ready(details) if details.id == selected_id
                    );
                    if ready_for_selected || matches!(&repo.commit_details, Loadable::Error(_)) {
                        return;
                    }

                    if let Some(state) = this.commit_details_delay.as_mut()
                        && state.repo_id == repo_id
                        && state.commit_id == selected_id
                        && !state.show_loading
                    {
                        state.show_loading = true;
                        cx.notify();
                    }
                });
            },
        )
        .detach();
    }
}
