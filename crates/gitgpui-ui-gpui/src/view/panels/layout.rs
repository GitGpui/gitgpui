use super::*;

impl GitGpuiView {
    pub(in super::super) fn sidebar(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let Some(repo) = self.active_repo() else {
            return div()
                .flex()
                .flex_col()
                .h_full()
                .min_h(px(0.0))
                .child(zed::empty_state(
                    theme,
                    "Branches",
                    "No repository selected.",
                ));
        };

        let row_count = Self::branch_sidebar_rows(repo).len();
        let list = uniform_list(
            "branch_sidebar",
            row_count,
            cx.processor(Self::render_branch_sidebar_rows),
        )
        .flex_1()
        .min_h(px(0.0))
        .track_scroll(self.branches_scroll.clone());
        let scroll_handle = self.branches_scroll.0.borrow().base_handle.clone();
        let panel_body: AnyElement = div()
            .id("branch_sidebar_scroll_container")
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .child(list)
            .child(zed::Scrollbar::new("branch_sidebar_scrollbar", scroll_handle).render(theme))
            .into_any_element();

        div()
            .flex()
            .flex_col()
            .h_full()
            .min_h(px(0.0))
            .child(panel_body)
    }

    pub(in super::super) fn commit_details_view(
        &mut self,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let active_repo_id = self.active_repo_id();
        let selected_id = active_repo_id.and_then(|repo_id| {
            self.state
                .repos
                .iter()
                .find(|r| r.id == repo_id)
                .and_then(|r| r.selected_commit.clone())
        });

        if let (Some(repo_id), Some(selected_id)) = (active_repo_id, selected_id)
        {
            let show_delayed_loading = self.commit_details_delay.as_ref().is_some_and(|s| {
                s.repo_id == repo_id && s.commit_id == selected_id && s.show_loading
            });
            let diff_target = self
                .state
                .repos
                .iter()
                .find(|r| r.id == repo_id)
                .and_then(|r| r.diff_target.clone());

            let details_snapshot = self
                .state
                .repos
                .iter()
                .find(|r| r.id == repo_id)
                .and_then(|r| match &r.commit_details {
                    Loadable::Ready(details) => Some((details.id.clone(), details.files.len())),
                    _ => None,
                });

            if let Some((details_id, total_files)) = details_snapshot {
                if self.commit_details_files_commit.as_ref() != Some(&details_id)
                    || self.commit_details_files_limit == 0
                {
                    self.commit_details_files_commit = Some(details_id);
                    self.commit_details_files_limit = COMMIT_DETAILS_FILES_INITIAL_RENDER_LIMIT;
                }

                let scroll_y_now = absolute_scroll_y(&self.commit_scroll);
                let scrolled = scroll_y_now != self.commit_details_scroll_last_y;
                if scrolled {
                    self.commit_details_scroll_last_y = scroll_y_now;
                }

                if scrolled
                    && self.commit_details_files_limit < total_files
                    && scroll_is_near_bottom(&self.commit_scroll, px(200.0))
                {
                    self.commit_details_files_limit = (self.commit_details_files_limit
                        + COMMIT_DETAILS_FILES_RENDER_CHUNK)
                        .min(total_files);
                    cx.notify();
                }
            }

            let files_limit = self.commit_details_files_limit;

            let header_title: SharedString = "Commit details".into();

            let header = div()
                .flex()
                .items_center()
                .justify_between()
                .h(px(zed::CONTROL_HEIGHT_MD_PX))
                .px_2()
                .bg(theme.colors.surface_bg_elevated)
                .border_b_1()
                .border_color(theme.colors.border)
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .line_clamp(1)
                        .child(header_title),
                )
                .child(
                    zed::Button::new("commit_details_close", "✕")
                        .style(zed::ButtonStyle::Transparent)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            if let Some(repo_id) = this.active_repo_id() {
                                this.store.dispatch(Msg::ClearCommitSelection { repo_id });
                                this.store.dispatch(Msg::ClearDiffSelection { repo_id });
                            }
                            cx.notify();
                        })
                        .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                            let text: SharedString = "Close commit details".into();
                            if *hovering {
                                this.tooltip_text = Some(text);
                            } else if this.tooltip_text.as_ref() == Some(&text) {
                                this.tooltip_text = None;
                            }
                            cx.notify();
                        })),
                );

            let body: AnyElement = match self
                .state
                .repos
                .iter()
                .find(|r| r.id == repo_id)
                .map(|r| &r.commit_details)
            {
                None => zed::empty_state(theme, "Commit", "No repository.").into_any_element(),
                Some(Loadable::Loading) => {
                    if show_delayed_loading {
                        zed::empty_state(theme, "Commit", "Loading…").into_any_element()
                    } else {
                        div().into_any_element()
                    }
                }
                Some(Loadable::Error(e)) => {
                    zed::empty_state(theme, "Commit", e.clone()).into_any_element()
                }
                Some(Loadable::NotLoaded) => {
                    if show_delayed_loading {
                        zed::empty_state(theme, "Commit", "Loading…").into_any_element()
                    } else {
                        div().into_any_element()
                    }
                }
                Some(Loadable::Ready(details)) => {
                    if &details.id != &selected_id {
                        if show_delayed_loading {
                            zed::empty_state(theme, "Commit", "Loading…").into_any_element()
                        } else {
                            let parent = details
                                .parent_ids
                                .first()
                                .map(|p| p.as_ref().to_string())
                                .unwrap_or_else(|| "—".to_string());

                            let files = if details.files.is_empty() {
                                div()
                                    .text_sm()
                                    .text_color(theme.colors.text_muted)
                                    .child("No files.")
                                    .into_any_element()
                            } else {
                                use std::hash::{Hash, Hasher};
                                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                                details.id.as_ref().hash(&mut hasher);
                                let commit_key = hasher.finish();
                                let commit_id = details.id.clone();

                                let total_files = details.files.len();
                                let shown_files = total_files.min(files_limit);

                                let mut list = div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .min_h(px(0.0))
                                        .when(total_files > shown_files, |d| {
                                            d.child(
                                                div()
                                                    .px_2()
                                                    .text_xs()
                                                    .text_color(theme.colors.text_muted)
                                                    .child(format!(
                                                        "Showing {shown_files} of {total_files} files (scroll to load more)",
                                                    )),
                                            )
                                        })
                                        .children(
                                            details
                                                .files
                                                .iter()
                                                .take(shown_files)
                                                .enumerate()
                                                .map(|(ix, f)| {
                                            let commit_id = commit_id.clone();
                                            let row_id = commit_key.wrapping_add(ix as u64);
                                            let (icon, color) = match f.kind {
                                                FileStatusKind::Added => (Some("+"), theme.colors.success),
                                                FileStatusKind::Modified => (Some("✎"), theme.colors.warning),
                                                FileStatusKind::Deleted => (None, theme.colors.text_muted),
                                                FileStatusKind::Renamed => (Some("→"), theme.colors.accent),
                                                FileStatusKind::Untracked => (Some("?"), theme.colors.warning),
                                                FileStatusKind::Conflicted => (Some("!"), theme.colors.danger),
                                            };

                                            let path = f.path.clone();
                                            let selected = diff_target.as_ref().is_some_and(|t| match t {
                                                DiffTarget::Commit { commit_id: t_commit_id, path: Some(t_path) } => {
                                                    t_commit_id == &commit_id && t_path == &path
                                                }
                                                _ => false,
                                            });

                                            let commit_id_for_click = commit_id.clone();
                                            let path_for_click = path.clone();
                                            let commit_id_for_menu = commit_id.clone();
                                            let path_for_menu = path.clone();
                                            let tooltip: SharedString = path.display().to_string().into();

                                            let mut row = div()
                                                .id(("commit_file", row_id))
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .px_2()
                                                .py_1()
                                                .w_full()
                                                .rounded(px(theme.radii.row))
                                                .hover(move |s| s.bg(theme.colors.hover))
                                                .active(move |s| s.bg(theme.colors.active))
                                                .child(
                                                    div()
                                                        .w(px(16.0))
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .when_some(icon, |this, icon| {
                                                            this.child(
                                                                div()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::BOLD)
                                                                    .text_color(color)
                                                                    .child(icon),
                                                            )
                                                        }),
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w(px(0.0))
                                                        .text_sm()
                                                        .line_clamp(1)
                                                        .whitespace_nowrap()
                                                        .child(path.display().to_string()),
                                                )
                                                .on_click(cx.listener(move |this, _e: &ClickEvent, window, cx| {
                                                    window.focus(&this.diff_panel_focus_handle);
                                                    this.store.dispatch(Msg::SelectDiff {
                                                        repo_id,
                                                        target: DiffTarget::Commit {
                                                            commit_id: commit_id_for_click.clone(),
                                                            path: Some(path_for_click.clone()),
                                                        },
                                                    });
                                                    this.rebuild_diff_cache();
                                                    cx.notify();
                                                }))
                                                .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                                                    if *hovering {
                                                        this.tooltip_text = Some(tooltip.clone());
                                                    } else if this.tooltip_text.as_ref() == Some(&tooltip) {
                                                        this.tooltip_text = None;
                                                    }
                                                    cx.notify();
                                                }));

                                            row = row.on_mouse_down(
                                                MouseButton::Right,
                                                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                                                    cx.stop_propagation();
                                                    this.open_popover_at(
                                                        PopoverKind::CommitFileMenu {
                                                            repo_id,
                                                            commit_id: commit_id_for_menu.clone(),
                                                            path: path_for_menu.clone(),
                                                        },
                                                        e.position,
                                                        window,
                                                        cx,
                                                    );
                                                }),
                                            );

                                            if selected {
                                                row = row.bg(with_alpha(
                                                    theme.colors.accent,
                                                    if theme.is_dark { 0.16 } else { 0.10 },
                                                ));
                                            }

                                            row.into_any_element()
                                        }),
                                        );
                                if total_files > shown_files {
                                    let omitted = total_files - shown_files;
                                    list = list.child(
                                        div()
                                            .px_2()
                                            .py_1()
                                            .text_sm()
                                            .text_color(theme.colors.text_muted)
                                            .child(format!(
                                                "… and {omitted} more files (not shown)",
                                            )),
                                    );
                                }

                                list.into_any_element()
                            };

                            let needs_update = self.commit_details_message_input.read(cx).text()
                                != details.message.as_str();
                            if needs_update {
                                self.commit_details_message_input.update(cx, |input, cx| {
                                    input.set_text(details.message.clone(), cx);
                                });
                            }

                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .w_full()
                                        .min_w(px(0.0))
                                        .child(self.commit_details_message_input.clone()),
                                )
                                .child(zed::key_value(
                                    theme,
                                    "Commit SHA",
                                    details.id.as_ref().to_string(),
                                ))
                                .child(zed::key_value(
                                    theme,
                                    "Commit date",
                                    details.committed_at.clone(),
                                ))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(theme.colors.text_muted)
                                                .child("Parent commit SHA"),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .whitespace_nowrap()
                                                .line_clamp(1)
                                                .child(parent),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(theme.colors.text_muted)
                                                .child("Committed files"),
                                        )
                                        .child(files),
                                )
                                .into_any_element()
                        }
                    } else {
                        let parent = details
                            .parent_ids
                            .first()
                            .map(|p| p.as_ref().to_string())
                            .unwrap_or_else(|| "—".to_string());

                        let files = if details.files.is_empty() {
                            div()
                                .text_sm()
                                .text_color(theme.colors.text_muted)
                                .child("No files.")
                                .into_any_element()
                        } else {
                            use std::hash::{Hash, Hasher};
                            let mut hasher = std::collections::hash_map::DefaultHasher::new();
                            details.id.as_ref().hash(&mut hasher);
                            let commit_key = hasher.finish();

                            let total_files = details.files.len();
                            let shown_files = total_files.min(files_limit);
                            let commit_id = details.id.clone();

                            let mut list = div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .min_h(px(0.0))
                                .when(total_files > shown_files, |d| {
                                    d.child(
                                        div()
                                            .px_2()
                                            .text_xs()
                                            .text_color(theme.colors.text_muted)
                                            .child(format!(
                                                "Showing {shown_files} of {total_files} files (scroll to load more)",
                                            )),
                                    )
                                })
                                .children(details.files.iter().take(shown_files).enumerate().map(
                                    |(ix, f)| {
                                        let commit_id = commit_id.clone();
                                        let row_id = commit_key.wrapping_add(ix as u64);
                                        let (icon, color) = match f.kind {
                                            FileStatusKind::Added => {
                                                (Some("+"), theme.colors.success)
                                            }
                                            FileStatusKind::Modified => {
                                                (Some("✎"), theme.colors.warning)
                                            }
                                            FileStatusKind::Deleted => {
                                                (None, theme.colors.text_muted)
                                            }
                                            FileStatusKind::Renamed => {
                                                (Some("→"), theme.colors.accent)
                                            }
                                            FileStatusKind::Untracked => {
                                                (Some("?"), theme.colors.warning)
                                            }
                                            FileStatusKind::Conflicted => {
                                                (Some("!"), theme.colors.danger)
                                            }
                                        };

                                        let path = f.path.clone();
                                        let selected =
                                            diff_target.as_ref().is_some_and(|t| match t {
                                                DiffTarget::Commit {
                                                    commit_id: t_commit_id,
                                                    path: Some(t_path),
                                                } => t_commit_id == &commit_id && t_path == &path,
                                                _ => false,
                                            });

                                        let commit_id_for_click = commit_id.clone();
                                        let path_for_click = path.clone();
                                        let commit_id_for_menu = commit_id.clone();
                                        let path_for_menu = path.clone();
                                        let tooltip: SharedString =
                                            path.display().to_string().into();

                                        let mut row = div()
                                            .id(("commit_file", row_id))
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .px_2()
                                            .py_1()
                                            .w_full()
                                            .rounded(px(theme.radii.row))
                                            .hover(move |s| s.bg(theme.colors.hover))
                                            .active(move |s| s.bg(theme.colors.active))
                                            .child(
                                                div()
                                                    .w(px(16.0))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .when_some(icon, |this, icon| {
                                                        this.child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(color)
                                                                .child(icon),
                                                        )
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w(px(0.0))
                                                    .text_sm()
                                                    .line_clamp(1)
                                                    .whitespace_nowrap()
                                                    .child(path.display().to_string()),
                                            )
                                            .on_click(cx.listener(
                                                move |this, _e: &ClickEvent, window, cx| {
                                                    window.focus(&this.diff_panel_focus_handle);
                                                    this.store.dispatch(Msg::SelectDiff {
                                                        repo_id,
                                                        target: DiffTarget::Commit {
                                                            commit_id: commit_id_for_click.clone(),
                                                            path: Some(path_for_click.clone()),
                                                        },
                                                    });
                                                    this.rebuild_diff_cache();
                                                    cx.notify();
                                                },
                                            ))
                                            .on_hover(cx.listener(
                                                move |this, hovering: &bool, _w, cx| {
                                                    if *hovering {
                                                        this.tooltip_text = Some(tooltip.clone());
                                                    } else if this.tooltip_text.as_ref()
                                                        == Some(&tooltip)
                                                    {
                                                        this.tooltip_text = None;
                                                    }
                                                    cx.notify();
                                                },
                                            ));

                                        row = row.on_mouse_down(
                                            MouseButton::Right,
                                            cx.listener(
                                                move |this, e: &MouseDownEvent, window, cx| {
                                                    cx.stop_propagation();
                                                    this.open_popover_at(
                                                        PopoverKind::CommitFileMenu {
                                                            repo_id,
                                                            commit_id: commit_id_for_menu.clone(),
                                                            path: path_for_menu.clone(),
                                                        },
                                                        e.position,
                                                        window,
                                                        cx,
                                                    );
                                                },
                                            ),
                                        );

                                        if selected {
                                            row = row.bg(with_alpha(
                                                theme.colors.accent,
                                                if theme.is_dark { 0.16 } else { 0.10 },
                                            ));
                                        }

                                        row.into_any_element()
                                    },
                                ));
                            if total_files > shown_files {
                                let omitted = total_files - shown_files;
                                list = list.child(
                                    div()
                                        .px_2()
                                        .py_1()
                                        .text_sm()
                                        .text_color(theme.colors.text_muted)
                                        .child(format!("… and {omitted} more files (not shown)",)),
                                );
                            }

                            list.into_any_element()
                        };

                        let needs_update = self.commit_details_message_input.read(cx).text()
                            != details.message.as_str();
                        if needs_update {
                            self.commit_details_message_input.update(cx, |input, cx| {
                                input.set_text(details.message.clone(), cx);
                            });
                        }

                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .w_full()
                                    .min_w(px(0.0))
                                    .child(self.commit_details_message_input.clone()),
                            )
                            .child(zed::key_value(
                                theme,
                                "Commit SHA",
                                details.id.as_ref().to_string(),
                            ))
                            .child(zed::key_value(
                                theme,
                                "Commit date",
                                details.committed_at.clone(),
                            ))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.colors.text_muted)
                                            .child("Parent commit SHA"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .whitespace_nowrap()
                                            .line_clamp(1)
                                            .child(parent),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.colors.text_muted)
                                            .child("Committed files"),
                                    )
                                    .child(files),
                            )
                            .into_any_element()
                    }
                }
            };

            return div()
                .id("commit_details_container")
                .relative()
                .flex()
                .flex_col()
                .flex_1()
                .h_full()
                .min_h(px(0.0))
                .child(header)
                .child({
                    let scroll_surface = div()
                        .id("commit_details_scroll_surface")
                        .relative()
                        .flex_1()
                        .h_full()
                        .min_h(px(0.0))
                        .overflow_y_scroll()
                        .track_scroll(&self.commit_scroll)
                        .child(div().flex().flex_col().gap_2().p_2().w_full().child(body));

                    div()
                        .id("commit_details_body_container")
                        .relative()
                        .flex_1()
                        .h_full()
                        .min_h(px(0.0))
                        .child(scroll_surface)
                        .child(
                            zed::Scrollbar::new(
                                "commit_details_scrollbar",
                                self.commit_scroll.clone(),
                            )
                            .render(theme),
                        )
                })
                .into_any_element();
        }

        let repo = self.active_repo();
        let (staged_count, unstaged_count) = repo
            .and_then(|r| match &r.status {
                Loadable::Ready(s) => Some((s.staged.len(), s.unstaged.len())),
                _ => None,
            })
            .unwrap_or((0, 0));

        let repo_id = self.active_repo_id();

        let stage_all = zed::Button::new("stage_all", "Stage all changes")
            .style(zed::ButtonStyle::Subtle)
            .on_click(theme, cx, |this, _e, _w, cx| {
                let Some(repo_id) = this.active_repo_id() else {
                    return;
                };
                this.store.dispatch(Msg::StagePaths {
                    repo_id,
                    paths: Vec::new(),
                });
                cx.notify();
            })
            .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                let text: SharedString = "Stage all changes".into();
                if *hovering {
                    this.tooltip_text = Some(text);
                } else if this.tooltip_text.as_ref() == Some(&text) {
                    this.tooltip_text = None;
                }
                cx.notify();
            }));

        let unstage_all = zed::Button::new("unstage_all", "Unstage all changes")
            .style(zed::ButtonStyle::Subtle)
            .on_click(theme, cx, |this, _e, _w, cx| {
                let Some(repo_id) = this.active_repo_id() else {
                    return;
                };
                this.store.dispatch(Msg::UnstagePaths {
                    repo_id,
                    paths: Vec::new(),
                });
                cx.notify();
            })
            .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                let text: SharedString = "Unstage all changes".into();
                if *hovering {
                    this.tooltip_text = Some(text);
                } else if this.tooltip_text.as_ref() == Some(&text) {
                    this.tooltip_text = None;
                }
                cx.notify();
            }));

        let section_header = |id: &'static str,
                              label: &'static str,
                              show_action: bool,
                              action: gpui::AnyElement|
         -> gpui::AnyElement {
            div()
                .id(id)
                .flex()
                .items_center()
                .justify_between()
                .h(px(zed::CONTROL_HEIGHT_MD_PX))
                .px_2()
                .bg(theme.colors.surface_bg_elevated)
                .border_b_1()
                .border_color(theme.colors.border)
                .child(div().text_sm().font_weight(FontWeight::BOLD).child(label))
                .when(show_action, |d| d.child(action))
                .into_any_element()
        };

        let unstaged_body = if unstaged_count == 0 {
            zed::empty_state(theme, "Unstaged", "Clean.").into_any_element()
        } else {
            self.status_list(cx, DiffArea::Unstaged, unstaged_count)
        };

        let staged_list = if staged_count == 0 {
            zed::empty_state(theme, "Staged", "No staged changes.").into_any_element()
        } else {
            self.status_list(cx, DiffArea::Staged, staged_count)
        };

        let unstaged_section = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0))
            .child(section_header(
                "unstaged_header",
                "Unstaged",
                unstaged_count > 0,
                stage_all.into_any_element(),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(unstaged_body),
            );

        let staged_section = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0))
            .child(section_header(
                "staged_header",
                "Staged",
                staged_count > 0,
                unstage_all.into_any_element(),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(staged_list),
            );

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0))
            .h_full()
            .child(if repo_id.is_some() {
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(unstaged_section)
                    .child(div().border_t_1().border_color(theme.colors.border))
                    .child(staged_section)
                    .child(
                        div()
                            .border_t_1()
                            .border_color(theme.colors.border)
                            .bg(theme.colors.surface_bg)
                            .px_2()
                            .py_2()
                            .child(self.commit_box(cx)),
                    )
                    .into_any_element()
            } else {
                zed::empty_state(theme, "Changes", "No repository selected.").into_any_element()
            })
            .into_any_element()
    }

    pub(in super::super) fn status_list(
        &mut self,
        cx: &mut gpui::Context<Self>,
        area: DiffArea,
        count: usize,
    ) -> AnyElement {
        let theme = self.theme;
        if count == 0 {
            return zed::empty_state(theme, "Status", "Clean.").into_any_element();
        }
        match area {
            DiffArea::Unstaged => {
                let list =
                    uniform_list("unstaged", count, cx.processor(Self::render_unstaged_rows))
                        .flex_1()
                        .min_h(px(0.0))
                        .track_scroll(self.unstaged_scroll.clone());
                let scroll_handle = self.unstaged_scroll.0.borrow().base_handle.clone();
                div()
                    .id("unstaged_scroll_container")
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .h_full()
                    .min_h(px(0.0))
                    .child(list)
                    .child(zed::Scrollbar::new("unstaged_scrollbar", scroll_handle).render(theme))
                    .into_any_element()
            }
            DiffArea::Staged => {
                let list = uniform_list("staged", count, cx.processor(Self::render_staged_rows))
                    .flex_1()
                    .min_h(px(0.0))
                    .track_scroll(self.staged_scroll.clone());
                let scroll_handle = self.staged_scroll.0.borrow().base_handle.clone();
                div()
                    .id("staged_scroll_container")
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .h_full()
                    .min_h(px(0.0))
                    .child(list)
                    .child(zed::Scrollbar::new("staged_scrollbar", scroll_handle).render(theme))
                    .into_any_element()
            }
        }
    }

    pub(in super::super) fn commit_box(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.commit_message_input.clone())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .child("Commit staged changes"),
                    )
                    .child(
                        zed::Button::new("commit", "Commit")
                            .style(zed::ButtonStyle::Filled)
                            .on_click(theme, cx, |this, _e, _w, cx| {
                                let Some(repo_id) = this.active_repo_id() else {
                                    return;
                                };
                                let message = this
                                    .commit_message_input
                                    .read_with(cx, |i, _| i.text().trim().to_string());
                                if message.is_empty() {
                                    return;
                                }
                                this.store.dispatch(Msg::Commit { repo_id, message });
                                this.commit_message_input
                                    .update(cx, |i, cx| i.set_text(String::new(), cx));
                                cx.notify();
                            })
                            .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                                let text: SharedString = "Commit staged changes".into();
                                if *hovering {
                                    this.tooltip_text = Some(text);
                                } else if this.tooltip_text.as_ref() == Some(&text) {
                                    this.tooltip_text = None;
                                }
                                cx.notify();
                            })),
                    ),
            )
    }
}
