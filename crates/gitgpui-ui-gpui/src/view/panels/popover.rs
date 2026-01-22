use super::*;

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
                | PopoverKind::HistoryBranchFilter { .. }
                | PopoverKind::CommitMenu { .. }
                | PopoverKind::StatusFileMenu { .. }
                | PopoverKind::BranchMenu { .. }
                | PopoverKind::BranchSectionMenu { .. }
                | PopoverKind::CommitFileMenu { .. }
        );

        self.popover = Some(kind.clone());
        self.popover_anchor = Some(anchor);
        self.context_menu_selected_ix = None;
        if is_context_menu {
            self.context_menu_selected_ix = self
                .context_menu_model(&kind)
                .and_then(|m| m.first_selectable());
            window.focus(&self.context_menu_focus_handle);
        }
        cx.notify();
    }

    fn context_menu_model(&self, kind: &PopoverKind) -> Option<ContextMenuModel> {
        match kind {
            PopoverKind::PullPicker => {
                let repo_id = self.active_repo_id();
                let disabled = repo_id.is_none();
                let repo_id = repo_id.unwrap_or(RepoId(0));

                Some(ContextMenuModel::new(vec![
                    ContextMenuItem::Header("Pull".into()),
                    ContextMenuItem::Separator,
                    ContextMenuItem::Entry {
                        label: "Pull (default)".into(),
                        icon: Some("↓".into()),
                        shortcut: Some("Enter".into()),
                        disabled,
                        action: ContextMenuAction::Pull {
                            repo_id,
                            mode: PullMode::Default,
                        },
                    },
                    ContextMenuItem::Entry {
                        label: "Pull (fast-forward if possible)".into(),
                        icon: Some("↓".into()),
                        shortcut: Some("F".into()),
                        disabled,
                        action: ContextMenuAction::Pull {
                            repo_id,
                            mode: PullMode::FastForwardIfPossible,
                        },
                    },
                    ContextMenuItem::Entry {
                        label: "Pull (fast-forward only)".into(),
                        icon: Some("↓".into()),
                        shortcut: Some("O".into()),
                        disabled,
                        action: ContextMenuAction::Pull {
                            repo_id,
                            mode: PullMode::FastForwardOnly,
                        },
                    },
                    ContextMenuItem::Entry {
                        label: "Pull (rebase)".into(),
                        icon: Some("↓".into()),
                        shortcut: Some("R".into()),
                        disabled,
                        action: ContextMenuAction::Pull {
                            repo_id,
                            mode: PullMode::Rebase,
                        },
                    },
                    ContextMenuItem::Separator,
                    ContextMenuItem::Entry {
                        label: "Fetch all".into(),
                        icon: Some("↓".into()),
                        shortcut: Some("A".into()),
                        disabled,
                        action: ContextMenuAction::FetchAll { repo_id },
                    },
                ]))
            }
            PopoverKind::CommitMenu { repo_id, commit_id } => {
                let sha = commit_id.as_ref().to_string();
                let short: SharedString = sha.get(0..8).unwrap_or(&sha).to_string().into();

                let commit_summary = self
                    .active_repo()
                    .and_then(|r| match &r.log {
                        Loadable::Ready(page) => page
                            .commits
                            .iter()
                            .find(|c| c.id == *commit_id)
                            .map(|c| format!("{} — {}", c.author, c.summary)),
                        _ => None,
                    })
                    .unwrap_or_default();

                let mut items = vec![ContextMenuItem::Header(format!("Commit {short}").into())];
                if !commit_summary.is_empty() {
                    items.push(ContextMenuItem::Label(commit_summary.into()));
                }
                items.push(ContextMenuItem::Separator);
                items.push(ContextMenuItem::Entry {
                    label: "Open diff".into(),
                    icon: Some("↗".into()),
                    shortcut: Some("Enter".into()),
                    disabled: false,
                    action: ContextMenuAction::SelectDiff {
                        repo_id: *repo_id,
                        target: DiffTarget::Commit {
                            commit_id: commit_id.clone(),
                            path: None,
                        },
                    },
                });
                items.push(ContextMenuItem::Entry {
                    label: "Checkout (detached)".into(),
                    icon: Some("⎇".into()),
                    shortcut: Some("D".into()),
                    disabled: false,
                    action: ContextMenuAction::CheckoutCommit {
                        repo_id: *repo_id,
                        commit_id: commit_id.clone(),
                    },
                });
                items.push(ContextMenuItem::Entry {
                    label: "Cherry-pick".into(),
                    icon: Some("⇡".into()),
                    shortcut: Some("P".into()),
                    disabled: false,
                    action: ContextMenuAction::CherryPickCommit {
                        repo_id: *repo_id,
                        commit_id: commit_id.clone(),
                    },
                });
                items.push(ContextMenuItem::Entry {
                    label: "Revert".into(),
                    icon: Some("↶".into()),
                    shortcut: Some("R".into()),
                    disabled: false,
                    action: ContextMenuAction::RevertCommit {
                        repo_id: *repo_id,
                        commit_id: commit_id.clone(),
                    },
                });

                Some(ContextMenuModel::new(items))
            }
            PopoverKind::StatusFileMenu {
                repo_id,
                area,
                path,
            } => {
                let mut items = vec![ContextMenuItem::Header(
                    path.file_name()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.display().to_string())
                        .into(),
                )];
                items.push(ContextMenuItem::Label(path.display().to_string().into()));
                items.push(ContextMenuItem::Separator);

                items.push(ContextMenuItem::Entry {
                    label: "Open diff".into(),
                    icon: Some("↗".into()),
                    shortcut: Some("Enter".into()),
                    disabled: false,
                    action: ContextMenuAction::SelectDiff {
                        repo_id: *repo_id,
                        target: DiffTarget::WorkingTree {
                            path: path.clone(),
                            area: *area,
                        },
                    },
                });

                match area {
                    DiffArea::Unstaged => items.push(ContextMenuItem::Entry {
                        label: "Stage".into(),
                        icon: Some("+".into()),
                        shortcut: Some("S".into()),
                        disabled: false,
                        action: ContextMenuAction::StagePath {
                            repo_id: *repo_id,
                            path: path.clone(),
                        },
                    }),
                    DiffArea::Staged => items.push(ContextMenuItem::Entry {
                        label: "Unstage".into(),
                        icon: Some("−".into()),
                        shortcut: Some("U".into()),
                        disabled: false,
                        action: ContextMenuAction::UnstagePath {
                            repo_id: *repo_id,
                            path: path.clone(),
                        },
                    }),
                };

                items.push(ContextMenuItem::Separator);
                items.push(ContextMenuItem::Entry {
                    label: "Copy path".into(),
                    icon: Some("⧉".into()),
                    shortcut: Some("C".into()),
                    disabled: false,
                    action: ContextMenuAction::CopyText {
                        text: path.display().to_string(),
                    },
                });

                Some(ContextMenuModel::new(items))
            }
            PopoverKind::BranchMenu {
                repo_id,
                section,
                name,
            } => {
                let header: SharedString = match section {
                    BranchSection::Local => "Local branch".into(),
                    BranchSection::Remote => "Remote branch".into(),
                };
                let mut items = vec![ContextMenuItem::Header(header)];
                items.push(ContextMenuItem::Label(name.clone().into()));
                items.push(ContextMenuItem::Separator);

                items.push(ContextMenuItem::Entry {
                    label: "Checkout".into(),
                    icon: Some("⎇".into()),
                    shortcut: Some("Enter".into()),
                    disabled: false,
                    action: ContextMenuAction::CheckoutBranch {
                        repo_id: *repo_id,
                        name: name.clone(),
                    },
                });
                items.push(ContextMenuItem::Entry {
                    label: "Copy name".into(),
                    icon: Some("⧉".into()),
                    shortcut: Some("C".into()),
                    disabled: false,
                    action: ContextMenuAction::CopyText { text: name.clone() },
                });

                if *section == BranchSection::Remote {
                    items.push(ContextMenuItem::Separator);
                    if let Some((remote, branch)) = name.split_once('/') {
                        items.push(ContextMenuItem::Entry {
                            label: "Pull into current".into(),
                            icon: Some("↓".into()),
                            shortcut: Some("P".into()),
                            disabled: false,
                            action: ContextMenuAction::PullBranch {
                                repo_id: *repo_id,
                                remote: remote.to_string(),
                                branch: branch.to_string(),
                            },
                        });
                        items.push(ContextMenuItem::Entry {
                            label: "Merge into current".into(),
                            icon: Some("⇄".into()),
                            shortcut: Some("M".into()),
                            disabled: false,
                            action: ContextMenuAction::MergeRef {
                                repo_id: *repo_id,
                                reference: name.clone(),
                            },
                        });
                        items.push(ContextMenuItem::Separator);
                    }
                    items.push(ContextMenuItem::Entry {
                        label: "Fetch all".into(),
                        icon: Some("↓".into()),
                        shortcut: Some("F".into()),
                        disabled: false,
                        action: ContextMenuAction::FetchAll { repo_id: *repo_id },
                    });
                }

                Some(ContextMenuModel::new(items))
            }
            PopoverKind::BranchSectionMenu { repo_id, section } => {
                let header: SharedString = match section {
                    BranchSection::Local => "Local".into(),
                    BranchSection::Remote => "Remote".into(),
                };
                let mut items = vec![ContextMenuItem::Header(header)];
                items.push(ContextMenuItem::Separator);
                items.push(ContextMenuItem::Entry {
                    label: "Switch branch…".into(),
                    icon: Some("⎇".into()),
                    shortcut: Some("Enter".into()),
                    disabled: false,
                    action: ContextMenuAction::OpenPopover {
                        kind: PopoverKind::BranchPicker,
                    },
                });

                if *section == BranchSection::Remote {
                    items.push(ContextMenuItem::Entry {
                        label: "Fetch all".into(),
                        icon: Some("↓".into()),
                        shortcut: Some("F".into()),
                        disabled: false,
                        action: ContextMenuAction::FetchAll { repo_id: *repo_id },
                    });
                }

                Some(ContextMenuModel::new(items))
            }
            PopoverKind::CommitFileMenu {
                repo_id,
                commit_id,
                path,
            } => {
                let mut items = vec![ContextMenuItem::Header(
                    path.file_name()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.display().to_string())
                        .into(),
                )];
                items.push(ContextMenuItem::Label(path.display().to_string().into()));
                items.push(ContextMenuItem::Separator);
                items.push(ContextMenuItem::Entry {
                    label: "Open diff".into(),
                    icon: Some("↗".into()),
                    shortcut: Some("Enter".into()),
                    disabled: false,
                    action: ContextMenuAction::SelectDiff {
                        repo_id: *repo_id,
                        target: DiffTarget::Commit {
                            commit_id: commit_id.clone(),
                            path: Some(path.clone()),
                        },
                    },
                });
                items.push(ContextMenuItem::Entry {
                    label: "Copy path".into(),
                    icon: Some("⧉".into()),
                    shortcut: Some("C".into()),
                    disabled: false,
                    action: ContextMenuAction::CopyText {
                        text: path.display().to_string(),
                    },
                });
                Some(ContextMenuModel::new(items))
            }
            PopoverKind::HistoryBranchFilter { repo_id } => Some(ContextMenuModel::new(vec![
                ContextMenuItem::Header("History scope".into()),
                ContextMenuItem::Separator,
                ContextMenuItem::Entry {
                    label: "Current branch".into(),
                    icon: Some("⎇".into()),
                    shortcut: Some("C".into()),
                    disabled: false,
                    action: ContextMenuAction::SetHistoryScope {
                        repo_id: *repo_id,
                        scope: gitgpui_core::domain::LogScope::CurrentBranch,
                    },
                },
                ContextMenuItem::Entry {
                    label: "All branches".into(),
                    icon: Some("∞".into()),
                    shortcut: Some("A".into()),
                    disabled: false,
                    action: ContextMenuAction::SetHistoryScope {
                        repo_id: *repo_id,
                        scope: gitgpui_core::domain::LogScope::AllBranches,
                    },
                },
            ])),
            _ => None,
        }
    }

    fn context_menu_activate_action(
        &mut self,
        action: ContextMenuAction,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        match action {
            ContextMenuAction::SelectDiff { repo_id, target } => {
                self.store.dispatch(Msg::SelectDiff { repo_id, target });
                self.rebuild_diff_cache();
            }
            ContextMenuAction::CheckoutCommit { repo_id, commit_id } => {
                self.store
                    .dispatch(Msg::CheckoutCommit { repo_id, commit_id });
            }
            ContextMenuAction::CherryPickCommit { repo_id, commit_id } => {
                self.store
                    .dispatch(Msg::CherryPickCommit { repo_id, commit_id });
            }
            ContextMenuAction::RevertCommit { repo_id, commit_id } => {
                self.store
                    .dispatch(Msg::RevertCommit { repo_id, commit_id });
            }
            ContextMenuAction::CheckoutBranch { repo_id, name } => {
                self.store.dispatch(Msg::CheckoutBranch { repo_id, name });
                self.rebuild_diff_cache();
            }
            ContextMenuAction::SetHistoryScope { repo_id, scope } => {
                self.store.dispatch(Msg::SetHistoryScope { repo_id, scope });
            }
            ContextMenuAction::StagePath { repo_id, path } => {
                self.store.dispatch(Msg::SelectDiff {
                    repo_id,
                    target: DiffTarget::WorkingTree {
                        path: path.clone(),
                        area: DiffArea::Unstaged,
                    },
                });
                self.store.dispatch(Msg::StagePath { repo_id, path });
                self.rebuild_diff_cache();
            }
            ContextMenuAction::UnstagePath { repo_id, path } => {
                self.store.dispatch(Msg::SelectDiff {
                    repo_id,
                    target: DiffTarget::WorkingTree {
                        path: path.clone(),
                        area: DiffArea::Staged,
                    },
                });
                self.store.dispatch(Msg::UnstagePath { repo_id, path });
                self.rebuild_diff_cache();
            }
            ContextMenuAction::FetchAll { repo_id } => {
                self.store.dispatch(Msg::FetchAll { repo_id });
            }
            ContextMenuAction::Pull { repo_id, mode } => {
                self.store.dispatch(Msg::Pull { repo_id, mode });
            }
            ContextMenuAction::PullBranch {
                repo_id,
                remote,
                branch,
            } => {
                self.store.dispatch(Msg::PullBranch {
                    repo_id,
                    remote,
                    branch,
                });
            }
            ContextMenuAction::MergeRef { repo_id, reference } => {
                self.store.dispatch(Msg::MergeRef { repo_id, reference });
            }
            ContextMenuAction::OpenPopover { kind } => {
                self.popover = Some(kind);
                self.context_menu_selected_ix = None;
                cx.notify();
                return;
            }
            ContextMenuAction::CopyText { text } => {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
            }
        }
        self.close_popover(cx);
    }

    fn context_menu_view(&mut self, kind: PopoverKind, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let model = self
            .context_menu_model(&kind)
            .unwrap_or_else(|| ContextMenuModel::new(vec![]));
        let model_for_keys = model.clone();

        let focus = self.context_menu_focus_handle.clone();
        let current_selected = self.context_menu_selected_ix;
        let selected_for_render = current_selected
            .filter(|&ix| model.is_selectable(ix))
            .or_else(|| model.first_selectable());

        zed::context_menu(
            theme,
            div()
                .track_focus(&focus)
                .key_context("ContextMenu")
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _e: &MouseDownEvent, window, _cx| {
                        window.focus(&this.context_menu_focus_handle);
                    }),
                )
                .on_key_down(
                    cx.listener(move |this, e: &gpui::KeyDownEvent, window, cx| {
                        let key = e.keystroke.key.as_str();
                        let mods = e.keystroke.modifiers;
                        if mods.control || mods.platform || mods.alt || mods.function {
                            return;
                        }

                        match key {
                            "escape" => {
                                this.close_popover(cx);
                            }
                            "up" => {
                                let next = model_for_keys
                                    .next_selectable(this.context_menu_selected_ix, -1);
                                this.context_menu_selected_ix = next;
                                cx.notify();
                            }
                            "down" => {
                                let next = model_for_keys
                                    .next_selectable(this.context_menu_selected_ix, 1);
                                this.context_menu_selected_ix = next;
                                cx.notify();
                            }
                            "home" => {
                                this.context_menu_selected_ix = model_for_keys.first_selectable();
                                cx.notify();
                            }
                            "end" => {
                                this.context_menu_selected_ix = model_for_keys.last_selectable();
                                cx.notify();
                            }
                            "enter" => {
                                let Some(ix) = this
                                    .context_menu_selected_ix
                                    .filter(|&ix| model_for_keys.is_selectable(ix))
                                    .or_else(|| model_for_keys.first_selectable())
                                else {
                                    return;
                                };
                                if let Some(ContextMenuItem::Entry { action, .. }) =
                                    model_for_keys.items.get(ix).cloned()
                                {
                                    this.context_menu_activate_action(action, window, cx);
                                }
                            }
                            _ => {
                                if key.chars().count() == 1 {
                                    let needle = key.to_ascii_uppercase();
                                    let hit = model_for_keys.items.iter().enumerate().find_map(
                                        |(ix, item)| {
                                            let ContextMenuItem::Entry {
                                                shortcut, disabled, ..
                                            } = item
                                            else {
                                                return None;
                                            };
                                            if *disabled {
                                                return None;
                                            }
                                            let shortcut =
                                                shortcut.as_ref()?.as_ref().to_ascii_uppercase();
                                            (shortcut == needle).then_some(ix)
                                        },
                                    );

                                    if let Some(ix) = hit
                                        && let Some(ContextMenuItem::Entry { action, .. }) =
                                            model_for_keys.items.get(ix).cloned()
                                    {
                                        this.context_menu_activate_action(action, window, cx);
                                    }
                                }
                            }
                        }
                    }),
                )
                .children(model.items.into_iter().enumerate().map(|(ix, item)| {
                    match item {
                        ContextMenuItem::Separator => zed::context_menu_separator(theme)
                            .id(("context_menu_sep", ix))
                            .into_any_element(),
                        ContextMenuItem::Header(title) => zed::context_menu_header(theme, title)
                            .id(("context_menu_header", ix))
                            .into_any_element(),
                        ContextMenuItem::Label(text) => zed::context_menu_label(theme, text)
                            .id(("context_menu_label", ix))
                            .into_any_element(),
                        ContextMenuItem::Entry {
                            label,
                            icon,
                            shortcut,
                            disabled,
                            action,
                        } => {
                            let selected = selected_for_render == Some(ix);
                            zed::context_menu_entry(
                                ("context_menu_entry", ix),
                                theme,
                                selected,
                                disabled,
                                icon,
                                label,
                                shortcut,
                                false,
                            )
                            .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                                if *hovering {
                                    this.context_menu_selected_ix = Some(ix);
                                    cx.notify();
                                }
                            }))
                            .when(!disabled, |row| {
                                row.on_click(cx.listener(
                                    move |this, _e: &ClickEvent, window, cx| {
                                        this.context_menu_activate_action(
                                            action.clone(),
                                            window,
                                            cx,
                                        );
                                    },
                                ))
                            })
                            .into_any_element()
                        }
                    }
                }))
                .into_any_element(),
        )
    }

    pub(super) fn history_column_headers(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let (show_date, show_sha) = self.history_visible_columns();
        let col_date = self.history_col_date;
        let col_sha = self.history_col_sha;
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
                .w(px(HISTORY_COL_HANDLE_PX))
                .h_full()
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
            .child(resize_handle(
                "history_col_resize_branch",
                HistoryColResizeHandle::Branch,
            ))
            .child(
                div()
                    .w(self.history_col_graph)
                    .flex()
                    .justify_center()
                    .whitespace_nowrap()
                    .child("GRAPH"),
            )
            .child(resize_handle(
                "history_col_resize_graph",
                HistoryColResizeHandle::Graph,
            ))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .whitespace_nowrap()
                    .child("COMMIT MESSAGE"),
            );

        if show_date {
            header = header.child(resize_handle(
                "history_col_resize_message",
                HistoryColResizeHandle::Message,
            ));
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
            header = header.child(resize_handle(
                "history_col_resize_date",
                HistoryColResizeHandle::Date,
            ));
            header = header.child(
                div()
                    .w(col_sha)
                    .flex()
                    .justify_end()
                    .whitespace_nowrap()
                    .child("SHA"),
            );
        }

        header
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
            | PopoverKind::CreateBranch
            | PopoverKind::StashPrompt
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
                            menu = menu.child(zed::context_menu_label(theme, "Loading…"));
                        }
                        Loadable::Error(e) => {
                            menu = menu.child(zed::context_menu_label(theme, e.clone()));
                        }
                        Loadable::NotLoaded => {
                            menu = menu.child(zed::context_menu_label(theme, "Not loaded"));
                        }
                    }
                }

                zed::context_menu(
                    theme,
                    menu.child(zed::context_menu_separator(theme))
                        .child(zed::context_menu_header(theme, "Create branch"))
                        .child(
                            div()
                                .px_2()
                                .w_full()
                                .min_w(px(0.0))
                                .child(self.create_branch_input.clone()),
                        )
                        .child(
                            div().px_2().child(
                                zed::Button::new("create_branch_go", "Create")
                                    .style(zed::ButtonStyle::Filled)
                                    .on_click(theme, cx, |this, _e, _w, cx| {
                                        let name = this
                                            .create_branch_input
                                            .read_with(cx, |i, _| i.text().trim().to_string());
                                        if let Some(repo_id) = this.active_repo_id()
                                            && !name.is_empty()
                                        {
                                            this.store
                                                .dispatch(Msg::CreateBranch { repo_id, name });
                                        }
                                        this.popover = None;
                                        this.popover_anchor = None;
                                        cx.notify();
                                    }),
                            ),
                        ),
                )
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
                                        this.store.dispatch(Msg::CreateBranch { repo_id, name });
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
            PopoverKind::HistoryBranchFilter { repo_id } => self
                .context_menu_view(PopoverKind::HistoryBranchFilter { repo_id }, cx)
                .min_w(px(160.0))
                .max_w(px(220.0)),
            PopoverKind::PullPicker => self.context_menu_view(PopoverKind::PullPicker, cx),
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
            PopoverKind::AppMenu => div()
                .flex()
                .flex_col()
                .min_w(px(200.0))
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
                ),
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
