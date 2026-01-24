use super::*;

impl GitGpuiView {
    pub(in super::super) fn history_view(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        self.ensure_history_cache(cx);
        let repo = self.active_repo();
        let commits_count = self
            .history_cache
            .as_ref()
            .map(|c| c.visible_indices.len())
            .unwrap_or(0);
        let show_working_tree_summary_row = repo
            .and_then(|r| match &r.status {
                Loadable::Ready(s) => Some(!s.unstaged.is_empty()),
                _ => None,
            })
            .unwrap_or(false);
        let count = commits_count + usize::from(show_working_tree_summary_row);

        let bg = theme.colors.window_bg;

        let body: AnyElement = if count == 0 {
            match repo.map(|r| &r.log) {
                None => zed::empty_state(theme, "History", "No repository.").into_any_element(),
                Some(Loadable::Loading) => {
                    zed::empty_state(theme, "History", "Loading").into_any_element()
                }
                Some(Loadable::Error(e)) => {
                    zed::empty_state(theme, "History", e.clone()).into_any_element()
                }
                Some(Loadable::NotLoaded) | Some(Loadable::Ready(_)) => {
                    zed::empty_state(theme, "History", "No commits.").into_any_element()
                }
            }
        } else {
            let list = uniform_list(
                "history_main",
                count,
                cx.processor(Self::render_history_table_rows),
            )
            .h_full()
            .track_scroll(self.history_scroll.clone());
            let (scroll_handle, should_load_more) = {
                let state = self.history_scroll.0.borrow();
                let scroll_handle = state.base_handle.clone();
                let max_offset = scroll_handle.max_offset().height.max(px(0.0));
                let should_load_by_scroll = if max_offset > px(0.0) {
                    scroll_is_near_bottom(&scroll_handle, px(240.0))
                } else {
                    true
                };
                let should_load_more = state.last_item_size.is_some() && repo.is_some_and(|repo| {
                    !repo.log_loading_more
                        && matches!(&repo.log, Loadable::Ready(page) if page.next_cursor.is_some())
                }) && should_load_by_scroll;
                (scroll_handle, should_load_more)
            };
            if should_load_more && let Some(repo_id) = self.active_repo_id() {
                self.store.dispatch(Msg::LoadMoreHistory { repo_id });
            }
            div()
                .id("history_main_scroll_container")
                .relative()
                .h_full()
                .child(list)
                .child(zed::Scrollbar::new("history_main_scrollbar", scroll_handle).render(theme))
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .h_full()
            .min_h(px(0.0))
            .bg(bg)
            .child(
                self.history_column_headers(cx)
                    .bg(bg)
                    .border_b_1()
                    .border_color(theme.colors.border),
            )
            .child(div().flex_1().min_h(px(0.0)).child(body))
    }

    pub(in super::super) fn diff_view(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let repo_id = self.active_repo_id();

        // Intentionally no outer panel header; keep diff controls in the inner header.

        let title: AnyElement = self
            .active_repo()
            .and_then(|r| r.diff_target.as_ref())
            .map(|t| {
                let (icon, color, text): (Option<&'static str>, gpui::Rgba, SharedString) =
                    match t {
                    DiffTarget::WorkingTree { path, area } => {
                        let kind = self.active_repo().and_then(|repo| match &repo.status {
                            Loadable::Ready(status) => {
                                let list = match area {
                                    DiffArea::Unstaged => &status.unstaged,
                                    DiffArea::Staged => &status.staged,
                                };
                                list.iter().find(|e| e.path == *path).map(|e| e.kind)
                            }
                            _ => None,
                        });

                        let (icon, color) = match kind.unwrap_or(FileStatusKind::Modified) {
                            FileStatusKind::Untracked | FileStatusKind::Added => {
                                ("+", theme.colors.success)
                            }
                            FileStatusKind::Modified => ("✎", theme.colors.warning),
                            FileStatusKind::Deleted => ("−", theme.colors.danger),
                            FileStatusKind::Renamed => ("→", theme.colors.accent),
                            FileStatusKind::Conflicted => ("!", theme.colors.danger),
                        };
                        (Some(icon), color, self.cached_path_display(path))
                    }
                    DiffTarget::Commit { commit_id: _, path } => match path {
                        Some(path) => (
                            Some("✎"),
                            theme.colors.text_muted,
                            self.cached_path_display(path),
                        ),
                        None => (Some("✎"), theme.colors.text_muted, "Full diff".into()),
                    },
                };

                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w(px(0.0))
                    .overflow_hidden()
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
                            .font_weight(FontWeight::BOLD)
                            .line_clamp(1)
                            .whitespace_nowrap()
                            .child(text),
                    )
                    .into_any_element()
            })
            .unwrap_or_else(|| {
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child("Select a file to view diff")
                    .into_any_element()
            });

        let untracked_preview_path = self.untracked_worktree_preview_path();
        let added_preview_path = self.added_file_preview_abs_path();

        if let Some(path) = untracked_preview_path.clone() {
            self.ensure_worktree_preview_loaded(path, cx);
        } else if let Some(path) = added_preview_path.clone() {
            self.ensure_preview_loading(path);
        }

        let is_file_preview = untracked_preview_path.is_some() || added_preview_path.is_some();
        let wants_file_diff = !is_file_preview
            && self
                .active_repo()
                .is_some_and(|r| Self::is_file_diff_target(r.diff_target.as_ref()));

        let repo = self.active_repo();

        let diff_nav_hotkey_hint = |label: &'static str| {
            div()
                .font_family("monospace")
                .text_xs()
                .text_color(theme.colors.text_muted)
                .child(label)
        };

        let mut controls = div().flex().items_center().gap_1();
        if !is_file_preview {
            let nav_entries = self.diff_nav_entries();
            let current_nav_ix = self.diff_selection_anchor.unwrap_or(0);
            let can_nav_prev = Self::diff_nav_prev_target(&nav_entries, current_nav_ix).is_some();
            let can_nav_next = Self::diff_nav_next_target(&nav_entries, current_nav_ix).is_some();

            controls = controls
                .child(
                    zed::Button::new("diff_inline", "Inline")
                        .style(if self.diff_view == DiffViewMode::Inline {
                            zed::ButtonStyle::Filled
                        } else {
                            zed::ButtonStyle::Outlined
                        })
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.diff_view = DiffViewMode::Inline;
                            this.diff_text_segments_cache.clear();
                            cx.notify();
                        })
                        .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                            let text: SharedString = "Inline diff view (Alt+I)".into();
                            let mut changed = false;
                            if *hovering {
                                changed |= this.set_tooltip_text_if_changed(Some(text));
                            } else if this.tooltip_text.as_ref() == Some(&text) {
                                changed |= this.set_tooltip_text_if_changed(None);
                            }
                            if changed {
                                cx.notify();
                            }
                        })),
                )
                .child(
                    zed::Button::new("diff_split", "Split")
                        .style(if self.diff_view == DiffViewMode::Split {
                            zed::ButtonStyle::Filled
                        } else {
                            zed::ButtonStyle::Outlined
                        })
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.diff_view = DiffViewMode::Split;
                            this.diff_text_segments_cache.clear();
                            cx.notify();
                        })
                        .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                            let text: SharedString = "Split diff view (Alt+S)".into();
                            let mut changed = false;
                            if *hovering {
                                changed |= this.set_tooltip_text_if_changed(Some(text));
                            } else if this.tooltip_text.as_ref() == Some(&text) {
                                changed |= this.set_tooltip_text_if_changed(None);
                            }
                            if changed {
                                cx.notify();
                            }
                        })),
                )
                .child(
                    zed::Button::new("diff_prev_hunk", "Prev")
                        .end_slot(diff_nav_hotkey_hint("F2"))
                        .style(zed::ButtonStyle::Outlined)
                        .disabled(!can_nav_prev)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.diff_jump_prev();
                            cx.notify();
                        })
                        .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                            let text: SharedString =
                                "Previous change (F2 / Shift+F7 / Alt+Up)".into();
                            let mut changed = false;
                            if *hovering {
                                changed |= this.set_tooltip_text_if_changed(Some(text));
                            } else if this.tooltip_text.as_ref() == Some(&text) {
                                changed |= this.set_tooltip_text_if_changed(None);
                            }
                            if changed {
                                cx.notify();
                            }
                        })),
                )
                .child(
                    zed::Button::new("diff_next_hunk", "Next")
                        .end_slot(diff_nav_hotkey_hint("F3"))
                        .style(zed::ButtonStyle::Outlined)
                        .disabled(!can_nav_next)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.diff_jump_next();
                            cx.notify();
                        })
                        .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                            let text: SharedString = "Next change (F3 / F7 / Alt+Down)".into();
                            let mut changed = false;
                            if *hovering {
                                changed |= this.set_tooltip_text_if_changed(Some(text));
                            } else if this.tooltip_text.as_ref() == Some(&text) {
                                changed |= this.set_tooltip_text_if_changed(None);
                            }
                            if changed {
                                cx.notify();
                            }
                        })),
                )
                .when(!wants_file_diff, |controls| {
                    controls.child(
                        zed::Button::new("diff_hunks", "Hunks")
                            .style(zed::ButtonStyle::Outlined)
                            .on_click(theme, cx, |this, e, window, cx| {
                                let _ = this.ensure_diff_hunk_picker_search_input(window, cx);
                                this.popover = Some(PopoverKind::DiffHunks);
                                this.popover_anchor = Some(e.position());
                                cx.notify();
                            })
                            .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                                let text: SharedString = "Jump to hunk (Alt+H)".into();
                                let mut changed = false;
                                if *hovering {
                                    changed |= this.set_tooltip_text_if_changed(Some(text));
                                } else if this.tooltip_text.as_ref() == Some(&text) {
                                    changed |= this.set_tooltip_text_if_changed(None);
                                }
                                if changed {
                                    cx.notify();
                                }
                            })),
                    )
                });
        }

        if let Some(repo_id) = repo_id {
            controls = controls.child(
                zed::Button::new("diff_close", "✕")
                    .style(zed::ButtonStyle::Transparent)
                    .on_click(theme, cx, move |this, _e, _w, cx| {
                        this.store.dispatch(Msg::ClearDiffSelection { repo_id });
                        cx.notify();
                    })
                    .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                        let text: SharedString = "Close diff".into();
                        let mut changed = false;
                        if *hovering {
                            changed |= this.set_tooltip_text_if_changed(Some(text));
                        } else if this.tooltip_text.as_ref() == Some(&text) {
                            changed |= this.set_tooltip_text_if_changed(None);
                        }
                        if changed {
                            cx.notify();
                        }
                    })),
            );
        }

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .child(title),
            )
            .child(controls);

        let body: AnyElement = if is_file_preview {
            if added_preview_path.is_some() {
                self.try_populate_worktree_preview_from_diff_file();
            }
            match &self.worktree_preview {
                Loadable::NotLoaded | Loadable::Loading => {
                    zed::empty_state(theme, "File", "Loading").into_any_element()
                }
                Loadable::Error(e) => {
                    self.diff_raw_input.update(cx, |input, cx| {
                        input.set_theme(theme, cx);
                        input.set_text(e.clone(), cx);
                        input.set_read_only(true, cx);
                    });
                    div()
                        .id("worktree_preview_error_scroll")
                        .font_family("monospace")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h(px(0.0))
                        .overflow_y_scroll()
                        .child(self.diff_raw_input.clone())
                        .into_any_element()
                }
                Loadable::Ready(lines) => {
                    if lines.is_empty() {
                        zed::empty_state(theme, "File", "Empty file.").into_any_element()
                    } else {
                        let list = uniform_list(
                            "worktree_preview_list",
                            lines.len(),
                            cx.processor(Self::render_worktree_preview_rows),
                        )
                        .h_full()
                        .min_h(px(0.0))
                        .track_scroll(self.worktree_preview_scroll.clone());

                        let scroll_handle =
                            self.worktree_preview_scroll.0.borrow().base_handle.clone();
                        div()
                            .id("worktree_preview_scroll_container")
                            .debug_selector(|| "worktree_preview_scroll_container".to_string())
                            .relative()
                            .h_full()
                            .min_h(px(0.0))
                            .child(list)
                            .child(
                                zed::Scrollbar::new("worktree_preview_scrollbar", scroll_handle)
                                    .render(theme),
                            )
                            .into_any_element()
                    }
                }
            }
        } else {
            match repo {
                None => zed::empty_state(theme, "Diff", "No repository.").into_any_element(),
                Some(repo) => match &repo.diff {
                    Loadable::NotLoaded => {
                        zed::empty_state(theme, "Diff", "Select a file.").into_any_element()
                    }
                    Loadable::Loading => {
                        zed::empty_state(theme, "Diff", "Loading").into_any_element()
                    }
                    Loadable::Error(e) => {
                        self.diff_raw_input.update(cx, |input, cx| {
                            input.set_theme(theme, cx);
                            input.set_text(e.clone(), cx);
                            input.set_read_only(true, cx);
                        });
                        div()
                            .id("diff_error_scroll")
                            .font_family("monospace")
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h(px(0.0))
                            .overflow_y_scroll()
                            .child(self.diff_raw_input.clone())
                            .into_any_element()
                    }
                    Loadable::Ready(diff) => {
                        if wants_file_diff {
                            enum DiffFileState {
                                NotLoaded,
                                Loading,
                                Error(String),
                                Ready { has_file: bool },
                            }

                            let diff_file_state = match &repo.diff_file {
                                Loadable::NotLoaded => DiffFileState::NotLoaded,
                                Loadable::Loading => DiffFileState::Loading,
                                Loadable::Error(e) => DiffFileState::Error(e.clone()),
                                Loadable::Ready(file) => DiffFileState::Ready {
                                    has_file: file.is_some(),
                                },
                            };

                            self.ensure_file_diff_cache();
                            match diff_file_state {
                                DiffFileState::NotLoaded => {
                                    zed::empty_state(theme, "Diff", "Select a file.")
                                        .into_any_element()
                                }
                                DiffFileState::Loading => {
                                    zed::empty_state(theme, "Diff", "Loading").into_any_element()
                                }
                                DiffFileState::Error(e) => {
                                    self.diff_raw_input.update(cx, |input, cx| {
                                        input.set_theme(theme, cx);
                                        input.set_text(e, cx);
                                        input.set_read_only(true, cx);
                                    });
                                    div()
                                        .id("diff_file_error_scroll")
                                        .font_family("monospace")
                                        .bg(theme.colors.window_bg)
                                        .flex()
                                        .flex_col()
                                        .flex_1()
                                        .min_h(px(0.0))
                                        .overflow_y_scroll()
                                        .child(self.diff_raw_input.clone())
                                        .into_any_element()
                                }
                                DiffFileState::Ready { has_file } => {
                                    if !has_file || !self.is_file_diff_view_active() {
                                        zed::empty_state(
                                            theme,
                                            "Diff",
                                            "No file contents available.",
                                        )
                                        .into_any_element()
                                    } else {
                                        self.ensure_diff_visible_indices();
                                        self.maybe_autoscroll_diff_to_first_change();

                                        let total_len = match self.diff_view {
                                            DiffViewMode::Inline => {
                                                self.file_diff_inline_cache.len()
                                            }
                                            DiffViewMode::Split => self.file_diff_cache_rows.len(),
                                        };
                                        if total_len == 0 {
                                            zed::empty_state(theme, "Diff", "Empty file.")
                                                .into_any_element()
                                        } else if self.diff_visible_indices.is_empty() {
                                            zed::empty_state(theme, "Diff", "Nothing to render.")
                                                .into_any_element()
                                        } else {
                                            let scroll_handle =
                                                self.diff_scroll.0.borrow().base_handle.clone();
                                            let markers = self.diff_scrollbar_markers_cache.clone();
                                            match self.diff_view {
                                                DiffViewMode::Inline => {
                                                    let list = uniform_list(
                                                        "diff",
                                                        self.diff_visible_indices.len(),
                                                        cx.processor(Self::render_diff_rows),
                                                    )
                                                    .h_full()
                                                    .min_h(px(0.0))
                                                    .track_scroll(self.diff_scroll.clone());
                                                    div()
                                                        .id("diff_scroll_container")
                                                        .relative()
                                                        .h_full()
                                                        .min_h(px(0.0))
                                                        .bg(theme.colors.window_bg)
                                                        .child(list)
                                                        .child(
                                                            zed::Scrollbar::new(
                                                                "diff_scrollbar",
                                                                scroll_handle,
                                                            )
                                                            .markers(markers)
                                                            .render(theme),
                                                        )
                                                        .into_any_element()
                                                }
                                                DiffViewMode::Split => {
                                                    let count = self.diff_visible_indices.len();
                                                    let left = uniform_list(
                                                        "diff_split_left",
                                                        count,
                                                        cx.processor(
                                                            Self::render_diff_split_left_rows,
                                                        ),
                                                    )
                                                    .h_full()
                                                    .min_h(px(0.0))
                                                    .track_scroll(self.diff_scroll.clone());
                                                    let right = uniform_list(
                                                        "diff_split_right",
                                                        count,
                                                        cx.processor(
                                                            Self::render_diff_split_right_rows,
                                                        ),
                                                    )
                                                    .h_full()
                                                    .min_h(px(0.0))
                                                    .track_scroll(self.diff_scroll.clone());

                                                    let columns_header = zed::split_columns_header(
                                                        theme,
                                                        "A (local / before)",
                                                        "B (remote / after)",
                                                    );

                                                    div()
                                                        .id("diff_split_scroll_container")
                                                        .relative()
                                                        .h_full()
                                                        .min_h(px(0.0))
                                                        .flex()
                                                        .flex_col()
                                                        .bg(theme.colors.window_bg)
                                                        .child(columns_header)
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .min_h(px(0.0))
                                                                .flex()
                                                                .child(
                                                                    div()
                                                                        .flex_1()
                                                                        .min_w(px(0.0))
                                                                        .h_full()
                                                                        .child(left),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .w(px(1.0))
                                                                        .h_full()
                                                                        .bg(theme.colors.border),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .flex_1()
                                                                        .min_w(px(0.0))
                                                                        .h_full()
                                                                        .child(right),
                                                                ),
                                                        )
                                                        .child(
                                                            zed::Scrollbar::new(
                                                                "diff_scrollbar",
                                                                scroll_handle,
                                                            )
                                                            .markers(markers)
                                                            .render(theme),
                                                        )
                                                        .into_any_element()
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            if self.diff_cache_repo_id != Some(repo.id)
                                || self.diff_cache_rev != repo.diff_rev
                                || self.diff_cache_target != repo.diff_target
                                || self.diff_cache.len() != diff.lines.len()
                            {
                                self.rebuild_diff_cache();
                            }

                            self.ensure_diff_visible_indices();
                            self.maybe_autoscroll_diff_to_first_change();
                            if self.diff_cache.is_empty() {
                                zed::empty_state(theme, "Diff", "No differences.")
                                    .into_any_element()
                            } else if self.diff_visible_indices.is_empty() {
                                zed::empty_state(theme, "Diff", "Nothing to render.")
                                    .into_any_element()
                            } else {
                                let scroll_handle = self.diff_scroll.0.borrow().base_handle.clone();
                                let markers = self.diff_scrollbar_markers_cache.clone();
                                match self.diff_view {
                                    DiffViewMode::Inline => {
                                        let list = uniform_list(
                                            "diff",
                                            self.diff_visible_indices.len(),
                                            cx.processor(Self::render_diff_rows),
                                        )
                                        .h_full()
                                        .min_h(px(0.0))
                                        .track_scroll(self.diff_scroll.clone());
                                        div()
                                            .id("diff_scroll_container")
                                            .relative()
                                            .h_full()
                                            .min_h(px(0.0))
                                            .bg(theme.colors.window_bg)
                                            .child(list)
                                            .child(
                                                zed::Scrollbar::new(
                                                    "diff_scrollbar",
                                                    scroll_handle,
                                                )
                                                .markers(markers)
                                                .render(theme),
                                            )
                                            .into_any_element()
                                    }
                                    DiffViewMode::Split => {
                                        let count = self.diff_visible_indices.len();
                                        let left = uniform_list(
                                            "diff_split_left",
                                            count,
                                            cx.processor(Self::render_diff_split_left_rows),
                                        )
                                        .h_full()
                                        .min_h(px(0.0))
                                        .track_scroll(self.diff_scroll.clone());
                                        let right = uniform_list(
                                            "diff_split_right",
                                            count,
                                            cx.processor(Self::render_diff_split_right_rows),
                                        )
                                        .h_full()
                                        .min_h(px(0.0))
                                        .track_scroll(self.diff_scroll.clone());

                                        let columns_header = zed::split_columns_header(
                                            theme,
                                            "A (local / before)",
                                            "B (remote / after)",
                                        );

                                        div()
                                            .id("diff_split_scroll_container")
                                            .relative()
                                            .h_full()
                                            .min_h(px(0.0))
                                            .flex()
                                            .flex_col()
                                            .bg(theme.colors.window_bg)
                                            .child(columns_header)
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_h(px(0.0))
                                                    .flex()
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .min_w(px(0.0))
                                                            .h_full()
                                                            .child(left),
                                                    )
                                                    .child(
                                                        div()
                                                            .w(px(1.0))
                                                            .h_full()
                                                            .bg(theme.colors.border),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .min_w(px(0.0))
                                                            .h_full()
                                                            .child(right),
                                                    ),
                                            )
                                            .child(
                                                zed::Scrollbar::new(
                                                    "diff_scrollbar",
                                                    scroll_handle,
                                                )
                                                .markers(markers)
                                                .render(theme),
                                            )
                                            .into_any_element()
                                    }
                                }
                            }
                        }
                    }
                },
            }
        };

        self.diff_text_layout_cache_epoch = self.diff_text_layout_cache_epoch.wrapping_add(1);
        self.diff_text_hitboxes.clear();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .h_full()
            .min_h(px(0.0))
            .bg(theme.colors.surface_bg_elevated)
            .track_focus(&self.diff_panel_focus_handle)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseDownEvent, window, _cx| {
                    window.focus(&this.diff_panel_focus_handle);
                }),
            )
            .on_key_down(cx.listener(|this, e: &gpui::KeyDownEvent, window, cx| {
                let key = e.keystroke.key.as_str();
                let mods = e.keystroke.modifiers;

                let mut handled = false;

                if key == "escape" && !mods.control && !mods.alt && !mods.platform && !mods.function
                {
                    if let Some(repo_id) = this.active_repo_id() {
                        this.store.dispatch(Msg::ClearDiffSelection { repo_id });
                        handled = true;
                    }
                }

                if !handled
                    && key == "space"
                    && !mods.control
                    && !mods.alt
                    && !mods.platform
                    && !mods.function
                    && !this
                        .diff_raw_input
                        .read(cx)
                        .focus_handle()
                        .is_focused(window)
                    && let Some(repo_id) = this.active_repo_id()
                    && let Some(repo) = this.active_repo()
                    && let Some(DiffTarget::WorkingTree { path, area }) = repo.diff_target.clone()
                {
                    let next_path_in_area = |entries: &[gitgpui_core::domain::FileStatus]| {
                        let Some(current_ix) = entries.iter().position(|e| e.path == path) else {
                            return None;
                        };
                        if entries.len() <= 1 {
                            return None;
                        }
                        let next_ix = if current_ix + 1 < entries.len() {
                            current_ix + 1
                        } else {
                            current_ix.saturating_sub(1)
                        };
                        entries.get(next_ix).map(|e| e.path.clone())
                    };

                    match (&repo.status, area) {
                        (Loadable::Ready(status), DiffArea::Unstaged) => {
                            this.store.dispatch(Msg::StagePath {
                                repo_id,
                                path: path.clone(),
                            });
                            if let Some(next_path) = next_path_in_area(&status.unstaged) {
                                this.store.dispatch(Msg::SelectDiff {
                                    repo_id,
                                    target: DiffTarget::WorkingTree {
                                        path: next_path,
                                        area: DiffArea::Unstaged,
                                    },
                                });
                            } else {
                                this.store.dispatch(Msg::ClearDiffSelection { repo_id });
                            }
                        }
                        (Loadable::Ready(status), DiffArea::Staged) => {
                            this.store.dispatch(Msg::UnstagePath {
                                repo_id,
                                path: path.clone(),
                            });
                            if let Some(next_path) = next_path_in_area(&status.staged) {
                                this.store.dispatch(Msg::SelectDiff {
                                    repo_id,
                                    target: DiffTarget::WorkingTree {
                                        path: next_path,
                                        area: DiffArea::Staged,
                                    },
                                });
                            } else {
                                this.store.dispatch(Msg::ClearDiffSelection { repo_id });
                            }
                        }
                        (_, DiffArea::Unstaged) => {
                            this.store.dispatch(Msg::StagePath {
                                repo_id,
                                path: path.clone(),
                            });
                        }
                        (_, DiffArea::Staged) => {
                            this.store.dispatch(Msg::UnstagePath {
                                repo_id,
                                path: path.clone(),
                            });
                        }
                    }
                    this.rebuild_diff_cache();
                    handled = true;
                }

                let is_file_preview = this.untracked_worktree_preview_path().is_some()
                    || this.added_file_preview_abs_path().is_some();
                if is_file_preview {
                    if handled {
                        cx.stop_propagation();
                        cx.notify();
                    }
                    return;
                }

                let copy_target_is_focused = this
                    .diff_raw_input
                    .read(cx)
                    .focus_handle()
                    .is_focused(window);

                if mods.alt && !mods.control && !mods.platform && !mods.function {
                    match key {
                        "i" => {
                            this.diff_view = DiffViewMode::Inline;
                            this.diff_text_segments_cache.clear();
                            handled = true;
                        }
                        "s" => {
                            this.diff_view = DiffViewMode::Split;
                            this.diff_text_segments_cache.clear();
                            handled = true;
                        }
                        "h" => {
                            let is_file_preview = this.untracked_worktree_preview_path().is_some()
                                || this.added_file_preview_abs_path().is_some();
                            if !is_file_preview
                                && !this.active_repo().is_some_and(|r| {
                                    Self::is_file_diff_target(r.diff_target.as_ref())
                                })
                            {
                                let _ = this.ensure_diff_hunk_picker_search_input(window, cx);
                                this.popover = Some(PopoverKind::DiffHunks);
                                this.popover_anchor = Some(this.last_mouse_pos);
                                handled = true;
                            }
                        }
                        "up" => {
                            this.diff_jump_prev();
                            handled = true;
                        }
                        "down" => {
                            this.diff_jump_next();
                            handled = true;
                        }
                        _ => {}
                    }
                }

                if !handled
                    && key == "f7"
                    && !mods.control
                    && !mods.alt
                    && !mods.platform
                    && !mods.function
                {
                    if mods.shift {
                        this.diff_jump_prev();
                    } else {
                        this.diff_jump_next();
                    }
                    handled = true;
                }

                if !handled
                    && key == "f2"
                    && !mods.control
                    && !mods.alt
                    && !mods.platform
                    && !mods.function
                {
                    this.diff_jump_prev();
                    handled = true;
                }

                if !handled
                    && key == "f3"
                    && !mods.control
                    && !mods.alt
                    && !mods.platform
                    && !mods.function
                {
                    this.diff_jump_next();
                    handled = true;
                }

                if !handled
                    && !copy_target_is_focused
                    && (mods.control || mods.platform)
                    && !mods.alt
                    && !mods.function
                    && key == "c"
                    && this.diff_text_has_selection()
                {
                    this.copy_selected_diff_text_to_clipboard(cx);
                    handled = true;
                }

                if !handled
                    && !copy_target_is_focused
                    && (mods.control || mods.platform)
                    && !mods.alt
                    && !mods.function
                    && key == "a"
                {
                    this.select_all_diff_text();
                    handled = true;
                }

                if handled {
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(
                header
                    .h(px(zed::CONTROL_HEIGHT_MD_PX))
                    .px_2()
                    .bg(theme.colors.surface_bg_elevated)
                    .border_b_1()
                    .border_color(theme.colors.border),
            )
            .child(div().flex_1().min_h(px(0.0)).w_full().h_full().child(body))
            .child(DiffTextSelectionTracker { view: cx.entity() })
    }
}
