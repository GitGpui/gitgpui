use super::*;

mod app_menu;
mod blame;
mod branch_picker;
mod clone_repo;
mod context_menu;
mod create_branch;
mod create_tag_prompt;
mod diff_hunks;
mod discard_changes_confirm;
mod file_history;
mod force_push_confirm;
mod push_set_upstream_prompt;
mod rebase_prompt;
mod remote_add_prompt;
mod remote_edit_url_prompt;
mod remote_remove_confirm;
mod remote_remove_picker;
mod remote_url_picker;
mod repo_picker;
mod reset_prompt;
mod settings;
mod stash_prompt;
mod submodule_add_prompt;
mod submodule_open_picker;
mod submodule_remove_confirm;
mod submodule_remove_picker;
mod worktree_add_prompt;
mod worktree_open_picker;
mod worktree_remove_confirm;
mod worktree_remove_picker;

impl GitGpuiView {
    pub(in super::super) fn close_popover(&mut self, cx: &mut gpui::Context<Self>) {
        self.popover = None;
        self.popover_anchor = None;
        self.context_menu_selected_ix = None;
        self.conflict_resolver = ConflictResolverUiState::default();
        cx.notify();
    }

    pub(in super::super) fn open_popover_at(
        &mut self,
        kind: PopoverKind,
        anchor: Point<Pixels>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let is_context_menu = matches!(
            &kind,
            PopoverKind::PullPicker
                | PopoverKind::PushPicker
                | PopoverKind::HistoryBranchFilter { .. }
                | PopoverKind::DiffHunkMenu { .. }
                | PopoverKind::DiffEditorMenu { .. }
                | PopoverKind::CommitMenu { .. }
                | PopoverKind::StatusFileMenu { .. }
                | PopoverKind::BranchMenu { .. }
                | PopoverKind::BranchSectionMenu { .. }
                | PopoverKind::CommitFileMenu { .. }
                | PopoverKind::TagMenu { .. }
        );

        self.popover = Some(kind.clone());
        self.popover_anchor = Some(anchor);
        self.context_menu_selected_ix = None;
        if is_context_menu {
            self.context_menu_selected_ix = self
                .context_menu_model(&kind, cx)
                .and_then(|m| m.first_selectable());
            window.focus(&self.context_menu_focus_handle);
        } else {
            match &kind {
                PopoverKind::RepoPicker => {
                    let _ = self.ensure_repo_picker_search_input(window, cx);
                }
                PopoverKind::BranchPicker => {
                    let _ = self.ensure_branch_picker_search_input(window, cx);
                }
                PopoverKind::CreateBranch => {
                    let theme = self.theme;
                    self.create_branch_input.update(cx, |input, cx| {
                        input.set_theme(theme, cx);
                        input.set_text("", cx);
                        cx.notify();
                    });
                    let focus = self
                        .create_branch_input
                        .read_with(cx, |i, _| i.focus_handle());
                    window.focus(&focus);
                }
                PopoverKind::StashPrompt => {
                    let theme = self.theme;
                    self.stash_message_input.update(cx, |input, cx| {
                        input.set_theme(theme, cx);
                        input.set_text("", cx);
                        cx.notify();
                    });
                    let focus = self
                        .stash_message_input
                        .read_with(cx, |i, _| i.focus_handle());
                    window.focus(&focus);
                }
                PopoverKind::CloneRepo => {
                    let theme = self.theme;
                    let url_text = self
                        .clone_repo_url_input
                        .read_with(cx, |i, _| i.text().to_string());
                    let parent_text = self
                        .clone_repo_parent_dir_input
                        .read_with(cx, |i, _| i.text().to_string());
                    self.clone_repo_url_input.update(cx, |input, cx| {
                        input.set_theme(theme, cx);
                        input.set_text(url_text, cx);
                        cx.notify();
                    });
                    self.clone_repo_parent_dir_input.update(cx, |input, cx| {
                        input.set_theme(theme, cx);
                        input.set_text(parent_text, cx);
                        cx.notify();
                    });
                    let focus = self
                        .clone_repo_url_input
                        .read_with(cx, |i, _| i.focus_handle());
                    window.focus(&focus);
                }
                PopoverKind::RebasePrompt { .. } => {
                    let theme = self.theme;
                    self.rebase_onto_input.update(cx, |input, cx| {
                        input.set_theme(theme, cx);
                        input.set_text("", cx);
                        cx.notify();
                    });
                    let focus = self
                        .rebase_onto_input
                        .read_with(cx, |i, _| i.focus_handle());
                    window.focus(&focus);
                }
                PopoverKind::CreateTagPrompt { .. } => {
                    let theme = self.theme;
                    self.create_tag_input.update(cx, |input, cx| {
                        input.set_theme(theme, cx);
                        input.set_text("", cx);
                        cx.notify();
                    });
                    let focus = self.create_tag_input.read_with(cx, |i, _| i.focus_handle());
                    window.focus(&focus);
                }
                PopoverKind::RemoteAddPrompt { .. } => {
                    let theme = self.theme;
                    self.remote_name_input.update(cx, |input, cx| {
                        input.set_theme(theme, cx);
                        input.set_text("", cx);
                        cx.notify();
                    });
                    self.remote_url_input.update(cx, |input, cx| {
                        input.set_theme(theme, cx);
                        input.set_text("", cx);
                        cx.notify();
                    });
                    let focus = self
                        .remote_name_input
                        .read_with(cx, |i, _| i.focus_handle());
                    window.focus(&focus);
                }
                PopoverKind::RemoteEditUrlPrompt { .. } => {
                    let theme = self.theme;
                    let text = self
                        .remote_url_edit_input
                        .read_with(cx, |i, _| i.text().to_string());
                    self.remote_url_edit_input.update(cx, |input, cx| {
                        input.set_theme(theme, cx);
                        input.set_text(text, cx);
                        cx.notify();
                    });
                    let focus = self
                        .remote_url_edit_input
                        .read_with(cx, |i, _| i.focus_handle());
                    window.focus(&focus);
                }
                PopoverKind::RemoteUrlPicker { .. } | PopoverKind::RemoteRemovePicker { .. } => {
                    let _ = self.ensure_remote_picker_search_input(window, cx);
                }
                PopoverKind::WorktreeAddPrompt { .. } => {
                    let theme = self.theme;
                    self.worktree_path_input.update(cx, |input, cx| {
                        input.set_theme(theme, cx);
                        input.set_text("", cx);
                        cx.notify();
                    });
                    self.worktree_ref_input.update(cx, |input, cx| {
                        input.set_theme(theme, cx);
                        input.set_text("", cx);
                        cx.notify();
                    });
                    let focus = self
                        .worktree_path_input
                        .read_with(cx, |i, _| i.focus_handle());
                    window.focus(&focus);
                }
                PopoverKind::WorktreeOpenPicker { repo_id }
                | PopoverKind::WorktreeRemovePicker { repo_id } => {
                    let _ = self.ensure_worktree_picker_search_input(window, cx);
                    self.store
                        .dispatch(Msg::LoadWorktrees { repo_id: *repo_id });
                }
                PopoverKind::SubmoduleAddPrompt { .. } => {
                    let theme = self.theme;
                    self.submodule_url_input.update(cx, |input, cx| {
                        input.set_theme(theme, cx);
                        input.set_text("", cx);
                        cx.notify();
                    });
                    self.submodule_path_input.update(cx, |input, cx| {
                        input.set_theme(theme, cx);
                        input.set_text("", cx);
                        cx.notify();
                    });
                    let focus = self
                        .submodule_url_input
                        .read_with(cx, |i, _| i.focus_handle());
                    window.focus(&focus);
                }
                PopoverKind::SubmoduleOpenPicker { repo_id }
                | PopoverKind::SubmoduleRemovePicker { repo_id } => {
                    let _ = self.ensure_submodule_picker_search_input(window, cx);
                    self.store
                        .dispatch(Msg::LoadSubmodules { repo_id: *repo_id });
                }
                PopoverKind::FileHistory { repo_id, path } => {
                    self.ensure_file_history_search_input(window, cx);
                    self.store.dispatch(Msg::LoadFileHistory {
                        repo_id: *repo_id,
                        path: path.clone(),
                        limit: 200,
                    });
                }
                PopoverKind::Blame { repo_id, path, rev } => {
                    self.blame_scroll = UniformListScrollHandle::default();
                    self.store.dispatch(Msg::LoadBlame {
                        repo_id: *repo_id,
                        path: path.clone(),
                        rev: rev.clone(),
                    });
                }
                PopoverKind::PushSetUpstreamPrompt { .. } => {
                    let theme = self.theme;
                    let text = self
                        .push_upstream_branch_input
                        .read_with(cx, |i, _| i.text().to_string());
                    self.push_upstream_branch_input.update(cx, |input, cx| {
                        input.set_theme(theme, cx);
                        input.set_text(text, cx);
                        cx.notify();
                    });
                    let focus = self
                        .push_upstream_branch_input
                        .read_with(cx, |i, _| i.focus_handle());
                    window.focus(&focus);
                }
                PopoverKind::DiffHunks => {
                    let _ = self.ensure_diff_hunk_picker_search_input(window, cx);
                }
                _ => {}
            }
        }
        cx.notify();
    }
}

impl MainPaneView {
    pub(super) fn history_column_headers(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let (show_date, show_sha) = self.history_visible_columns();
        let col_date = self.history_col_date;
        let col_sha = self.history_col_sha;
        let handle_w = px(HISTORY_COL_HANDLE_PX);
        let handle_half = px(HISTORY_COL_HANDLE_PX / 2.0);
        let scope_label: SharedString = self
            .active_repo()
            .map(|r| match r.history_scope {
                gitgpui_core::domain::LogScope::CurrentBranch => "Current branch".to_string(),
                gitgpui_core::domain::LogScope::AllBranches => "All branches".to_string(),
            })
            .unwrap_or_else(|| "Current branch".to_string())
            .into();
        let scope_repo_id = self.active_repo_id();

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
                        this.history_col_resize = Some(HistoryColResizeState {
                            handle,
                            start_x: e.position.x,
                            start_branch: this.history_col_branch,
                            start_graph: this.history_col_graph,
                            start_date: this.history_col_date,
                            start_sha: this.history_col_sha,
                        });
                        cx.notify();
                    }),
                )
                .on_drag_move(cx.listener(
                    move |this, e: &gpui::DragMoveEvent<HistoryColResizeHandle>, _w, cx| {
                        let Some(state) = this.history_col_resize else {
                            return;
                        };
                        if state.handle != *e.drag(cx) {
                            return;
                        }

                        let dx = e.event.position.x - state.start_x;
                        match state.handle {
                            HistoryColResizeHandle::Branch => {
                                this.history_col_branch =
                                    (state.start_branch + dx).max(px(HISTORY_COL_BRANCH_MIN_PX));
                            }
                            HistoryColResizeHandle::Graph => {
                                this.history_col_graph =
                                    (state.start_graph + dx).max(px(HISTORY_COL_GRAPH_MIN_PX));
                            }
                            HistoryColResizeHandle::Message => {
                                this.history_col_date =
                                    (state.start_date - dx).max(px(HISTORY_COL_DATE_MIN_PX));
                            }
                            HistoryColResizeHandle::Date => {
                                let total = state.start_date + state.start_sha;
                                let min_date = px(HISTORY_COL_DATE_MIN_PX);
                                let min_sha = px(HISTORY_COL_SHA_MIN_PX);
                                let max_date = (total - min_sha).max(min_date);
                                this.history_col_date =
                                    (state.start_date + dx).max(min_date).min(max_date);
                                this.history_col_sha = (total - this.history_col_date).max(min_sha);
                            }
                        }
                        cx.notify();
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
                            .hover(move |s| s.bg(with_alpha(theme.colors.hover, 0.55)))
                            .cursor(CursorStyle::PointingHand)
                            .child(
                                div()
                                    .min_w(px(0.0))
                                    .line_clamp(1)
                                    .whitespace_nowrap()
                                    .child(scope_label.clone()),
                            )
                            .child(
                                gpui::svg()
                                    .path("icons/chevron_down.svg")
                                    .w(px(12.0))
                                    .h(px(12.0))
                                    .text_color(theme.colors.text_muted)
                                    .flex_shrink_0(),
                            )
                            .when_some(scope_repo_id, |this, repo_id| {
                                this.on_click(cx.listener(
                                    move |this, e: &ClickEvent, window, cx| {
                                        this.open_popover_at(
                                            PopoverKind::HistoryBranchFilter { repo_id },
                                            e.position(),
                                            window,
                                            cx,
                                        );
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
                                    changed |=
                                        this.set_tooltip_text_if_changed(Some(text.clone()), cx);
                                } else {
                                    changed |= this.clear_tooltip_if_matches(&text, cx);
                                }
                                if changed {
                                    cx.notify();
                                }
                            })),
                    ),
            )
            .child(
                div()
                    .w(self.history_col_graph)
                    .flex()
                    .justify_center()
                    .whitespace_nowrap()
                    .child("GRAPH"),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .whitespace_nowrap()
                    .child("COMMIT MESSAGE"),
            );

        if show_date {
            header = header.child(
                div()
                    .w(col_date)
                    .flex()
                    .justify_end()
                    .whitespace_nowrap()
                    .child("Commit date"),
            );
        }

        if show_sha {
            header = header.child(
                div()
                    .w(col_sha)
                    .flex()
                    .justify_end()
                    .whitespace_nowrap()
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

        if show_date {
            let right_fixed = col_date + if show_sha { col_sha } else { px(0.0) };
            header_with_handles = header_with_handles.child(
                resize_handle(
                    "history_col_resize_message",
                    HistoryColResizeHandle::Message,
                )
                .right((right_fixed - handle_half).max(px(0.0))),
            );
        }

        if show_sha {
            header_with_handles = header_with_handles.child(
                resize_handle("history_col_resize_date", HistoryColResizeHandle::Date)
                    .right((col_sha - handle_half).max(px(0.0))),
            );
        }

        header_with_handles
    }
}

impl GitGpuiView {
    pub(in super::super) fn render_blame_popover_rows(
        this: &mut Self,
        range: std::ops::Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some((repo_id, path)) = this.popover.as_ref().and_then(|k| match k {
            PopoverKind::Blame { repo_id, path, .. } => Some((*repo_id, path.clone())),
            _ => None,
        }) else {
            return Vec::new();
        };

        let Some(repo) = this.state.repos.iter().find(|r| r.id == repo_id) else {
            return Vec::new();
        };
        let Loadable::Ready(lines) = &repo.blame else {
            return Vec::new();
        };

        let theme = this.theme;
        let mut rows = Vec::with_capacity(range.len());
        for ix in range {
            let Some(line) = lines.get(ix) else {
                continue;
            };
            let line_no = ix + 1;
            let sha = line.commit_id.clone();
            let short = sha.get(0..8).unwrap_or(&sha).to_string();
            let author: SharedString = line.author.clone().into();
            let code: SharedString = line.line.clone().into();
            let commit_id = CommitId(sha);
            let path = path.clone();

            rows.push(
                div()
                    .id(("blame_row", ix))
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .px_2()
                    .gap_2()
                    .hover(move |s| s.bg(theme.colors.hover))
                    .active(move |s| s.bg(theme.colors.active))
                    .child(
                        div()
                            .w(px(44.0))
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .whitespace_nowrap()
                            .child(format!("{line_no:>4}")),
                    )
                    .child(
                        div()
                            .w(px(76.0))
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .whitespace_nowrap()
                            .child(short),
                    )
                    .child(
                        div()
                            .w(px(140.0))
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .line_clamp(1)
                            .whitespace_nowrap()
                            .child(author),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .text_xs()
                            .font_family("monospace")
                            .line_clamp(1)
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .child(code),
                    )
                    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                        this.store.dispatch(Msg::SelectCommit {
                            repo_id,
                            commit_id: commit_id.clone(),
                        });
                        this.store.dispatch(Msg::SelectDiff {
                            repo_id,
                            target: DiffTarget::Commit {
                                commit_id: commit_id.clone(),
                                path: Some(path.clone()),
                            },
                        });
                        this.rebuild_diff_cache();
                        this.popover = None;
                        this.popover_anchor = None;
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }

        rows
    }

    pub(in super::super) fn popover_view(
        &mut self,
        kind: PopoverKind,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let anchor = self
            .popover_anchor
            .unwrap_or_else(|| point(px(64.0), px(64.0)));

        let is_app_menu = matches!(&kind, PopoverKind::AppMenu);
        let anchor_corner = match &kind {
            PopoverKind::PullPicker
            | PopoverKind::Settings
            | PopoverKind::PushPicker
            | PopoverKind::CreateBranch
            | PopoverKind::StashPrompt
            | PopoverKind::CloneRepo
            | PopoverKind::ResetPrompt { .. }
            | PopoverKind::RebasePrompt { .. }
            | PopoverKind::CreateTagPrompt { .. }
            | PopoverKind::RemoteAddPrompt { .. }
            | PopoverKind::RemoteUrlPicker { .. }
            | PopoverKind::RemoteRemovePicker { .. }
            | PopoverKind::RemoteEditUrlPrompt { .. }
            | PopoverKind::RemoteRemoveConfirm { .. }
            | PopoverKind::WorktreeAddPrompt { .. }
            | PopoverKind::WorktreeOpenPicker { .. }
            | PopoverKind::WorktreeRemovePicker { .. }
            | PopoverKind::WorktreeRemoveConfirm { .. }
            | PopoverKind::SubmoduleAddPrompt { .. }
            | PopoverKind::SubmoduleOpenPicker { .. }
            | PopoverKind::SubmoduleRemovePicker { .. }
            | PopoverKind::SubmoduleRemoveConfirm { .. }
            | PopoverKind::PushSetUpstreamPrompt { .. }
            | PopoverKind::ForcePushConfirm { .. }
            | PopoverKind::HistoryBranchFilter { .. } => Corner::TopRight,
            _ => Corner::TopLeft,
        };

        let panel = match kind {
            PopoverKind::RepoPicker => repo_picker::panel(self, cx),
            PopoverKind::Settings => settings::panel(self, cx),
            /* PopoverKind::ConflictResolver { repo_id, path } => {
                if let Some(repo) = self.state.repos.iter().find(|r| r.id == repo_id) {
                    let window_size = self.ui_window_size_last_seen;
                    let max_w = (window_size.width - px(96.0)).max(px(320.0));
                    let max_h = (window_size.height - px(120.0)).max(px(240.0));

                    let title: SharedString =
                        format!("Resolve conflict: {}", self.cached_path_display(&path)).into();

                    match &repo.conflict_file {
                    Loadable::NotLoaded | Loadable::Loading => {
                        zed::empty_state(theme, title, "Loading…")
                    }
                    Loadable::Error(e) => zed::empty_state(theme, title, e.clone()),
                    Loadable::Ready(None) => zed::empty_state(theme, title, "No conflict data."),
                    Loadable::Ready(Some(file)) => {
                        let ours = file.ours.clone().unwrap_or_default();
                        let theirs = file.theirs.clone().unwrap_or_default();
                        let has_current = file.current.is_some();

                        let mode = self.conflict_resolver.diff_mode;
                        let diff_len = match mode {
                            ConflictDiffMode::Split => self.conflict_resolver.diff_rows.len(),
                            ConflictDiffMode::Inline => self.conflict_resolver.inline_rows.len(),
                        };

                        let selection_empty = self.conflict_resolver_selection_is_empty();

                        let toggle_mode_split = |this: &mut GitGpuiView,
                                                 _e: &ClickEvent,
                                                 _w: &mut Window,
                                                 cx: &mut gpui::Context<Self>| {
                            this.conflict_resolver_set_mode(ConflictDiffMode::Split, cx);
                        };
                        let toggle_mode_inline = |this: &mut GitGpuiView,
                                                  _e: &ClickEvent,
                                                  _w: &mut Window,
                                                  cx: &mut gpui::Context<Self>| {
                            this.conflict_resolver_set_mode(ConflictDiffMode::Inline, cx);
                        };

                        let clear_selection = |this: &mut GitGpuiView,
                                               _e: &ClickEvent,
                                               _w: &mut Window,
                                               cx: &mut gpui::Context<Self>| {
                            this.conflict_resolver_clear_selection(cx);
                        };

                        let append_selection = |this: &mut GitGpuiView,
                                               _e: &ClickEvent,
                                               _w: &mut Window,
                                               cx: &mut gpui::Context<Self>| {
                            this.conflict_resolver_append_selection_to_output(cx);
                        };

                        let ours_for_btn = ours.clone();
                        let set_output_ours = move |this: &mut GitGpuiView,
                                                    _e: &ClickEvent,
                                                    _w: &mut Window,
                                                    cx: &mut gpui::Context<Self>| {
                            this.conflict_resolver_set_output(ours_for_btn.clone(), cx);
                        };
                        let theirs_for_btn = theirs.clone();
                        let set_output_theirs = move |this: &mut GitGpuiView,
                                                      _e: &ClickEvent,
                                                      _w: &mut Window,
                                                      cx: &mut gpui::Context<Self>| {
                            this.conflict_resolver_set_output(theirs_for_btn.clone(), cx);
                        };
                        let reset_from_markers = |this: &mut GitGpuiView,
                                                  _e: &ClickEvent,
                                                  _w: &mut Window,
                                                  cx: &mut gpui::Context<Self>| {
                            this.conflict_resolver_reset_output_from_markers(cx);
                        };

                        let save_path = path.clone();
                        let save_close = move |this: &mut GitGpuiView,
                                               _e: &ClickEvent,
                                               _w: &mut Window,
                                               cx: &mut gpui::Context<Self>| {
                            let text = this
                                .conflict_resolver_input
                                .read_with(cx, |i, _| i.text().to_string());
                            this.store.dispatch(Msg::SaveWorktreeFile {
                                repo_id,
                                path: save_path.clone(),
                                contents: text,
                                stage: false,
                            });
                            this.close_popover(cx);
                        };
                        let save_path = path.clone();
                        let save_stage_close = move |this: &mut GitGpuiView,
                                                     _e: &ClickEvent,
                                                     _w: &mut Window,
                                                     cx: &mut gpui::Context<Self>| {
                            let text = this
                                .conflict_resolver_input
                                .read_with(cx, |i, _| i.text().to_string());
                            this.store.dispatch(Msg::SaveWorktreeFile {
                                repo_id,
                                path: save_path.clone(),
                                contents: text,
                                stage: true,
                            });
                            this.close_popover(cx);
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
                                zed::Button::new("conflict_append_selected", "Append selection")
                                    .style(zed::ButtonStyle::Outlined)
                                    .disabled(selection_empty)
                                    .on_click(theme, cx, append_selection),
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
                                zed::Button::new("conflict_reset_markers", "Reset from markers")
                                    .style(zed::ButtonStyle::Transparent)
                                    .disabled(!has_current)
                                    .on_click(theme, cx, reset_from_markers),
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
                            .child(div().flex().items_center().gap_2().child(mode_controls).child(selection_controls));

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
                            zed::empty_state(theme, "Diff", "Ours/Theirs content not available.")
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
                                    .child("Resolved output (editable)"),
                            )
                            .child(start_controls);

                        div()
                            .flex()
                            .flex_col()
                            .max_w(max_w)
                            .max_h(max_h)
                            .min_w(px(720.0))
                            .min_h(px(520.0))
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .child(title.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1()
                                                .child(
                                                    zed::Button::new("conflict_save_close", "Save & close")
                                                        .style(zed::ButtonStyle::Outlined)
                                                        .on_click(theme, cx, save_close),
                                                )
                                                .child(
                                                    zed::Button::new("conflict_save_stage_close", "Save & stage & close")
                                                        .style(zed::ButtonStyle::Filled)
                                                        .on_click(theme, cx, save_stage_close),
                                                ),
                                    ),
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
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_h(px(0.0))
                                    .border_1()
                                    .border_color(theme.colors.border)
                                    .rounded(px(theme.radii.row))
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .id("conflict_resolver_output_scroll")
                                            .font_family("monospace")
                                            .h_full()
                                            .min_h(px(0.0))
                                            .overflow_y_scroll()
                                            .child(self.conflict_resolver_input.clone()),
                                    ),
                            )
                    }
                    }
                } else {
                    zed::empty_state(theme, "Conflicts", "Repository not found.")
                }
            } */
            PopoverKind::BranchPicker => branch_picker::panel(self, cx),
            PopoverKind::CreateBranch => create_branch::panel(self, cx),
            PopoverKind::StashPrompt => stash_prompt::panel(self, cx),
            PopoverKind::CloneRepo => clone_repo::panel(self, cx),
            PopoverKind::ResetPrompt {
                repo_id,
                target,
                mode,
            } => reset_prompt::panel(self, repo_id, target, mode, cx),
            PopoverKind::RebasePrompt { repo_id } => rebase_prompt::panel(self, repo_id, cx),
            PopoverKind::CreateTagPrompt { repo_id, target } => {
                create_tag_prompt::panel(self, repo_id, target, cx)
            }
            PopoverKind::RemoteAddPrompt { repo_id } => remote_add_prompt::panel(self, repo_id, cx),
            PopoverKind::RemoteUrlPicker { repo_id, kind } => {
                remote_url_picker::panel(self, repo_id, kind, cx)
            }
            PopoverKind::RemoteEditUrlPrompt {
                repo_id,
                name,
                kind,
            } => remote_edit_url_prompt::panel(self, repo_id, name, kind, cx),
            PopoverKind::RemoteRemovePicker { repo_id } => {
                remote_remove_picker::panel(self, repo_id, cx)
            }
            PopoverKind::RemoteRemoveConfirm { repo_id, name } => {
                remote_remove_confirm::panel(self, repo_id, name, cx)
            }
            PopoverKind::WorktreeAddPrompt { repo_id } => {
                worktree_add_prompt::panel(self, repo_id, cx)
            }
            PopoverKind::WorktreeOpenPicker { repo_id } => {
                worktree_open_picker::panel(self, repo_id, cx)
            }
            PopoverKind::WorktreeRemovePicker { repo_id } => {
                worktree_remove_picker::panel(self, repo_id, cx)
            }
            PopoverKind::WorktreeRemoveConfirm { repo_id, path } => {
                worktree_remove_confirm::panel(self, repo_id, path, cx)
            }
            PopoverKind::SubmoduleAddPrompt { repo_id } => {
                submodule_add_prompt::panel(self, repo_id, cx)
            }
            PopoverKind::SubmoduleOpenPicker { repo_id } => {
                submodule_open_picker::panel(self, repo_id, cx)
            }
            PopoverKind::SubmoduleRemovePicker { repo_id } => {
                submodule_remove_picker::panel(self, repo_id, cx)
            }
            PopoverKind::SubmoduleRemoveConfirm { repo_id, path } => {
                submodule_remove_confirm::panel(self, repo_id, path, cx)
            }
            PopoverKind::FileHistory { repo_id, path } => {
                file_history::panel(self, repo_id, path, cx)
            }
            PopoverKind::Blame { repo_id, path, rev } => blame::panel(self, repo_id, path, rev, cx),
            PopoverKind::PushSetUpstreamPrompt { repo_id, remote } => {
                push_set_upstream_prompt::panel(self, repo_id, remote, cx)
            }
            PopoverKind::ForcePushConfirm { repo_id } => {
                force_push_confirm::panel(self, repo_id, cx)
            }
            PopoverKind::DiscardChangesConfirm { repo_id, paths } => {
                discard_changes_confirm::panel(self, repo_id, paths.clone(), cx)
            }
            PopoverKind::HistoryBranchFilter { repo_id } => self
                .context_menu_view(PopoverKind::HistoryBranchFilter { repo_id }, cx)
                .min_w(px(160.0))
                .max_w(px(220.0)),
            PopoverKind::PullPicker => self.context_menu_view(PopoverKind::PullPicker, cx),
            PopoverKind::PushPicker => self.context_menu_view(PopoverKind::PushPicker, cx),
            PopoverKind::DiffHunks => diff_hunks::panel(self, cx),
            PopoverKind::CommitMenu { repo_id, commit_id } => {
                self.context_menu_view(PopoverKind::CommitMenu { repo_id, commit_id }, cx)
            }
            PopoverKind::TagMenu { repo_id, commit_id } => {
                self.context_menu_view(PopoverKind::TagMenu { repo_id, commit_id }, cx)
            }
            PopoverKind::DiffHunkMenu { repo_id, src_ix } => self
                .context_menu_view(PopoverKind::DiffHunkMenu { repo_id, src_ix }, cx)
                .min_w(px(160.0))
                .max_w(px(220.0)),
            PopoverKind::DiffEditorMenu {
                repo_id,
                area,
                path,
                hunk_patch,
                hunks_count,
                lines_patch,
                lines_count,
                copy_text,
            } => self
                .context_menu_view(
                    PopoverKind::DiffEditorMenu {
                        repo_id,
                        area,
                        path,
                        hunk_patch,
                        hunks_count,
                        lines_patch,
                        lines_count,
                        copy_text,
                    },
                    cx,
                )
                .min_w(px(160.0))
                .max_w(px(260.0)),
            PopoverKind::StatusFileMenu {
                repo_id,
                area,
                path,
                selection,
            } => self.context_menu_view(
                PopoverKind::StatusFileMenu {
                    repo_id,
                    area,
                    path,
                    selection,
                },
                cx,
            ),
            PopoverKind::BranchMenu {
                repo_id,
                section,
                name,
            } => self.context_menu_view(
                PopoverKind::BranchMenu {
                    repo_id,
                    section,
                    name,
                },
                cx,
            ),
            PopoverKind::BranchSectionMenu { repo_id, section } => {
                self.context_menu_view(PopoverKind::BranchSectionMenu { repo_id, section }, cx)
            }
            PopoverKind::CommitFileMenu {
                repo_id,
                commit_id,
                path,
            } => self.context_menu_view(
                PopoverKind::CommitFileMenu {
                    repo_id,
                    commit_id,
                    path,
                },
                cx,
            ),
            PopoverKind::AppMenu => app_menu::panel(self, cx),
        };

        let offset_y = if is_app_menu {
            px(40.0)
        } else if matches!(anchor_corner, Corner::TopRight) {
            px(10.0)
        } else {
            px(8.0)
        };

        anchored()
            .position(anchor)
            .anchor(anchor_corner)
            .offset(point(px(0.0), offset_y))
            .child(
                div()
                    .id("app_popover")
                    .debug_selector(|| "app_popover".to_string())
                    .on_any_mouse_down(|_e, _w, cx| cx.stop_propagation())
                    .occlude()
                    .bg(theme.colors.surface_bg_elevated)
                    .border_1()
                    .border_color(theme.colors.border)
                    .rounded(px(theme.radii.panel))
                    .shadow_lg()
                    .overflow_hidden()
                    .p_1()
                    .child(panel),
            )
    }
}

fn clone_repo_name_from_url(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches(['/', '\\']);
    let last = trimmed
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or(trimmed);
    let name = last.strip_suffix(".git").unwrap_or(last).trim();
    if name.is_empty() {
        "repo".to_string()
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitgpui_core::error::{Error, ErrorKind};
    use gitgpui_core::services::{GitBackend, GitRepository, Result};
    use std::path::Path;
    use std::sync::Arc;
    use std::time::SystemTime;

    struct TestBackend;

    impl GitBackend for TestBackend {
        fn open(&self, _workdir: &Path) -> Result<Arc<dyn GitRepository>> {
            Err(Error::new(ErrorKind::Unsupported(
                "Test backend does not open repositories",
            )))
        }
    }

    #[gpui::test]
    fn commit_menu_has_add_tag_entry(cx: &mut gpui::TestAppContext) {
        let (store, events) = AppStore::new(Arc::new(TestBackend));
        let (view, cx) =
            cx.add_window_view(|window, cx| GitGpuiView::new(store, events, None, window, cx));

        let repo_id = RepoId(1);
        let commit_id = CommitId("deadbeefdeadbeef".to_string());
        let workdir = std::env::temp_dir().join(format!(
            "gitgpui_ui_test_{}_commit_menu_tag",
            std::process::id()
        ));

        cx.update(|_window, app| {
            view.update(app, |this, cx| {
                let mut repo = RepoState::new_opening(
                    repo_id,
                    gitgpui_core::domain::RepoSpec {
                        workdir: workdir.clone(),
                    },
                );
                repo.log = Loadable::Ready(
                    gitgpui_core::domain::LogPage {
                        commits: vec![gitgpui_core::domain::Commit {
                            id: commit_id.clone(),
                            parent_ids: vec![],
                            summary: "Hello".to_string(),
                            author: "Alice".to_string(),
                            time: SystemTime::UNIX_EPOCH,
                        }],
                        next_cursor: None,
                    }
                    .into(),
                );
                repo.tags = Loadable::Ready(vec![]);

                this.state = Arc::new(AppState {
                    repos: vec![repo],
                    active_repo: Some(repo_id),
                    ..Default::default()
                });
                cx.notify();
            });
        });

        cx.update(|_window, app| {
            let model = view
                .update(app, |this, cx| {
                    this.context_menu_model(
                        &PopoverKind::CommitMenu {
                            repo_id,
                            commit_id: commit_id.clone(),
                        },
                        cx,
                    )
                })
                .expect("expected commit context menu model");

            let add_tag_action = model.items.iter().find_map(|item| match item {
                ContextMenuItem::Entry { label, action, .. } if label.as_ref() == "Add tag…" => {
                    Some(action.clone())
                }
                _ => None,
            });

            let Some(ContextMenuAction::OpenPopover { kind }) = add_tag_action else {
                panic!("expected Add tag… to open a popover");
            };

            let PopoverKind::CreateTagPrompt {
                repo_id: rid,
                target,
            } = kind
            else {
                panic!("expected Add tag… to open CreateTagPrompt");
            };

            assert_eq!(rid, repo_id);
            assert_eq!(target, commit_id.as_ref().to_string());
        });
    }

    #[gpui::test]
    fn tag_menu_lists_delete_entries_for_commit_tags(cx: &mut gpui::TestAppContext) {
        let (store, events) = AppStore::new(Arc::new(TestBackend));
        let (view, cx) =
            cx.add_window_view(|window, cx| GitGpuiView::new(store, events, None, window, cx));

        let repo_id = RepoId(2);
        let commit_id = CommitId("0123456789abcdef".to_string());
        let other_commit = CommitId("aaaaaaaaaaaaaaaa".to_string());
        let workdir =
            std::env::temp_dir().join(format!("gitgpui_ui_test_{}_tag_menu", std::process::id()));

        cx.update(|_window, app| {
            view.update(app, |this, cx| {
                let mut repo = RepoState::new_opening(
                    repo_id,
                    gitgpui_core::domain::RepoSpec {
                        workdir: workdir.clone(),
                    },
                );
                repo.log = Loadable::Ready(
                    gitgpui_core::domain::LogPage {
                        commits: vec![gitgpui_core::domain::Commit {
                            id: commit_id.clone(),
                            parent_ids: vec![],
                            summary: "Hello".to_string(),
                            author: "Alice".to_string(),
                            time: SystemTime::UNIX_EPOCH,
                        }],
                        next_cursor: None,
                    }
                    .into(),
                );
                repo.tags = Loadable::Ready(vec![
                    gitgpui_core::domain::Tag {
                        name: "release".to_string(),
                        target: commit_id.clone(),
                    },
                    gitgpui_core::domain::Tag {
                        name: "v1.0.0".to_string(),
                        target: commit_id.clone(),
                    },
                    gitgpui_core::domain::Tag {
                        name: "other".to_string(),
                        target: other_commit,
                    },
                ]);

                this.state = Arc::new(AppState {
                    repos: vec![repo],
                    active_repo: Some(repo_id),
                    ..Default::default()
                });
                cx.notify();
            });
        });

        cx.update(|_window, app| {
            let model = view
                .update(app, |this, cx| {
                    this.context_menu_model(
                        &PopoverKind::TagMenu {
                            repo_id,
                            commit_id: commit_id.clone(),
                        },
                        cx,
                    )
                })
                .expect("expected tag context menu model");

            for name in ["release", "v1.0.0"] {
                let expected_label = format!("Delete tag {name}");
                let delete_action = model.items.iter().find_map(|item| match item {
                    ContextMenuItem::Entry { label, action, .. }
                        if label.as_ref() == expected_label.as_str() =>
                    {
                        Some(action.clone())
                    }
                    _ => None,
                });
                match delete_action {
                    Some(ContextMenuAction::DeleteTag {
                        repo_id: rid,
                        name: n,
                    }) => {
                        assert_eq!(rid, repo_id);
                        assert_eq!(n, name);
                    }
                    _ => panic!("expected Delete tag {name} action"),
                }
            }

            let has_other = model.items.iter().any(|item| match item {
                ContextMenuItem::Entry { label, .. } => label.as_ref() == "Delete tag other",
                _ => false,
            });
            assert!(
                !has_other,
                "tag menu should only show tags on the clicked commit"
            );
        });
    }

    #[gpui::test]
    fn status_file_menu_uses_multi_selection_for_stage(cx: &mut gpui::TestAppContext) {
        let (store, events) = AppStore::new(Arc::new(TestBackend));
        let (view, cx) =
            cx.add_window_view(|window, cx| GitGpuiView::new(store, events, None, window, cx));

        let repo_id = RepoId(3);
        let workdir = std::env::temp_dir().join(format!(
            "gitgpui_ui_test_{}_status_menu",
            std::process::id()
        ));

        let a = std::path::PathBuf::from("a.txt");
        let b = std::path::PathBuf::from("b.txt");

        cx.update(|_window, app| {
            view.update(app, |this, cx| {
                let mut repo = RepoState::new_opening(
                    repo_id,
                    gitgpui_core::domain::RepoSpec {
                        workdir: workdir.clone(),
                    },
                );
                repo.status = Loadable::Ready(
                    gitgpui_core::domain::RepoStatus {
                        staged: vec![],
                        unstaged: vec![
                            gitgpui_core::domain::FileStatus {
                                path: a.clone(),
                                kind: gitgpui_core::domain::FileStatusKind::Modified,
                                conflict: None,
                            },
                            gitgpui_core::domain::FileStatus {
                                path: b.clone(),
                                kind: gitgpui_core::domain::FileStatusKind::Modified,
                                conflict: None,
                            },
                        ],
                    }
                    .into(),
                );

                this.state = Arc::new(AppState {
                    repos: vec![repo],
                    active_repo: Some(repo_id),
                    ..Default::default()
                });
                let _ = this.details_pane.update(cx, |pane, cx| {
                    pane.status_multi_selection.insert(
                        repo_id,
                        StatusMultiSelection {
                            unstaged: vec![a.clone(), b.clone()],
                            unstaged_anchor: Some(a.clone()),
                            staged: vec![],
                            staged_anchor: None,
                        },
                    );
                    cx.notify();
                });
                cx.notify();
            });
        });

        cx.update(|_window, app| {
            let model = view
                .update(app, |this, cx| {
                    this.context_menu_model(
                        &PopoverKind::StatusFileMenu {
                            repo_id,
                            area: DiffArea::Unstaged,
                            path: a.clone(),
                            selection: vec![a.clone(), b.clone()],
                        },
                        cx,
                    )
                })
                .expect("expected status file context menu model");

            let stage_action = model.items.iter().find_map(|item| match item {
                ContextMenuItem::Entry { label, action, .. } if label.as_ref() == "Stage (2)" => {
                    Some(action.clone())
                }
                _ => None,
            });

            match stage_action {
                Some(ContextMenuAction::StagePaths {
                    repo_id: rid,
                    paths,
                }) => {
                    assert_eq!(rid, repo_id);
                    assert_eq!(paths.len(), 2);
                    assert!(paths.contains(&a));
                    assert!(paths.contains(&b));
                }
                _ => panic!("expected Stage (2) to stage selected paths"),
            }
        });
    }

    #[gpui::test]
    fn status_file_menu_uses_multi_selection_for_unstage(cx: &mut gpui::TestAppContext) {
        let (store, events) = AppStore::new(Arc::new(TestBackend));
        let (view, cx) =
            cx.add_window_view(|window, cx| GitGpuiView::new(store, events, None, window, cx));

        let repo_id = RepoId(4);
        let workdir = std::env::temp_dir().join(format!(
            "gitgpui_ui_test_{}_status_menu_staged",
            std::process::id()
        ));

        let a = std::path::PathBuf::from("a.txt");
        let b = std::path::PathBuf::from("b.txt");

        cx.update(|_window, app| {
            view.update(app, |this, cx| {
                let mut repo = RepoState::new_opening(
                    repo_id,
                    gitgpui_core::domain::RepoSpec {
                        workdir: workdir.clone(),
                    },
                );
                repo.status = Loadable::Ready(
                    gitgpui_core::domain::RepoStatus {
                        staged: vec![
                            gitgpui_core::domain::FileStatus {
                                path: a.clone(),
                                kind: gitgpui_core::domain::FileStatusKind::Modified,
                                conflict: None,
                            },
                            gitgpui_core::domain::FileStatus {
                                path: b.clone(),
                                kind: gitgpui_core::domain::FileStatusKind::Modified,
                                conflict: None,
                            },
                        ],
                        unstaged: vec![],
                    }
                    .into(),
                );

                this.state = Arc::new(AppState {
                    repos: vec![repo],
                    active_repo: Some(repo_id),
                    ..Default::default()
                });
                let _ = this.details_pane.update(cx, |pane, cx| {
                    pane.status_multi_selection.insert(
                        repo_id,
                        StatusMultiSelection {
                            unstaged: vec![],
                            unstaged_anchor: None,
                            staged: vec![a.clone(), b.clone()],
                            staged_anchor: Some(a.clone()),
                        },
                    );
                    cx.notify();
                });
                cx.notify();
            });
        });

        cx.update(|_window, app| {
            let model = view
                .update(app, |this, cx| {
                    this.context_menu_model(
                        &PopoverKind::StatusFileMenu {
                            repo_id,
                            area: DiffArea::Staged,
                            path: a.clone(),
                            selection: vec![a.clone(), b.clone()],
                        },
                        cx,
                    )
                })
                .expect("expected status file context menu model");

            let unstage_action = model.items.iter().find_map(|item| match item {
                ContextMenuItem::Entry { label, action, .. } if label.as_ref() == "Unstage (2)" => {
                    Some(action.clone())
                }
                _ => None,
            });

            match unstage_action {
                Some(ContextMenuAction::UnstagePaths {
                    repo_id: rid,
                    paths,
                }) => {
                    assert_eq!(rid, repo_id);
                    assert_eq!(paths.len(), 2);
                    assert!(paths.contains(&a));
                    assert!(paths.contains(&b));
                }
                _ => panic!("expected Unstage (2) to unstage selected paths"),
            }
        });
    }

    #[gpui::test]
    fn status_file_menu_offers_resolve_actions_for_conflicts(cx: &mut gpui::TestAppContext) {
        let (store, events) = AppStore::new(Arc::new(TestBackend));
        let (view, cx) =
            cx.add_window_view(|window, cx| GitGpuiView::new(store, events, None, window, cx));

        let repo_id = RepoId(5);
        let workdir = std::env::temp_dir().join(format!(
            "gitgpui_ui_test_{}_status_menu_conflict",
            std::process::id()
        ));
        let path = std::path::PathBuf::from("conflict.txt");

        cx.update(|_window, app| {
            view.update(app, |this, cx| {
                let mut repo = RepoState::new_opening(
                    repo_id,
                    gitgpui_core::domain::RepoSpec {
                        workdir: workdir.clone(),
                    },
                );
                repo.status = Loadable::Ready(
                    gitgpui_core::domain::RepoStatus {
                        staged: vec![],
                        unstaged: vec![gitgpui_core::domain::FileStatus {
                            path: path.clone(),
                            kind: gitgpui_core::domain::FileStatusKind::Conflicted,
                            conflict: None,
                        }],
                    }
                    .into(),
                );
                this.state = Arc::new(AppState {
                    repos: vec![repo],
                    active_repo: Some(repo_id),
                    ..Default::default()
                });
                cx.notify();
            });
        });

        cx.update(|_window, app| {
            let model = view
                .update(app, |this, cx| {
                    this.context_menu_model(
                        &PopoverKind::StatusFileMenu {
                            repo_id,
                            area: DiffArea::Unstaged,
                            path: path.clone(),
                            selection: Vec::new(),
                        },
                        cx,
                    )
                })
                .expect("expected status file context menu model");

            let has_ours = model.items.iter().any(|item| match item {
                ContextMenuItem::Entry { label, action, .. }
                    if label.as_ref() == "Resolve using ours" =>
                {
                    matches!(
                        action,
                        ContextMenuAction::CheckoutConflictSide {
                            repo_id: rid,
                            paths,
                            side: gitgpui_core::services::ConflictSide::Ours
                        } if rid.0 == repo_id.0 && paths.first() == Some(&path)
                    )
                }
                _ => false,
            });
            let has_theirs = model.items.iter().any(|item| match item {
                ContextMenuItem::Entry { label, action, .. }
                    if label.as_ref() == "Resolve using theirs" =>
                {
                    matches!(
                        action,
                        ContextMenuAction::CheckoutConflictSide {
                            repo_id: rid,
                            paths,
                            side: gitgpui_core::services::ConflictSide::Theirs
                        } if rid.0 == repo_id.0 && paths.first() == Some(&path)
                    )
                }
                _ => false,
            });
            let has_manual = model.items.iter().any(|item| match item {
                ContextMenuItem::Entry { label, action, .. }
                    if label.as_ref() == "Resolve manually…" =>
                {
                    matches!(
                        action,
                        ContextMenuAction::SelectDiff {
                            repo_id: rid,
                            target: DiffTarget::WorkingTree { path: p, area: DiffArea::Unstaged }
                        } if rid.0 == repo_id.0 && p.as_path() == path.as_path()
                    )
                }
                _ => false,
            });

            assert!(has_ours);
            assert!(has_theirs);
            assert!(has_manual);
        });
    }

    #[gpui::test]
    fn status_file_menu_open_from_details_pane_does_not_double_lease_panic(
        cx: &mut gpui::TestAppContext,
    ) {
        let (store, events) = AppStore::new(Arc::new(TestBackend));
        let (view, cx) =
            cx.add_window_view(|window, cx| GitGpuiView::new(store, events, None, window, cx));

        let repo_id = RepoId(6);
        let workdir = std::env::temp_dir().join(format!(
            "gitgpui_ui_test_{}_status_menu_reentrant",
            std::process::id()
        ));
        let path = std::path::PathBuf::from("conflict.txt");

        cx.update(|_window, app| {
            view.update(app, |this, cx| {
                let mut repo = RepoState::new_opening(
                    repo_id,
                    gitgpui_core::domain::RepoSpec {
                        workdir: workdir.clone(),
                    },
                );
                repo.status = Loadable::Ready(
                    gitgpui_core::domain::RepoStatus {
                        staged: vec![],
                        unstaged: vec![gitgpui_core::domain::FileStatus {
                            path: path.clone(),
                            kind: gitgpui_core::domain::FileStatusKind::Conflicted,
                            conflict: None,
                        }],
                    }
                    .into(),
                );
                this.state = Arc::new(AppState {
                    repos: vec![repo],
                    active_repo: Some(repo_id),
                    ..Default::default()
                });
                cx.notify();
            });
        });

        cx.update(|window, app| {
            let details_pane = view.read(app).details_pane.clone();
            let anchor = point(px(0.0), px(0.0));
            let _ = details_pane.update(app, |pane, cx| {
                pane.open_popover_at(
                    PopoverKind::StatusFileMenu {
                        repo_id,
                        area: DiffArea::Unstaged,
                        path: path.clone(),
                        selection: Vec::new(),
                    },
                    anchor,
                    window,
                    cx,
                );
            });
        });
    }
}
