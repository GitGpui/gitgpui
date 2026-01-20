use super::*;

#[derive(Clone)]
enum ContextMenuAction {
    SelectDiff { repo_id: RepoId, target: DiffTarget },
    CheckoutCommit { repo_id: RepoId, commit_id: CommitId },
    CherryPickCommit { repo_id: RepoId, commit_id: CommitId },
    RevertCommit { repo_id: RepoId, commit_id: CommitId },
    CheckoutBranch { repo_id: RepoId, name: String },
    StagePath { repo_id: RepoId, path: std::path::PathBuf },
    UnstagePath { repo_id: RepoId, path: std::path::PathBuf },
    FetchAll { repo_id: RepoId },
    Pull { repo_id: RepoId, mode: PullMode },
    OpenPopover { kind: PopoverKind },
    CopyText { text: String },
}

#[derive(Clone)]
enum ContextMenuItem {
    Separator,
    Header(SharedString),
    Label(SharedString),
    Entry {
        label: SharedString,
        icon: Option<SharedString>,
        shortcut: Option<SharedString>,
        disabled: bool,
        action: ContextMenuAction,
    },
}

#[derive(Clone)]
struct ContextMenuModel {
    items: Vec<ContextMenuItem>,
}

impl ContextMenuModel {
    fn new(items: Vec<ContextMenuItem>) -> Self {
        Self { items }
    }

    fn is_selectable(&self, ix: usize) -> bool {
        matches!(
            self.items.get(ix),
            Some(ContextMenuItem::Entry { disabled, .. }) if !*disabled
        )
    }

    fn first_selectable(&self) -> Option<usize> {
        (0..self.items.len()).find(|&ix| self.is_selectable(ix))
    }

    fn last_selectable(&self) -> Option<usize> {
        (0..self.items.len()).rev().find(|&ix| self.is_selectable(ix))
    }

    fn next_selectable(&self, from: Option<usize>, dir: isize) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        let Some(mut ix) = from else {
            return if dir >= 0 {
                self.first_selectable()
            } else {
                self.last_selectable()
            };
        };

        let n = self.items.len() as isize;
        for _ in 0..self.items.len() {
            ix = ((ix as isize + dir).rem_euclid(n)) as usize;
            if self.is_selectable(ix) {
                return Some(ix);
            }
        }
        None
    }
}

struct HistoryColResizeDragGhost;

impl Render for HistoryColResizeDragGhost {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div().w(px(0.0)).h(px(0.0))
    }
}

impl GitGpuiView {
    pub(super) fn close_popover(&mut self, cx: &mut gpui::Context<Self>) {
        self.popover = None;
        self.popover_anchor = None;
        self.context_menu_selected_ix = None;
        cx.notify();
    }

    pub(super) fn open_popover_at(
        &mut self,
        kind: PopoverKind,
        anchor: Point<Pixels>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let is_context_menu = matches!(
            &kind,
            PopoverKind::PullPicker
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
            PopoverKind::StatusFileMenu { repo_id, area, path } => {
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
                self.store.dispatch(Msg::CheckoutCommit { repo_id, commit_id });
            }
            ContextMenuAction::CherryPickCommit { repo_id, commit_id } => {
                self.store.dispatch(Msg::CherryPickCommit { repo_id, commit_id });
            }
            ContextMenuAction::RevertCommit { repo_id, commit_id } => {
                self.store.dispatch(Msg::RevertCommit { repo_id, commit_id });
            }
            ContextMenuAction::CheckoutBranch { repo_id, name } => {
                self.store.dispatch(Msg::CheckoutBranch { repo_id, name });
                self.rebuild_diff_cache();
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

    fn context_menu_view(
        &mut self,
        kind: PopoverKind,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::Div {
        let theme = self.theme;
        let model = self.context_menu_model(&kind).unwrap_or_else(|| ContextMenuModel::new(vec![]));
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
                .on_mouse_down(MouseButton::Left, cx.listener(|this, _e: &MouseDownEvent, window, _cx| {
                    window.focus(&this.context_menu_focus_handle);
                }))
                .on_key_down(cx.listener(move |this, e: &gpui::KeyDownEvent, window, cx| {
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
                            let next = model_for_keys.next_selectable(this.context_menu_selected_ix, -1);
                            this.context_menu_selected_ix = next;
                            cx.notify();
                        }
                        "down" => {
                            let next = model_for_keys.next_selectable(this.context_menu_selected_ix, 1);
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
                                let hit = model_for_keys.items.iter().enumerate().find_map(|(ix, item)| {
                                    let ContextMenuItem::Entry { shortcut, disabled, .. } = item else {
                                        return None;
                                    };
                                    if *disabled {
                                        return None;
                                    }
                                    let shortcut = shortcut.as_ref()?.as_ref().to_ascii_uppercase();
                                    (shortcut == needle).then_some(ix)
                                });

                                if let Some(ix) = hit
                                    && let Some(ContextMenuItem::Entry { action, .. }) =
                                        model_for_keys.items.get(ix).cloned()
                                {
                                    this.context_menu_activate_action(action, window, cx);
                                }
                            }
                        }
                    }
                }))
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
                                row.on_click(cx.listener(move |this, _e: &ClickEvent, window, cx| {
                                    this.context_menu_activate_action(action.clone(), window, cx);
                                }))
                            })
                            .into_any_element()
                        }
                    }
                }))
                .into_any_element(),
        )
    }

    fn history_column_headers(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let (show_date, show_sha) = self.history_visible_columns();
        let col_date = self.history_col_date;
        let col_sha = self.history_col_sha;
        let resize_handle =
            |id: &'static str, handle: HistoryColResizeHandle| {
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
                    .on_drag_move(cx.listener(move |this, e: &gpui::DragMoveEvent<HistoryColResizeHandle>, _w, cx| {
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
                                this.history_col_date = (state.start_date + dx)
                                    .max(min_date)
                                    .min(max_date);
                                this.history_col_sha = (total - this.history_col_date).max(min_sha);
                            }
                        }
                        cx.notify();
                    }))
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
                    .whitespace_nowrap()
                    .child("Branch / Tag"),
            )
            .child(resize_handle("history_col_resize_branch", HistoryColResizeHandle::Branch))
            .child(
                div()
                    .w(self.history_col_graph)
                    .flex()
                    .justify_center()
                    .whitespace_nowrap()
                    .child("GRAPH"),
            )
            .child(resize_handle("history_col_resize_graph", HistoryColResizeHandle::Graph))
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
                    .child("COMMIT DATE / TIME"),
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

    pub(super) fn repo_tabs_bar(&mut self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let active = self.active_repo_id();
        let repos_len = self.state.repos.len();
        let active_ix = active.and_then(|id| self.state.repos.iter().position(|r| r.id == id));

        let mut bar = zed::TabBar::new("repo_tab_bar");
        for (ix, repo) in self.state.repos.iter().enumerate() {
            let repo_id = repo.id;
            let is_active = Some(repo_id) == active;
            let label: SharedString = repo
                .spec
                .workdir
                .file_name()
                .and_then(|s| s.to_str())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| repo.spec.workdir.display().to_string())
                .into();

            let position = if ix == 0 {
                zed::TabPosition::First
            } else if ix + 1 == repos_len {
                zed::TabPosition::Last
            } else {
                let ordering = match (is_active, active_ix) {
                    (true, _) => std::cmp::Ordering::Equal,
                    (false, Some(active_ix)) => ix.cmp(&active_ix),
                    (false, None) => std::cmp::Ordering::Equal,
                };
                zed::TabPosition::Middle(ordering)
            };

            let tab = zed::Tab::new(("repo_tab", repo_id.0))
                .selected(is_active)
                .position(position)
                .child(div().text_sm().line_clamp(1).child(label))
                .render(theme)
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.store.dispatch(Msg::SetActiveRepo { repo_id });
                    this.rebuild_diff_cache();
                    cx.notify();
                }));

            bar = bar.tab(tab);
        }

        let show_add_repo_tooltip = self.add_repo_button_hovered;
        bar.end_child(
            div()
                .id("add_repo_container")
                .relative()
                .h_full()
                .flex()
                .items_center()
                .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                    this.add_repo_button_hovered = *hovering;
                    cx.notify();
                }))
                .when(show_add_repo_tooltip, |this| {
                    this.child(
                        div()
                            .id("add_repo_tooltip")
                            .absolute()
                            .right(px(34.0))
                            .top_0()
                            .bottom_0()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .bg(theme.colors.surface_bg_elevated)
                                    .border_1()
                                    .border_color(theme.colors.border)
                                    .rounded(px(theme.radii.row))
                                    .shadow_sm()
                                    .text_xs()
                                    .text_color(theme.colors.text)
                                    .child("Add repository"),
                            ),
                    )
                })
                .child(
                    zed::Button::new("add_repo", "⨁")
                        .style(zed::ButtonStyle::Subtle)
                        .on_click(theme, cx, |this, _e, window, cx| {
                            this.prompt_open_repo(window, cx)
                        }),
                ),
        )
        .render(theme)
    }

    pub(super) fn open_repo_panel(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        if !self.open_repo_panel {
            return div();
        }

        div()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .bg(theme.colors.surface_bg)
            .border_1()
            .border_color(theme.colors.border)
            .rounded(px(theme.radii.panel))
            .shadow_sm()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.colors.text_muted)
                    .child("Path"),
            )
            .child(div().flex_1().child(self.open_repo_input.clone()))
            .child(
                zed::Button::new("open_repo_go", "Open")
                    .style(zed::ButtonStyle::Filled)
                    .on_click(theme, cx, |this, _e, _w, cx| {
                        let path = this
                            .open_repo_input
                            .read_with(cx, |input, _| input.text().trim().to_string());
                        if !path.is_empty() {
                            this.store.dispatch(Msg::OpenRepo(path.into()));
                            this.open_repo_panel = false;
                        }
                        cx.notify();
                    }),
            )
            .child(zed::Button::new("open_repo_cancel", "Cancel").on_click(
                theme,
                cx,
                |this, _e, _w, cx| {
                    this.open_repo_panel = false;
                    cx.notify();
                },
            ))
    }

    pub(super) fn action_bar(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let icon = |path: &'static str| {
            gpui::svg()
                .path(path)
                .w(px(14.0))
                .h(px(14.0))
                .text_color(theme.colors.text)
        };
        let count_badge = |count: usize| {
            div()
                .px(px(6.0))
                .py(px(1.0))
                .rounded(px(theme.radii.pill))
                .bg(with_alpha(
                    theme.colors.text_muted,
                    if theme.is_dark { 0.22 } else { 0.18 },
                ))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .child(count.to_string())
                .into_any_element()
        };

        let repo_title: SharedString = self
            .active_repo()
            .map(|r| r.spec.workdir.display().to_string().into())
            .unwrap_or_else(|| "No repository".into());

        let branch: SharedString = self
            .active_repo()
            .map(|r| match &r.head_branch {
                Loadable::Ready(name) => name.clone().into(),
                Loadable::Loading => "…".into(),
                Loadable::Error(_) => "error".into(),
                Loadable::NotLoaded => "—".into(),
            })
            .unwrap_or_else(|| "—".into());

        let (pull_count, push_count) = self
            .active_repo()
            .and_then(|r| match &r.upstream_divergence {
                Loadable::Ready(Some(d)) => Some((d.behind, d.ahead)),
                _ => None,
            })
            .unwrap_or((0, 0));

        let repo_picker = div()
            .id("repo_picker")
            .debug_selector(|| "repo_picker".to_string())
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .rounded(px(theme.radii.row))
            .hover(move |s| s.bg(theme.colors.hover))
            .active(move |s| s.bg(theme.colors.active))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child("Repository"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.colors.text_muted)
                    .line_clamp(1)
                    .child(repo_title),
            )
            .on_click(cx.listener(|this, e: &ClickEvent, window, cx| {
                let _ = this.ensure_repo_picker_search_input(window, cx);
                this.popover = Some(PopoverKind::RepoPicker);
                this.popover_anchor = Some(e.position());
                cx.notify();
            }));

        let branch_picker = div()
            .id("branch_picker")
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .rounded(px(theme.radii.row))
            .hover(move |s| s.bg(theme.colors.hover))
            .active(move |s| s.bg(theme.colors.active))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child("Branch"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.colors.text_muted)
                    .child(branch),
            )
            .on_click(cx.listener(|this, e: &ClickEvent, window, cx| {
                let _ = this.ensure_branch_picker_search_input(window, cx);
                this.popover = Some(PopoverKind::BranchPicker);
                this.popover_anchor = Some(e.position());
                cx.notify();
            }));

        let mut pull_main = zed::Button::new("pull_main", "Pull")
            .start_slot(icon("icons/arrow_down.svg"))
            .style(zed::ButtonStyle::Subtle);
        if pull_count > 0 {
            pull_main = pull_main.end_slot(count_badge(pull_count));
        }
        let pull_menu = zed::Button::new("pull_menu", "")
            .start_slot(icon("icons/chevron_down.svg"))
            .style(zed::ButtonStyle::Subtle);

        let pull = zed::SplitButton::new(
            pull_main.on_click(theme, cx, |this, _e, _w, cx| {
                if let Some(repo_id) = this.active_repo_id() {
                    this.store.dispatch(Msg::Pull {
                        repo_id,
                        mode: PullMode::Default,
                    });
                }
                cx.notify();
            }),
            pull_menu.on_click(theme, cx, |this, e, window, cx| {
                this.open_popover_at(PopoverKind::PullPicker, e.position(), window, cx);
            }),
        )
        .style(zed::SplitButtonStyle::Outlined)
        .render(theme);

        let mut push = zed::Button::new("push", "Push")
            .start_slot(icon("icons/arrow_up.svg"))
            .style(zed::ButtonStyle::Outlined);
        if push_count > 0 {
            push = push.end_slot(count_badge(push_count));
        }
        let push = push.on_click(theme, cx, |this, _e, _w, cx| {
            if let Some(repo_id) = this.active_repo_id() {
                this.store.dispatch(Msg::Push { repo_id });
            }
            cx.notify();
        });

        let stash = zed::Button::new("stash", "Stash")
            .start_slot(icon("icons/box.svg"))
            .style(zed::ButtonStyle::Outlined)
            .on_click(theme, cx, |this, _e, _w, cx| {
                if let Some(repo_id) = this.active_repo_id() {
                    this.store.dispatch(Msg::Stash {
                        repo_id,
                        message: "WIP".to_string(),
                        include_untracked: true,
                    });
                }
                cx.notify();
            });

        let create_branch = zed::Button::new("create_branch", "Branch")
            .start_slot(icon("icons/git_branch.svg"))
            .style(zed::ButtonStyle::Outlined)
            .on_click(theme, cx, |this, e, window, cx| {
                this.create_branch_input
                    .update(cx, |i, cx| i.set_text(String::new(), cx));
                let focus = this
                    .create_branch_input
                    .read_with(cx, |i, _| i.focus_handle());
                window.focus(&focus);
                this.open_popover_at(PopoverKind::CreateBranch, e.position(), window, cx);
            });

        div()
            .flex()
            .items_center()
            .justify_between()
            .px_2()
            .py_1()
            .bg(theme.colors.active_section)
            .border_b_1()
            .border_color(theme.colors.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .flex_1()
                    .child(repo_picker)
                    .child(branch_picker),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(pull)
                    .child(push)
                    .child(create_branch)
                    .child(stash),
            )
    }

    pub(super) fn popover_view(
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
            PopoverKind::PullPicker | PopoverKind::CreateBranch => Corner::TopRight,
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
                    .min_w(px(320.0))
                } else {
                    let mut menu = div().flex().flex_col().min_w(px(320.0));
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
                            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                    this.store.dispatch(Msg::SetActiveRepo { repo_id: id });
                                    this.popover = None;
                                    this.popover_anchor = None;
                                    this.rebuild_diff_cache();
                                    cx.notify();
                                })),
                        );
                    }
                    zed::context_menu(theme, menu)
                }
            }
            PopoverKind::BranchPicker => {
                let mut menu = div().flex().flex_col().min_w(px(260.0));

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
                                        .on_click(cx.listener(
                                            move |this, _e: &ClickEvent, _w, cx| {
                                                this.store.dispatch(Msg::CheckoutBranch {
                                                    repo_id,
                                                    name: name.clone(),
                                                });
                                                this.popover = None;
                                                this.popover_anchor = None;
                                                cx.notify();
                                            },
                                        )),
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
                        .child(div().px_2().child(
                            zed::Button::new("create_branch_go", "Create")
                                .style(zed::ButtonStyle::Filled)
                                .on_click(theme, cx, |this, _e, _w, cx| {
                                    let name = this
                                        .create_branch_input
                                        .read_with(cx, |i, _| i.text().trim().to_string());
                                    if let Some(repo_id) = this.active_repo_id() && !name.is_empty() {
                                        this.store.dispatch(Msg::CreateBranch { repo_id, name });
                                    }
                                    this.popover = None;
                                    this.popover_anchor = None;
                                    cx.notify();
                                }),
                        )),
                )
                .min_w(px(260.0))
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
            PopoverKind::HistoryBranchFilter { repo_id } => {
                let mut menu = div().flex().flex_col().min_w(px(260.0));

                menu = menu
                    .child(
                        div()
                            .id("history_scope_current_branch")
                            .px_2()
                            .py_1()
                            .hover(move |s| s.bg(theme.colors.hover))
                            .child("Current branch")
                            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.store.dispatch(Msg::SetHistoryScope {
                                    repo_id,
                                    scope: gitgpui_core::domain::LogScope::CurrentBranch,
                                });
                                this.popover = None;
                                this.popover_anchor = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("history_scope_all_branches")
                            .px_2()
                            .py_1()
                            .hover(move |s| s.bg(theme.colors.hover))
                            .child("All branches")
                            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.store.dispatch(Msg::SetHistoryScope {
                                    repo_id,
                                    scope: gitgpui_core::domain::LogScope::AllBranches,
                                });
                                this.popover = None;
                                this.popover_anchor = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("history_scope_close")
                            .px_2()
                            .py_1()
                            .hover(move |s| s.bg(theme.colors.hover))
                            .child("Close")
                            .on_click(close),
                    );

                menu
            }
            PopoverKind::PullPicker => {
                self.context_menu_view(PopoverKind::PullPicker, cx)
            }
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
                                } else if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
                                    && line.text.starts_with("diff --git ")
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
            PopoverKind::CommitModal { repo_id } => {
                let staged = self
                    .state
                    .repos
                    .iter()
                    .find(|r| r.id == repo_id)
                    .and_then(|r| match &r.status {
                        Loadable::Ready(s) => Some(s.staged.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();

                let staged_summary = format!("Staged files: {}", staged.len());
                let staged_list = if staged.is_empty() {
                    div()
                        .px_2()
                        .py_1()
                        .text_sm()
                        .text_color(theme.colors.text_muted)
                        .child("No staged changes.")
                        .into_any_element()
                } else {
                    let rows = staged
                        .into_iter()
                        .take(8)
                        .map(|f| {
                            let (label, color) = match f.kind {
                                FileStatusKind::Added => ("Added", theme.colors.success),
                                FileStatusKind::Modified => ("Modified", theme.colors.accent),
                                FileStatusKind::Deleted => ("Deleted", theme.colors.danger),
                                FileStatusKind::Renamed => ("Renamed", theme.colors.accent),
                                FileStatusKind::Untracked => ("Untracked", theme.colors.warning),
                                FileStatusKind::Conflicted => ("Conflicted", theme.colors.danger),
                            };

                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_2()
                                .py_1()
                                .child(zed::pill(theme, label, color))
                                .child(
                                    div()
                                        .text_sm()
                                        .line_clamp(1)
                                        .child(f.path.display().to_string()),
                                )
                        })
                        .collect::<Vec<_>>();
                    div().flex().flex_col().children(rows).into_any_element()
                };

                let input = self
                    .commit_modal_input
                    .clone()
                    .map(|i| i.into_any_element())
                    .unwrap_or_else(|| {
                        zed::empty_state(theme, "Commit", "Input not initialized.")
                            .into_any_element()
                    });

                div()
                    .flex()
                    .flex_col()
                    .min_w(px(520.0))
                    .max_w(px(760.0))
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .child("Commit"),
                            )
                            .child(
                                div()
                                    .id("commit_modal_close")
                                    .px_2()
                                    .py_1()
                                    .rounded(px(theme.radii.row))
                                    .hover(move |s| s.bg(theme.colors.hover))
                                    .child("✕")
                                    .on_click(close),
                            ),
                    )
                    .child(div().border_t_1().border_color(theme.colors.border))
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .child(staged_summary),
                    )
                    .child(staged_list)
                    .child(div().border_t_1().border_color(theme.colors.border))
                    .child(div().px_2().py_1().h(px(140.0)).child(input))
                    .child(div().border_t_1().border_color(theme.colors.border))
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                zed::Button::new("commit_modal_cancel", "Cancel")
                                    .style(zed::ButtonStyle::Outlined)
                                    .on_click(theme, cx, |this, _e, _w, cx| {
                                        this.popover = None;
                                        this.popover_anchor = None;
                                        cx.notify();
                                    }),
                            )
                            .child(
                                zed::Button::new("commit_modal_commit", "Commit")
                                    .style(zed::ButtonStyle::Filled)
                                    .on_click(theme, cx, move |this, _e, _w, cx| {
                                        let message = this
                                            .commit_modal_input
                                            .as_ref()
                                            .map(|i| {
                                                i.read_with(cx, |i, _| i.text().trim().to_string())
                                            })
                                            .unwrap_or_default();
                                        if !message.is_empty() {
                                            this.store.dispatch(Msg::Commit { repo_id, message });
                                            this.commit_message_input
                                                .update(cx, |i, cx| i.set_text(String::new(), cx));
                                            if let Some(input) = &this.commit_modal_input {
                                                input.update(cx, |i, cx| {
                                                    i.set_text(String::new(), cx)
                                                });
                                            }
                                            this.popover = None;
                                            this.popover_anchor = None;
                                        }
                                        cx.notify();
                                    }),
                            ),
                    )
            }
            PopoverKind::CommitMenu { repo_id, commit_id } => {
                self.context_menu_view(
                    PopoverKind::CommitMenu { repo_id, commit_id },
                    cx,
                )
            }
            PopoverKind::StatusFileMenu { repo_id, area, path } => self.context_menu_view(
                PopoverKind::StatusFileMenu { repo_id, area, path },
                cx,
            ),
            PopoverKind::BranchMenu { repo_id, section, name } => self.context_menu_view(
                PopoverKind::BranchMenu { repo_id, section, name },
                cx,
            ),
            PopoverKind::BranchSectionMenu { repo_id, section } => self.context_menu_view(
                PopoverKind::BranchSectionMenu { repo_id, section },
                cx,
            ),
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

    pub(super) fn sidebar(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let Some(repo) = self.active_repo() else {
            return div()
                .flex()
                .flex_col()
                .h_full()
                .min_h(px(0.0))
                .gap_2()
                .child(zed::panel(
                    theme,
                    "Branches",
                    None,
                    zed::empty_state(theme, "Branches", "No repository selected."),
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
            .gap_2()
            .child(zed::panel(theme, "Branches", None, panel_body).flex_1().min_h(px(0.0)))
    }

    pub(super) fn commit_details_view(&mut self, cx: &mut gpui::Context<Self>) -> AnyElement {
        let theme = self.theme;
        let repo = self.active_repo();

        if let Some(repo) = repo
            && let Some(selected_id) = repo.selected_commit.as_ref()
        {
            let header = div()
                .flex()
                .items_center()
                .justify_end()
                .child(
                    zed::Button::new("commit_details_close", "✕")
                        .style(zed::ButtonStyle::Transparent)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            if let Some(repo_id) = this.active_repo_id() {
                                this.store.dispatch(Msg::ClearCommitSelection { repo_id });
                            }
                            cx.notify();
                        }),
                );

            let body: AnyElement = match &repo.commit_details {
                    Loadable::Loading => {
                        zed::empty_state(theme, "Commit", "Loading…").into_any_element()
                    }
                    Loadable::Error(e) => {
                        zed::empty_state(theme, "Commit", e.clone()).into_any_element()
                    }
                    Loadable::NotLoaded => {
                        zed::empty_state(theme, "Commit", "Select a commit in History.")
                            .into_any_element()
                    }
	                    Loadable::Ready(details) => {
	                        if &details.id != selected_id {
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

                                let list = uniform_list(
                                    ("commit_files", commit_key),
                                    details.files.len(),
                                    cx.processor(Self::render_commit_file_rows),
                                )
                                .h_full()
                                .min_h(px(0.0))
                                .track_scroll(self.commit_files_scroll.clone());

                                let scroll_handle =
                                    self.commit_files_scroll.0.borrow().base_handle.clone();

                                div()
                                    .id(("commit_files_scroll_container", commit_key))
                                    .relative()
                                    .h(px(240.0))
                                    .min_h(px(0.0))
                                    .child(list)
                                    .child(
                                        zed::Scrollbar::new(
                                            ("commit_files_scrollbar", commit_key),
                                            scroll_handle,
                                        )
                                        .render(theme),
                                    )
                                    .into_any_element()
                            };

	                            let needs_update = self
	                                .commit_details_message_input
	                                .read(cx)
	                                .text()
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
	                                        .flex()
	                                        .flex_col()
	                                        .gap_1()
	                                        .child(
	                                            div()
	                                                .text_sm()
	                                                .text_color(theme.colors.text_muted)
	                                                .child("Commit message"),
	                                        )
	                                        .child(
	                                            div()
	                                                .min_w(px(0.0))
	                                                .child(self.commit_details_message_input.clone()),
	                                        ),
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

            let content = div()
                .flex()
                .flex_col()
                .gap_2()
	                .child(zed::panel(
	                    theme,
	                    "",
	                    None,
	                    div().flex().flex_col().gap_2().child(header).child(body),
	                ));

            return div()
                .id("commit_details_scroll")
                .relative()
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .track_scroll(&self.commit_scroll)
                .child(content)
                .child(
                    zed::Scrollbar::new("commit_details_scrollbar", self.commit_scroll.clone())
                        .render(theme),
                )
                .into_any_element();
        }

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
            });

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
            });

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
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .child(label),
                )
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

        let staged_body = div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .min_h(px(0.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(staged_list),
            )
            .child(self.commit_box(cx).flex_none())
            .into_any_element();

        let unstaged_section = div()
            .flex()
            .flex_col()
            .flex_none()
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
                    .when(unstaged_count > 0, |d| d.h(px(180.0)))
                    .min_h(px(0.0))
                    .p_2()
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
                    .p_2()
                    .child(staged_body),
            );

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0))
            .h_full()
	            .child(if repo_id.is_some() {
	                zed::panel(
	                    theme,
	                    "",
	                    None,
	                    div()
	                        .flex()
	                        .flex_col()
	                        .flex_1()
	                        .min_h(px(0.0))
	                        .child(unstaged_section)
	                        .child(div().border_t_1().border_color(theme.colors.border))
	                        .child(staged_section),
	                )
	                .flex_1()
	                .min_h(px(0.0))
	            } else {
	                zed::panel(
	                    theme,
	                    "",
	                    None,
	                    zed::empty_state(theme, "Changes", "No repository selected."),
	                )
	            })
            .into_any_element()
    }

    pub(super) fn status_list(
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
                let list = uniform_list("unstaged", count, cx.processor(Self::render_unstaged_rows))
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

    pub(super) fn commit_box(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
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
                            .on_click(theme, cx, |this, e, window, cx| {
                                let message = this
                                    .commit_message_input
                                    .read_with(cx, |i, _| i.text().to_string());
                                let input = this.ensure_commit_modal_input(window, cx);
                                input.update(cx, |i, cx| i.set_text(message, cx));
                                if let Some(repo_id) = this.active_repo_id() {
                                    this.popover = Some(PopoverKind::CommitModal { repo_id });
                                    this.popover_anchor = Some(e.position());
                                }
                                cx.notify();
                            }),
                    ),
            )
    }

    pub(super) fn history_view(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;

        let tabs = {
            let tab = self.history_tab;
            let mut bar = zed::TabBar::new("history_tab_bar");
            for (ix, (label, value)) in [
                ("History", HistoryTab::Log),
                ("Stash", HistoryTab::Stash),
                ("Reflog", HistoryTab::Reflog),
            ]
            .into_iter()
            .enumerate()
            {
                let selected = tab == value;
                let position = if ix == 0 {
                    zed::TabPosition::First
                } else if ix == 2 {
                    zed::TabPosition::Last
                } else {
                    zed::TabPosition::Middle(std::cmp::Ordering::Equal)
                };
                let value_for_click = value;
                let t = zed::Tab::new(("history_tab", ix))
                    .selected(selected)
                    .position(position)
                    .child(div().text_sm().child(label))
                    .render(theme)
                    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                        this.history_tab = value_for_click;
                        cx.notify();
                    }));
                bar = bar.tab(t);
            }
            bar.render(theme)
        };

        let (_title, body): (SharedString, AnyElement) = match self.history_tab {
            HistoryTab::Log => {
                self.update_history_search_debounce(cx);
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

                let filter = div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .child(self.history_search_input.clone()),
                    )
                    .child({
                        let current: SharedString = repo
                            .map(|r| match r.history_scope {
                                gitgpui_core::domain::LogScope::CurrentBranch => {
                                    "Current branch".to_string()
                                }
                                gitgpui_core::domain::LogScope::AllBranches => {
                                    "All branches".to_string()
                                }
                            })
                            .unwrap_or_else(|| "Current branch".to_string())
                            .into();
                        div()
                            .id("history_branch_filter")
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .py_1()
                            .bg(theme.colors.surface_bg)
                            .border_1()
                            .border_color(theme.colors.border)
                            .rounded(px(theme.radii.row))
                            .hover(move |s| s.bg(theme.colors.hover))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.text_muted)
                                    .child("Branch"),
                            )
                            .child(div().text_sm().child(current))
                            .on_click(cx.listener(|this, e: &ClickEvent, _window, cx| {
                                if let Some(repo_id) = this.active_repo_id() {
                                    this.popover =
                                        Some(PopoverKind::HistoryBranchFilter { repo_id });
                                    this.popover_anchor = Some(e.position());
                                    cx.notify();
                                }
                            }))
                    });

                let body: AnyElement = if count == 0 {
                    match repo.map(|r| &r.log) {
                        None => {
                            zed::empty_state(theme, "History", "No repository.").into_any_element()
                        }
                        Some(Loadable::Loading) => {
                            zed::empty_state(theme, "History", "Loading…").into_any_element()
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
                    let scroll_handle = self.history_scroll.0.borrow().base_handle.clone();
                    div()
                        .id("history_main_scroll_container")
                        .relative()
                        .h_full()
                        .child(list)
                        .child(
                            zed::Scrollbar::new("history_main_scrollbar", scroll_handle)
                                .render(theme),
                        )
                        .into_any_element()
                };

                let table = div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .h_full()
                    .child(filter)
                    .child(self.history_column_headers(cx))
                    .child(div().flex_1().child(body));

                ("History".into(), table.into_any_element())
            }
            HistoryTab::Stash => {
                let repo = self.active_repo();
                let count = repo
                    .and_then(|r| match &r.stashes {
                        Loadable::Ready(v) => Some(v.len()),
                        _ => None,
                    })
                    .unwrap_or(0);

                let body: AnyElement = if count == 0 {
                    match repo.map(|r| &r.stashes) {
                        None => {
                            zed::empty_state(theme, "Stash", "No repository.").into_any_element()
                        }
                        Some(Loadable::Loading) => {
                            zed::empty_state(theme, "Stash", "Loading…").into_any_element()
                        }
                        Some(Loadable::Error(e)) => {
                            zed::empty_state(theme, "Stash", e.clone()).into_any_element()
                        }
                        Some(Loadable::NotLoaded) | Some(Loadable::Ready(_)) => {
                            zed::empty_state(theme, "Stash", "No stashes.").into_any_element()
                        }
                    }
                } else {
                    let list =
                        uniform_list("stash_main", count, cx.processor(Self::render_stash_rows))
                            .h_full()
                            .track_scroll(self.stashes_scroll.clone());
                    let scroll_handle = self.stashes_scroll.0.borrow().base_handle.clone();
                    div()
                        .id("stash_main_scroll_container")
                        .relative()
                        .h_full()
                        .child(list)
                        .child(
                            zed::Scrollbar::new("stash_main_scrollbar", scroll_handle)
                                .render(theme),
                        )
                        .into_any_element()
                };

                ("Stash".into(), body)
            }
            HistoryTab::Reflog => {
                let repo = self.active_repo();
                let count = repo
                    .and_then(|r| match &r.reflog {
                        Loadable::Ready(v) => Some(v.len()),
                        _ => None,
                    })
                    .unwrap_or(0);

                let body: AnyElement = if count == 0 {
                    match repo.map(|r| &r.reflog) {
                        None => {
                            zed::empty_state(theme, "Reflog", "No repository.").into_any_element()
                        }
                        Some(Loadable::Loading) => {
                            zed::empty_state(theme, "Reflog", "Loading…").into_any_element()
                        }
                        Some(Loadable::Error(e)) => {
                            zed::empty_state(theme, "Reflog", e.clone()).into_any_element()
                        }
                        Some(Loadable::NotLoaded) | Some(Loadable::Ready(_)) => {
                            zed::empty_state(theme, "Reflog", "No reflog.").into_any_element()
                        }
                    }
                } else {
                    let list =
                        uniform_list("reflog_main", count, cx.processor(Self::render_reflog_rows))
                            .h_full()
                            .track_scroll(self.reflog_scroll.clone());
                    let scroll_handle = self.reflog_scroll.0.borrow().base_handle.clone();
                    div()
                        .id("reflog_main_scroll_container")
                        .relative()
                        .h_full()
                        .child(list)
                        .child(
                            zed::Scrollbar::new("reflog_main_scrollbar", scroll_handle)
                                .render(theme),
                        )
                        .into_any_element()
                };

                ("Reflog".into(), body)
            }
        };

        div()
            .flex()
            .flex_col()
            .gap_2()
            .flex_1()
            .w_full()
            .h_full()
            .min_h(px(0.0))
	            .child(
	                zed::panel(
	                    theme,
	                    "",
	                    None,
	                    div()
	                        .flex()
	                        .flex_col()
	                        .gap_2()
	                        .h_full()
	                        .child(tabs)
	                        .child(div().flex_1().child(body)),
	                )
	                .flex_1(),
	            )
    }

    pub(super) fn diff_view(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let repo_id = self.active_repo_id();

        // Intentionally no outer panel header; keep diff controls in the inner header.

        let title = self
            .active_repo()
            .and_then(|r| r.diff_target.as_ref())
            .map(|t| match t {
                DiffTarget::WorkingTree { path, area } => format!(
                    "{}: {}",
                    if *area == DiffArea::Staged {
                        "staged"
                    } else {
                        "unstaged"
                    },
                    path.display()
                ),
                DiffTarget::Commit { commit_id, path } => {
                    let sha = commit_id.as_ref();
                    let short = sha.get(0..8).unwrap_or(sha);
                    match path {
                        Some(path) => format!("{short}: {}", path.display()),
                        None => format!("{short}: full diff"),
                    }
                }
            })
            .unwrap_or_else(|| "Select a file to view diff".to_string());

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

        let mut controls = div().flex().items_center().gap_1();
        if !is_file_preview {
            let nav_entries = self.diff_nav_entries();
            let current_nav_ix = self.diff_selection_anchor.unwrap_or(0);
            let can_nav_prev =
                Self::diff_nav_prev_target(&nav_entries, current_nav_ix).is_some();
            let can_nav_next =
                Self::diff_nav_next_target(&nav_entries, current_nav_ix).is_some();

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
                            this.diff_text_segments_cache_query.clear();
                            this.diff_text_segments_cache.clear();
                            cx.notify();
                        }),
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
                            this.diff_text_segments_cache_query.clear();
                            this.diff_text_segments_cache.clear();
                            cx.notify();
                        }),
                )
                .child(
                    zed::Button::new("diff_raw", "Raw")
                        .style(if self.diff_raw_mode {
                            zed::ButtonStyle::Filled
                        } else {
                            zed::ButtonStyle::Outlined
                        })
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.diff_raw_mode = !this.diff_raw_mode;
                            cx.notify();
                        }),
                )
                .child(
                    zed::Button::new("diff_prev_hunk", "Prev")
                        .style(zed::ButtonStyle::Outlined)
                        .disabled(!can_nav_prev)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.diff_jump_prev();
                            cx.notify();
                        }),
                )
                .child(
                    zed::Button::new("diff_next_hunk", "Next")
                        .style(zed::ButtonStyle::Outlined)
                        .disabled(!can_nav_next)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.diff_jump_next();
                            cx.notify();
                        }),
                )
                .when(!wants_file_diff, |controls| {
                    controls.child(
                        zed::Button::new("diff_hunks", "Hunks…")
                            .style(zed::ButtonStyle::Outlined)
                            .on_click(theme, cx, |this, e, window, cx| {
                                let _ = this.ensure_diff_hunk_picker_search_input(window, cx);
                                this.popover = Some(PopoverKind::DiffHunks);
                                this.popover_anchor = Some(e.position());
                                cx.notify();
                            }),
                    )
                })
                ;
        }

        if let Some(repo_id) = repo_id {
            controls = controls.child(
                zed::Button::new("diff_close", "✕")
                    .style(zed::ButtonStyle::Transparent)
                    .on_click(theme, cx, move |this, _e, _w, cx| {
                        this.store.dispatch(Msg::ClearDiffSelection { repo_id });
                        cx.notify();
                    }),
            );
        }

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w(px(0.0))
                    .child(div().text_sm().font_weight(FontWeight::BOLD).child(title))
                    .when(!is_file_preview, |this| {
                        this.child(div().w(px(220.0)).child(self.diff_search_input.clone()))
                    }),
            )
            .child(controls);

        let body: AnyElement = if is_file_preview {
            if added_preview_path.is_some() {
                self.try_populate_worktree_preview_from_diff_file();
            }
            match &self.worktree_preview {
                Loadable::NotLoaded | Loadable::Loading => {
                    zed::empty_state(theme, "File", "Loading…").into_any_element()
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
                        let text = lines.join("\n");
                        self.diff_raw_input.update(cx, |input, cx| {
                            input.set_theme(theme, cx);
                            input.set_text(text, cx);
                            input.set_read_only(true, cx);
                        });
                        let scroll_handle = self.worktree_preview_scroll.0.borrow().base_handle.clone();
                        div()
                            .id("worktree_preview_scroll_container")
                            .relative()
                            .h_full()
                            .min_h(px(0.0))
                            .overflow_y_scroll()
                            .track_scroll(&scroll_handle)
                            .child(div().font_family("monospace").child(self.diff_raw_input.clone()))
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
                Loadable::Loading => zed::empty_state(theme, "Diff", "Loading…").into_any_element(),
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
	                    if self.diff_raw_mode {
	                        let text = diff
	                            .lines
	                            .iter()
	                            .map(|l| l.text.as_str())
	                            .collect::<Vec<_>>()
	                            .join("\n");
	                        self.diff_raw_input.update(cx, |input, cx| {
	                            input.set_theme(theme, cx);
	                            input.set_text(text, cx);
	                            input.set_read_only(true, cx);
	                        });
	                        div()
	                            .id("diff_raw_scroll")
	                            .font_family("monospace")
	                            .flex()
	                            .flex_col()
	                            .flex_1()
	                            .min_h(px(0.0))
	                            .overflow_y_scroll()
	                            .child(self.diff_raw_input.clone())
	                            .into_any_element()
	                    } else if wants_file_diff {
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
                            Loadable::Ready(file) => {
                                DiffFileState::Ready { has_file: file.is_some() }
                            }
                        };

                        self.ensure_file_diff_cache();
                        match diff_file_state {
                            DiffFileState::NotLoaded => {
                                zed::empty_state(theme, "Diff", "Select a file.").into_any_element()
                            }
                            DiffFileState::Loading => {
                                zed::empty_state(theme, "Diff", "Loading…").into_any_element()
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
                                    zed::empty_state(theme, "Diff", "No file contents available.")
                                        .into_any_element()
                                } else {
                                    self.update_diff_search_debounce(cx);
                                    self.ensure_diff_visible_indices();
                                    self.maybe_autoscroll_diff_to_first_change();

                                    let total_len = match self.diff_view {
                                        DiffViewMode::Inline => self.file_diff_inline_cache.len(),
                                        DiffViewMode::Split => self.file_diff_cache_rows.len(),
                                    };
                                    if total_len == 0 {
                                        zed::empty_state(theme, "Diff", "Empty file.").into_any_element()
                                    } else if !self.diff_visible_query.is_empty()
                                        && self.diff_query_match_count == 0
                                    {
                                        zed::empty_state(theme, "Diff", "No matches.").into_any_element()
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

                        self.update_diff_search_debounce(cx);
                        self.ensure_diff_visible_indices();
                        self.maybe_autoscroll_diff_to_first_change();
                        if self.diff_cache.is_empty() {
                            zed::empty_state(theme, "Diff", "No differences.").into_any_element()
                        } else if !self.diff_visible_query.is_empty()
                            && self.diff_query_match_count == 0
                        {
                            zed::empty_state(theme, "Diff", "No matches.").into_any_element()
                        } else if self.diff_visible_indices.is_empty() {
                            zed::empty_state(theme, "Diff", "Nothing to render.").into_any_element()
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
                                        .child(list)
                                        .child(
                                            zed::Scrollbar::new("diff_scrollbar", scroll_handle)
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
                                                .child(div().w(px(1.0)).h_full().bg(theme.colors.border))
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w(px(0.0))
                                                        .h_full()
                                                        .child(right),
                                                ),
                                        )
                                        .child(
                                            zed::Scrollbar::new("diff_scrollbar", scroll_handle)
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

        self.diff_text_hitboxes.clear();

        div()
            .flex()
            .flex_col()
            .gap_2()
            .flex_1()
            .w_full()
            .h_full()
            .min_h(px(0.0))
	            .child(
	                zed::panel(
	                    theme,
	                    "",
	                    None,
	                    div()
	                        .flex()
	                        .flex_col()
	                        .gap_2()
	                        .track_focus(&self.diff_panel_focus_handle)
	                        .on_mouse_down(
	                            MouseButton::Left,
	                            cx.listener(|this, _e: &MouseDownEvent, window, _cx| {
	                                window.focus(&this.diff_panel_focus_handle);
	                            }),
	                        )
	                        .on_key_down(cx.listener(|this, e: &gpui::KeyDownEvent, window, cx| {
	                            let is_file_preview = this.untracked_worktree_preview_path().is_some()
	                                || this.added_file_preview_abs_path().is_some();
	                            if is_file_preview {
	                                return;
	                            }

                            let key = e.keystroke.key.as_str();
                            let mods = e.keystroke.modifiers;

                            let mut handled = false;

                            let copy_target_is_focused = this
                                .diff_search_input
                                .read(cx)
                                .focus_handle()
                                .is_focused(window)
                                || this
                                    .diff_raw_input
                                    .read(cx)
                                    .focus_handle()
                                    .is_focused(window);

                            if mods.alt && !mods.control && !mods.platform && !mods.function {
                                match key {
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
	                        .h_full()
	                        .child(header)
	                        .child(
	                            div()
	                                .flex_1()
	                                .min_h(px(120.0))
	                                .w_full()
	                                .h_full()
	                                .child(body),
	                        )
                            .child(DiffTextSelectionTracker { view: cx.entity() }),
	                )
	                .flex_1(),
	            )
    }
}
