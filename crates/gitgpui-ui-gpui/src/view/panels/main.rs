use super::*;

impl MainPaneView {
    pub(in super::super) fn history_view(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        self.ensure_history_cache(cx);
        let (show_working_tree_summary_row, _) = self.ensure_history_worktree_summary_cache();
        let repo = self.active_repo();
        let commits_count = self
            .history_cache
            .as_ref()
            .map(|c| c.visible_indices.len())
            .unwrap_or(0);
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

    pub(in super::super) fn conflict_requires_resolver(
        conflict: Option<gitgpui_core::domain::FileConflictKind>,
    ) -> bool {
        matches!(
            conflict,
            Some(gitgpui_core::domain::FileConflictKind::BothModified)
        )
    }

    pub(in super::super) fn diff_view(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let repo_id = self.active_repo_id();

        // Intentionally no outer panel header; keep diff controls in the inner header.

        let title: AnyElement = self
            .active_repo()
            .and_then(|r| r.diff_target.as_ref())
            .map(|t| {
                let (icon, color, text): (Option<&'static str>, gpui::Rgba, SharedString) = match t
                {
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
        let deleted_preview_path = self.deleted_file_preview_abs_path();

        if let Some(path) = untracked_preview_path.clone() {
            self.ensure_worktree_preview_loaded(path, cx);
        } else if let Some(path) = added_preview_path.clone().or(deleted_preview_path.clone()) {
            self.ensure_preview_loading(path);
        }

        let is_file_preview = untracked_preview_path.is_some()
            || added_preview_path.is_some()
            || deleted_preview_path.is_some();
        let wants_file_diff = !is_file_preview
            && self
                .active_repo()
                .is_some_and(|r| Self::is_file_diff_target(r.diff_target.as_ref()));

        let repo = self.active_repo();
        let conflict_target = repo.and_then(|repo| {
            let DiffTarget::WorkingTree { path, area } = repo.diff_target.as_ref()? else {
                return None;
            };
            if *area != DiffArea::Unstaged {
                return None;
            }
            match &repo.status {
                Loadable::Ready(status) => {
                    let conflict = status
                        .unstaged
                        .iter()
                        .find(|e| e.path == *path && e.kind == FileStatusKind::Conflicted)?;
                    Some((path.clone(), conflict.conflict))
                }
                _ => None,
            }
        });
        let (conflict_target_path, conflict_kind) = conflict_target
            .map(|(path, kind)| (Some(path), kind))
            .unwrap_or((None, None));
        let is_conflict_resolver = Self::conflict_requires_resolver(conflict_kind);
        let is_conflict_compare = conflict_target_path.is_some() && !is_conflict_resolver;

        let diff_nav_hotkey_hint = |label: &'static str| {
            div()
                .font_family("monospace")
                .text_xs()
                .text_color(theme.colors.text_muted)
                .child(label)
        };

        let file_nav_controls = (|| {
            let repo_id = repo_id?;
            let repo = self.active_repo()?;
            let DiffTarget::WorkingTree { path, area } = repo.diff_target.as_ref()? else {
                return None;
            };
            let area = *area;

            let (prev, next) = match &repo.status {
                Loadable::Ready(status) => {
                    let entries = match area {
                        DiffArea::Unstaged => status.unstaged.as_slice(),
                        DiffArea::Staged => status.staged.as_slice(),
                    };
                    if let Some(current_ix) = entries.iter().position(|e| e.path == *path) {
                        let prev = current_ix
                            .checked_sub(1)
                            .and_then(|ix| entries.get(ix).map(|e| (ix, e.path.clone())));
                        let next_ix = current_ix + 1;
                        let next = (next_ix < entries.len())
                            .then(|| (next_ix, entries[next_ix].path.clone()));
                        (prev, next)
                    } else {
                        (None, None)
                    }
                }
                _ => (None, None),
            };

            let prev_disabled = prev.is_none();
            let next_disabled = next.is_none();

            let prev_target = prev;
            let next_target = next;

            let prev_tooltip: SharedString = "Previous file (F1)".into();
            let next_tooltip: SharedString = "Next file (F4)".into();

            let prev_btn = zed::Button::new("diff_prev_file", "Prev file")
                .end_slot(diff_nav_hotkey_hint("F1"))
                .style(zed::ButtonStyle::Outlined)
                .disabled(prev_disabled)
                .on_click(theme, cx, move |this, _e, window, cx| {
                    let Some((target_ix, target_path)) = prev_target.as_ref() else {
                        return;
                    };
                    window.focus(&this.diff_panel_focus_handle);
                    this.clear_status_multi_selection(repo_id, cx);
                    this.store.dispatch(Msg::SelectDiff {
                        repo_id,
                        target: DiffTarget::WorkingTree {
                            path: target_path.clone(),
                            area,
                        },
                    });
                    this.scroll_status_list_to_ix(area, *target_ix, cx);
                    cx.notify();
                })
                .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                    let mut changed = false;
                    if *hovering {
                        changed |= this.set_tooltip_text_if_changed(Some(prev_tooltip.clone()), cx);
                    } else {
                        changed |= this.clear_tooltip_if_matches(&prev_tooltip, cx);
                    }
                    if changed {
                        cx.notify();
                    }
                }));

            let next_btn = zed::Button::new("diff_next_file", "Next file")
                .end_slot(diff_nav_hotkey_hint("F4"))
                .style(zed::ButtonStyle::Outlined)
                .disabled(next_disabled)
                .on_click(theme, cx, move |this, _e, window, cx| {
                    let Some((target_ix, target_path)) = next_target.as_ref() else {
                        return;
                    };
                    window.focus(&this.diff_panel_focus_handle);
                    this.clear_status_multi_selection(repo_id, cx);
                    this.store.dispatch(Msg::SelectDiff {
                        repo_id,
                        target: DiffTarget::WorkingTree {
                            path: target_path.clone(),
                            area,
                        },
                    });
                    this.scroll_status_list_to_ix(area, *target_ix, cx);
                    cx.notify();
                })
                .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                    let mut changed = false;
                    if *hovering {
                        changed |= this.set_tooltip_text_if_changed(Some(next_tooltip.clone()), cx);
                    } else {
                        changed |= this.clear_tooltip_if_matches(&next_tooltip, cx);
                    }
                    if changed {
                        cx.notify();
                    }
                }));

            Some(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .flex_shrink_0()
                    .child(prev_btn)
                    .child(next_btn)
                    .into_any_element(),
            )
        })();

        let mut controls = div().flex().items_center().gap_1();
        if is_conflict_resolver {
            let nav_entries = self.conflict_nav_entries();
            let current_nav_ix = self.conflict_resolver.nav_anchor.unwrap_or(0);
            let can_nav_prev =
                diff_navigation::diff_nav_prev_target(&nav_entries, current_nav_ix).is_some();
            let can_nav_next =
                diff_navigation::diff_nav_next_target(&nav_entries, current_nav_ix).is_some();

            controls = controls
                .child(
                    zed::Button::new("conflict_prev", "Prev")
                        .end_slot(diff_nav_hotkey_hint("F2"))
                        .style(zed::ButtonStyle::Outlined)
                        .disabled(!can_nav_prev)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.conflict_jump_prev();
                            cx.notify();
                        }),
                )
                .child(
                    zed::Button::new("conflict_next", "Next")
                        .end_slot(diff_nav_hotkey_hint("F3"))
                        .style(zed::ButtonStyle::Outlined)
                        .disabled(!can_nav_next)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.conflict_jump_next();
                            cx.notify();
                        }),
                );

            if let (Some(repo_id), Some(path)) = (repo_id, conflict_target_path.clone()) {
                let save_path = path.clone();
                controls = controls
                    .child(
                        zed::Button::new("conflict_save", "Save")
                            .style(zed::ButtonStyle::Outlined)
                            .on_click(theme, cx, move |this, _e, _w, cx| {
                                let text = this
                                    .conflict_resolver_input
                                    .read_with(cx, |i, _| i.text().to_string());
                                this.store.dispatch(Msg::SaveWorktreeFile {
                                    repo_id,
                                    path: save_path.clone(),
                                    contents: text,
                                    stage: false,
                                });
                            }),
                    )
                    .child({
                        let save_path = path.clone();
                        zed::Button::new("conflict_save_stage", "Save & stage")
                            .style(zed::ButtonStyle::Filled)
                            .on_click(theme, cx, move |this, _e, _w, cx| {
                                let text = this
                                    .conflict_resolver_input
                                    .read_with(cx, |i, _| i.text().to_string());
                                this.store.dispatch(Msg::SaveWorktreeFile {
                                    repo_id,
                                    path: save_path.clone(),
                                    contents: text,
                                    stage: true,
                                });
                            })
                    });
            }
        } else if !is_file_preview {
            let nav_entries = self.diff_nav_entries();
            let current_nav_ix = self.diff_selection_anchor.unwrap_or(0);
            let can_nav_prev =
                diff_navigation::diff_nav_prev_target(&nav_entries, current_nav_ix).is_some();
            let can_nav_next =
                diff_navigation::diff_nav_next_target(&nav_entries, current_nav_ix).is_some();

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
                            if this.diff_search_active
                                && !this.diff_search_query.as_ref().trim().is_empty()
                            {
                                this.diff_search_recompute_matches();
                            }
                            cx.notify();
                        })
                        .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                            let text: SharedString = "Inline diff view (Alt+I)".into();
                            let mut changed = false;
                            if *hovering {
                                changed |= this.set_tooltip_text_if_changed(Some(text.clone()), cx);
                            } else {
                                changed |= this.clear_tooltip_if_matches(&text, cx);
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
                            if this.diff_search_active
                                && !this.diff_search_query.as_ref().trim().is_empty()
                            {
                                this.diff_search_recompute_matches();
                            }
                            cx.notify();
                        })
                        .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                            let text: SharedString = "Split diff view (Alt+S)".into();
                            let mut changed = false;
                            if *hovering {
                                changed |= this.set_tooltip_text_if_changed(Some(text.clone()), cx);
                            } else {
                                changed |= this.clear_tooltip_if_matches(&text, cx);
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
                                changed |= this.set_tooltip_text_if_changed(Some(text.clone()), cx);
                            } else {
                                changed |= this.clear_tooltip_if_matches(&text, cx);
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
                                changed |= this.set_tooltip_text_if_changed(Some(text.clone()), cx);
                            } else {
                                changed |= this.clear_tooltip_if_matches(&text, cx);
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
                                this.open_popover_at(
                                    PopoverKind::DiffHunks,
                                    e.position(),
                                    window,
                                    cx,
                                );
                                cx.notify();
                            })
                            .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                                let text: SharedString = "Jump to hunk (Alt+H)".into();
                                let mut changed = false;
                                if *hovering {
                                    changed |=
                                        this.set_tooltip_text_if_changed(Some(text.clone()), cx);
                                } else {
                                    changed |= this.clear_tooltip_if_matches(&text, cx);
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
                            changed |= this.set_tooltip_text_if_changed(Some(text.clone()), cx);
                        } else {
                            changed |= this.clear_tooltip_if_matches(&text, cx);
                        }
                        if changed {
                            cx.notify();
                        }
                    })),
            );
        }

        if self.diff_search_active {
            let query = self.diff_search_query.as_ref().trim();
            let match_label: SharedString = if query.is_empty() {
                "Type to search".into()
            } else if self.diff_search_matches.is_empty() {
                "No matches".into()
            } else {
                let ix = self
                    .diff_search_match_ix
                    .unwrap_or(0)
                    .min(self.diff_search_matches.len().saturating_sub(1));
                format!("{}/{}", ix + 1, self.diff_search_matches.len()).into()
            };

            controls = controls
                .child(
                    div()
                        .w(px(240.0))
                        .min_w(px(120.0))
                        .child(self.diff_search_input.clone()),
                )
                .child(
                    div()
                        .font_family("monospace")
                        .text_xs()
                        .text_color(theme.colors.text_muted)
                        .child(match_label),
                )
                .child(
                    zed::Button::new("diff_search_close", "✕")
                        .style(zed::ButtonStyle::Transparent)
                        .on_click(theme, cx, |this, _e, window, cx| {
                            this.diff_search_active = false;
                            this.diff_search_matches.clear();
                            this.diff_search_match_ix = None;
                            this.diff_text_segments_cache.clear();
                            this.worktree_preview_segments_cache_path = None;
                            this.worktree_preview_segments_cache.clear();
                            this.conflict_diff_segments_cache_split.clear();
                            this.conflict_diff_segments_cache_inline.clear();
                            window.focus(&this.diff_panel_focus_handle);
                            cx.notify();
                        }),
                );
        }

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(zed::CONTROL_HEIGHT_MD_PX))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .child(div().flex_1().min_w(px(0.0)).overflow_hidden().child(title))
                    .when_some(file_nav_controls, |d, controls| d.child(controls)),
            )
            .child(controls);

        let body: AnyElement = if is_file_preview {
            if added_preview_path.is_some() || deleted_preview_path.is_some() {
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
        } else if is_conflict_resolver {
            match (repo, conflict_target_path) {
                (None, _) => {
                    zed::empty_state(theme, "Resolve", "No repository.").into_any_element()
                }
                (_, None) => zed::empty_state(theme, "Resolve", "No conflicted file selected.")
                    .into_any_element(),
                (Some(repo), Some(path)) => {
                    let title: SharedString =
                        format!("Resolve conflict: {}", self.cached_path_display(&path)).into();

                    match &repo.conflict_file {
                        Loadable::NotLoaded | Loadable::Loading => {
                            zed::empty_state(theme, title, "Loading conflict data…")
                                .into_any_element()
                        }
                        Loadable::Error(e) => {
                            zed::empty_state(theme, title, e.clone()).into_any_element()
                        }
                        Loadable::Ready(None) => {
                            zed::empty_state(theme, title, "No conflict data.").into_any_element()
                        }
                        Loadable::Ready(Some(file)) => {
                            let ours = file.ours.clone().unwrap_or_default();
                            let theirs = file.theirs.clone().unwrap_or_default();
                            let has_current = file.current.is_some();

                            let mode = self.conflict_resolver.diff_mode;
                            let diff_len = match mode {
                                ConflictDiffMode::Split => self.conflict_resolver.diff_rows.len(),
                                ConflictDiffMode::Inline => {
                                    self.conflict_resolver.inline_rows.len()
                                }
                            };

                            let selection_empty = self.conflict_resolver_selection_is_empty();

                            let toggle_mode_split =
                                |this: &mut Self,
                                 _e: &ClickEvent,
                                 _w: &mut Window,
                                 cx: &mut gpui::Context<Self>| {
                                    this.conflict_resolver_set_mode(ConflictDiffMode::Split, cx);
                                };
                            let toggle_mode_inline =
                                |this: &mut Self,
                                 _e: &ClickEvent,
                                 _w: &mut Window,
                                 cx: &mut gpui::Context<Self>| {
                                    this.conflict_resolver_set_mode(ConflictDiffMode::Inline, cx);
                                };

                            let clear_selection =
                                |this: &mut Self,
                                 _e: &ClickEvent,
                                 _w: &mut Window,
                                 cx: &mut gpui::Context<Self>| {
                                    this.conflict_resolver_clear_selection(cx)
                                };

                            let append_selection =
                                |this: &mut Self,
                                 _e: &ClickEvent,
                                 _w: &mut Window,
                                 cx: &mut gpui::Context<Self>| {
                                    this.conflict_resolver_append_selection_to_output(cx);
                                };

                            let ours_for_btn = ours.clone();
                            let set_output_ours = move |this: &mut Self,
                                                        _e: &ClickEvent,
                                                        _w: &mut Window,
                                                        cx: &mut gpui::Context<Self>| {
                                this.conflict_resolver_set_output(ours_for_btn.clone(), cx);
                            };
                            let theirs_for_btn = theirs.clone();
                            let set_output_theirs = move |this: &mut Self,
                                                          _e: &ClickEvent,
                                                          _w: &mut Window,
                                                          cx: &mut gpui::Context<Self>| {
                                this.conflict_resolver_set_output(theirs_for_btn.clone(), cx);
                            };
                            let reset_from_markers =
                                |this: &mut Self,
                                 _e: &ClickEvent,
                                 _w: &mut Window,
                                 cx: &mut gpui::Context<Self>| {
                                    this.conflict_resolver_reset_output_from_markers(cx);
                                };

                            let mode_controls = div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    zed::Button::new("conflict_mode_split", "Split")
                                        .style(if mode == ConflictDiffMode::Split {
                                            zed::ButtonStyle::Filled
                                        } else {
                                            zed::ButtonStyle::Outlined
                                        })
                                        .on_click(theme, cx, toggle_mode_split),
                                )
                                .child(
                                    zed::Button::new("conflict_mode_inline", "Inline")
                                        .style(if mode == ConflictDiffMode::Inline {
                                            zed::ButtonStyle::Filled
                                        } else {
                                            zed::ButtonStyle::Outlined
                                        })
                                        .on_click(theme, cx, toggle_mode_inline),
                                );

                            let selection_controls = div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    zed::Button::new(
                                        "conflict_append_selected",
                                        "Append selection",
                                    )
                                    .style(zed::ButtonStyle::Outlined)
                                    .disabled(selection_empty)
                                    .on_click(
                                        theme,
                                        cx,
                                        append_selection,
                                    ),
                                )
                                .child(
                                    zed::Button::new("conflict_clear_selected", "Clear selection")
                                        .style(zed::ButtonStyle::Transparent)
                                        .disabled(selection_empty)
                                        .on_click(theme, cx, clear_selection),
                                );

                            let start_controls = div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    zed::Button::new("conflict_use_ours", "Use ours")
                                        .style(zed::ButtonStyle::Transparent)
                                        .disabled(file.ours.is_none())
                                        .on_click(theme, cx, set_output_ours),
                                )
                                .child(
                                    zed::Button::new("conflict_use_theirs", "Use theirs")
                                        .style(zed::ButtonStyle::Transparent)
                                        .disabled(file.theirs.is_none())
                                        .on_click(theme, cx, set_output_theirs),
                                )
                                .child(
                                    zed::Button::new(
                                        "conflict_reset_markers",
                                        "Reset from markers",
                                    )
                                    .style(zed::ButtonStyle::Transparent)
                                    .disabled(!has_current)
                                    .on_click(
                                        theme,
                                        cx,
                                        reset_from_markers,
                                    ),
                                );

                            let diff_header = div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.colors.text_muted)
                                        .child("Diff (ours ↔ theirs)"),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(mode_controls)
                                        .child(selection_controls),
                                );

                            let diff_title_row = div()
                                .h(px(22.0))
                                .flex()
                                .items_center()
                                .when(mode == ConflictDiffMode::Split, |d| {
                                    d.child(
                                        div()
                                            .flex_1()
                                            .px_2()
                                            .text_xs()
                                            .text_color(theme.colors.text_muted)
                                            .child("Ours (index :2)"),
                                    )
                                    .child(div().w(px(1.0)).h_full().bg(theme.colors.border))
                                    .child(
                                        div()
                                            .flex_1()
                                            .px_2()
                                            .text_xs()
                                            .text_color(theme.colors.text_muted)
                                            .child("Theirs (index :3)"),
                                    )
                                })
                                .when(mode == ConflictDiffMode::Inline, |d| d);

                            let diff_body: AnyElement = if diff_len == 0 {
                                zed::empty_state(
                                    theme,
                                    "Diff",
                                    "Ours/Theirs content not available.",
                                )
                                .into_any_element()
                            } else {
                                let list = uniform_list(
                                    "conflict_resolver_diff_list",
                                    diff_len,
                                    cx.processor(Self::render_conflict_resolver_diff_rows),
                                )
                                .h_full()
                                .min_h(px(0.0))
                                .track_scroll(self.conflict_resolver_diff_scroll.clone());

                                let scroll_handle = self
                                    .conflict_resolver_diff_scroll
                                    .0
                                    .borrow()
                                    .base_handle
                                    .clone();

                                div()
                                    .id("conflict_resolver_diff_scroll")
                                    .relative()
                                    .h_full()
                                    .min_h(px(0.0))
                                    .bg(theme.colors.window_bg)
                                    .child(list)
                                    .child(
                                        zed::Scrollbar::new(
                                            "conflict_resolver_diff_scrollbar",
                                            scroll_handle,
                                        )
                                        .always_visible()
                                        .render(theme),
                                    )
                                    .into_any_element()
                            };

                            let output_header = div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.colors.text_muted)
                                        .child("Resolved output"),
                                )
                                .child(start_controls);

                            let preview_count = self.conflict_resolved_preview_lines.len();
                            let preview_body: AnyElement = if preview_count == 0 {
                                zed::empty_state(theme, "Preview", "Empty.").into_any_element()
                            } else {
                                let list = uniform_list(
                                    "conflict_resolved_preview_list",
                                    preview_count,
                                    cx.processor(Self::render_conflict_resolved_preview_rows),
                                )
                                .h_full()
                                .min_h(px(0.0))
                                .track_scroll(self.conflict_resolved_preview_scroll.clone());
                                let scroll_handle = self
                                    .conflict_resolved_preview_scroll
                                    .0
                                    .borrow()
                                    .base_handle
                                    .clone();

                                div()
                                    .id("conflict_resolved_preview_scroll")
                                    .relative()
                                    .h_full()
                                    .min_h(px(0.0))
                                    .bg(theme.colors.window_bg)
                                    .child(list)
                                    .child(
                                        zed::Scrollbar::new(
                                            "conflict_resolved_preview_scrollbar",
                                            scroll_handle,
                                        )
                                        .render(theme),
                                    )
                                    .into_any_element()
                            };

                            let output_columns_header =
                                zed::split_columns_header(theme, "Resolved (editable)", "Preview");

                            div()
                        .id("conflict_resolver_panel")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h(px(0.0))
                        .gap_2()
                        .px_2()
                        .py_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .child(title.clone()),
                        )
                        .child(div().border_t_1().border_color(theme.colors.border))
                        .child(diff_header)
                        .child(
                            div()
                                .h(px(240.0))
                                .min_h(px(0.0))
                                .border_1()
                                .border_color(theme.colors.border)
                                .rounded(px(theme.radii.row))
                                .overflow_hidden()
                                .flex()
                                .flex_col()
                                .child(diff_title_row)
                                .child(div().border_t_1().border_color(theme.colors.border))
                                .child(diff_body),
                        )
	                        .child(div().border_t_1().border_color(theme.colors.border))
	                        .child(output_header)
	                        .child(
	                            div()
	                                .id("conflict_resolver_output_split")
	                                .relative()
	                                .h_full()
	                                .min_h(px(0.0))
	                                .flex()
	                                .flex_col()
	                                .flex_1()
	                                .min_h(px(0.0))
	                                .border_1()
	                                .border_color(theme.colors.border)
	                                .rounded(px(theme.radii.row))
	                                .overflow_hidden()
	                                .bg(theme.colors.window_bg)
	                                .child(output_columns_header)
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
	                                                .overflow_hidden()
	                                                .child(
	                                                    div()
	                                                        .id("conflict_resolver_output_scroll")
	                                                        .h_full()
	                                                        .min_h(px(0.0))
	                                                        .overflow_y_scroll()
	                                                        .child(
	                                                            div()
	                                                                .p_2()
	                                                                .child(
	                                                                    self.conflict_resolver_input.clone(),
	                                                                ),
	                                                        ),
	                                                ),
	                                        )
	                                        .child(div().w(px(1.0)).h_full().bg(theme.colors.border))
	                                        .child(
	                                            div()
	                                                .flex_1()
	                                                .min_w(px(0.0))
	                                                .h_full()
	                                                .overflow_hidden()
	                                                .child(preview_body),
	                                        ),
	                                ),
	                        )
	                        .into_any_element()
                        }
                    }
                }
            }
        } else if is_conflict_compare {
            match (repo, conflict_target_path) {
                (None, _) => {
                    zed::empty_state(theme, "Resolve", "No repository.").into_any_element()
                }
                (_, None) => zed::empty_state(theme, "Resolve", "No conflicted file selected.")
                    .into_any_element(),
                (Some(repo), Some(path)) => {
                    let title: SharedString =
                        format!("Resolve conflict: {}", self.cached_path_display(&path)).into();

                    match &repo.conflict_file {
                        Loadable::NotLoaded | Loadable::Loading => {
                            zed::empty_state(theme, title, "Loading conflict data…")
                                .into_any_element()
                        }
                        Loadable::Error(e) => {
                            zed::empty_state(theme, title, e.clone()).into_any_element()
                        }
                        Loadable::Ready(None) => {
                            zed::empty_state(theme, title, "No conflict data.").into_any_element()
                        }
                        Loadable::Ready(Some(file)) => {
                            if file.path != path {
                                zed::empty_state(theme, title, "Loading conflict data…")
                                    .into_any_element()
                            } else {
                                let ours_label: SharedString = if file.ours.is_some() {
                                    "Ours".into()
                                } else {
                                    "Ours (deleted)".into()
                                };
                                let theirs_label: SharedString = if file.theirs.is_some() {
                                    "Theirs".into()
                                } else {
                                    "Theirs (deleted)".into()
                                };

                                let columns_header =
                                    zed::split_columns_header(theme, ours_label, theirs_label);

                                let diff_len = match self.diff_view {
                                    DiffViewMode::Split => self.conflict_resolver.diff_rows.len(),
                                    DiffViewMode::Inline => {
                                        self.conflict_resolver.inline_rows.len()
                                    }
                                };

                                let diff_body: AnyElement = if diff_len == 0 {
                                    zed::empty_state(theme, "Diff", "No conflict diff to show.")
                                        .into_any_element()
                                } else {
                                    let scroll_handle =
                                        self.diff_scroll.0.borrow().base_handle.clone();
                                    let list = uniform_list(
                                        "conflict_compare_diff",
                                        diff_len,
                                        cx.processor(Self::render_conflict_compare_diff_rows),
                                    )
                                    .h_full()
                                    .min_h(px(0.0))
                                    .track_scroll(self.diff_scroll.clone());

                                    div()
                                        .id("conflict_compare_container")
                                        .relative()
                                        .flex()
                                        .flex_col()
                                        .h_full()
                                        .min_h(px(0.0))
                                        .bg(theme.colors.window_bg)
                                        .child(columns_header)
                                        .child(
                                            div()
                                                .id("conflict_compare_scroll_container")
                                                .relative()
                                                .flex_1()
                                                .min_h(px(0.0))
                                                .child(list)
                                                .child(
                                                    zed::Scrollbar::new(
                                                        "conflict_compare_scrollbar",
                                                        scroll_handle,
                                                    )
                                                    .always_visible()
                                                    .render(theme),
                                                ),
                                        )
                                        .into_any_element()
                                };

                                diff_body
                            }
                        }
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
                            if !matches!(repo.diff_file_image, Loadable::NotLoaded) {
                                enum DiffFileImageState {
                                    NotLoaded,
                                    Loading,
                                    Error(String),
                                    Ready { has_file: bool },
                                }

                                let diff_file_state = match &repo.diff_file_image {
                                    Loadable::NotLoaded => DiffFileImageState::NotLoaded,
                                    Loadable::Loading => DiffFileImageState::Loading,
                                    Loadable::Error(e) => DiffFileImageState::Error(e.clone()),
                                    Loadable::Ready(file) => DiffFileImageState::Ready {
                                        has_file: file.is_some(),
                                    },
                                };

                                self.ensure_file_image_diff_cache();
                                match diff_file_state {
                                    DiffFileImageState::NotLoaded => {
                                        zed::empty_state(theme, "Diff", "Select a file.")
                                            .into_any_element()
                                    }
                                    DiffFileImageState::Loading => {
                                        zed::empty_state(theme, "Diff", "Loading")
                                            .into_any_element()
                                    }
                                    DiffFileImageState::Error(e) => {
                                        self.diff_raw_input.update(cx, |input, cx| {
                                            input.set_theme(theme, cx);
                                            input.set_text(e, cx);
                                            input.set_read_only(true, cx);
                                        });
                                        div()
                                            .id("diff_file_image_error_scroll")
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
                                    DiffFileImageState::Ready { has_file } => {
                                        if !has_file || !self.is_file_image_diff_view_active() {
                                            zed::empty_state(
                                                theme,
                                                "Diff",
                                                "No image contents available.",
                                            )
                                            .into_any_element()
                                        } else {
                                            let old = self.file_image_diff_cache_old.clone();
                                            let new = self.file_image_diff_cache_new.clone();

                                            let cell = |id: &'static str,
                                                        image: Option<Arc<gpui::Image>>| {
                                                div()
                                                    .id(id)
                                                    .flex_1()
                                                    .min_w(px(0.0))
                                                    .h_full()
                                                    .overflow_hidden()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(match image {
                                                        Some(img_data) => gpui::img(img_data)
                                                            .w_full()
                                                            .h_full()
                                                            .object_fit(gpui::ObjectFit::Contain)
                                                            .into_any_element(),
                                                        None => div()
                                                            .text_sm()
                                                            .text_color(theme.colors.text_muted)
                                                            .child("No image")
                                                            .into_any_element(),
                                                    })
                                            };

                                            let columns_header = zed::split_columns_header(
                                                theme,
                                                "A (before)",
                                                "B (after)",
                                            );

                                            div()
                                                .id("diff_image_container")
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
                                                        .child(cell("diff_image_left", old))
                                                        .child(
                                                            div()
                                                                .w(px(1.0))
                                                                .h_full()
                                                                .bg(theme.colors.border),
                                                        )
                                                        .child(cell("diff_image_right", new)),
                                                )
                                                .into_any_element()
                                        }
                                    }
                                }
                            } else {
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
                                        zed::empty_state(theme, "Diff", "Loading")
                                            .into_any_element()
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
                                                DiffViewMode::Split => {
                                                    self.file_diff_cache_rows.len()
                                                }
                                            };
                                            if total_len == 0 {
                                                zed::empty_state(theme, "Diff", "Empty file.")
                                                    .into_any_element()
                                            } else if self.diff_visible_indices.is_empty() {
                                                zed::empty_state(
                                                    theme,
                                                    "Diff",
                                                    "Nothing to render.",
                                                )
                                                .into_any_element()
                                            } else {
                                                let scroll_handle =
                                                    self.diff_scroll.0.borrow().base_handle.clone();
                                                let markers =
                                                    self.diff_scrollbar_markers_cache.clone();
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
                                                                .always_visible()
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

                                                        let columns_header =
                                                            zed::split_columns_header(
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
                                                                            .bg(theme
                                                                                .colors
                                                                                .border),
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
                                                                .always_visible()
                                                                .render(theme),
                                                            )
                                                            .into_any_element()
                                                    }
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
                                                .always_visible()
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
                                                .always_visible()
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
                    if this.diff_search_active {
                        this.diff_search_active = false;
                        this.diff_search_matches.clear();
                        this.diff_search_match_ix = None;
                        this.diff_text_segments_cache.clear();
                        this.worktree_preview_segments_cache_path = None;
                        this.worktree_preview_segments_cache.clear();
                        this.conflict_diff_segments_cache_split.clear();
                        this.conflict_diff_segments_cache_inline.clear();
                        window.focus(&this.diff_panel_focus_handle);
                        handled = true;
                    }
                    if !handled && let Some(repo_id) = this.active_repo_id() {
                        this.clear_status_multi_selection(repo_id, cx);
                        this.store.dispatch(Msg::ClearDiffSelection { repo_id });
                        handled = true;
                    }
                }

                if !handled
                    && (mods.control || mods.platform)
                    && !mods.alt
                    && !mods.function
                    && key == "f"
                {
                    this.diff_search_active = true;
                    this.diff_text_segments_cache.clear();
                    this.worktree_preview_segments_cache_path = None;
                    this.worktree_preview_segments_cache.clear();
                    this.conflict_diff_segments_cache_split.clear();
                    this.conflict_diff_segments_cache_inline.clear();
                    this.diff_search_recompute_matches();
                    let focus = this.diff_search_input.read(cx).focus_handle();
                    window.focus(&focus);
                    handled = true;
                }

                if !handled
                    && this.diff_search_active
                    && key == "f2"
                    && !mods.control
                    && !mods.alt
                    && !mods.platform
                    && !mods.function
                {
                    this.diff_search_prev_match();
                    handled = true;
                }

                if !handled
                    && this.diff_search_active
                    && key == "f3"
                    && !mods.control
                    && !mods.alt
                    && !mods.platform
                    && !mods.function
                {
                    this.diff_search_next_match();
                    handled = true;
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

                if !handled
                    && (key == "f1" || key == "f4")
                    && !mods.control
                    && !mods.alt
                    && !mods.platform
                    && !mods.function
                    && let Some(repo_id) = this.active_repo_id()
                    && let Some(repo) = this.active_repo()
                    && let Some(DiffTarget::WorkingTree { path, area }) = repo.diff_target.clone()
                    && let Loadable::Ready(status) = &repo.status
                {
                    let entries = match area {
                        DiffArea::Unstaged => status.unstaged.as_slice(),
                        DiffArea::Staged => status.staged.as_slice(),
                    };

                    let target = (|| {
                        let current_ix = entries.iter().position(|e| e.path == path)?;
                        let target_ix = if key == "f1" {
                            current_ix.checked_sub(1)?
                        } else {
                            let next_ix = current_ix + 1;
                            if next_ix < entries.len() {
                                next_ix
                            } else {
                                return None;
                            }
                        };
                        Some((target_ix, entries.get(target_ix)?.path.clone()))
                    })();

                    if let Some((target_ix, target_path)) = target {
                        this.clear_status_multi_selection(repo_id, cx);
                        this.store.dispatch(Msg::SelectDiff {
                            repo_id,
                            target: DiffTarget::WorkingTree {
                                path: target_path,
                                area,
                            },
                        });
                        this.scroll_status_list_to_ix(area, target_ix, cx);

                        handled = true;
                    }
                }

                let is_file_preview = this.untracked_worktree_preview_path().is_some()
                    || this.added_file_preview_abs_path().is_some()
                    || this.deleted_file_preview_abs_path().is_some();
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

                let conflict_resolver_active = this.active_repo().is_some_and(|repo| {
                    let Some(DiffTarget::WorkingTree { path, area }) = repo.diff_target.as_ref()
                    else {
                        return false;
                    };
                    if *area != DiffArea::Unstaged {
                        return false;
                    }
                    let Loadable::Ready(status) = &repo.status else {
                        return false;
                    };
                    let conflict = status.unstaged.iter().find(|e| {
                        e.path == *path
                            && e.kind == gitgpui_core::domain::FileStatusKind::Conflicted
                    });
                    conflict.is_some_and(|e| Self::conflict_requires_resolver(e.conflict))
                });

                if mods.alt && !mods.control && !mods.platform && !mods.function {
                    match key {
                        "i" => {
                            if conflict_resolver_active {
                                this.conflict_resolver_set_mode(ConflictDiffMode::Inline, cx);
                            } else {
                                this.diff_view = DiffViewMode::Inline;
                                this.diff_text_segments_cache.clear();
                            }
                            handled = true;
                        }
                        "s" => {
                            if conflict_resolver_active {
                                this.conflict_resolver_set_mode(ConflictDiffMode::Split, cx);
                            } else {
                                this.diff_view = DiffViewMode::Split;
                                this.diff_text_segments_cache.clear();
                            }
                            handled = true;
                        }
                        "h" => {
                            let is_file_preview = this.untracked_worktree_preview_path().is_some()
                                || this.added_file_preview_abs_path().is_some()
                                || this.deleted_file_preview_abs_path().is_some();
                            if !is_file_preview
                                && !this.active_repo().is_some_and(|r| {
                                    Self::is_file_diff_target(r.diff_target.as_ref())
                                })
                            {
                                this.open_popover_at_cursor(PopoverKind::DiffHunks, window, cx);
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
                        if conflict_resolver_active {
                            this.conflict_jump_prev();
                        } else {
                            this.diff_jump_prev();
                        }
                    } else {
                        if conflict_resolver_active {
                            this.conflict_jump_next();
                        } else {
                            this.diff_jump_next();
                        }
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
                    if conflict_resolver_active {
                        this.conflict_jump_prev();
                    } else {
                        this.diff_jump_prev();
                    }
                    handled = true;
                }

                if !handled
                    && key == "f3"
                    && !mods.control
                    && !mods.alt
                    && !mods.platform
                    && !mods.function
                {
                    if conflict_resolver_active {
                        this.conflict_jump_next();
                    } else {
                        this.diff_jump_next();
                    }
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
