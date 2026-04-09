use super::super::super::*;
use super::HistorySearchOperatorOption;
use super::HistorySearchUiState;
use std::cell::RefCell;
use std::rc::Rc;

use super::HistoryView;

impl Render for HistoryView {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        self.last_window_size = window.viewport_size();
        self.history_view_inner(window, cx)
    }
}

impl HistoryView {
    fn history_view_inner(
        &mut self,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::Div {
        let theme = self.theme;
        self.sync_history_search_repo_binding(cx);
        self.ensure_history_cache(cx);
        let (mut show_working_tree_summary_row, _) = self.ensure_history_worktree_summary_cache();
        if self.history_has_active_query() {
            show_working_tree_summary_row = false;
        }
        let commits_count = self
            .history_cache
            .as_ref()
            .map(|c| c.visible_indices.len())
            .unwrap_or(0);
        let count = commits_count + usize::from(show_working_tree_summary_row);
        let search_visible = self.history_search_is_visible();

        let bg = theme.colors.window_bg;

        let body: AnyElement = if count == 0 {
            match self.active_repo().map(|r| &r.log) {
                None => {
                    components::empty_state(theme, "History", "No repository.").into_any_element()
                }
                Some(Loadable::Loading) => {
                    let label = if self.history_has_active_query() {
                        "Searching history"
                    } else {
                        "Loading"
                    };
                    components::empty_state(theme, "History", label).into_any_element()
                }
                Some(Loadable::Error(e)) => {
                    components::empty_state(theme, "History", e.clone()).into_any_element()
                }
                Some(Loadable::NotLoaded) | Some(Loadable::Ready(_)) => {
                    let label = if self.history_has_active_query() {
                        "No matching commits."
                    } else {
                        "No commits."
                    };
                    components::empty_state(theme, "History", label).into_any_element()
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
            let should_load_more = {
                let state = self.history_scroll.0.borrow();
                let scroll_handle = state.base_handle.clone();
                let max_offset = scroll_handle.max_offset().height.max(px(0.0));
                let should_load_by_scroll = if max_offset > px(0.0) {
                    scroll_is_near_bottom(&scroll_handle, px(240.0))
                } else {
                    true
                };

                state.last_item_size.is_some()
                    && self.active_repo().is_some_and(|repo| {
                        !repo.log_loading_more
                            && matches!(
                                &repo.log,
                                Loadable::Ready(page) if page.next_cursor.is_some()
                            )
                    })
                    && should_load_by_scroll
            };
            if should_load_more && let Some(repo_id) = self.active_repo_id() {
                self.store.dispatch(Msg::LoadMoreHistory { repo_id });
            }
            let scrollbar_gutter = components::Scrollbar::visible_gutter(
                self.history_scroll.clone(),
                components::ScrollbarAxis::Vertical,
            );
            div()
                .id("history_main_scroll_container")
                .relative()
                .h_full()
                .child(
                    div()
                        .h_full()
                        .min_h(px(0.0))
                        .pr(scrollbar_gutter)
                        .child(list),
                )
                .child(
                    components::Scrollbar::new(
                        "history_main_scrollbar",
                        self.history_scroll.clone(),
                    )
                    .render(theme),
                )
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
            .track_focus(&self.history_panel_focus_handle)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseDownEvent, window, _cx| {
                    window.focus(&this.history_panel_focus_handle);
                }),
            )
            .on_key_down(cx.listener(|this, e: &gpui::KeyDownEvent, window, cx| {
                let key = e.keystroke.key.as_str();
                let mods = e.keystroke.modifiers;
                let search_input_focused = this
                    .history_search_input
                    .read(cx)
                    .focus_handle()
                    .is_focused(window);
                let picker_active = search_input_focused && this.history_search_picker_is_active();

                let mut handled = false;

                if key == "escape" && !mods.control && !mods.alt && !mods.platform && !mods.function
                {
                    if this.history_search_is_visible() {
                        this.deactivate_history_search(window, cx);
                        handled = true;
                    }
                }

                if !handled
                    && (mods.control || mods.platform)
                    && !mods.alt
                    && !mods.function
                    && key == "f"
                {
                    this.activate_history_search(window, cx);
                    handled = true;
                }

                if !handled
                    && picker_active
                    && !mods.control
                    && !mods.alt
                    && !mods.platform
                    && !mods.function
                    && !mods.shift
                {
                    handled = match key {
                        "up" => {
                            this.move_history_search_helper_selection(-1, cx);
                            true
                        }
                        "down" => {
                            this.move_history_search_helper_selection(1, cx);
                            true
                        }
                        "home" => {
                            this.set_history_search_helper_selection_edge(false, cx);
                            true
                        }
                        "end" => {
                            this.set_history_search_helper_selection_edge(true, cx);
                            true
                        }
                        "enter" => {
                            if let Some(field) = this.selected_history_search_helper_field() {
                                this.apply_history_search_operator(field, cx);
                            }
                            true
                        }
                        _ => false,
                    };
                }

                if !handled
                    && !search_input_focused
                    && !mods.control
                    && !mods.alt
                    && !mods.platform
                    && !mods.function
                    && !mods.shift
                {
                    handled = match key {
                        "up" => this.history_select_adjacent_commit(-1, cx),
                        "down" => this.history_select_adjacent_commit(1, cx),
                        _ => false,
                    };
                }

                if handled {
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(
                self.history_column_headers(cx)
                    .bg(bg)
                    .border_b_1()
                    .border_color(theme.colors.border),
            )
            .when(search_visible, |d| {
                d.child(self.history_search_panel(commits_count, window, cx))
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(div().flex_1().min_h(px(0.0)).child(body)),
            )
    }

    fn history_select_adjacent_commit(
        &mut self,
        direction: i8,
        _cx: &mut gpui::Context<Self>,
    ) -> bool {
        let Some(repo_id) = self.active_repo_id() else {
            return false;
        };

        let (mut show_working_tree_summary_row, _) = self.ensure_history_worktree_summary_cache();
        if self.history_has_active_query() {
            show_working_tree_summary_row = false;
        }
        let offset = usize::from(show_working_tree_summary_row);

        let (selected_commit, page, log_rev, stashes_rev, history_scope) = match self.active_repo()
        {
            Some(repo) => {
                let page = match &repo.log {
                    Loadable::Ready(page) => Arc::clone(page),
                    _ => return false,
                };
                (
                    repo.history_state.selected_commit.clone(),
                    page,
                    repo.log_rev,
                    repo.stashes_rev,
                    repo.history_state.history_scope,
                )
            }
            None => return false,
        };

        let cache = self
            .history_cache
            .as_ref()
            .filter(|c| c.request.repo_id == repo_id);
        let Some(cache) = cache else {
            return false;
        };

        let total_commits = cache.visible_indices.len();
        if total_commits == 0 {
            return false;
        }

        let list_len = total_commits + offset;

        let current_list_ix = super::resolve_history_selected_list_index(
            &mut self.history_selected_list_index_cache,
            repo_id,
            log_rev,
            stashes_rev,
            history_scope,
            show_working_tree_summary_row,
            selected_commit.as_ref(),
            &cache.visible_indices,
            &page.commits,
        );

        let next_list_ix = match (current_list_ix, direction.is_negative()) {
            (Some(current_list_ix), true) => current_list_ix.saturating_sub(1),
            (Some(current_list_ix), false) => {
                let next = current_list_ix + 1;
                if next < list_len {
                    next
                } else {
                    current_list_ix
                }
            }
            (None, true) => list_len.saturating_sub(1),
            (None, false) => offset,
        };

        if current_list_ix.is_some_and(|ix| ix == next_list_ix) {
            return true;
        }

        if show_working_tree_summary_row && next_list_ix == 0 {
            self.store.dispatch(Msg::ClearCommitSelection { repo_id });
            self.store.dispatch(Msg::ClearDiffSelection { repo_id });
            super::set_history_selected_list_index_cache(
                &mut self.history_selected_list_index_cache,
                repo_id,
                log_rev,
                stashes_rev,
                history_scope,
                show_working_tree_summary_row,
                None,
                0,
            );
            self.history_scroll
                .scroll_to_item_strict(0, gpui::ScrollStrategy::Center);
            return true;
        }

        let visible_ix = next_list_ix.saturating_sub(offset);
        let Some(commit_ix) = cache.visible_indices.get(visible_ix) else {
            return false;
        };
        let Some(commit) = page.commits.get(commit_ix) else {
            return false;
        };

        self.store.dispatch(Msg::SelectCommit {
            repo_id,
            commit_id: commit.id.clone(),
        });
        super::set_history_selected_list_index_cache(
            &mut self.history_selected_list_index_cache,
            repo_id,
            log_rev,
            stashes_rev,
            history_scope,
            show_working_tree_summary_row,
            Some(commit.id.clone()),
            next_list_ix,
        );
        self.history_scroll
            .scroll_to_item_strict(next_list_ix, gpui::ScrollStrategy::Center);
        true
    }

    fn history_search_status_label(&self, match_count: usize) -> SharedString {
        if let Some(filter_text) = self.history_search_picker_filter_text() {
            if filter_text.is_empty() {
                return "Choose a field".into();
            }
            let count = self.history_search_operator_options().len();
            return if count == 0 {
                "No fields".into()
            } else if count == 1 {
                "1 field".into()
            } else {
                format!("{count} fields").into()
            };
        }

        if self.history_search_dispatch_pending {
            return "Updating…".into();
        }

        if matches!(
            &self.history_search_state,
            HistorySearchUiState::Tagged { value, .. } if value.as_ref().trim().is_empty()
        ) {
            return "Type a search value".into();
        }

        match self.active_repo() {
            Some(repo) if matches!(repo.log, Loadable::Loading) => "Searching history…".into(),
            Some(repo) if repo.log_loading_more => format!("{match_count} loaded").into(),
            Some(_) if match_count == 0 => "No matches".into(),
            Some(_) if match_count == 1 => "1 match".into(),
            Some(_) => format!("{match_count} matches").into(),
            None => "No repository".into(),
        }
    }

    fn history_search_field_chip(
        &self,
        field: gitcomet_core::history_query::HistoryQueryField,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        div()
            .id("history_search_field_chip")
            .debug_selector(|| "history_search_field_chip".to_string())
            .h(px(18.0))
            .px(px(6.0))
            .flex()
            .items_center()
            .rounded(px(theme.radii.pill))
            .border_1()
            .border_color(with_alpha(theme.colors.accent, 0.34))
            .bg(with_alpha(
                theme.colors.accent,
                if theme.is_dark { 0.16 } else { 0.10 },
            ))
            .cursor(CursorStyle::PointingHand)
            .hover(move |d| {
                d.bg(with_alpha(
                    theme.colors.accent,
                    if theme.is_dark { 0.22 } else { 0.16 },
                ))
            })
            .active(move |d| d.bg(with_alpha(theme.colors.accent, 0.24)))
            .child(
                div()
                    .font_family(UI_MONOSPACE_FONT_FAMILY)
                    .text_xs()
                    .text_color(theme.colors.accent)
                    .child(format!("{field}:")),
            )
            .on_click(cx.listener(|this, _e: &ClickEvent, window, cx| {
                this.begin_history_search_retagging(cx);
                let focus = this.history_search_input.read(cx).focus_handle();
                window.focus(&focus);
            }))
    }

    fn history_search_operator_row(
        &self,
        option: HistorySearchOperatorOption,
        option_ix: usize,
        selected: bool,
        id: impl Into<SharedString>,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        div()
            .id(id.into())
            .h(px(24.0))
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .rounded(px(theme.radii.row))
            .border_1()
            .border_color(if selected {
                with_alpha(theme.colors.accent, 0.34)
            } else {
                with_alpha(theme.colors.border, 0.92)
            })
            .bg(if selected {
                with_alpha(theme.colors.accent, if theme.is_dark { 0.14 } else { 0.08 })
            } else {
                with_alpha(
                    theme.colors.window_bg,
                    if theme.is_dark { 0.26 } else { 0.62 },
                )
            })
            .cursor(CursorStyle::PointingHand)
            .hover(move |d| {
                d.bg(with_alpha(
                    if selected {
                        theme.colors.accent
                    } else {
                        theme.colors.hover
                    },
                    if theme.is_dark { 0.20 } else { 0.12 },
                ))
            })
            .active(move |d| d.bg(with_alpha(theme.colors.accent, 0.22)))
            .child(
                div()
                    .font_family(UI_MONOSPACE_FONT_FAMILY)
                    .text_xs()
                    .text_color(if selected {
                        theme.colors.accent
                    } else {
                        theme.colors.text
                    })
                    .child(option.label),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .text_xs()
                    .text_color(theme.colors.text_muted)
                    .line_clamp(1)
                    .child(option.description),
            )
            .on_hover(cx.listener(move |this, hovering: &bool, _window, cx| {
                if *hovering && this.history_search_helper_selected_ix != Some(option_ix) {
                    this.history_search_helper_selected_ix = Some(option_ix);
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _e: &ClickEvent, window, cx| {
                this.apply_history_search_operator(option.field, cx);
                let focus = this.history_search_input.read(cx).focus_handle();
                window.focus(&focus);
            }))
    }

    fn history_search_panel(
        &mut self,
        match_count: usize,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let picker_active = self.history_search_picker_is_active();
        let status_label = self.history_search_status_label(match_count);
        let search_input_focused = self
            .history_search_input
            .read(cx)
            .focus_handle()
            .is_focused(window);

        let close_button = div()
            .id("history_search_close")
            .debug_selector(|| "history_search_close".to_string())
            .size(px(20.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme.radii.pill))
            .cursor(CursorStyle::PointingHand)
            .hover(move |d| d.bg(theme.colors.hover))
            .active(move |d| d.bg(theme.colors.active))
            .child(svg_icon(
                "icons/generic_close.svg",
                theme.colors.text_muted,
                px(10.0),
            ))
            .on_click(cx.listener(|this, _e: &ClickEvent, window, cx| {
                this.deactivate_history_search(window, cx);
            }));

        let mut input_shell = div()
            .id("history_search_bar")
            .debug_selector(|| "history_search_bar".to_string())
            .h(px(24.0))
            .flex()
            .items_center()
            .gap_1()
            .px(px(6.0))
            .bg(with_alpha(
                theme.colors.window_bg,
                if theme.is_dark { 0.90 } else { 0.98 },
            ))
            .border_1()
            .border_color(if search_input_focused {
                with_alpha(theme.colors.accent, 0.48)
            } else {
                theme.colors.border
            })
            .rounded(px(theme.radii.pill));

        if let Some(field) = self.history_search_selected_field() {
            input_shell = input_shell.child(self.history_search_field_chip(field, cx));
        }

        input_shell = input_shell.child(
            div()
                .flex_1()
                .min_w(px(72.0))
                .child(self.history_search_input.clone()),
        );

        let mut panel = div()
            .id("history_search_panel")
            .debug_selector(|| "history_search_panel".to_string())
            .mx_2()
            .mt_1()
            .mb_1()
            .px_1()
            .py(px(3.0))
            .flex()
            .flex_col()
            .gap_1()
            .bg(theme.colors.surface_bg_elevated)
            .border_1()
            .border_color(theme.colors.border)
            .rounded(px(theme.radii.row))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(div().flex_1().min_w(px(160.0)).child(input_shell))
                    .child(
                        div()
                            .whitespace_nowrap()
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .child(status_label),
                    )
                    .child(close_button),
            );

        if picker_active {
            let options = self.history_search_operator_options();
            let selected_ix = super::history_search_helper_selected_ix(
                self.history_search_helper_selected_ix,
                options.len(),
            );
            let mut list = div()
                .id("history_search_operator_list")
                .flex()
                .flex_col()
                .gap_1()
                .overflow_y_scroll()
                .max_h(px(156.0))
                .track_scroll(&self.history_search_helper_scroll);

            if options.is_empty() {
                list = list.child(
                    div()
                        .px_1()
                        .py_1()
                        .text_xs()
                        .text_color(theme.colors.text_muted)
                        .child("No fields match that filter."),
                );
            } else {
                for (ix, option) in options.into_iter().enumerate() {
                    let row_id: SharedString =
                        format!("history_search_operator_{}", option.field.as_str()).into();
                    list = list.child(self.history_search_operator_row(
                        option,
                        ix,
                        selected_ix == Some(ix),
                        row_id,
                        cx,
                    ));
                }
            }

            panel = panel.child(
                div()
                    .pt_1()
                    .border_t_1()
                    .border_color(theme.colors.border)
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(list)
                    .child(
                        div()
                            .px_1()
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .child("Up/Down selects. Enter chooses."),
                    ),
            );
        }

        panel
    }

    fn history_column_headers(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let icon_muted = with_alpha(theme.colors.accent, if theme.is_dark { 0.72 } else { 0.82 });
        let (show_author, show_date, show_sha) = self.history_visible_columns();
        let col_author = self.history_col_author;
        let col_date = self.history_col_date;
        let col_sha = self.history_col_sha;
        let handle_w = px(HISTORY_COL_HANDLE_PX);
        let handle_half = px(HISTORY_COL_HANDLE_PX / 2.0);
        let cell_pad = handle_half;
        let scope_label: SharedString = self
            .active_repo()
            .map(|r| match r.history_state.history_scope {
                gitcomet_core::domain::LogScope::CurrentBranch => "Current branch".to_string(),
                gitcomet_core::domain::LogScope::AllBranches => "All branches".to_string(),
            })
            .unwrap_or_else(|| "Current branch".to_string())
            .into();
        let scope_repo_id = self.active_repo_id();
        let scope_invoker: SharedString = "history_scope_header".into();
        let scope_anchor_bounds: Rc<RefCell<Option<Bounds<Pixels>>>> = Rc::new(RefCell::new(None));
        let scope_anchor_bounds_for_prepaint = Rc::clone(&scope_anchor_bounds);
        let scope_anchor_bounds_for_click = Rc::clone(&scope_anchor_bounds);
        let scope_active = self
            .active_context_menu_invoker
            .as_ref()
            .is_some_and(|id| id.as_ref() == scope_invoker.as_ref());
        let column_settings_invoker: SharedString = "history_columns_settings_btn".into();
        let column_settings_anchor_bounds: Rc<RefCell<Option<Bounds<Pixels>>>> =
            Rc::new(RefCell::new(None));
        let column_settings_anchor_bounds_for_prepaint = Rc::clone(&column_settings_anchor_bounds);
        let column_settings_anchor_bounds_for_click = Rc::clone(&column_settings_anchor_bounds);
        let column_settings_active =
            self.active_context_menu_invoker.as_ref() == Some(&column_settings_invoker);
        let open_column_settings = {
            let column_settings_invoker = column_settings_invoker.clone();
            cx.listener(move |this, e: &ClickEvent, window, cx| {
                this.activate_context_menu_invoker(column_settings_invoker.clone(), cx);
                if let Some(bounds) = *column_settings_anchor_bounds_for_click.borrow() {
                    this.open_popover_for_bounds(
                        PopoverKind::HistoryColumnSettings,
                        bounds,
                        window,
                        cx,
                    );
                } else {
                    this.open_popover_at(
                        PopoverKind::HistoryColumnSettings,
                        e.position(),
                        window,
                        cx,
                    );
                }
            })
        };
        let column_settings_btn_inner = div()
            .id("history_columns_settings_btn")
            .flex()
            .items_center()
            .justify_center()
            .w(px(18.0))
            .h(px(18.0))
            .rounded(px(theme.radii.row))
            .when(column_settings_active, |d| d.bg(theme.colors.active))
            .hover(move |s| {
                if column_settings_active {
                    s.bg(theme.colors.active)
                } else {
                    s.bg(with_alpha(theme.colors.hover, 0.55))
                }
            })
            .active(move |s| s.bg(theme.colors.active))
            .cursor(CursorStyle::PointingHand)
            .child(svg_icon("icons/cog.svg", icon_muted, px(12.0)))
            .on_click(open_column_settings)
            .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                let text: SharedString = "History columns".into();
                let mut changed = false;
                if *hovering {
                    changed |= this.set_tooltip_text_if_changed(Some(text.clone()), cx);
                } else {
                    changed |= this.clear_tooltip_if_matches(&text, cx);
                }
                if changed {
                    cx.notify();
                }
            }));
        let column_settings_btn = div()
            .on_children_prepainted(move |children_bounds, _w, _cx| {
                if let Some(bounds) = children_bounds.first() {
                    *column_settings_anchor_bounds_for_prepaint.borrow_mut() = Some(*bounds);
                }
            })
            .child(column_settings_btn_inner);

        let resize_handle = |id: &'static str, handle: HistoryColResizeHandle| {
            div()
                .id(id)
                .absolute()
                .w(handle_w)
                .top_0()
                .bottom_0()
                .flex()
                .items_center()
                .justify_center()
                .cursor(CursorStyle::ResizeLeftRight)
                .hover(move |s| s.bg(theme.colors.hover))
                .active(move |s| s.bg(theme.colors.active))
                .child(div().w(px(1.0)).h(px(14.0)).bg(theme.colors.border))
                .on_drag(handle, |_handle, _offset, _window, cx| {
                    cx.new(|_cx| HistoryColResizeDragGhost)
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, e: &MouseDownEvent, _w, cx| {
                        cx.stop_propagation();
                        if handle == HistoryColResizeHandle::Graph {
                            this.history_col_graph_auto = false;
                        }
                        let available_width = this.history_content_width;
                        let drag_layout = super::HistoryColumnDragLayout {
                            show_author: this.history_show_author,
                            show_date: this.history_show_date,
                            show_sha: this.history_show_sha,
                            branch_w: this.history_col_branch,
                            graph_w: this.history_col_graph,
                            author_w: this.history_col_author,
                            date_w: this.history_col_date,
                            sha_w: this.history_col_sha,
                        };
                        this.history_col_resize = Some(super::history_column_resize_state(
                            handle,
                            e.position.x,
                            available_width,
                            drag_layout,
                        ));
                        cx.notify();
                    }),
                )
                .on_drag_move(cx.listener(
                    move |this, e: &gpui::DragMoveEvent<HistoryColResizeHandle>, _w, cx| {
                        let Some(mut state) = this.history_col_resize else {
                            return;
                        };
                        if state.handle != *e.drag(cx) {
                            return;
                        }

                        let available_width = this.history_content_width;
                        let next = super::history_column_drag_clamped_width_for_state(
                            &mut state,
                            e.event.position.x,
                            available_width,
                        );
                        let width = this.history_column_width_mut(state.handle);
                        let changed = *width != next;
                        if changed {
                            *width = next;
                        }
                        this.history_col_resize = Some(state);
                        if changed {
                            cx.notify();
                        }
                    },
                ))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _e, _w, cx| {
                        this.history_col_resize = None;
                        cx.notify();
                    }),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|this, _e, _w, cx| {
                        this.history_col_resize = None;
                        cx.notify();
                    }),
                )
        };

        let mut header = div()
            .relative()
            .flex()
            .h(px(24.0))
            .w_full()
            .items_center()
            .px_2()
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .text_color(theme.colors.text_muted)
            .child(
                div()
                    .w(self.history_col_branch)
                    .flex()
                    .items_center()
                    .gap_1()
                    .min_w(px(0.0))
                    .px(cell_pad)
                    .overflow_hidden()
                    .child(
                        div()
                            .on_children_prepainted(move |children_bounds, _w, _cx| {
                                if let Some(bounds) = children_bounds.first() {
                                    *scope_anchor_bounds_for_prepaint.borrow_mut() = Some(*bounds);
                                }
                            })
                            .child(
                                div()
                                    .id("history_scope_header")
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .px_1()
                                    .h(px(18.0))
                                    .line_height(px(18.0))
                                    .rounded(px(theme.radii.row))
                                    .when(scope_active, |d| d.bg(theme.colors.active))
                                    .hover(move |s| {
                                        if scope_active {
                                            s.bg(theme.colors.active)
                                        } else {
                                            s.bg(with_alpha(theme.colors.hover, 0.55))
                                        }
                                    })
                                    .active(move |s| s.bg(theme.colors.active))
                                    .cursor(CursorStyle::PointingHand)
                                    .child(
                                        div()
                                            .min_w(px(0.0))
                                            .line_clamp(1)
                                            .whitespace_nowrap()
                                            .child(scope_label.clone()),
                                    )
                                    .child(svg_icon("icons/chevron_down.svg", icon_muted, px(12.0)))
                                    .when_some(scope_repo_id, |this, repo_id| {
                                        let scope_invoker = scope_invoker.clone();
                                        let scope_anchor_bounds_for_click =
                                            Rc::clone(&scope_anchor_bounds_for_click);
                                        this.on_click(cx.listener(
                                            move |this, e: &ClickEvent, window, cx| {
                                                this.activate_context_menu_invoker(
                                                    scope_invoker.clone(),
                                                    cx,
                                                );
                                                if let Some(bounds) =
                                                    *scope_anchor_bounds_for_click.borrow()
                                                {
                                                    this.open_popover_for_bounds(
                                                        PopoverKind::HistoryBranchFilter {
                                                            repo_id,
                                                        },
                                                        bounds,
                                                        window,
                                                        cx,
                                                    );
                                                } else {
                                                    this.open_popover_at(
                                                        PopoverKind::HistoryBranchFilter {
                                                            repo_id,
                                                        },
                                                        e.position(),
                                                        window,
                                                        cx,
                                                    );
                                                }
                                            },
                                        ))
                                    })
                                    .when(scope_repo_id.is_none(), |this| {
                                        this.opacity(0.6).cursor(CursorStyle::Arrow)
                                    })
                                    .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                                        let text: SharedString =
                                            "History scope (Current branch / All branches)".into();
                                        let mut changed = false;
                                        if *hovering {
                                            changed |= this.set_tooltip_text_if_changed(
                                                Some(text.clone()),
                                                cx,
                                            );
                                        } else {
                                            changed |= this.clear_tooltip_if_matches(&text, cx);
                                        }
                                        if changed {
                                            cx.notify();
                                        }
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .w(self.history_col_graph)
                    .flex()
                    .justify_center()
                    .px(cell_pad)
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .child("GRAPH"),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(cell_pad)
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .line_clamp(1)
                            .whitespace_nowrap()
                            .child("COMMIT MESSAGE"),
                    )
                    .child(column_settings_btn),
            )
            .when(show_author, |header| {
                header.child(
                    div()
                        .w(col_author)
                        .flex()
                        .items_center()
                        .justify_end()
                        .px(cell_pad)
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .child("AUTHOR"),
                )
            });

        if show_date {
            header = header.child(
                div()
                    .w(col_date)
                    .flex()
                    .items_center()
                    .justify_end()
                    .px(cell_pad)
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .font_family(UI_MONOSPACE_FONT_FAMILY)
                    .child("Commit date"),
            );
        }

        if show_sha {
            header = header.child(
                div()
                    .w(col_sha)
                    .flex()
                    .items_center()
                    .justify_end()
                    .px(cell_pad)
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .font_family(UI_MONOSPACE_FONT_FAMILY)
                    .child("SHA"),
            );
        }

        let mut header_with_handles = header
            .child(
                resize_handle("history_col_resize_branch", HistoryColResizeHandle::Branch)
                    .left((self.history_col_branch - handle_half).max(px(0.0))),
            )
            .child(
                resize_handle("history_col_resize_graph", HistoryColResizeHandle::Graph).left(
                    (self.history_col_branch + self.history_col_graph - handle_half).max(px(0.0)),
                ),
            );

        if show_author {
            let right_fixed = col_author
                + if show_date { col_date } else { px(0.0) }
                + if show_sha { col_sha } else { px(0.0) };
            header_with_handles = header_with_handles.child(
                resize_handle("history_col_resize_author", HistoryColResizeHandle::Author)
                    .right((right_fixed - handle_half).max(px(0.0))),
            );
        }

        if show_date {
            let right_fixed = col_date + if show_sha { col_sha } else { px(0.0) };
            header_with_handles = header_with_handles.child(
                resize_handle("history_col_resize_date", HistoryColResizeHandle::Date)
                    .right((right_fixed - handle_half).max(px(0.0))),
            );
        }

        if show_sha {
            header_with_handles = header_with_handles.child(
                resize_handle("history_col_resize_sha", HistoryColResizeHandle::Sha)
                    .right((col_sha - handle_half).max(px(0.0))),
            );
        }

        header_with_handles
    }
}
