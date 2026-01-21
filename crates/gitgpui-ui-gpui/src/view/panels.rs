use super::*;

const COMMIT_DETAILS_FILES_RENDER_LIMIT: usize = 500;

#[derive(Clone)]
enum ContextMenuAction {
    SelectDiff { repo_id: RepoId, target: DiffTarget },
    CheckoutCommit { repo_id: RepoId, commit_id: CommitId },
    CherryPickCommit { repo_id: RepoId, commit_id: CommitId },
    RevertCommit { repo_id: RepoId, commit_id: CommitId },
    CheckoutBranch { repo_id: RepoId, name: String },
    SetHistoryScope {
        repo_id: RepoId,
        scope: gitgpui_core::domain::LogScope,
    },
    StagePath { repo_id: RepoId, path: std::path::PathBuf },
    UnstagePath { repo_id: RepoId, path: std::path::PathBuf },
    FetchAll { repo_id: RepoId },
    Pull { repo_id: RepoId, mode: PullMode },
    PullBranch {
        repo_id: RepoId,
        remote: String,
        branch: String,
    },
    MergeRef {
        repo_id: RepoId,
        reference: String,
    },
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
        let scope_label: SharedString = self
            .active_repo()
            .map(|r| match r.history_scope {
                gitgpui_core::domain::LogScope::CurrentBranch => "Current branch".to_string(),
                gitgpui_core::domain::LogScope::AllBranches => "All branches".to_string(),
            })
            .unwrap_or_else(|| "Current branch".to_string())
            .into();
        let scope_repo_id = self.active_repo_id();

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

    pub(super) fn repo_tabs_bar(&mut self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let active = self.active_repo_id();
        let repos_len = self.state.repos.len();
        let active_ix = active.and_then(|id| self.state.repos.iter().position(|r| r.id == id));

        let mut bar = zed::TabBar::new("repo_tab_bar");
        for (ix, repo) in self.state.repos.iter().enumerate() {
            let repo_id = repo.id;
            let is_active = Some(repo_id) == active;
            let show_close = self.hovered_repo_tab == Some(repo_id);
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

            let tooltip: SharedString = repo.spec.workdir.display().to_string().into();
            let close_tooltip: SharedString = "Close repository".into();
            let close_button = div()
                .id(("repo_tab_close", repo_id.0))
                .flex()
                .items_center()
                .justify_center()
                .size(px(14.0))
                .rounded(px(theme.radii.row))
                .text_xs()
                .text_color(theme.colors.text_muted)
                .cursor_pointer()
                .hover(move |s| s.bg(theme.colors.hover).text_color(theme.colors.text))
                .active(move |s| s.bg(theme.colors.active).text_color(theme.colors.text))
                .child("✕")
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    cx.stop_propagation();
                    this.hovered_repo_tab = None;
                    this.store.dispatch(Msg::CloseRepo { repo_id });
                    cx.notify();
                }))
                .on_hover(cx.listener({
                    let tooltip = tooltip.clone();
                    let close_tooltip = close_tooltip.clone();
                    move |this, hovering: &bool, _w, cx| {
                        if *hovering {
                            this.tooltip_text = Some(close_tooltip.clone());
                        } else if this.tooltip_text.as_ref() == Some(&close_tooltip) {
                            if this.hovered_repo_tab == Some(repo_id) {
                                this.tooltip_text = Some(tooltip.clone());
                            } else {
                                this.tooltip_text = None;
                            }
                        }
                        cx.notify();
                    }
                }));

            let mut tab = zed::Tab::new(("repo_tab", repo_id.0))
                .selected(is_active)
                .position(position);
            if show_close {
                tab = tab.end_slot(close_button);
            }

            let tab = tab
                .child(div().text_sm().line_clamp(1).child(label))
                .render(theme)
                .on_hover(cx.listener({
                    move |this, hovering: &bool, _w, cx| {
                        if *hovering {
                            this.hovered_repo_tab = Some(repo_id);
                            this.tooltip_text = Some(tooltip.clone());
                        } else {
                            if this.hovered_repo_tab == Some(repo_id) {
                                this.hovered_repo_tab = None;
                            }
                            if this.tooltip_text.as_ref() == Some(&tooltip)
                                || this.tooltip_text.as_ref() == Some(&close_tooltip)
                            {
                                this.tooltip_text = None;
                            }
                        }
                        cx.notify();
                    }
                }))
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.store.dispatch(Msg::SetActiveRepo { repo_id });
                    this.rebuild_diff_cache();
                    cx.notify();
                }));

            bar = bar.tab(tab);
        }

        bar.end_child(
            div()
                .id("add_repo_container")
                .relative()
                .h_full()
                .flex()
                .items_center()
                .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                    let text: SharedString = "Add repository".into();
                    if *hovering {
                        this.tooltip_text = Some(text);
                    } else if this.tooltip_text.as_ref() == Some(&text) {
                        this.tooltip_text = None;
                    }
                    cx.notify();
                }))
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
        let hover_bg = with_alpha(theme.colors.text, if theme.is_dark { 0.06 } else { 0.04 });
        let active_bg = with_alpha(theme.colors.text, if theme.is_dark { 0.10 } else { 0.07 });
        let icon = |path: &'static str, color: gpui::Rgba| {
            gpui::svg()
                .path(path)
                .w(px(14.0))
                .h(px(14.0))
                .text_color(color)
        };
        let count_badge = |count: usize, color: gpui::Rgba| {
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(color)
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

        let can_stash = self
            .active_repo()
            .and_then(|r| match &r.status {
                Loadable::Ready(s) => Some(!s.staged.is_empty() || !s.unstaged.is_empty()),
                _ => None,
            })
            .unwrap_or(false);

        let repo_picker = div()
            .id("repo_picker")
            .debug_selector(|| "repo_picker".to_string())
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .rounded(px(theme.radii.row))
            .hover(move |s| s.bg(hover_bg))
            .active(move |s| s.bg(active_bg))
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
            }))
            .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                let text: SharedString = "Select repository".into();
                if *hovering {
                    this.tooltip_text = Some(text);
                } else if this.tooltip_text.as_ref() == Some(&text) {
                    this.tooltip_text = None;
                }
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
            .hover(move |s| s.bg(hover_bg))
            .active(move |s| s.bg(active_bg))
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
            }))
            .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                let text: SharedString = "Select branch".into();
                if *hovering {
                    this.tooltip_text = Some(text);
                } else if this.tooltip_text.as_ref() == Some(&text) {
                    this.tooltip_text = None;
                }
                cx.notify();
            }));

        let pull_color = if pull_count > 0 {
            theme.colors.warning
        } else {
            theme.colors.text
        };
        let mut pull_main = zed::Button::new("pull_main", "Pull")
            .start_slot(icon("icons/arrow_down.svg", pull_color))
            .style(zed::ButtonStyle::Subtle);
        if pull_count > 0 {
            pull_main = pull_main.end_slot(count_badge(pull_count, pull_color));
        }
        let pull_menu = zed::Button::new("pull_menu", "")
            .start_slot(icon("icons/chevron_down.svg", theme.colors.text))
            .style(zed::ButtonStyle::Subtle);

        let pull = div()
            .id("pull")
            .child(
                zed::SplitButton::new(
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
                .render(theme),
            )
            .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                let text: SharedString = format!("Pull ({pull_count} behind)").into();
                if *hovering {
                    this.tooltip_text = Some(text);
                } else if this.tooltip_text.as_ref() == Some(&text) {
                    this.tooltip_text = None;
                }
                cx.notify();
            }));

        let push_color = if push_count > 0 {
            theme.colors.success
        } else {
            theme.colors.text
        };
        let mut push = zed::Button::new("push", "Push")
            .start_slot(icon("icons/arrow_up.svg", push_color))
            .style(zed::ButtonStyle::Outlined);
        if push_count > 0 {
            push = push.end_slot(count_badge(push_count, push_color));
        }
        let push = push.on_click(theme, cx, |this, _e, _w, cx| {
            if let Some(repo_id) = this.active_repo_id() {
                this.store.dispatch(Msg::Push { repo_id });
            }
            cx.notify();
        })
        .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
            let text: SharedString = format!("Push ({push_count} ahead)").into();
            if *hovering {
                this.tooltip_text = Some(text);
            } else if this.tooltip_text.as_ref() == Some(&text) {
                this.tooltip_text = None;
            }
            cx.notify();
        }));

        let stash = zed::Button::new("stash", "Stash")
            .start_slot(icon("icons/box.svg", theme.colors.text))
            .style(zed::ButtonStyle::Outlined)
            .disabled(!can_stash)
            .on_click(theme, cx, |this, e, window, cx| {
                this.stash_message_input
                    .update(cx, |i, cx| i.set_text(String::new(), cx));
                let focus = this
                    .stash_message_input
                    .read_with(cx, |i, _| i.focus_handle());
                window.focus(&focus);
                this.open_popover_at(PopoverKind::StashPrompt, e.position(), window, cx);
            })
            .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                let text: SharedString = if can_stash {
                    "Create stash…".into()
                } else {
                    "No changes to stash".into()
                };
                if *hovering {
                    this.tooltip_text = Some(text);
                } else if this.tooltip_text.as_ref() == Some(&text) {
                    this.tooltip_text = None;
                }
                cx.notify();
            }));

        let create_branch = zed::Button::new("create_branch", "Branch")
            .start_slot(icon("icons/git_branch.svg", theme.colors.text))
            .style(zed::ButtonStyle::Outlined)
            .on_click(theme, cx, |this, e, window, cx| {
                this.create_branch_input
                    .update(cx, |i, cx| i.set_text(String::new(), cx));
                let focus = this
                    .create_branch_input
                    .read_with(cx, |i, _| i.focus_handle());
                window.focus(&focus);
                this.open_popover_at(PopoverKind::CreateBranch, e.position(), window, cx);
            })
            .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                let text: SharedString = "Create branch…".into();
                if *hovering {
                    this.tooltip_text = Some(text);
                } else if this.tooltip_text.as_ref() == Some(&text) {
                    this.tooltip_text = None;
                }
                cx.notify();
            }));

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
                    let mut menu = div()
                        .flex()
                        .flex_col()
                        .min_w(px(260.0))
                        .max_w(px(420.0));
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
                let mut menu = div()
                    .flex()
                    .flex_col()
                    .min_w(px(240.0))
                    .max_w(px(420.0));

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
            PopoverKind::HistoryBranchFilter { repo_id } => {
                self.context_menu_view(PopoverKind::HistoryBranchFilter { repo_id }, cx)
                    .min_w(px(160.0))
                    .max_w(px(220.0))
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

    pub(super) fn commit_details_view(&mut self, cx: &mut gpui::Context<Self>) -> AnyElement {
        let theme = self.theme;
        let repo = self.active_repo();

        if let Some(repo) = repo
            && let Some(selected_id) = repo.selected_commit.as_ref()
        {
            let show_delayed_loading = self
                .commit_details_delay
                .as_ref()
                .is_some_and(|s| s.repo_id == repo.id && &s.commit_id == selected_id && s.show_loading);
            let repo_id = repo.id;

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

            let body: AnyElement = match &repo.commit_details {
                    Loadable::Loading => {
                        if show_delayed_loading {
                            zed::empty_state(theme, "Commit", "Loading…").into_any_element()
                        } else {
                            div().into_any_element()
                        }
                    }
                    Loadable::Error(e) => {
                        zed::empty_state(theme, "Commit", e.clone()).into_any_element()
                    }
                    Loadable::NotLoaded => {
                        if show_delayed_loading {
                            zed::empty_state(theme, "Commit", "Loading…").into_any_element()
                        } else {
                            div().into_any_element()
                        }
                    }
	                    Loadable::Ready(details) => {
	                        if &details.id != selected_id {
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

                                    let total_files = details.files.len();
                                    let shown_files =
                                        total_files.min(COMMIT_DETAILS_FILES_RENDER_LIMIT);

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
                                                        "Showing {shown_files} of {total_files} files",
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
                                            let commit_id = details.id.clone();
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
                                            let selected = repo.diff_target.as_ref().is_some_and(|t| match t {
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
	                                .child(div().w_full().min_w(px(0.0)).child(
	                                    self.commit_details_message_input.clone(),
	                                ))
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
                                    let shown_files =
                                        total_files.min(COMMIT_DETAILS_FILES_RENDER_LIMIT);

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
                                                        "Showing {shown_files} of {total_files} files",
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
                                            let commit_id = details.id.clone();
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
                                            let selected = repo.diff_target.as_ref().is_some_and(|t| match t {
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
	                                .child(div().w_full().min_w(px(0.0)).child(
	                                    self.commit_details_message_input.clone(),
	                                ))
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
                    let body_container = div()
                        .id("commit_details_body_container")
                        .relative()
                        .flex_1()
                        .h_full()
                        .min_h(px(0.0))
                        .overflow_y_scroll()
                        .track_scroll(&self.commit_scroll)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .p_2()
                                .w_full()
                                .child(body),
                        )
                        .child(
                            zed::Scrollbar::new(
                                "commit_details_scrollbar",
                                self.commit_scroll.clone(),
                            )
                            .render(theme),
                        );
                    body_container
                })
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

    pub(super) fn history_view(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
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
                    zed::empty_state(theme, "History", "Loading…").into_any_element()
                }
                Some(Loadable::Error(e)) => zed::empty_state(theme, "History", e.clone()).into_any_element(),
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
            .child(self.history_column_headers(cx).bg(bg).border_b_1().border_color(theme.colors.border))
            .child(div().flex_1().min_h(px(0.0)).child(body))
    }

    pub(super) fn diff_view(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let repo_id = self.active_repo_id();

        // Intentionally no outer panel header; keep diff controls in the inner header.

        // Diff search is intentionally hidden in the UI; keep it inactive to avoid "invisible filters".
        self.diff_raw_mode = false;
        if !self.diff_search_debounced.is_empty() || !self.diff_search_raw.is_empty() {
            self.diff_search_debounced.clear();
            self.diff_search_raw.clear();
            self.diff_visible_indices.clear();
            self.diff_visible_cache_len = 0;
            self.diff_scrollbar_markers_cache.clear();
        }
        let search_text = self.diff_search_input.read_with(cx, |i, _| i.text().to_string());
        if !search_text.is_empty() {
            self.diff_search_input
                .update(cx, |i, cx| i.set_text(String::new(), cx));
        }

        let title: AnyElement = self
            .active_repo()
            .and_then(|r| r.diff_target.as_ref())
            .map(|t| {
                let (icon, color, text) = match t {
                    DiffTarget::WorkingTree { path, area } => {
                        let kind = self.active_repo().and_then(|repo| match &repo.status {
                            Loadable::Ready(status) => {
                                let list = match area {
                                    DiffArea::Unstaged => &status.unstaged,
                                    DiffArea::Staged => &status.staged,
                                };
                                list.iter()
                                    .find(|e| e.path == *path)
                                    .map(|e| e.kind)
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
                        (Some(icon), color, path.display().to_string())
                    }
                    DiffTarget::Commit { commit_id: _, path } => match path {
                        Some(path) => (Some("✎"), theme.colors.text_muted, path.display().to_string()),
                        None => (Some("✎"), theme.colors.text_muted, "Full diff".to_string()),
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
                        })
                        .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                            let text: SharedString = "Inline diff view (Alt+I)".into();
                            if *hovering {
                                this.tooltip_text = Some(text);
                            } else if this.tooltip_text.as_ref() == Some(&text) {
                                this.tooltip_text = None;
                            }
                            cx.notify();
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
                            this.diff_text_segments_cache_query.clear();
                            this.diff_text_segments_cache.clear();
                            cx.notify();
                        })
                        .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                            let text: SharedString = "Split diff view (Alt+S)".into();
                            if *hovering {
                                this.tooltip_text = Some(text);
                            } else if this.tooltip_text.as_ref() == Some(&text) {
                                this.tooltip_text = None;
                            }
                            cx.notify();
                        })),
                )
                .child(
                    zed::Button::new("diff_prev_hunk", "Prev")
                        .style(zed::ButtonStyle::Outlined)
                        .disabled(!can_nav_prev)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.diff_jump_prev();
                            cx.notify();
                        })
                        .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                            let text: SharedString = "Previous change (Shift+F7 / Alt+Up)".into();
                            if *hovering {
                                this.tooltip_text = Some(text);
                            } else if this.tooltip_text.as_ref() == Some(&text) {
                                this.tooltip_text = None;
                            }
                            cx.notify();
                        })),
                )
                .child(
                    zed::Button::new("diff_next_hunk", "Next")
                        .style(zed::ButtonStyle::Outlined)
                        .disabled(!can_nav_next)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.diff_jump_next();
                            cx.notify();
                        })
                        .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                            let text: SharedString = "Next change (F7 / Alt+Down)".into();
                            if *hovering {
                                this.tooltip_text = Some(text);
                            } else if this.tooltip_text.as_ref() == Some(&text) {
                                this.tooltip_text = None;
                            }
                            cx.notify();
                        })),
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
                            })
                            .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                                let text: SharedString = "Jump to hunk… (Alt+H)".into();
                                if *hovering {
                                    this.tooltip_text = Some(text);
                                } else if this.tooltip_text.as_ref() == Some(&text) {
                                    this.tooltip_text = None;
                                }
                                cx.notify();
                            })),
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
                    })
                    .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                        let text: SharedString = "Close diff".into();
                        if *hovering {
                            this.tooltip_text = Some(text);
                        } else if this.tooltip_text.as_ref() == Some(&text) {
                            this.tooltip_text = None;
                        }
                        cx.notify();
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
                                    // Diff search is intentionally hidden; keep it disabled.
                                    self.diff_search_debounced.clear();
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

	                        // Diff search is intentionally hidden; keep it disabled.
	                        self.diff_search_debounced.clear();
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
            .flex_1()
            .w_full()
            .h_full()
            .min_h(px(0.0))
            .bg(theme.colors.surface_bg)
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

                            if key == "escape"
                                && !mods.control
                                && !mods.alt
                                && !mods.platform
                                && !mods.function
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
                                    .diff_search_input
                                    .read(cx)
                                    .focus_handle()
                                    .is_focused(window)
                                && !this.diff_raw_input.read(cx).focus_handle().is_focused(window)
                                && let Some(repo_id) = this.active_repo_id()
                                && let Some(repo) = this.active_repo()
                                && let Some(DiffTarget::WorkingTree { path, area }) = repo.diff_target.clone()
                            {
                                let next_area = match area {
                                    DiffArea::Unstaged => {
                                        this.store.dispatch(Msg::StagePath {
                                            repo_id,
                                            path: path.clone(),
                                        });
                                        DiffArea::Staged
                                    }
                                    DiffArea::Staged => {
                                        this.store.dispatch(Msg::UnstagePath {
                                            repo_id,
                                            path: path.clone(),
                                        });
                                        DiffArea::Unstaged
                                    }
                                };
                                this.store.dispatch(Msg::SelectDiff {
                                    repo_id,
                                    target: DiffTarget::WorkingTree { path, area: next_area },
                                });
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
                                    "i" => {
                                        this.diff_view = DiffViewMode::Inline;
                                        this.diff_text_segments_cache_query.clear();
                                        this.diff_text_segments_cache.clear();
                                        handled = true;
                                    }
                                    "s" => {
                                        this.diff_view = DiffViewMode::Split;
                                        this.diff_text_segments_cache_query.clear();
                                        this.diff_text_segments_cache.clear();
                                        handled = true;
                                    }
                                    "h" => {
                                        let is_file_preview =
                                            this.untracked_worktree_preview_path().is_some()
                                                || this.added_file_preview_abs_path().is_some();
                                        if !is_file_preview
                                            && !this.active_repo().is_some_and(|r| {
                                                Self::is_file_diff_target(r.diff_target.as_ref())
                                            })
                                        {
                                            let _ = this.ensure_diff_hunk_picker_search_input(
                                                window, cx,
                                            );
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
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .w_full()
                    .h_full()
                    .child(body),
            )
            .child(DiffTextSelectionTracker { view: cx.entity() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_details_large_file_list_is_truncated() {
        let total_files = 12 + 15762;
        let shown = total_files.min(COMMIT_DETAILS_FILES_RENDER_LIMIT);
        let omitted = total_files.saturating_sub(shown);

        assert_eq!(shown, COMMIT_DETAILS_FILES_RENDER_LIMIT);
        assert_eq!(omitted, total_files - COMMIT_DETAILS_FILES_RENDER_LIMIT);
    }
}
