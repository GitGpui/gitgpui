use super::*;

mod context_menu;

impl GitGpuiView {
    pub(in super::super) fn close_popover(&mut self, cx: &mut gpui::Context<Self>) {
        self.popover = None;
        self.popover_anchor = None;
        self.context_menu_selected_ix = None;
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
                .context_menu_model(&kind)
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
                    let focus = self.rebase_onto_input.read_with(cx, |i, _| i.focus_handle());
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
                    let focus = self.remote_name_input.read_with(cx, |i, _| i.focus_handle());
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
                    let focus = self.worktree_path_input.read_with(cx, |i, _| i.focus_handle());
                    window.focus(&focus);
                }
                PopoverKind::WorktreeOpenPicker { repo_id }
                | PopoverKind::WorktreeRemovePicker { repo_id } => {
                    let _ = self.ensure_worktree_picker_search_input(window, cx);
                    self.store.dispatch(Msg::LoadWorktrees { repo_id: *repo_id });
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
                    let focus = self.submodule_url_input.read_with(cx, |i, _| i.focus_handle());
                    window.focus(&focus);
                }
                PopoverKind::SubmoduleOpenPicker { repo_id }
                | PopoverKind::SubmoduleRemovePicker { repo_id } => {
                    let _ = self.ensure_submodule_picker_search_input(window, cx);
                    self.store.dispatch(Msg::LoadSubmodules { repo_id: *repo_id });
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
            .w_full()
            .items_center()
            .px_2()
            .py_1()
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
                            .py(px(1.0))
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
                                    .text_color(theme.colors.text_muted),
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
                                if *hovering {
                                    this.tooltip_text = Some(text);
                                } else if this.tooltip_text.as_ref() == Some(&text) {
                                    this.tooltip_text = None;
                                }
                                cx.notify();
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

        let close = cx.listener(|this, _e: &ClickEvent, _w, cx| this.close_popover(cx));

        let panel = match kind {
            PopoverKind::RepoPicker => {
                if let Some(search) = self.repo_picker_search_input.clone() {
                    let repo_ids = self.state.repos.iter().map(|r| r.id).collect::<Vec<_>>();
                    let items = self
                        .state
                        .repos
                        .iter()
                        .map(|r| r.spec.workdir.display().to_string().into())
                        .collect::<Vec<SharedString>>();

                    zed::context_menu(
                        theme,
                        zed::PickerPrompt::new(search)
                            .items(items)
                            .empty_text("No repositories")
                            .max_height(px(260.0))
                            .render(theme, cx, move |this, ix, _e, _w, cx| {
                                if let Some(&repo_id) = repo_ids.get(ix) {
                                    this.store.dispatch(Msg::SetActiveRepo { repo_id });
                                    this.rebuild_diff_cache();
                                }
                                this.popover = None;
                                this.popover_anchor = None;
                                cx.notify();
                            }),
                    )
                    .min_w(px(260.0))
                    .max_w(px(420.0))
                } else {
                    let mut menu = div().flex().flex_col().min_w(px(260.0)).max_w(px(420.0));
                    for repo in self.state.repos.iter() {
                        let id = repo.id;
                        let label: SharedString = repo.spec.workdir.display().to_string().into();
                        menu = menu.child(
                            zed::context_menu_entry(
                                ("repo_item", id.0),
                                theme,
                                false,
                                false,
                                None,
                                label.clone(),
                                None,
                                false,
                            )
                            .on_click(cx.listener(
                                move |this, _e: &ClickEvent, _w, cx| {
                                    this.store.dispatch(Msg::SetActiveRepo { repo_id: id });
                                    this.popover = None;
                                    this.popover_anchor = None;
                                    this.rebuild_diff_cache();
                                    cx.notify();
                                },
                            )),
                        );
                    }
                    zed::context_menu(theme, menu)
                }
            }
            PopoverKind::Settings => {
                let current = self.date_time_format;
                let preview_now = std::time::SystemTime::now();

                let row = |id: &'static str,
                           label: &'static str,
                           value: SharedString,
                           open: bool| {
                    div()
                        .id(id)
                        .px_2()
                        .py_1()
                        .flex()
                        .items_center()
                        .justify_between()
                        .rounded(px(theme.radii.row))
                        .hover(move |s| s.bg(theme.colors.hover))
                        .active(move |s| s.bg(theme.colors.active))
                        .cursor(CursorStyle::PointingHand)
                        .child(div().text_sm().child(label))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .text_sm()
                                .text_color(theme.colors.text_muted)
                                .child(value)
                                .child(div().font_family("monospace").child(if open { "▴" } else { "▾" })),
                        )
                };

                let mut dropdown = div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .px_2()
                    .pb_2();

                if self.settings_date_format_open {
                    for fmt in DateTimeFormat::all() {
                        let selected = *fmt == current;
                        let fmt_val = *fmt;
                        let preview: SharedString =
                            super::super::format_datetime_utc(preview_now, fmt_val).into();
                        dropdown = dropdown.child(
                            div()
                                .id(("settings_date_format_item", *fmt as usize))
                                .px_2()
                                .py_1()
                                .rounded(px(theme.radii.row))
                                .when(!selected, |d| {
                                    d.hover(move |s| s.bg(theme.colors.hover))
                                        .active(move |s| s.bg(theme.colors.active))
                                })
                                .when(selected, |d| d.bg(with_alpha(theme.colors.accent, 0.15)))
                                .cursor(CursorStyle::PointingHand)
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap_2()
                                        .child(div().text_sm().child(fmt.label()))
                                        .child(
                                            div()
                                                .font_family("monospace")
                                                .text_xs()
                                                .text_color(theme.colors.text_muted)
                                                .child(preview),
                                        ),
                                )
                                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                    this.date_time_format = fmt_val;
                                    this.settings_date_format_open = false;
                                    this.schedule_ui_settings_persist(cx);
                                    cx.notify();
                                })),
                        );
                    }
                }

                let header = div()
                    .px_2()
                    .py_1()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child("Settings");

                let section_label = div()
                    .px_2()
                    .pt(px(6.0))
                    .pb(px(4.0))
                    .text_xs()
                    .text_color(theme.colors.text_muted)
                    .child("General");

                let date_row = row(
                    "settings_date_format",
                    "Date format",
                    current.label().into(),
                    self.settings_date_format_open,
                )
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.settings_date_format_open = !this.settings_date_format_open;
                    cx.notify();
                }));

                zed::context_menu(
                    theme,
                    div()
                        .flex()
                        .flex_col()
                        .min_w(px(560.0))
                        .max_w(px(720.0))
                        .child(header)
                        .child(div().border_t_1().border_color(theme.colors.border))
                        .child(section_label)
                        .child(div().px_2().pb_1().child(date_row))
                        .when(self.settings_date_format_open, |d| {
                            d.child(div().px_2().pb_1().text_xs().text_color(theme.colors.text_muted).child("Choose a format:"))
                                .child(dropdown)
                        }),
                )
            }
            PopoverKind::BranchPicker => {
                let mut menu = div().flex().flex_col().min_w(px(240.0)).max_w(px(420.0));

                if let Some(repo) = self.active_repo() {
                    match &repo.branches {
                        Loadable::Ready(branches) => {
                            if let Some(search) = self.branch_picker_search_input.clone() {
                                let repo_id = repo.id;
                                let branch_names =
                                    branches.iter().map(|b| b.name.clone()).collect::<Vec<_>>();
                                let items = branch_names
                                    .iter()
                                    .map(|name| name.clone().into())
                                    .collect::<Vec<SharedString>>();

                                menu = menu.child(
                                    zed::PickerPrompt::new(search)
                                        .items(items)
                                        .empty_text("No branches")
                                        .max_height(px(240.0))
                                        .render(theme, cx, move |this, ix, _e, _w, cx| {
                                            if let Some(name) = branch_names.get(ix).cloned() {
                                                this.store.dispatch(Msg::CheckoutBranch {
                                                    repo_id,
                                                    name,
                                                });
                                            }
                                            this.popover = None;
                                            this.popover_anchor = None;
                                            cx.notify();
                                        }),
                                );
                            } else {
                                for (ix, branch) in branches.iter().enumerate() {
                                    let repo_id = repo.id;
                                    let name = branch.name.clone();
                                    let label: SharedString = name.clone().into();
                                    menu = menu.child(
                                        zed::context_menu_entry(
                                            ("branch_item", ix),
                                            theme,
                                            false,
                                            false,
                                            None,
                                            label,
                                            None,
                                            false,
                                        )
                                        .on_click(
                                            cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                                this.store.dispatch(Msg::CheckoutBranch {
                                                    repo_id,
                                                    name: name.clone(),
                                                });
                                                this.popover = None;
                                                this.popover_anchor = None;
                                                cx.notify();
                                            }),
                                        ),
                                    );
                                }
                            }
                        }
                        Loadable::Loading => {
                            menu = menu.child(zed::context_menu_label(theme, "Loading"));
                        }
                        Loadable::Error(e) => {
                            menu = menu.child(zed::context_menu_label(theme, e.clone()));
                        }
                        Loadable::NotLoaded => {
                            menu = menu.child(zed::context_menu_label(theme, "Not loaded"));
                        }
                    }
                }

                zed::context_menu(theme, menu)
                    .min_w(px(240.0))
                    .max_w(px(420.0))
            }
            PopoverKind::CreateBranch => div()
                .flex()
                .flex_col()
                .min_w(px(260.0))
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .child("Create branch"),
                )
                .child(div().border_t_1().border_color(theme.colors.border))
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .w_full()
                        .min_w(px(0.0))
                        .child(self.create_branch_input.clone()),
                )
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            zed::Button::new("create_branch_cancel", "Cancel")
                                .style(zed::ButtonStyle::Outlined)
                                .on_click(theme, cx, |this, _e, _w, cx| {
                                    this.popover = None;
                                    this.popover_anchor = None;
                                    cx.notify();
                                }),
                        )
                        .child(
                            zed::Button::new("create_branch_go", "Create")
                                .style(zed::ButtonStyle::Filled)
                                .on_click(theme, cx, |this, _e, _w, cx| {
                                    let name = this
                                        .create_branch_input
                                        .read_with(cx, |i, _| i.text().trim().to_string());
                                    if let Some(repo_id) = this.active_repo_id()
                                        && !name.is_empty()
                                    {
                                        this.store.dispatch(Msg::CreateBranchAndCheckout {
                                            repo_id,
                                            name,
                                        });
                                    }
                                    this.popover = None;
                                    this.popover_anchor = None;
                                    cx.notify();
                                }),
                        ),
                ),
            PopoverKind::StashPrompt => div()
                .flex()
                .flex_col()
                .min_w(px(260.0))
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .child("Create stash"),
                )
                .child(div().border_t_1().border_color(theme.colors.border))
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .w_full()
                        .min_w(px(0.0))
                        .child(self.stash_message_input.clone()),
                )
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            zed::Button::new("stash_cancel", "Cancel")
                                .style(zed::ButtonStyle::Outlined)
                                .on_click(theme, cx, |this, _e, _w, cx| {
                                    this.popover = None;
                                    this.popover_anchor = None;
                                    cx.notify();
                                }),
                        )
                        .child(
                            zed::Button::new("stash_go", "Stash")
                                .style(zed::ButtonStyle::Filled)
                                .on_click(theme, cx, |this, _e, _w, cx| {
                                    let message = this
                                        .stash_message_input
                                        .read_with(cx, |i, _| i.text().trim().to_string());
                                    let message = if message.is_empty() {
                                        "WIP".to_string()
                                    } else {
                                        message
                                    };
                                    if let Some(repo_id) = this.active_repo_id() {
                                        this.store.dispatch(Msg::Stash {
                                            repo_id,
                                            message,
                                            include_untracked: true,
                                        });
                                    }
                                    this.popover = None;
                                    this.popover_anchor = None;
                                    cx.notify();
                                }),
                        ),
                ),
	            PopoverKind::CloneRepo => {
	                div()
                    .flex()
                    .flex_col()
                    .min_w(px(420.0))
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .child("Clone repository"),
                    )
                    .child(div().border_t_1().border_color(theme.colors.border))
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .child("Repository URL / Path"),
                    )
                    .child(
                        div()
                            .px_2()
                            .pb_1()
                            .w_full()
                            .min_w(px(0.0))
                            .child(self.clone_repo_url_input.clone()),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .child("Destination parent folder"),
                    )
                    .child(
                        div()
                            .px_2()
                            .pb_1()
                            .w_full()
                            .min_w(px(0.0))
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().flex_1().min_w(px(0.0)).child(
                                self.clone_repo_parent_dir_input.clone(),
                            ))
                            .child(
                                zed::Button::new("clone_repo_browse", "Browse")
                                    .style(zed::ButtonStyle::Outlined)
                                    .on_click(theme, cx, |_this, _e, window, cx| {
                                        cx.stop_propagation();
                                        let view = cx.weak_entity();
                                        let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
                                            files: false,
                                            directories: true,
                                            multiple: false,
                                            prompt: Some("Clone into folder".into()),
                                        });

                                        window
                                            .spawn(cx, async move |cx| {
                                                let result = rx.await;
                                                let paths = match result {
                                                    Ok(Ok(Some(paths))) => paths,
                                                    Ok(Ok(None)) => return,
                                                    Ok(Err(_)) | Err(_) => return,
                                                };
                                                let Some(path) = paths.into_iter().next() else {
                                                    return;
                                                };
                                                let _ = view.update(cx, |this, cx| {
                                                    this.clone_repo_parent_dir_input.update(
                                                        cx,
                                                        |input, cx| {
                                                            input.set_text(
                                                                path.display().to_string(),
                                                                cx,
                                                            );
                                                        },
                                                    );
                                                    cx.notify();
                                                });
                                            })
                                            .detach();
                                    }),
                            ),
                    )
                    .child(div().border_t_1().border_color(theme.colors.border))
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                zed::Button::new("clone_repo_cancel", "Cancel")
                                    .style(zed::ButtonStyle::Outlined)
                                    .on_click(theme, cx, |this, _e, _w, cx| {
                                        this.popover = None;
                                        this.popover_anchor = None;
                                        cx.notify();
                                    }),
                            )
                            .child(
                                zed::Button::new("clone_repo_go", "Clone")
                                    .style(zed::ButtonStyle::Filled)
                                    .on_click(theme, cx, |this, _e, _w, cx| {
                                        let url = this
                                            .clone_repo_url_input
                                            .read_with(cx, |i, _| i.text().trim().to_string());
                                        let parent = this
                                            .clone_repo_parent_dir_input
                                            .read_with(cx, |i, _| i.text().trim().to_string());
                                        if url.is_empty() || parent.is_empty() {
                                            this.push_toast(
                                                zed::ToastKind::Error,
                                                "Clone: URL and destination are required"
                                                    .to_string(),
                                                cx,
                                            );
                                            return;
                                        }

                                        let repo_name = clone_repo_name_from_url(&url);
                                        let dest = std::path::PathBuf::from(parent).join(repo_name);
                                        this.store.dispatch(Msg::CloneRepo { url, dest });
                                        this.popover = None;
                                        this.popover_anchor = None;
                                        cx.notify();
                                    }),
                            ),
	                    )
	            }
		            PopoverKind::ResetPrompt {
		                repo_id,
		                target,
		                mode,
		            } => {
	                let mode_label = match mode {
	                    ResetMode::Soft => "--soft",
	                    ResetMode::Mixed => "--mixed",
	                    ResetMode::Hard => "--hard",
	                };

	                div()
	                    .flex()
	                    .flex_col()
	                    .min_w(px(380.0))
	                    .child(
	                        div()
	                            .px_2()
	                            .py_1()
	                            .text_sm()
	                            .font_weight(FontWeight::BOLD)
	                            .child("Reset"),
	                    )
	                    .child(div().border_t_1().border_color(theme.colors.border))
	                    .child(
	                        div()
	                            .px_2()
	                            .py_1()
	                            .text_sm()
	                            .text_color(theme.colors.text_muted)
	                            .child(format!("{mode_label} → {target}")),
	                    )
	                    .child(
	                        div()
	                            .px_2()
	                            .pb_1()
	                            .text_xs()
	                            .text_color(theme.colors.text_muted)
	                            .child(match mode {
	                                ResetMode::Hard => {
	                                    "Hard reset updates index + working tree (destructive)."
	                                }
	                                ResetMode::Mixed => "Mixed reset updates index only.",
	                                ResetMode::Soft => "Soft reset moves HEAD only.",
	                            }),
	                    )
	                    .child(div().border_t_1().border_color(theme.colors.border))
	                    .child(
	                        div()
	                            .px_2()
	                            .py_1()
	                            .flex()
	                            .items_center()
	                            .justify_between()
	                            .child(
	                                zed::Button::new("reset_cancel", "Cancel")
	                                    .style(zed::ButtonStyle::Outlined)
	                                    .on_click(theme, cx, |this, _e, _w, cx| {
	                                        this.popover = None;
	                                        this.popover_anchor = None;
	                                        cx.notify();
	                                    }),
	                            )
	                            .child(
	                                zed::Button::new("reset_go", "Reset")
	                                    .style(zed::ButtonStyle::Filled)
	                                    .on_click(theme, cx, move |this, _e, _w, cx| {
	                                        this.store.dispatch(Msg::Reset {
	                                            repo_id,
	                                            target: target.clone(),
	                                            mode,
	                                        });
	                                        this.popover = None;
	                                        this.popover_anchor = None;
	                                        cx.notify();
	                                    }),
	                            ),
		                    )
		            }
		            PopoverKind::RebasePrompt { repo_id } => div()
		                .flex()
		                .flex_col()
		                .min_w(px(420.0))
		                .child(
		                    div()
		                        .px_2()
		                        .py_1()
		                        .text_sm()
		                        .font_weight(FontWeight::BOLD)
		                        .child("Rebase"),
		                )
		                .child(div().border_t_1().border_color(theme.colors.border))
		                .child(
		                    div()
		                        .px_2()
		                        .py_1()
		                        .text_xs()
		                        .text_color(theme.colors.text_muted)
		                        .child("Rebase current branch onto"),
		                )
		                .child(
		                    div()
		                        .px_2()
		                        .pb_1()
		                        .w_full()
		                        .min_w(px(0.0))
		                        .child(self.rebase_onto_input.clone()),
		                )
		                .child(div().border_t_1().border_color(theme.colors.border))
		                .child(
		                    div()
		                        .px_2()
		                        .py_1()
		                        .flex()
		                        .items_center()
		                        .justify_between()
		                        .child(
		                            zed::Button::new("rebase_cancel", "Cancel")
		                                .style(zed::ButtonStyle::Outlined)
		                                .on_click(theme, cx, |this, _e, _w, cx| {
		                                    this.popover = None;
		                                    this.popover_anchor = None;
		                                    cx.notify();
		                                }),
		                        )
		                        .child(
		                            zed::Button::new("rebase_go", "Rebase")
		                                .style(zed::ButtonStyle::Filled)
		                                .on_click(theme, cx, move |this, _e, _w, cx| {
		                                    let onto = this
		                                        .rebase_onto_input
		                                        .read_with(cx, |i, _| i.text().trim().to_string());
		                                    if onto.is_empty() {
		                                        this.push_toast(
		                                            zed::ToastKind::Error,
		                                            "Rebase: target is required".to_string(),
		                                            cx,
		                                        );
		                                        return;
		                                    }
		                                    this.store.dispatch(Msg::Rebase { repo_id, onto });
		                                    this.popover = None;
		                                    this.popover_anchor = None;
		                                    cx.notify();
		                                }),
		                        ),
		                ),
		            PopoverKind::CreateTagPrompt { repo_id, target } => div()
		                .flex()
		                .flex_col()
		                .min_w(px(420.0))
		                .child(
		                    div()
		                        .px_2()
		                        .py_1()
		                        .text_sm()
		                        .font_weight(FontWeight::BOLD)
		                        .child("Create tag"),
		                )
		                .child(div().border_t_1().border_color(theme.colors.border))
		                .child(
		                    div()
		                        .px_2()
		                        .py_1()
		                        .text_xs()
		                        .text_color(theme.colors.text_muted)
		                        .child(format!("Target: {target}")),
		                )
		                .child(
		                    div()
		                        .px_2()
		                        .pb_1()
		                        .w_full()
		                        .min_w(px(0.0))
		                        .child(self.create_tag_input.clone()),
		                )
		                .child(div().border_t_1().border_color(theme.colors.border))
		                .child(
		                    div()
		                        .px_2()
		                        .py_1()
		                        .flex()
		                        .items_center()
		                        .justify_between()
		                        .child(
		                            zed::Button::new("create_tag_cancel", "Cancel")
		                                .style(zed::ButtonStyle::Outlined)
		                                .on_click(theme, cx, |this, _e, _w, cx| {
		                                    this.popover = None;
		                                    this.popover_anchor = None;
		                                    cx.notify();
		                                }),
		                        )
		                        .child(
		                            zed::Button::new("create_tag_go", "Create")
		                                .style(zed::ButtonStyle::Filled)
		                                .on_click(theme, cx, move |this, _e, _w, cx| {
		                                    let name = this
		                                        .create_tag_input
		                                        .read_with(cx, |i, _| i.text().trim().to_string());
		                                    if name.is_empty() {
		                                        this.push_toast(
		                                            zed::ToastKind::Error,
		                                            "Tag: name is required".to_string(),
		                                            cx,
		                                        );
		                                        return;
		                                    }
		                                    this.store.dispatch(Msg::CreateTag {
		                                        repo_id,
		                                        name,
		                                        target: target.clone(),
		                                    });
		                                    this.popover = None;
		                                    this.popover_anchor = None;
		                                    cx.notify();
		                                }),
		                        ),
		                ),
		            PopoverKind::RemoteAddPrompt { repo_id } => div()
		                .flex()
		                .flex_col()
		                .min_w(px(420.0))
		                .child(
		                    div()
		                        .px_2()
		                        .py_1()
		                        .text_sm()
		                        .font_weight(FontWeight::BOLD)
		                        .child("Add remote"),
		                )
		                .child(div().border_t_1().border_color(theme.colors.border))
		                .child(
		                    div()
		                        .px_2()
		                        .py_1()
		                        .text_xs()
		                        .text_color(theme.colors.text_muted)
		                        .child("Name"),
		                )
		                .child(div().px_2().pb_1().min_w(px(0.0)).child(
		                    self.remote_name_input.clone(),
		                ))
		                .child(
		                    div()
		                        .px_2()
		                        .py_1()
		                        .text_xs()
		                        .text_color(theme.colors.text_muted)
		                        .child("URL"),
		                )
		                .child(div().px_2().pb_1().min_w(px(0.0)).child(
		                    self.remote_url_input.clone(),
		                ))
		                .child(div().border_t_1().border_color(theme.colors.border))
		                .child(
		                    div()
		                        .px_2()
		                        .py_1()
		                        .flex()
		                        .items_center()
		                        .justify_between()
		                        .child(
		                            zed::Button::new("add_remote_cancel", "Cancel")
		                                .style(zed::ButtonStyle::Outlined)
		                                .on_click(theme, cx, |this, _e, _w, cx| {
		                                    this.popover = None;
		                                    this.popover_anchor = None;
		                                    cx.notify();
		                                }),
		                        )
		                        .child(
		                            zed::Button::new("add_remote_go", "Add")
		                                .style(zed::ButtonStyle::Filled)
		                                .on_click(theme, cx, move |this, _e, _w, cx| {
		                                    let name = this
		                                        .remote_name_input
		                                        .read_with(cx, |i, _| i.text().trim().to_string());
		                                    let url = this
		                                        .remote_url_input
		                                        .read_with(cx, |i, _| i.text().trim().to_string());
		                                    if name.is_empty() || url.is_empty() {
		                                        this.push_toast(
		                                            zed::ToastKind::Error,
		                                            "Remote: name and URL are required".to_string(),
		                                            cx,
		                                        );
		                                        return;
		                                    }
		                                    this.store.dispatch(Msg::AddRemote { repo_id, name, url });
		                                    this.popover = None;
		                                    this.popover_anchor = None;
		                                    cx.notify();
		                                }),
		                        ),
		                ),
		            PopoverKind::RemoteUrlPicker { repo_id, kind } => {
		                let remotes = self
		                    .active_repo()
		                    .and_then(|r| match &r.remotes {
		                        Loadable::Ready(remotes) => Some(remotes.clone()),
		                        _ => None,
		                    })
		                    .unwrap_or_default();
		                let items = remotes
		                    .iter()
		                    .map(|r| r.name.clone().into())
		                    .collect::<Vec<_>>();
		                let names = remotes.iter().map(|r| r.name.clone()).collect::<Vec<_>>();
		                if let Some(search) = self.remote_picker_search_input.clone() {
		                    zed::context_menu(
		                        theme,
		                        zed::PickerPrompt::new(search)
		                            .items(items)
		                            .empty_text("No remotes")
		                            .max_height(px(260.0))
		                            .render(theme, cx, move |this, ix, e, window, cx| {
		                                let Some(name) = names.get(ix).cloned() else {
		                                    return;
		                                };
		                                let url = this
		                                    .active_repo()
		                                    .and_then(|r| match &r.remotes {
		                                        Loadable::Ready(remotes) => remotes
		                                            .iter()
		                                            .find(|rr| rr.name == name)
		                                            .and_then(|rr| rr.url.clone()),
		                                        _ => None,
		                                    })
		                                    .unwrap_or_default();
		                                this.remote_url_edit_input
		                                    .update(cx, |i, cx| i.set_text(url, cx));
		                                this.open_popover_at(
		                                    PopoverKind::RemoteEditUrlPrompt { repo_id, name, kind },
		                                    e.position(),
		                                    window,
		                                    cx,
		                                );
		                            }),
		                    )
		                    .min_w(px(260.0))
		                    .max_w(px(420.0))
		                } else {
		                    let mut menu = div().flex().flex_col().min_w(px(260.0)).max_w(px(420.0));
		                    for (ix, item) in items.into_iter().enumerate() {
		                        let name = names.get(ix).cloned().unwrap_or_default();
		                        menu = menu.child(
		                            div()
		                                .id(("remote_url_item", ix))
		                                .px_2()
		                                .py_1()
		                                .hover(move |s| s.bg(theme.colors.hover))
		                                .child(div().text_sm().line_clamp(1).child(item))
		                                .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
		                                    let url = this
		                                        .active_repo()
		                                        .and_then(|r| match &r.remotes {
		                                            Loadable::Ready(remotes) => remotes
		                                                .iter()
		                                                .find(|rr| rr.name == name)
		                                                .and_then(|rr| rr.url.clone()),
		                                            _ => None,
		                                        })
		                                        .unwrap_or_default();
		                                    this.remote_url_edit_input
		                                        .update(cx, |i, cx| i.set_text(url, cx));
		                                    this.open_popover_at(
		                                        PopoverKind::RemoteEditUrlPrompt {
		                                            repo_id,
		                                            name: name.clone(),
		                                            kind,
		                                        },
		                                        e.position(),
		                                        window,
		                                        cx,
		                                    );
		                                })),
		                        );
		                    }
		                    menu.child(
		                        div()
		                            .id("remote_url_close")
		                            .px_2()
		                            .py_1()
		                            .hover(move |s| s.bg(theme.colors.hover))
		                            .child("Close")
		                            .on_click(close),
		                    )
		                }
		            }
		            PopoverKind::RemoteEditUrlPrompt {
		                repo_id,
		                name,
		                kind,
		            } => {
		                let kind_label = match kind {
		                    RemoteUrlKind::Fetch => "fetch",
		                    RemoteUrlKind::Push => "push",
		                };
		                div()
		                    .flex()
		                    .flex_col()
		                    .min_w(px(420.0))
		                    .child(
		                        div()
		                            .px_2()
		                            .py_1()
		                            .text_sm()
		                            .font_weight(FontWeight::BOLD)
		                            .child(format!("Edit remote URL ({kind_label})")),
		                    )
		                    .child(div().border_t_1().border_color(theme.colors.border))
		                    .child(
		                        div()
		                            .px_2()
		                            .py_1()
		                            .text_xs()
		                            .text_color(theme.colors.text_muted)
		                            .child(format!("Remote: {name}")),
		                    )
		                    .child(div().px_2().pb_1().min_w(px(0.0)).child(
		                        self.remote_url_edit_input.clone(),
		                    ))
		                    .child(div().border_t_1().border_color(theme.colors.border))
		                    .child(
		                        div()
		                            .px_2()
		                            .py_1()
		                            .flex()
		                            .items_center()
		                            .justify_between()
		                            .child(
		                                zed::Button::new("edit_remote_url_cancel", "Cancel")
		                                    .style(zed::ButtonStyle::Outlined)
		                                    .on_click(theme, cx, |this, _e, _w, cx| {
		                                        this.popover = None;
		                                        this.popover_anchor = None;
		                                        cx.notify();
		                                    }),
		                            )
		                            .child(
		                                zed::Button::new("edit_remote_url_go", "Save")
		                                    .style(zed::ButtonStyle::Filled)
		                                    .on_click(theme, cx, move |this, _e, _w, cx| {
		                                        let url = this
		                                            .remote_url_edit_input
		                                            .read_with(cx, |i, _| i.text().trim().to_string());
		                                        if url.is_empty() {
		                                            this.push_toast(
		                                                zed::ToastKind::Error,
		                                                "Remote URL cannot be empty".to_string(),
		                                                cx,
		                                            );
		                                            return;
		                                        }
		                                        this.store.dispatch(Msg::SetRemoteUrl {
		                                            repo_id,
		                                            name: name.clone(),
		                                            url,
		                                            kind,
		                                        });
		                                        this.popover = None;
		                                        this.popover_anchor = None;
		                                        cx.notify();
		                                    }),
		                            ),
		                    )
		            }
		            PopoverKind::RemoteRemovePicker { repo_id } => {
		                let remotes = self
		                    .active_repo()
		                    .and_then(|r| match &r.remotes {
		                        Loadable::Ready(remotes) => Some(remotes.clone()),
		                        _ => None,
		                    })
		                    .unwrap_or_default();
		                let items = remotes
		                    .iter()
		                    .map(|r| r.name.clone().into())
		                    .collect::<Vec<_>>();
		                let names = remotes.iter().map(|r| r.name.clone()).collect::<Vec<_>>();
		                if let Some(search) = self.remote_picker_search_input.clone() {
		                    zed::context_menu(
		                        theme,
		                        zed::PickerPrompt::new(search)
		                            .items(items)
		                            .empty_text("No remotes")
		                            .max_height(px(260.0))
		                            .render(theme, cx, move |this, ix, e, window, cx| {
		                                let Some(name) = names.get(ix).cloned() else {
		                                    return;
		                                };
		                                this.open_popover_at(
		                                    PopoverKind::RemoteRemoveConfirm { repo_id, name },
		                                    e.position(),
		                                    window,
		                                    cx,
		                                );
		                            }),
		                    )
		                    .min_w(px(260.0))
		                    .max_w(px(420.0))
		                } else {
		                    let mut menu = div().flex().flex_col().min_w(px(260.0)).max_w(px(420.0));
		                    for (ix, item) in items.into_iter().enumerate() {
		                        let name = names.get(ix).cloned().unwrap_or_default();
		                        menu = menu.child(
		                            div()
		                                .id(("remote_remove_item", ix))
		                                .px_2()
		                                .py_1()
		                                .hover(move |s| s.bg(theme.colors.hover))
		                                .child(div().text_sm().line_clamp(1).child(item))
		                                .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
		                                    this.open_popover_at(
		                                        PopoverKind::RemoteRemoveConfirm {
		                                            repo_id,
		                                            name: name.clone(),
		                                        },
		                                        e.position(),
		                                        window,
		                                        cx,
		                                    );
		                                })),
		                        );
		                    }
		                    menu.child(
		                        div()
		                            .id("remote_remove_close")
		                            .px_2()
		                            .py_1()
		                            .hover(move |s| s.bg(theme.colors.hover))
		                            .child("Close")
		                            .on_click(close),
		                    )
		                }
		            }
		            PopoverKind::RemoteRemoveConfirm { repo_id, name } => div()
		                .flex()
		                .flex_col()
		                .min_w(px(380.0))
		                .child(
		                    div()
		                        .px_2()
		                        .py_1()
		                        .text_sm()
		                        .font_weight(FontWeight::BOLD)
		                        .child("Remove remote"),
		                )
		                .child(div().border_t_1().border_color(theme.colors.border))
		                .child(
		                    div()
		                        .px_2()
		                        .py_1()
		                        .text_sm()
		                        .text_color(theme.colors.text_muted)
		                        .child(format!("Remote: {name}")),
		                )
		                .child(div().border_t_1().border_color(theme.colors.border))
		                .child(
		                    div()
		                        .px_2()
		                        .py_1()
		                        .flex()
		                        .items_center()
		                        .justify_between()
		                        .child(
		                            zed::Button::new("remove_remote_cancel", "Cancel")
		                                .style(zed::ButtonStyle::Outlined)
		                                .on_click(theme, cx, |this, _e, _w, cx| {
		                                    this.popover = None;
		                                    this.popover_anchor = None;
		                                    cx.notify();
		                                }),
		                        )
		                        .child(
		                            zed::Button::new("remove_remote_go", "Remove")
		                                .style(zed::ButtonStyle::Danger)
		                                .on_click(theme, cx, move |this, _e, _w, cx| {
		                                    this.store.dispatch(Msg::RemoveRemote {
		                                        repo_id,
		                                        name: name.clone(),
		                                    });
		                                    this.popover = None;
		                                    this.popover_anchor = None;
		                                    cx.notify();
		                                }),
		                        ),
		                ),
                    PopoverKind::WorktreeAddPrompt { repo_id } => div()
                        .flex()
                        .flex_col()
                        .min_w(px(520.0))
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .child("Add worktree"),
                        )
                        .child(div().border_t_1().border_color(theme.colors.border))
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .text_xs()
                                .text_color(theme.colors.text_muted)
                                .child("Worktree folder"),
                        )
                        .child(
                            div()
                                .px_2()
                                .pb_1()
                                .w_full()
                                .min_w(px(0.0))
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().flex_1().min_w(px(0.0)).child(
                                    self.worktree_path_input.clone(),
                                ))
                                .child(
                                    zed::Button::new("worktree_browse", "Browse")
                                        .style(zed::ButtonStyle::Outlined)
                                        .on_click(theme, cx, |_this, _e, window, cx| {
                                            cx.stop_propagation();
                                            let view = cx.weak_entity();
                                            let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
                                                files: false,
                                                directories: true,
                                                multiple: false,
                                                prompt: Some("Select worktree folder".into()),
                                            });

                                            window
                                                .spawn(cx, async move |cx| {
                                                    let result = rx.await;
                                                    let paths = match result {
                                                        Ok(Ok(Some(paths))) => paths,
                                                        Ok(Ok(None)) => return,
                                                        Ok(Err(_)) | Err(_) => return,
                                                    };
                                                    let Some(path) = paths.into_iter().next()
                                                    else {
                                                        return;
                                                    };
                                                    let _ = view.update(cx, |this, cx| {
                                                        this.worktree_path_input.update(
                                                            cx,
                                                            |input, cx| {
                                                                input.set_text(
                                                                    path.display().to_string(),
                                                                    cx,
                                                                );
                                                            },
                                                        );
                                                        cx.notify();
                                                    });
                                                })
                                                .detach();
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .text_xs()
                                .text_color(theme.colors.text_muted)
                                .child("Branch / commit (optional)"),
                        )
                        .child(
                            div()
                                .px_2()
                                .pb_1()
                                .w_full()
                                .min_w(px(0.0))
                                .child(self.worktree_ref_input.clone()),
                        )
                        .child(div().border_t_1().border_color(theme.colors.border))
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    zed::Button::new("worktree_add_cancel", "Cancel")
                                        .style(zed::ButtonStyle::Outlined)
                                        .on_click(theme, cx, |this, _e, _w, cx| {
                                            this.popover = None;
                                            this.popover_anchor = None;
                                            cx.notify();
                                        }),
                                )
                                .child(
                                    zed::Button::new("worktree_add_go", "Add")
                                        .style(zed::ButtonStyle::Filled)
                                        .on_click(theme, cx, move |this, _e, _w, cx| {
                                            let folder = this.worktree_path_input.read_with(
                                                cx,
                                                |i, _| i.text().trim().to_string(),
                                            );
                                            if folder.is_empty() {
                                                this.push_toast(
                                                    zed::ToastKind::Error,
                                                    "Worktree folder is required".to_string(),
                                                    cx,
                                                );
                                                return;
                                            }
                                            let reference = this.worktree_ref_input.read_with(
                                                cx,
                                                |i, _| i.text().trim().to_string(),
                                            );
                                            let reference = (!reference.is_empty())
                                                .then_some(reference);
                                            this.store.dispatch(Msg::AddWorktree {
                                                repo_id,
                                                path: std::path::PathBuf::from(folder),
                                                reference,
                                            });
                                            this.popover = None;
                                            this.popover_anchor = None;
                                            cx.notify();
                                        }),
                                ),
                        ),
                    PopoverKind::WorktreeOpenPicker { repo_id } => {
                        if let Some(repo) = self.state.repos.iter().find(|r| r.id == repo_id) {
                            match &repo.worktrees {
                                Loadable::Loading => zed::context_menu_label(theme, "Loading"),
                                Loadable::NotLoaded => zed::context_menu_label(theme, "Not loaded"),
                                Loadable::Error(e) => zed::context_menu_label(theme, e.clone()),
                                Loadable::Ready(worktrees) => {
                                    let workdir = repo.spec.workdir.clone();
                                    let items = worktrees
                                        .iter()
                                        .filter(|w| w.path != workdir)
                                        .map(|w| {
                                            let label = if let Some(branch) = &w.branch {
                                                format!("{branch}  {}", w.path.display())
                                            } else if w.detached {
                                                format!("(detached)  {}", w.path.display())
                                            } else {
                                                w.path.display().to_string()
                                            };
                                            label.into()
                                        })
                                        .collect::<Vec<SharedString>>();
                                    let paths = worktrees
                                        .iter()
                                        .filter(|w| w.path != workdir)
                                        .map(|w| w.path.clone())
                                        .collect::<Vec<_>>();

                                    if let Some(search) = self.worktree_picker_search_input.clone() {
                                        zed::context_menu(
                                            theme,
                                            zed::PickerPrompt::new(search)
                                                .items(items)
                                                .empty_text("No worktrees")
                                                .max_height(px(260.0))
                                                .render(theme, cx, move |this, ix, _e, _w, cx| {
                                                    let Some(path) = paths.get(ix).cloned() else {
                                                        return;
                                                    };
                                                    this.store.dispatch(Msg::OpenRepo(path));
                                                    this.popover = None;
                                                    this.popover_anchor = None;
                                                    cx.notify();
                                                }),
                                        )
                                        .min_w(px(420.0))
                                        .max_w(px(820.0))
                                    } else {
                                        zed::context_menu_label(
                                            theme,
                                            "Search input not initialized",
                                        )
                                    }
                                }
                            }
                        } else {
                            zed::context_menu_label(theme, "No repository")
                        }
                    }
                    PopoverKind::WorktreeRemovePicker { repo_id } => {
                        if let Some(repo) = self.state.repos.iter().find(|r| r.id == repo_id) {
                            match &repo.worktrees {
                                Loadable::Loading => zed::context_menu_label(theme, "Loading"),
                                Loadable::NotLoaded => zed::context_menu_label(theme, "Not loaded"),
                                Loadable::Error(e) => zed::context_menu_label(theme, e.clone()),
                                Loadable::Ready(worktrees) => {
                                    let workdir = repo.spec.workdir.clone();
                                    let items = worktrees
                                        .iter()
                                        .filter(|w| w.path != workdir)
                                        .map(|w| w.path.display().to_string().into())
                                        .collect::<Vec<SharedString>>();
                                    let paths = worktrees
                                        .iter()
                                        .filter(|w| w.path != workdir)
                                        .map(|w| w.path.clone())
                                        .collect::<Vec<_>>();

                                    if let Some(search) = self.worktree_picker_search_input.clone() {
                                        zed::context_menu(
                                            theme,
                                            zed::PickerPrompt::new(search)
                                                .items(items)
                                                .empty_text("No worktrees")
                                                .max_height(px(260.0))
                                                .render(
                                                    theme,
                                                    cx,
                                                    move |this, ix, e, window, cx| {
                                                        let Some(path) =
                                                            paths.get(ix).cloned() else {
                                                                return;
                                                            };
                                                        this.open_popover_at(
                                                            PopoverKind::WorktreeRemoveConfirm {
                                                                repo_id,
                                                                path,
                                                            },
                                                            e.position(),
                                                            window,
                                                            cx,
                                                        );
                                                    },
                                                ),
                                        )
                                        .min_w(px(420.0))
                                        .max_w(px(820.0))
                                    } else {
                                        zed::context_menu_label(
                                            theme,
                                            "Search input not initialized",
                                        )
                                    }
                                }
                            }
                        } else {
                            zed::context_menu_label(theme, "No repository")
                        }
                    }
                    PopoverKind::WorktreeRemoveConfirm { repo_id, path } => div()
                        .flex()
                        .flex_col()
                        .min_w(px(420.0))
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .child("Remove worktree"),
                        )
                        .child(div().border_t_1().border_color(theme.colors.border))
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .text_sm()
                                .text_color(theme.colors.text_muted)
                                .child(path.display().to_string()),
                        )
                        .child(div().border_t_1().border_color(theme.colors.border))
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    zed::Button::new("worktree_remove_cancel", "Cancel")
                                        .style(zed::ButtonStyle::Outlined)
                                        .on_click(theme, cx, |this, _e, _w, cx| {
                                            this.popover = None;
                                            this.popover_anchor = None;
                                            cx.notify();
                                        }),
                                )
                                .child(
                                    zed::Button::new("worktree_remove_go", "Remove")
                                        .style(zed::ButtonStyle::Danger)
                                        .on_click(theme, cx, move |this, _e, _w, cx| {
                                            this.store.dispatch(Msg::RemoveWorktree { repo_id, path: path.clone() });
                                            this.popover = None;
                                            this.popover_anchor = None;
                                            cx.notify();
                                        }),
                                ),
                        ),
                    PopoverKind::SubmoduleAddPrompt { repo_id } => div()
                        .flex()
                        .flex_col()
                        .min_w(px(520.0))
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .child("Add submodule"),
                        )
                        .child(div().border_t_1().border_color(theme.colors.border))
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .text_xs()
                                .text_color(theme.colors.text_muted)
                                .child("URL"),
                        )
                        .child(
                            div()
                                .px_2()
                                .pb_1()
                                .w_full()
                                .min_w(px(0.0))
                                .child(self.submodule_url_input.clone()),
                        )
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .text_xs()
                                .text_color(theme.colors.text_muted)
                                .child("Path (relative)"),
                        )
                        .child(
                            div()
                                .px_2()
                                .pb_1()
                                .w_full()
                                .min_w(px(0.0))
                                .child(self.submodule_path_input.clone()),
                        )
                        .child(div().border_t_1().border_color(theme.colors.border))
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    zed::Button::new("submodule_add_cancel", "Cancel")
                                        .style(zed::ButtonStyle::Outlined)
                                        .on_click(theme, cx, |this, _e, _w, cx| {
                                            this.popover = None;
                                            this.popover_anchor = None;
                                            cx.notify();
                                        }),
                                )
                                .child(
                                    zed::Button::new("submodule_add_go", "Add")
                                        .style(zed::ButtonStyle::Filled)
                                        .on_click(theme, cx, move |this, _e, _w, cx| {
                                            let url = this.submodule_url_input.read_with(
                                                cx,
                                                |i, _| i.text().trim().to_string(),
                                            );
                                            let path_text = this.submodule_path_input.read_with(
                                                cx,
                                                |i, _| i.text().trim().to_string(),
                                            );
                                            if url.is_empty() || path_text.is_empty() {
                                                this.push_toast(
                                                    zed::ToastKind::Error,
                                                    "Submodule URL and path are required".to_string(),
                                                    cx,
                                                );
                                                return;
                                            }
                                            this.store.dispatch(Msg::AddSubmodule {
                                                repo_id,
                                                url,
                                                path: std::path::PathBuf::from(path_text),
                                            });
                                            this.popover = None;
                                            this.popover_anchor = None;
                                            cx.notify();
                                        }),
                                ),
                        ),
                    PopoverKind::SubmoduleOpenPicker { repo_id } => {
                        if let Some(repo) = self.state.repos.iter().find(|r| r.id == repo_id) {
                            match &repo.submodules {
                                Loadable::Loading => zed::context_menu_label(theme, "Loading"),
                                Loadable::NotLoaded => zed::context_menu_label(theme, "Not loaded"),
                                Loadable::Error(e) => zed::context_menu_label(theme, e.clone()),
                                Loadable::Ready(subs) => {
                                    let base = repo.spec.workdir.clone();
                                    let items = subs
                                        .iter()
                                        .map(|s| s.path.display().to_string().into())
                                        .collect::<Vec<SharedString>>();
                                    let paths = subs
                                        .iter()
                                        .map(|s| base.join(&s.path))
                                        .collect::<Vec<_>>();
                                    if let Some(search) = self.submodule_picker_search_input.clone()
                                    {
                                        zed::context_menu(
                                            theme,
                                            zed::PickerPrompt::new(search)
                                                .items(items)
                                                .empty_text("No submodules")
                                                .max_height(px(260.0))
                                                .render(theme, cx, move |this, ix, _e, _w, cx| {
                                                    let Some(path) =
                                                        paths.get(ix).cloned() else {
                                                            return;
                                                        };
                                                    this.store.dispatch(Msg::OpenRepo(path));
                                                    this.popover = None;
                                                    this.popover_anchor = None;
                                                    cx.notify();
                                                }),
                                        )
                                        .min_w(px(420.0))
                                        .max_w(px(820.0))
                                    } else {
                                        zed::context_menu_label(
                                            theme,
                                            "Search input not initialized",
                                        )
                                    }
                                }
                            }
                        } else {
                            zed::context_menu_label(theme, "No repository")
                        }
                    }
                    PopoverKind::SubmoduleRemovePicker { repo_id } => {
                        if let Some(repo) = self.state.repos.iter().find(|r| r.id == repo_id) {
                            match &repo.submodules {
                                Loadable::Loading => zed::context_menu_label(theme, "Loading"),
                                Loadable::NotLoaded => zed::context_menu_label(theme, "Not loaded"),
                                Loadable::Error(e) => zed::context_menu_label(theme, e.clone()),
                                Loadable::Ready(subs) => {
                                    let items = subs
                                        .iter()
                                        .map(|s| s.path.display().to_string().into())
                                        .collect::<Vec<SharedString>>();
                                    let paths =
                                        subs.iter().map(|s| s.path.clone()).collect::<Vec<_>>();
                                    if let Some(search) = self.submodule_picker_search_input.clone()
                                    {
                                        zed::context_menu(
                                            theme,
                                            zed::PickerPrompt::new(search)
                                                .items(items)
                                                .empty_text("No submodules")
                                                .max_height(px(260.0))
                                                .render(
                                                    theme,
                                                    cx,
                                                    move |this, ix, e, window, cx| {
                                                        let Some(path) =
                                                            paths.get(ix).cloned() else {
                                                                return;
                                                            };
                                                        this.open_popover_at(
                                                            PopoverKind::SubmoduleRemoveConfirm {
                                                                repo_id,
                                                                path,
                                                            },
                                                            e.position(),
                                                            window,
                                                            cx,
                                                        );
                                                    },
                                                ),
                                        )
                                        .min_w(px(420.0))
                                        .max_w(px(820.0))
                                    } else {
                                        zed::context_menu_label(
                                            theme,
                                            "Search input not initialized",
                                        )
                                    }
                                }
                            }
                        } else {
                            zed::context_menu_label(theme, "No repository")
                        }
                    }
                    PopoverKind::SubmoduleRemoveConfirm { repo_id, path } => div()
                        .flex()
                        .flex_col()
                        .min_w(px(420.0))
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .child("Remove submodule"),
                        )
                        .child(div().border_t_1().border_color(theme.colors.border))
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .text_sm()
                                .text_color(theme.colors.text_muted)
                                .child(path.display().to_string()),
                        )
                        .child(div().border_t_1().border_color(theme.colors.border))
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    zed::Button::new("submodule_remove_cancel", "Cancel")
                                        .style(zed::ButtonStyle::Outlined)
                                        .on_click(theme, cx, |this, _e, _w, cx| {
                                            this.popover = None;
                                            this.popover_anchor = None;
                                            cx.notify();
                                        }),
                                )
                                .child(
                                    zed::Button::new("submodule_remove_go", "Remove")
                                        .style(zed::ButtonStyle::Danger)
                                        .on_click(theme, cx, move |this, _e, _w, cx| {
                                            this.store.dispatch(Msg::RemoveSubmodule { repo_id, path: path.clone() });
                                            this.popover = None;
                                            this.popover_anchor = None;
                                            cx.notify();
                                        }),
                                ),
                        ),
                    PopoverKind::FileHistory { repo_id, path } => {
                        let repo = self.state.repos.iter().find(|r| r.id == repo_id);
                        let title: SharedString = path.display().to_string().into();

                        let header = div()
                            .px_2()
                            .py_1()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .min_w(px(0.0))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .child("File history"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.colors.text_muted)
                                            .line_clamp(1)
                                            .whitespace_nowrap()
                                            .child(title),
                                    ),
                            )
                            .child(
                                zed::Button::new("file_history_close", "Close")
                                    .style(zed::ButtonStyle::Outlined)
                                    .on_click(theme, cx, |this, _e, _w, cx| {
                                        this.popover = None;
                                        this.popover_anchor = None;
                                        cx.notify();
                                    }),
                            );

                        let body: AnyElement = match repo.map(|r| &r.file_history) {
                            None => zed::context_menu_label(theme, "No repository")
                                .into_any_element(),
                            Some(Loadable::Loading) => {
                                zed::context_menu_label(theme, "Loading").into_any_element()
                            }
                            Some(Loadable::Error(e)) => {
                                zed::context_menu_label(theme, e.clone()).into_any_element()
                            }
                            Some(Loadable::NotLoaded) => {
                                zed::context_menu_label(theme, "Not loaded").into_any_element()
                            }
                            Some(Loadable::Ready(page)) => {
                                let commit_ids = page
                                    .commits
                                    .iter()
                                    .map(|c| c.id.clone())
                                    .collect::<Vec<_>>();
                                let items = page
                                    .commits
                                    .iter()
                                    .map(|c| {
                                        let sha = c.id.as_ref();
                                        let short = sha.get(0..8).unwrap_or(sha);
                                        format!("{short}  {}", c.summary).into()
                                    })
                                    .collect::<Vec<SharedString>>();

                                if let Some(search) = self.file_history_search_input.clone() {
                                    zed::PickerPrompt::new(search)
                                        .items(items)
                                        .empty_text("No commits")
                                        .max_height(px(340.0))
                                        .render(theme, cx, move |this, ix, _e, _w, cx| {
                                            let Some(commit_id) =
                                                commit_ids.get(ix).cloned() else {
                                                    return;
                                                };
                                            this.store.dispatch(Msg::SelectCommit {
                                                repo_id,
                                                commit_id: commit_id.clone(),
                                            });
                                            this.store.dispatch(Msg::SelectDiff {
                                                repo_id,
                                                target: DiffTarget::Commit {
                                                    commit_id,
                                                    path: Some(path.clone()),
                                                },
                                            });
                                            this.rebuild_diff_cache();
                                            this.popover = None;
                                            this.popover_anchor = None;
                                            cx.notify();
                                        })
                                        .into_any_element()
                                } else {
                                    zed::context_menu_label(
                                        theme,
                                        "Search input not initialized",
                                    )
                                    .into_any_element()
                                }
                            }
                        };

                        zed::context_menu(
                            theme,
                            div()
                                .flex()
                                .flex_col()
                                .min_w(px(520.0))
                                .max_w(px(820.0))
                                .child(header)
                                .child(div().border_t_1().border_color(theme.colors.border))
                                .child(body),
                        )
                    }
                    PopoverKind::Blame { repo_id, path, rev } => {
                        let repo = self.state.repos.iter().find(|r| r.id == repo_id);
                        let title: SharedString = path.display().to_string().into();
                        let subtitle: SharedString = rev
                            .clone()
                            .map(|r| format!("rev: {r}").into())
                            .unwrap_or_else(|| "rev: HEAD".into());

                        let header = div()
                            .px_2()
                            .py_1()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .min_w(px(0.0))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .child("Blame"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.colors.text_muted)
                                            .line_clamp(1)
                                            .whitespace_nowrap()
                                            .child(title),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.colors.text_muted)
                                            .line_clamp(1)
                                            .whitespace_nowrap()
                                            .child(subtitle),
                                    ),
                            )
                            .child(
                                zed::Button::new("blame_close", "Close")
                                    .style(zed::ButtonStyle::Outlined)
                                    .on_click(theme, cx, |this, _e, _w, cx| {
                                        this.popover = None;
                                        this.popover_anchor = None;
                                        cx.notify();
                                    }),
                            );

                        let body: AnyElement = match repo.map(|r| &r.blame) {
                            None => zed::context_menu_label(theme, "No repository")
                                .into_any_element(),
                            Some(Loadable::Loading) => {
                                zed::context_menu_label(theme, "Loading").into_any_element()
                            }
                            Some(Loadable::Error(e)) => {
                                zed::context_menu_label(theme, e.clone()).into_any_element()
                            }
                            Some(Loadable::NotLoaded) => {
                                zed::context_menu_label(theme, "Not loaded").into_any_element()
                            }
                            Some(Loadable::Ready(lines)) => {
                                let count = lines.len();
                                let list = uniform_list(
                                    "blame_popover",
                                    count,
                                    cx.processor(Self::render_blame_popover_rows),
                                )
                                .h(px(360.0))
                                .track_scroll(self.blame_scroll.clone());
                                let scroll_handle = {
                                    let state = self.blame_scroll.0.borrow();
                                    state.base_handle.clone()
                                };

                                div()
                                    .relative()
                                    .child(list)
                                    .child(
                                        zed::Scrollbar::new("blame_popover_scrollbar", scroll_handle)
                                            .render(theme),
                                    )
                                    .into_any_element()
                            }
                        };

                        div()
                            .flex()
                            .flex_col()
                            .min_w(px(720.0))
                            .max_w(px(980.0))
                            .child(header)
                            .child(div().border_t_1().border_color(theme.colors.border))
                            .child(body)
                    }
		            PopoverKind::PushSetUpstreamPrompt { repo_id, remote } => {
		                let remote = remote.clone();
		                div()
                    .flex()
                    .flex_col()
                    .min_w(px(320.0))
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .child("Set upstream and push"),
                    )
                    .child(div().border_t_1().border_color(theme.colors.border))
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .text_sm()
                            .text_color(theme.colors.text_muted)
                            .child(format!("Remote: {remote}")),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .w_full()
                            .min_w(px(0.0))
                            .child(self.push_upstream_branch_input.clone()),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                zed::Button::new("push_upstream_cancel", "Cancel")
                                    .style(zed::ButtonStyle::Outlined)
                                    .on_click(theme, cx, |this, _e, _w, cx| {
                                        this.popover = None;
                                        this.popover_anchor = None;
                                        cx.notify();
                                    }),
                            )
                            .child(
                                zed::Button::new("push_upstream_go", "Push")
                                    .style(zed::ButtonStyle::Filled)
                                    .on_click(theme, cx, move |this, _e, _w, cx| {
                                        let branch = this
                                            .push_upstream_branch_input
                                            .read_with(cx, |i, _| i.text().trim().to_string());
                                        if branch.is_empty() {
                                            return;
                                        }
                                        this.store.dispatch(Msg::PushSetUpstream {
                                            repo_id,
                                            remote: remote.clone(),
                                            branch,
                                        });
                                        this.popover = None;
                                        this.popover_anchor = None;
                                        cx.notify();
                                    }),
                            ),
                    )
            }
            PopoverKind::ForcePushConfirm { repo_id } => div()
                .flex()
                .flex_col()
                .min_w(px(420.0))
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .child("Force push"),
                )
                .child(div().border_t_1().border_color(theme.colors.border))
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .text_sm()
                        .text_color(theme.colors.text_muted)
                        .child("This will overwrite remote history if your branch has diverged."),
                )
                .child(
                    div()
                        .px_2()
                        .pb_1()
                        .text_xs()
                        .font_family("monospace")
                        .text_color(theme.colors.text_muted)
                        .child("git push --force-with-lease"),
                )
                .child(div().border_t_1().border_color(theme.colors.border))
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            zed::Button::new("force_push_cancel", "Cancel")
                                .style(zed::ButtonStyle::Outlined)
                                .on_click(theme, cx, |this, _e, _w, cx| {
                                    this.popover = None;
                                    this.popover_anchor = None;
                                    cx.notify();
                                }),
                        )
                        .child(
                            zed::Button::new("force_push_go", "Force push")
                                .style(zed::ButtonStyle::Danger)
                                .on_click(theme, cx, move |this, _e, _w, cx| {
                                    this.store.dispatch(Msg::ForcePush { repo_id });
                                    this.popover = None;
                                    this.popover_anchor = None;
                                    cx.notify();
                                }),
                        ),
                ),
            PopoverKind::HistoryBranchFilter { repo_id } => self
                .context_menu_view(PopoverKind::HistoryBranchFilter { repo_id }, cx)
                .min_w(px(160.0))
                .max_w(px(220.0)),
            PopoverKind::PullPicker => self.context_menu_view(PopoverKind::PullPicker, cx),
            PopoverKind::PushPicker => self.context_menu_view(PopoverKind::PushPicker, cx),
            PopoverKind::DiffHunks => {
                let mut items: Vec<SharedString> = Vec::new();
                let mut targets: Vec<usize> = Vec::new();
                let mut current_file: Option<String> = None;

                for (visible_ix, &ix) in self.diff_visible_indices.iter().enumerate() {
                    let (src_ix, click_kind) = match self.diff_view {
                        DiffViewMode::Inline => {
                            let Some(line) = self.diff_cache.get(ix) else {
                                continue;
                            };
                            let kind =
                                if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk) {
                                    DiffClickKind::HunkHeader
                                } else if matches!(
                                    line.kind,
                                    gitgpui_core::domain::DiffLineKind::Header
                                ) && line.text.starts_with("diff --git ")
                                {
                                    DiffClickKind::FileHeader
                                } else {
                                    DiffClickKind::Line
                                };
                            (ix, kind)
                        }
                        DiffViewMode::Split => {
                            let Some(row) = self.diff_split_cache.get(ix) else {
                                continue;
                            };
                            let PatchSplitRow::Raw { src_ix, click_kind } = row else {
                                continue;
                            };
                            (*src_ix, *click_kind)
                        }
                    };

                    let Some(line) = self.diff_cache.get(src_ix) else {
                        continue;
                    };

                    if matches!(click_kind, DiffClickKind::FileHeader) {
                        current_file = parse_diff_git_header_path(&line.text);
                    }

                    if !matches!(click_kind, DiffClickKind::HunkHeader) {
                        continue;
                    }

                    let label =
                        if let Some(parsed) = parse_unified_hunk_header_for_display(&line.text) {
                            let file = current_file.as_deref().unwrap_or("<file>").to_string();
                            let heading = parsed.heading.unwrap_or_default();
                            if heading.is_empty() {
                                format!("{file}: {} {}", parsed.old, parsed.new)
                            } else {
                                format!("{file}: {} {} {heading}", parsed.old, parsed.new)
                            }
                        } else {
                            current_file.as_deref().unwrap_or("<file>").to_string()
                        };

                    items.push(label.into());
                    targets.push(visible_ix);
                }

                if let Some(search) = self.diff_hunk_picker_search_input.clone() {
                    zed::PickerPrompt::new(search)
                        .items(items)
                        .empty_text("No hunks")
                        .max_height(px(260.0))
                        .render(theme, cx, move |this, ix, _e, _w, cx| {
                            let Some(&target) = targets.get(ix) else {
                                return;
                            };
                            this.diff_scroll
                                .scroll_to_item(target, gpui::ScrollStrategy::Top);
                            this.diff_selection_anchor = Some(target);
                            this.diff_selection_range = Some((target, target));
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        })
                        .min_w(px(520.0))
                        .child(div().border_t_1().border_color(theme.colors.border))
                        .child(
                            div()
                                .id("diff_hunks_close")
                                .px_2()
                                .py_1()
                                .hover(move |s| s.bg(theme.colors.hover))
                                .child("Close")
                                .on_click(close),
                        )
                } else {
                    let mut menu = div().flex().flex_col().min_w(px(520.0));
                    for (ix, label) in items.into_iter().enumerate() {
                        let target = targets.get(ix).copied().unwrap_or(0);
                        menu = menu.child(
                            div()
                                .id(("diff_hunk_item", ix))
                                .px_2()
                                .py_1()
                                .hover(move |s| s.bg(theme.colors.hover))
                                .child(div().text_sm().line_clamp(1).child(label))
                                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                    this.diff_scroll
                                        .scroll_to_item(target, gpui::ScrollStrategy::Top);
                                    this.diff_selection_anchor = Some(target);
                                    this.diff_selection_range = Some((target, target));
                                    this.popover = None;
                                    this.popover_anchor = None;
                                    cx.notify();
                                })),
                        );
                    }
                    menu.child(
                        div()
                            .id("diff_hunks_close")
                            .px_2()
                            .py_1()
                            .hover(move |s| s.bg(theme.colors.hover))
                            .child("Close")
                            .on_click(close),
                    )
                }
            }
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
            PopoverKind::StatusFileMenu {
                repo_id,
                area,
                path,
            } => self.context_menu_view(
                PopoverKind::StatusFileMenu {
                    repo_id,
                    area,
                    path,
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
	            PopoverKind::AppMenu => {
	                let active_repo = self.active_repo();
	                let active_repo_id = active_repo.map(|r| r.id);
	                let rebase_in_progress = active_repo
	                    .and_then(|r| match &r.rebase_in_progress {
	                        Loadable::Ready(v) => Some(*v),
	                        _ => None,
	                    })
	                    .unwrap_or(false);
                    let selected_commit = active_repo.and_then(|r| r.selected_commit.clone());
                    let selected_short = selected_commit.as_ref().map(|id| {
                        let sha = id.as_ref();
                        sha.get(0..8).unwrap_or(sha).to_string()
                    });

	                let separator = || {
	                    div()
	                        .h(px(1.0))
	                        .w_full()
	                        .bg(theme.colors.border)
	                        .my(px(4.0))
	                };

	                let section_label = |id: &'static str, text: &'static str| {
	                    div()
	                        .id(id)
	                        .px_2()
	                        .pt(px(6.0))
	                        .pb(px(4.0))
	                        .text_xs()
	                        .text_color(theme.colors.text_muted)
	                        .child(text)
	                };

	                let entry = |id: &'static str, label: SharedString, disabled: bool| {
	                    div()
	                        .id(id)
	                        .debug_selector(move || id.to_string())
	                        .px_2()
	                        .py_1()
	                        .when(!disabled, |d| {
	                            d.hover(move |s| s.bg(theme.colors.hover))
	                                .active(move |s| s.bg(theme.colors.active))
	                        })
	                        .when(disabled, |d| d.text_color(theme.colors.text_muted))
	                        .child(label)
	                };

	                let mut install_desktop = div()
	                    .id("app_menu_install_desktop")
	                    .debug_selector(|| "app_menu_install_desktop".to_string())
                    .px_2()
                    .py_1()
                    .hover(move |s| s.bg(theme.colors.hover))
                    .active(move |s| s.bg(theme.colors.active))
                    .child("Install desktop integration");

                #[cfg(any(target_os = "linux", target_os = "freebsd"))]
                {
                    install_desktop =
                        install_desktop.on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.install_linux_desktop_integration(cx);
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }));
                }

                #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
                {
                    install_desktop = install_desktop.text_color(theme.colors.text_muted);
	                }

		                let menu = div()
	                    .flex()
	                    .flex_col()
	                    .min_w(px(200.0))
                        .child(section_label("app_menu_app_section", "Application"))
                        .child(
                            entry("app_menu_settings", "Settings…".into(), false).on_click(
                                cx.listener(|this, e: &ClickEvent, _w, cx| {
                                    this.popover = Some(PopoverKind::Settings);
                                    this.popover_anchor = Some(e.position());
                                    this.settings_date_format_open = false;
                                    cx.notify();
                                }),
                            ),
                        )
                        .child(separator())
	                    .child(section_label("app_menu_repo_section", "Repository"))
	                    .child(
	                        entry(
	                            "app_menu_rebase",
	                            "Rebase onto…".into(),
	                            active_repo_id.is_none() || rebase_in_progress,
	                        )
	                        .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
	                            let Some(repo_id) = active_repo_id else {
	                                return;
	                            };
	                            if rebase_in_progress {
	                                return;
	                            }
	                            this.open_popover_at(
	                                PopoverKind::RebasePrompt { repo_id },
	                                e.position(),
	                                window,
	                                cx,
	                            );
	                        })),
	                    )
	                    .child(
	                        entry(
	                            "app_menu_rebase_continue",
	                            "Rebase --continue".into(),
	                            active_repo_id.is_none() || !rebase_in_progress,
	                        )
	                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
	                            let Some(repo_id) = active_repo_id else {
	                                return;
	                            };
	                            if !rebase_in_progress {
	                                return;
	                            }
	                            this.store.dispatch(Msg::RebaseContinue { repo_id });
	                            this.popover = None;
	                            this.popover_anchor = None;
	                            cx.notify();
	                        })),
	                    )
	                    .child(
	                        entry(
	                            "app_menu_rebase_abort",
	                            "Rebase --abort".into(),
	                            active_repo_id.is_none() || !rebase_in_progress,
	                        )
	                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
	                            let Some(repo_id) = active_repo_id else {
	                                return;
	                            };
	                            if !rebase_in_progress {
	                                return;
	                            }
	                            this.store.dispatch(Msg::RebaseAbort { repo_id });
	                            this.popover = None;
	                            this.popover_anchor = None;
	                            cx.notify();
	                        })),
	                    )
	                    .child(separator())
	                    .child(section_label("app_menu_remotes_section", "Remotes"))
	                    .child(
	                        entry(
	                            "app_menu_add_remote",
	                            "Add remote…".into(),
	                            active_repo_id.is_none(),
	                        )
	                        .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
	                            let Some(repo_id) = active_repo_id else {
	                                return;
	                            };
	                            this.open_popover_at(
	                                PopoverKind::RemoteAddPrompt { repo_id },
	                                e.position(),
	                                window,
	                                cx,
	                            );
	                        })),
	                    )
	                    .child(
	                        entry(
	                            "app_menu_edit_remote_fetch_url",
	                            "Edit remote fetch URL…".into(),
	                            active_repo_id.is_none(),
	                        )
	                        .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
	                            let Some(repo_id) = active_repo_id else {
	                                return;
	                            };
	                            this.ensure_remote_picker_search_input(window, cx);
	                            this.open_popover_at(
	                                PopoverKind::RemoteUrlPicker {
	                                    repo_id,
	                                    kind: RemoteUrlKind::Fetch,
	                                },
	                                e.position(),
	                                window,
	                                cx,
	                            );
	                        })),
	                    )
	                    .child(
	                        entry(
	                            "app_menu_edit_remote_push_url",
	                            "Edit remote push URL…".into(),
	                            active_repo_id.is_none(),
	                        )
	                        .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
	                            let Some(repo_id) = active_repo_id else {
	                                return;
	                            };
	                            this.ensure_remote_picker_search_input(window, cx);
	                            this.open_popover_at(
	                                PopoverKind::RemoteUrlPicker {
	                                    repo_id,
	                                    kind: RemoteUrlKind::Push,
	                                },
	                                e.position(),
	                                window,
	                                cx,
	                            );
	                        })),
	                    )
	                    .child(
	                        entry(
	                            "app_menu_remove_remote",
	                            "Remove remote…".into(),
	                            active_repo_id.is_none(),
	                        )
	                        .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
	                            let Some(repo_id) = active_repo_id else {
	                                return;
	                            };
	                            this.ensure_remote_picker_search_input(window, cx);
	                            this.open_popover_at(
	                                PopoverKind::RemoteRemovePicker { repo_id },
	                                e.position(),
	                                window,
	                                cx,
	                            );
	                        })),
	                    )
	                    .child(separator())
	                    .child(section_label("app_menu_patches_section", "Patches"))
	                    .child({
	                        let disabled = active_repo_id.is_none() || selected_commit.is_none();
	                        let label: SharedString = selected_short
	                            .as_ref()
	                            .map(|s| format!("Export patch for {s}…").into())
	                            .unwrap_or_else(|| "Export patch…".into());
	                        let selected_commit = selected_commit.clone();
	                        let selected_short = selected_short.clone();
	                        entry("app_menu_export_patch", label, disabled).on_click(cx.listener(
	                            move |this, _e: &ClickEvent, window, cx| {
	                                let Some(repo_id) = active_repo_id else {
	                                    return;
	                                };
	                                let Some(commit_id) = selected_commit.clone() else {
	                                    return;
	                                };
	                                cx.stop_propagation();
	                                let view = cx.weak_entity();
	                                let short = selected_short.clone().unwrap_or_else(|| {
	                                    let sha = commit_id.as_ref();
	                                    sha.get(0..8).unwrap_or(sha).to_string()
	                                });
	                                let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
	                                    files: false,
	                                    directories: true,
	                                    multiple: false,
	                                    prompt: Some("Export patch to folder".into()),
	                                });
	                                window
	                                    .spawn(cx, async move |cx| {
	                                        let result = rx.await;
	                                        let paths = match result {
	                                            Ok(Ok(Some(paths))) => paths,
	                                            Ok(Ok(None)) => return,
	                                            Ok(Err(_)) | Err(_) => return,
	                                        };
	                                        let Some(folder) = paths.into_iter().next() else {
	                                            return;
	                                        };
	                                        let dest =
	                                            folder.join(format!("commit-{short}.patch"));
	                                        let _ = view.update(cx, |this, cx| {
	                                            this.store.dispatch(Msg::ExportPatch {
	                                                repo_id,
	                                                commit_id: commit_id.clone(),
	                                                dest,
	                                            });
	                                            cx.notify();
	                                        });
	                                    })
	                                    .detach();
	                                this.popover = None;
	                                this.popover_anchor = None;
	                                cx.notify();
	                            },
	                        ))
	                    })
	                    .child(
	                        entry(
	                            "app_menu_apply_patch",
	                            "Apply patch…".into(),
	                            active_repo_id.is_none(),
	                        )
	                        .on_click(cx.listener(move |this, _e: &ClickEvent, window, cx| {
	                            let Some(repo_id) = active_repo_id else {
	                                return;
	                            };
	                            cx.stop_propagation();
	                            let view = cx.weak_entity();
	                            let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
	                                files: true,
	                                directories: false,
	                                multiple: false,
	                                prompt: Some("Select patch file".into()),
	                            });
	                            window
	                                .spawn(cx, async move |cx| {
	                                    let result = rx.await;
	                                    let paths = match result {
	                                        Ok(Ok(Some(paths))) => paths,
	                                        Ok(Ok(None)) => return,
	                                        Ok(Err(_)) | Err(_) => return,
	                                    };
	                                    let Some(patch) = paths.into_iter().next() else {
	                                        return;
	                                    };
	                                    let _ = view.update(cx, |this, cx| {
	                                        this.store.dispatch(Msg::ApplyPatch { repo_id, patch });
	                                        cx.notify();
	                                    });
	                                })
	                                .detach();
	                            this.popover = None;
	                            this.popover_anchor = None;
	                            cx.notify();
	                        })),
	                    )
	                    .child(separator())
	                    .child(section_label("app_menu_worktrees_section", "Worktrees"))
	                    .child(
	                        entry(
	                            "app_menu_add_worktree",
	                            "Add worktree…".into(),
	                            active_repo_id.is_none(),
	                        )
	                        .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
	                            let Some(repo_id) = active_repo_id else {
	                                return;
	                            };
	                            this.open_popover_at(
	                                PopoverKind::WorktreeAddPrompt { repo_id },
	                                e.position(),
	                                window,
	                                cx,
	                            );
	                        })),
	                    )
	                    .child(
	                        entry(
	                            "app_menu_open_worktree",
	                            "Open worktree…".into(),
	                            active_repo_id.is_none(),
	                        )
	                        .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
	                            let Some(repo_id) = active_repo_id else {
	                                return;
	                            };
	                            this.open_popover_at(
	                                PopoverKind::WorktreeOpenPicker { repo_id },
	                                e.position(),
	                                window,
	                                cx,
	                            );
	                        })),
	                    )
	                    .child(
	                        entry(
	                            "app_menu_remove_worktree",
	                            "Remove worktree…".into(),
	                            active_repo_id.is_none(),
	                        )
	                        .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
	                            let Some(repo_id) = active_repo_id else {
	                                return;
	                            };
	                            this.open_popover_at(
	                                PopoverKind::WorktreeRemovePicker { repo_id },
	                                e.position(),
	                                window,
	                                cx,
	                            );
	                        })),
	                    )
	                    .child(separator())
	                    .child(section_label("app_menu_submodules_section", "Submodules"))
	                    .child(
	                        entry(
	                            "app_menu_add_submodule",
	                            "Add submodule…".into(),
	                            active_repo_id.is_none(),
	                        )
	                        .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
	                            let Some(repo_id) = active_repo_id else {
	                                return;
	                            };
	                            this.open_popover_at(
	                                PopoverKind::SubmoduleAddPrompt { repo_id },
	                                e.position(),
	                                window,
	                                cx,
	                            );
	                        })),
	                    )
	                    .child(
	                        entry(
	                            "app_menu_update_submodules",
	                            "Update submodules".into(),
	                            active_repo_id.is_none(),
	                        )
	                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
	                            let Some(repo_id) = active_repo_id else {
	                                return;
	                            };
	                            this.store.dispatch(Msg::UpdateSubmodules { repo_id });
	                            this.popover = None;
	                            this.popover_anchor = None;
	                            cx.notify();
	                        })),
	                    )
	                    .child(
	                        entry(
	                            "app_menu_open_submodule",
	                            "Open submodule…".into(),
	                            active_repo_id.is_none(),
	                        )
	                        .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
	                            let Some(repo_id) = active_repo_id else {
	                                return;
	                            };
	                            this.open_popover_at(
	                                PopoverKind::SubmoduleOpenPicker { repo_id },
	                                e.position(),
	                                window,
	                                cx,
	                            );
	                        })),
	                    )
	                    .child(
	                        entry(
	                            "app_menu_remove_submodule",
	                            "Remove submodule…".into(),
	                            active_repo_id.is_none(),
	                        )
	                        .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
	                            let Some(repo_id) = active_repo_id else {
	                                return;
	                            };
	                            this.open_popover_at(
	                                PopoverKind::SubmoduleRemovePicker { repo_id },
	                                e.position(),
	                                window,
	                                cx,
	                            );
	                        })),
	                    )
	                    .child(separator())
	                    .child(install_desktop)
	                    .child(
	                        div()
                            .id("app_menu_quit")
                            .debug_selector(|| "app_menu_quit".to_string())
                            .px_2()
                            .py_1()
                            .hover(move |s| s.bg(theme.colors.hover))
                            .active(move |s| s.bg(theme.colors.active))
                            .child("Quit")
                            .on_click(cx.listener(|_this, _e: &ClickEvent, _w, cx| {
                                cx.quit();
                            })),
                    )
                    .child(
                        div()
                            .id("app_menu_close")
                            .debug_selector(|| "app_menu_close".to_string())
                            .px_2()
                            .py_1()
                            .hover(move |s| s.bg(theme.colors.hover))
                            .active(move |s| s.bg(theme.colors.active))
	                            .child("Close")
	                            .on_click(close),
	                    )
	                    ;

	                menu
	            }
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
        let (view, cx) = cx.add_window_view(|window, cx| GitGpuiView::new(store, events, None, window, cx));

        let repo_id = RepoId(1);
        let commit_id = CommitId("deadbeefdeadbeef".to_string());
        let workdir = std::env::temp_dir()
            .join(format!("gitgpui_ui_test_{}_commit_menu_tag", std::process::id()));

        cx.update(|_window, app| {
            view.update(app, |this, cx| {
                let mut repo = RepoState::new_opening(
                    repo_id,
                    gitgpui_core::domain::RepoSpec {
                        workdir: workdir.clone(),
                    },
                );
                repo.log = Loadable::Ready(gitgpui_core::domain::LogPage {
                    commits: vec![gitgpui_core::domain::Commit {
                        id: commit_id.clone(),
                        parent_ids: vec![],
                        summary: "Hello".to_string(),
                        author: "Alice".to_string(),
                        time: SystemTime::UNIX_EPOCH,
                    }],
                    next_cursor: None,
                });
                repo.tags = Loadable::Ready(vec![]);

                this.state.active_repo = Some(repo_id);
                this.state.repos = vec![repo];
                cx.notify();
            });
        });

        cx.update(|_window, app| {
            let this = view.read(app);
            let model = this
                .context_menu_model(&PopoverKind::CommitMenu {
                    repo_id,
                    commit_id: commit_id.clone(),
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

            let PopoverKind::CreateTagPrompt { repo_id: rid, target } = kind else {
                panic!("expected Add tag… to open CreateTagPrompt");
            };

            assert_eq!(rid, repo_id);
            assert_eq!(target, commit_id.as_ref().to_string());
        });
    }

    #[gpui::test]
    fn tag_menu_lists_delete_entries_for_commit_tags(cx: &mut gpui::TestAppContext) {
        let (store, events) = AppStore::new(Arc::new(TestBackend));
        let (view, cx) = cx.add_window_view(|window, cx| GitGpuiView::new(store, events, None, window, cx));

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
                repo.log = Loadable::Ready(gitgpui_core::domain::LogPage {
                    commits: vec![gitgpui_core::domain::Commit {
                        id: commit_id.clone(),
                        parent_ids: vec![],
                        summary: "Hello".to_string(),
                        author: "Alice".to_string(),
                        time: SystemTime::UNIX_EPOCH,
                    }],
                    next_cursor: None,
                });
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

                this.state.active_repo = Some(repo_id);
                this.state.repos = vec![repo];
                cx.notify();
            });
        });

        cx.update(|_window, app| {
            let this = view.read(app);
            let model = this
                .context_menu_model(&PopoverKind::TagMenu {
                    repo_id,
                    commit_id: commit_id.clone(),
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
                    Some(ContextMenuAction::DeleteTag { repo_id: rid, name: n }) => {
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
            assert!(!has_other, "tag menu should only show tags on the clicked commit");
        });
    }
}
