use super::*;

impl GitGpuiView {
    fn history_column_headers(&self) -> gpui::Div {
        let theme = self.theme;
        div()
            .flex()
            .w_full()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .text_color(theme.colors.text_muted)
            .child(
                div()
                    .w(px(HISTORY_COL_BRANCH_PX))
                    .whitespace_nowrap()
                    .child("Branch / Tag"),
            )
            .child(
                div()
                    .w(px(HISTORY_COL_GRAPH_PX))
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
            )
            .child(
                div()
                    .w(px(HISTORY_COL_DATE_PX))
                    .flex()
                    .justify_end()
                    .whitespace_nowrap()
                    .child("COMMIT DATE / TIME"),
            )
            .child(
                div()
                    .w(px(HISTORY_COL_SHA_PX))
                    .flex()
                    .justify_end()
                    .whitespace_nowrap()
                    .child("SHA"),
            )
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

            let position = if repos_len <= 1 {
                zed::TabPosition::First
            } else if ix == 0 {
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

        bar.end_child(
            zed::Button::new("add_repo", "＋")
                .style(zed::ButtonStyle::Subtle)
                .on_click(theme, cx, |this, _e, window, cx| {
                    this.prompt_open_repo(window, cx)
                }),
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
            .px_3()
            .py_2()
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

        let pull = zed::SplitButton::new(
            zed::Button::new("pull_main", "Pull")
                .style(zed::ButtonStyle::Subtle)
                .on_click(theme, cx, |this, _e, _w, cx| {
                    if let Some(repo_id) = this.active_repo_id() {
                        this.store.dispatch(Msg::Pull {
                            repo_id,
                            mode: PullMode::Default,
                        });
                    }
                    cx.notify();
                }),
            zed::Button::new("pull_menu", "▾")
                .style(zed::ButtonStyle::Subtle)
                .on_click(theme, cx, |this, e, _w, cx| {
                    this.popover = Some(PopoverKind::PullPicker);
                    this.popover_anchor = Some(e.position());
                    cx.notify();
                }),
        )
        .style(zed::SplitButtonStyle::Outlined)
        .render(theme);

        let push = zed::Button::new("push", "Push")
            .style(zed::ButtonStyle::Outlined)
            .on_click(theme, cx, |this, _e, _w, cx| {
                if let Some(repo_id) = this.active_repo_id() {
                    this.store.dispatch(Msg::Push { repo_id });
                }
                cx.notify();
            });

        let stash = zed::Button::new("stash", "Stash")
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

        let create_branch = zed::Button::new("create_branch", "Branch…")
            .style(zed::ButtonStyle::Outlined)
            .on_click(theme, cx, |this, _e, window, cx| {
                let _ = this.ensure_branch_picker_search_input(window, cx);
                this.popover = Some(PopoverKind::BranchPicker);
                this.popover_anchor = Some(point(px(300.0), px(120.0)));
                cx.notify();
            });

        let bar = div()
            .flex()
            .items_center()
            .justify_between()
            .px_2()
            .py_1()
            .bg(theme.colors.surface_bg_elevated)
            .border_1()
            .border_color(theme.colors.border)
            .rounded(px(theme.radii.panel))
            .shadow_sm()
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
            );

        bar
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

        let close = cx.listener(|this, _e: &ClickEvent, _w, cx| {
            this.popover = None;
            this.popover_anchor = None;
            cx.notify();
        });

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
                        })
                        .min_w(px(320.0))
                        .child(div().border_t_1().border_color(theme.colors.border))
                        .child(
                            div()
                                .id("repo_popover_close")
                                .debug_selector(|| "repo_popover_close".to_string())
                                .px_3()
                                .py_2()
                                .hover(move |s| s.bg(theme.colors.hover))
                                .active(move |s| s.bg(theme.colors.active))
                                .child("Close")
                                .on_click(close),
                        )
                } else {
                    let mut menu = div().flex().flex_col().min_w(px(320.0));
                    for repo in self.state.repos.iter() {
                        let id = repo.id;
                        let label: SharedString = repo.spec.workdir.display().to_string().into();
                        menu = menu.child(
                            div()
                                .id(("repo_item", id.0))
                                .px_3()
                                .py_2()
                                .hover(move |s| s.bg(theme.colors.hover))
                                .active(move |s| s.bg(theme.colors.active))
                                .child(div().text_sm().line_clamp(1).child(label))
                                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                    this.store.dispatch(Msg::SetActiveRepo { repo_id: id });
                                    this.popover = None;
                                    this.popover_anchor = None;
                                    this.rebuild_diff_cache();
                                    cx.notify();
                                })),
                        );
                    }
                    menu.child(
                        div()
                            .id("repo_popover_close")
                            .debug_selector(|| "repo_popover_close".to_string())
                            .px_3()
                            .py_2()
                            .hover(move |s| s.bg(theme.colors.hover))
                            .active(move |s| s.bg(theme.colors.active))
                            .child("Close")
                            .on_click(close),
                    )
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
                                            if let Some(name) =
                                                branch_names.get(ix).map(|n| n.clone())
                                            {
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
                                    menu = menu.child(
                                        div()
                                            .id(("branch_item", ix))
                                            .px_3()
                                            .py_2()
                                            .hover(move |s| s.bg(theme.colors.hover))
                                            .active(move |s| s.bg(theme.colors.active))
                                            .child(name.clone())
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
                            menu = menu.child(div().px_3().py_2().child("Loading…"));
                        }
                        Loadable::Error(e) => {
                            menu = menu.child(div().px_3().py_2().child(e.clone()));
                        }
                        Loadable::NotLoaded => {
                            menu = menu.child(div().px_3().py_2().child("Not loaded"));
                        }
                    }
                }

                menu.child(div().border_t_1().border_color(theme.colors.border))
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .text_color(theme.colors.text_muted)
                            .child("Create branch"),
                    )
                    .child(self.create_branch_input.clone())
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
                    )
                    .child(
                        div()
                            .id(("branch_popover_close", 0usize))
                            .px_3()
                            .py_2()
                            .hover(move |s| s.bg(theme.colors.hover))
                            .active(move |s| s.bg(theme.colors.active))
                            .child("Close")
                            .on_click(close),
                    )
            }
            PopoverKind::HistoryBranchFilter { repo_id } => {
                let mut menu = div().flex().flex_col().min_w(px(260.0));

                if let Some(search) = self.history_branch_picker_search_input.clone() {
                    let mut items: Vec<SharedString> = vec!["All branches".into()];
                    let mut branch_names: Vec<String> = Vec::new();
                    if let Some(repo) = self.state.repos.iter().find(|r| r.id == repo_id)
                        && let Loadable::Ready(branches) = &repo.branches
                    {
                        branch_names = branches
                            .iter()
                            .map(|b| b.name.clone())
                            .collect::<Vec<String>>();
                        items.extend(branch_names.iter().map(|name| name.clone().into()));
                    }

                    menu = menu.child(
                        zed::PickerPrompt::new(search)
                            .items(items)
                            .empty_text("No branches")
                            .max_height(px(260.0))
                            .render(theme, cx, move |this, ix, _e, _w, cx| {
                                if ix == 0 {
                                    this.history_branch_filter = None;
                                } else if let Some(name) = branch_names.get(ix - 1) {
                                    this.history_branch_filter = Some(name.clone());
                                }
                                this.popover = None;
                                this.popover_anchor = None;
                                cx.notify();
                            }),
                    );
                } else {
                    menu = menu.child(
                        div()
                            .id("history_branch_all")
                            .px_3()
                            .py_2()
                            .hover(move |s| s.bg(theme.colors.hover))
                            .child("All branches")
                            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.history_branch_filter = None;
                                this.popover = None;
                                this.popover_anchor = None;
                                cx.notify();
                            })),
                    );

                    if let Some(repo) = self.state.repos.iter().find(|r| r.id == repo_id) {
                        match &repo.branches {
                            Loadable::Ready(branches) => {
                                for (ix, branch) in branches.iter().enumerate() {
                                    let name = branch.name.clone();
                                    menu = menu.child(
                                        div()
                                            .id(("history_branch_item", ix))
                                            .px_3()
                                            .py_2()
                                            .hover(move |s| s.bg(theme.colors.hover))
                                            .child(name.clone())
                                            .on_click(cx.listener(
                                                move |this, _e: &ClickEvent, _w, cx| {
                                                    this.history_branch_filter = Some(name.clone());
                                                    this.popover = None;
                                                    this.popover_anchor = None;
                                                    cx.notify();
                                                },
                                            )),
                                    );
                                }
                            }
                            Loadable::Loading => {
                                menu = menu.child(div().px_3().py_2().child("Loading…"));
                            }
                            Loadable::Error(e) => {
                                menu = menu.child(div().px_3().py_2().child(e.clone()));
                            }
                            Loadable::NotLoaded => {
                                menu = menu.child(div().px_3().py_2().child("Not loaded"));
                            }
                        }
                    }

                    menu = menu.child(
                        div()
                            .id("history_branch_close")
                            .px_3()
                            .py_2()
                            .hover(move |s| s.bg(theme.colors.hover))
                            .child("Close")
                            .on_click(close),
                    );
                }

                menu
            }
            PopoverKind::PullPicker => {
                let repo_id = self.active_repo_id();
                let mut menu = div().flex().flex_col().min_w(px(280.0));
                for (ix, (label, mode)) in [
                    ("Fetch all", None),
                    ("Pull (default)", Some(PullMode::Default)),
                    (
                        "Pull (fast-forward if possible)",
                        Some(PullMode::FastForwardIfPossible),
                    ),
                    ("Pull (fast-forward only)", Some(PullMode::FastForwardOnly)),
                    ("Pull (rebase)", Some(PullMode::Rebase)),
                ]
                .into_iter()
                .enumerate()
                {
                    menu = menu.child(
                        div()
                            .id(("pull_item", ix))
                            .px_3()
                            .py_2()
                            .hover(move |s| s.bg(theme.colors.hover))
                            .child(label)
                            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                if let Some(repo_id) = repo_id {
                                    if let Some(mode) = mode {
                                        this.store.dispatch(Msg::Pull { repo_id, mode });
                                    } else {
                                        this.store.dispatch(Msg::FetchAll { repo_id });
                                    }
                                }
                                this.popover = None;
                                this.popover_anchor = None;
                                cx.notify();
                            })),
                    );
                }
                menu.child(
                    div()
                        .id(("pull_popover_close", 0usize))
                        .px_3()
                        .py_2()
                        .hover(move |s| s.bg(theme.colors.hover))
                        .child("Close")
                        .on_click(close),
                )
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
                                .px_3()
                                .py_2()
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
                                .px_3()
                                .py_2()
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
                            .px_3()
                            .py_2()
                            .hover(move |s| s.bg(theme.colors.hover))
                            .child("Close")
                            .on_click(close),
                    )
                }
            }
            PopoverKind::CommandLogDetails { repo_id, index } => {
                let entry = self
                    .state
                    .repos
                    .iter()
                    .find(|r| r.id == repo_id)
                    .and_then(|r| r.command_log.get(index))
                    .cloned();

                match entry {
                    None => div()
                        .flex()
                        .flex_col()
                        .min_w(px(520.0))
                        .child(div().px_3().py_2().child("Missing log entry"))
                        .child(
                            div()
                                .id("cmdlog_missing_close")
                                .px_3()
                                .py_2()
                                .hover(move |s| s.bg(theme.colors.hover))
                                .child("Close")
                                .on_click(close),
                        ),
                    Some(entry) => {
                        let combined = {
                            let mut s = String::new();
                            if !entry.stdout.trim().is_empty() {
                                s.push_str(entry.stdout.trim_end());
                                s.push('\n');
                            }
                            if !entry.stderr.trim().is_empty() {
                                s.push_str(entry.stderr.trim_end());
                                s.push('\n');
                            }
                            s.trim_end().to_string()
                        };

                        let combined_for_clipboard = combined.clone();

                        div()
                            .flex()
                            .flex_col()
                            .min_w(px(520.0))
                            .max_w(px(760.0))
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .child(entry.summary.clone()),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .pb_2()
                                    .text_xs()
                                    .font_family("monospace")
                                    .text_color(theme.colors.text_muted)
                                    .child(entry.command.clone()),
                            )
                            .child(div().border_t_1().border_color(theme.colors.border))
                            .child(
                                div()
                                    .id("cmdlog_output")
                                    .px_3()
                                    .py_2()
                                    .h(px(220.0))
                                    .overflow_y_scroll()
                                    .font_family("monospace")
                                    .text_xs()
                                    .child(if combined.is_empty() {
                                        "<no output>".to_string()
                                    } else {
                                        combined
                                    }),
                            )
                            .child(div().border_t_1().border_color(theme.colors.border))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .px_3()
                                    .py_2()
                                    .child(
                                        zed::Button::new("cmdlog_copy", "Copy")
                                            .style(zed::ButtonStyle::Outlined)
                                            .on_click(theme, cx, move |_this, _e, _w, cx| {
                                                if !combined_for_clipboard.is_empty() {
                                                    cx.write_to_clipboard(
                                                        gpui::ClipboardItem::new_string(
                                                            combined_for_clipboard.clone(),
                                                        ),
                                                    );
                                                }
                                                cx.notify();
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("cmdlog_close")
                                            .px_2()
                                            .py_1()
                                            .rounded(px(theme.radii.row))
                                            .hover(move |s| s.bg(theme.colors.hover))
                                            .child("Close")
                                            .on_click(close),
                                    ),
                            )
                    }
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
                        .px_3()
                        .py_2()
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
                                .px_3()
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
                            .px_3()
                            .py_2()
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
                            .px_3()
                            .py_2()
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .child(staged_summary),
                    )
                    .child(staged_list)
                    .child(div().border_t_1().border_color(theme.colors.border))
                    .child(div().px_3().py_2().h(px(140.0)).child(input))
                    .child(div().border_t_1().border_color(theme.colors.border))
                    .child(
                        div()
                            .px_3()
                            .py_2()
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
                let sha = commit_id.as_ref().to_string();
                let short = sha.get(0..8).unwrap_or(&sha).to_string();
                let sha_for_clipboard = sha.clone();
                let commit_id_open_diff = commit_id.clone();
                let commit_id_checkout = commit_id.clone();
                let commit_id_cherry_pick = commit_id.clone();
                let commit_id_revert = commit_id.clone();

                let commit_summary = self
                    .active_repo()
                    .and_then(|r| match &r.log {
                        Loadable::Ready(page) => page
                            .commits
                            .iter()
                            .find(|c| c.id == commit_id)
                            .map(|c| format!("{} — {}", c.author, c.summary)),
                        _ => None,
                    })
                    .unwrap_or_default();

                div()
                    .flex()
                    .flex_col()
                    .min_w(px(240.0))
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .child(format!("Commit {short}")),
                    )
                    .when(!commit_summary.is_empty(), |this| {
                        this.child(
                            div()
                                .px_3()
                                .pb_2()
                                .text_sm()
                                .line_clamp(2)
                                .child(commit_summary),
                        )
                    })
                    .child(div().border_t_1().border_color(theme.colors.border))
                    .child(
                        div()
                            .id("commit_menu_open_diff")
                            .px_3()
                            .py_2()
                            .hover(move |s| s.bg(theme.colors.hover))
                            .child("Open diff  (Enter)")
                            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.store.dispatch(Msg::SelectDiff {
                                    repo_id,
                                    target: DiffTarget::Commit {
                                        commit_id: commit_id_open_diff.clone(),
                                        path: None,
                                    },
                                });
                                this.rebuild_diff_cache();
                                this.popover = None;
                                this.popover_anchor = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("commit_menu_copy_sha")
                            .px_3()
                            .py_2()
                            .hover(move |s| s.bg(theme.colors.hover))
                            .child("Copy SHA  (C)")
                            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                    sha_for_clipboard.clone(),
                                ));
                                this.popover = None;
                                this.popover_anchor = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("commit_menu_checkout")
                            .px_3()
                            .py_2()
                            .hover(move |s| s.bg(theme.colors.hover))
                            .child("Checkout (detached)  (D)")
                            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.store.dispatch(Msg::CheckoutCommit {
                                    repo_id,
                                    commit_id: commit_id_checkout.clone(),
                                });
                                this.popover = None;
                                this.popover_anchor = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("commit_menu_cherry_pick")
                            .px_3()
                            .py_2()
                            .hover(move |s| s.bg(theme.colors.hover))
                            .child("Cherry-pick  (P)")
                            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.store.dispatch(Msg::CherryPickCommit {
                                    repo_id,
                                    commit_id: commit_id_cherry_pick.clone(),
                                });
                                this.popover = None;
                                this.popover_anchor = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("commit_menu_revert")
                            .px_3()
                            .py_2()
                            .hover(move |s| s.bg(theme.colors.hover))
                            .child("Revert  (R)")
                            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.store.dispatch(Msg::RevertCommit {
                                    repo_id,
                                    commit_id: commit_id_revert.clone(),
                                });
                                this.popover = None;
                                this.popover_anchor = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("commit_menu_close")
                            .px_3()
                            .py_2()
                            .hover(move |s| s.bg(theme.colors.hover))
                            .active(move |s| s.bg(theme.colors.active))
                            .child("Close")
                            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.popover = None;
                                this.popover_anchor = None;
                                cx.notify();
                            })),
                    )
            }
            PopoverKind::AppMenu => div()
                .flex()
                .flex_col()
                .min_w(px(200.0))
                .child(
                    div()
                        .id("app_menu_diagnostics")
                        .debug_selector(|| "app_menu_diagnostics".to_string())
                        .px_3()
                        .py_2()
                        .hover(move |s| s.bg(theme.colors.hover))
                        .active(move |s| s.bg(theme.colors.active))
                        .child("Tools")
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.show_diagnostics_view = !this.show_diagnostics_view;
                            if this.show_diagnostics_view {
                                this.tools_tab = ToolsTab::Diagnostics;
                            }
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .id("app_menu_quit")
                        .debug_selector(|| "app_menu_quit".to_string())
                        .px_3()
                        .py_2()
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
                        .px_3()
                        .py_2()
                        .hover(move |s| s.bg(theme.colors.hover))
                        .active(move |s| s.bg(theme.colors.active))
                        .child("Close")
                        .on_click(close),
                ),
        };

        let offset_y = if is_app_menu { px(40.0) } else { px(8.0) };

        anchored()
            .position(anchor)
            .anchor(Corner::TopLeft)
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
        let repo = self.active_repo();

        let branches_count = repo
            .and_then(|r| match &r.branches {
                Loadable::Ready(v) => Some(v.len()),
                _ => None,
            })
            .unwrap_or(0);

        let remotes_count = repo
            .map(Self::remote_rows)
            .map(|rows| rows.len())
            .unwrap_or(0);

        let branches_list: AnyElement = if branches_count == 0 {
            zed::empty_state(theme, "Local", "No branches loaded.").into_any_element()
        } else {
            let list = uniform_list(
                "branches",
                branches_count,
                cx.processor(Self::render_branch_rows),
            )
            .h(px(140.0))
            .track_scroll(self.branches_scroll.clone());
            let scroll_handle = self.branches_scroll.0.borrow().base_handle.clone();
            div()
                .id("branches_scroll_container")
                .relative()
                .h(px(140.0))
                .child(list)
                .child(zed::Scrollbar::new("branches_scrollbar", scroll_handle).render(theme))
                .into_any_element()
        };

        let remotes_list: AnyElement = if remotes_count == 0 {
            zed::empty_state(theme, "Remote", "No remotes loaded.").into_any_element()
        } else {
            let list = uniform_list(
                "remotes",
                remotes_count,
                cx.processor(Self::render_remote_rows),
            )
            .h(px(160.0))
            .track_scroll(self.remotes_scroll.clone());
            let scroll_handle = self.remotes_scroll.0.borrow().base_handle.clone();
            div()
                .id("remotes_scroll_container")
                .relative()
                .h(px(160.0))
                .child(list)
                .child(zed::Scrollbar::new("remotes_scrollbar", scroll_handle).render(theme))
                .into_any_element()
        };

        let diagnostics_count = repo.map(|r| r.diagnostics.len()).unwrap_or(0);
        let output_count = repo.map(|r| r.command_log.len()).unwrap_or(0);
        let conflicts_count = repo
            .and_then(|r| match &r.status {
                Loadable::Ready(status) => Some(
                    status
                        .unstaged
                        .iter()
                        .chain(status.staged.iter())
                        .filter(|e| e.kind == FileStatusKind::Conflicted)
                        .count(),
                ),
                _ => None,
            })
            .unwrap_or(0);

        let repo_id = repo.map(|r| r.id);
        let blame_target = repo
            .and_then(|r| r.diff_target.as_ref())
            .and_then(|t| match t {
                DiffTarget::WorkingTree { path, .. } => Some(path.clone()),
                DiffTarget::Commit {
                    path: Some(path), ..
                } => Some(path.clone()),
                _ => None,
            });

        let tools_panel = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                zed::Button::new(
                    "tools_diagnostics",
                    format!("Diagnostics ({diagnostics_count})"),
                )
                .style(zed::ButtonStyle::Outlined)
                .on_click(theme, cx, |this, _e, _w, cx| {
                    this.show_diagnostics_view = true;
                    this.tools_tab = ToolsTab::Diagnostics;
                    cx.notify();
                }),
            )
            .child(
                zed::Button::new("tools_output", format!("Output ({output_count})"))
                    .style(zed::ButtonStyle::Outlined)
                    .on_click(theme, cx, |this, _e, _w, cx| {
                        this.show_diagnostics_view = true;
                        this.tools_tab = ToolsTab::Output;
                        cx.notify();
                    }),
            )
            .child(
                zed::Button::new("tools_conflicts", format!("Conflicts ({conflicts_count})"))
                    .style(zed::ButtonStyle::Outlined)
                    .on_click(theme, cx, |this, _e, _w, cx| {
                        this.show_diagnostics_view = true;
                        this.tools_tab = ToolsTab::Conflicts;
                        cx.notify();
                    }),
            )
            .child(
                zed::Button::new("tools_blame", "Blame")
                    .style(zed::ButtonStyle::Outlined)
                    .on_click(theme, cx, move |this, _e, _w, cx| {
                        if let Some(repo_id) = repo_id
                            && let Some(path) = blame_target.clone()
                        {
                            this.store.dispatch(Msg::LoadBlame {
                                repo_id,
                                path,
                                rev: None,
                            });
                        }
                        this.show_diagnostics_view = true;
                        this.tools_tab = ToolsTab::Blame;
                        cx.notify();
                    }),
            );

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(zed::panel(theme, "Local", None, branches_list))
            .child(zed::panel(theme, "Remote", None, remotes_list))
            .child(zed::panel(theme, "Tools", None, tools_panel))
    }

    pub(super) fn commit_details_view(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let repo = self.active_repo();

        if let Some(repo) = repo {
            if let Some(selected_id) = repo.selected_commit.as_ref() {
                let header = div()
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

                            div()
                                .flex()
                                .flex_col()
                                .gap_3()
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
                                                .text_sm()
                                                .min_w(px(0.0))
                                                .line_clamp(8)
                                                .child(details.message.clone()),
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

                return div().flex().flex_col().gap_3().child(zed::panel(
                    theme,
                    "Commit",
                    None,
                    div().flex().flex_col().gap_2().child(header).child(body),
                ));
            }
        }

        let (staged_count, unstaged_count) = repo
            .and_then(|r| match &r.status {
                Loadable::Ready(s) => Some((s.staged.len(), s.unstaged.len())),
                _ => None,
            })
            .unwrap_or((0, 0));

        let unstaged_list = self.status_list(cx, DiffArea::Unstaged, unstaged_count);
        let staged_list = self.status_list(cx, DiffArea::Staged, staged_count);
        let commit_box = self.commit_box(cx);

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(zed::panel(
                theme,
                "Unstaged",
                Some(format!("{unstaged_count}").into()),
                unstaged_list,
            ))
            .child(zed::panel(
                theme,
                "Staged",
                Some(format!("{staged_count}").into()),
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(staged_list)
                    .child(commit_box),
            ))
    }

    pub(super) fn diagnostics_view(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let repo = self.active_repo();

        let tabs = {
            let tab = self.tools_tab;
            let mut bar = zed::TabBar::new("tools_tab_bar");
            for (ix, (label, value)) in [
                ("Diagnostics", ToolsTab::Diagnostics),
                ("Output", ToolsTab::Output),
                ("Conflicts", ToolsTab::Conflicts),
                ("Blame", ToolsTab::Blame),
            ]
            .into_iter()
            .enumerate()
            {
                let selected = tab == value;
                let position = if ix == 0 {
                    zed::TabPosition::First
                } else if ix == 3 {
                    zed::TabPosition::Last
                } else {
                    zed::TabPosition::Middle(std::cmp::Ordering::Equal)
                };
                let value_for_click = value;
                let t = zed::Tab::new(("tools_tab", ix))
                    .selected(selected)
                    .position(position)
                    .child(div().text_sm().child(label))
                    .render(theme)
                    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                        this.tools_tab = value_for_click;
                        cx.notify();
                    }));
                bar = bar.tab(t);
            }
            bar.render(theme)
        };

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(div().text_sm().font_weight(FontWeight::BOLD).child("Tools"))
            .child(
                zed::Button::new("diagnostics_close", "✕")
                    .style(zed::ButtonStyle::Transparent)
                    .on_click(theme, cx, |this, _e, _w, cx| {
                        this.show_diagnostics_view = false;
                        cx.notify();
                    }),
            );

        let body: AnyElement = match self.tools_tab {
            ToolsTab::Diagnostics => {
                let diagnostics_count = repo.map(|r| r.diagnostics.len()).unwrap_or(0);
                if diagnostics_count == 0 {
                    match repo {
                        None => zed::empty_state(theme, "Diagnostics", "No repository.")
                            .into_any_element(),
                        Some(_) => {
                            zed::empty_state(theme, "Diagnostics", "No issues.").into_any_element()
                        }
                    }
                } else {
                    let list = uniform_list(
                        "diagnostics_main",
                        diagnostics_count,
                        cx.processor(Self::render_diagnostic_rows),
                    )
                    .h_full()
                    .track_scroll(self.diagnostics_scroll.clone());
                    let scroll_handle = self.diagnostics_scroll.0.borrow().base_handle.clone();
                    div()
                        .id("diagnostics_main_scroll_container")
                        .relative()
                        .h_full()
                        .child(list)
                        .child(
                            zed::Scrollbar::new("diagnostics_main_scrollbar", scroll_handle)
                                .render(theme),
                        )
                        .into_any_element()
                }
            }
            ToolsTab::Output => {
                let count = repo.map(|r| r.command_log.len()).unwrap_or(0);
                if count == 0 {
                    match repo {
                        None => {
                            zed::empty_state(theme, "Output", "No repository.").into_any_element()
                        }
                        Some(_) => {
                            zed::empty_state(theme, "Output", "No commands yet.").into_any_element()
                        }
                    }
                } else {
                    let list = uniform_list(
                        "output_main",
                        count,
                        cx.processor(Self::render_command_log_rows),
                    )
                    .h_full()
                    .track_scroll(self.output_scroll.clone());
                    let scroll_handle = self.output_scroll.0.borrow().base_handle.clone();
                    div()
                        .id("output_main_scroll_container")
                        .relative()
                        .h_full()
                        .child(list)
                        .child(
                            zed::Scrollbar::new("output_main_scrollbar", scroll_handle)
                                .render(theme),
                        )
                        .into_any_element()
                }
            }
            ToolsTab::Conflicts => {
                let count = repo
                    .and_then(|r| match &r.status {
                        Loadable::Ready(status) => Some(
                            status
                                .unstaged
                                .iter()
                                .chain(status.staged.iter())
                                .filter(|e| e.kind == FileStatusKind::Conflicted)
                                .count(),
                        ),
                        _ => None,
                    })
                    .unwrap_or(0);

                if count == 0 {
                    match repo {
                        None => zed::empty_state(theme, "Conflicts", "No repository.")
                            .into_any_element(),
                        Some(_) => {
                            zed::empty_state(theme, "Conflicts", "No conflicts.").into_any_element()
                        }
                    }
                } else {
                    let list = uniform_list(
                        "conflicts_main",
                        count,
                        cx.processor(Self::render_conflict_rows),
                    )
                    .h_full()
                    .track_scroll(self.conflicts_scroll.clone());
                    let scroll_handle = self.conflicts_scroll.0.borrow().base_handle.clone();
                    div()
                        .id("conflicts_main_scroll_container")
                        .relative()
                        .h_full()
                        .child(list)
                        .child(
                            zed::Scrollbar::new("conflicts_main_scrollbar", scroll_handle)
                                .render(theme),
                        )
                        .into_any_element()
                }
            }
            ToolsTab::Blame => match repo {
                None => zed::empty_state(theme, "Blame", "No repository.").into_any_element(),
                Some(repo) => match (&repo.blame_target, &repo.blame) {
                    (None, _) => {
                        zed::empty_state(theme, "Blame", "Select a file in Diff, then click Blame.")
                            .into_any_element()
                    }
                    (Some(path), Loadable::Loading) => {
                        zed::empty_state(theme, "Blame", format!("Loading… {}", path.display()))
                            .into_any_element()
                    }
                    (Some(_), Loadable::Error(e)) => {
                        zed::empty_state(theme, "Blame", e.clone()).into_any_element()
                    }
                    (Some(_), Loadable::NotLoaded) => {
                        zed::empty_state(theme, "Blame", "Not loaded.").into_any_element()
                    }
                    (Some(_), Loadable::Ready(lines)) => {
                        if lines.is_empty() {
                            zed::empty_state(theme, "Blame", "No blame data.").into_any_element()
                        } else {
                            let list = uniform_list(
                                "blame_main",
                                lines.len(),
                                cx.processor(Self::render_blame_rows),
                            )
                            .h_full()
                            .track_scroll(self.blame_scroll.clone());
                            let scroll_handle = self.blame_scroll.0.borrow().base_handle.clone();
                            div()
                                .id("blame_main_scroll_container")
                                .relative()
                                .h_full()
                                .child(list)
                                .child(
                                    zed::Scrollbar::new("blame_main_scrollbar", scroll_handle)
                                        .render(theme),
                                )
                                .into_any_element()
                        }
                    }
                },
            },
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .flex_1()
            .w_full()
            .h_full()
            .min_h(px(0.0))
            .child(zed::panel(
                theme,
                "Tools",
                None,
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .h_full()
                    .child(tabs)
                    .child(header)
                    .child(div().flex_1().child(body)),
            ))
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
                let list =
                    uniform_list("unstaged", count, cx.processor(Self::render_unstaged_rows))
                        .h(px(140.0))
                        .track_scroll(self.unstaged_scroll.clone());
                let scroll_handle = self.unstaged_scroll.0.borrow().base_handle.clone();
                div()
                    .id("unstaged_scroll_container")
                    .relative()
                    .h(px(140.0))
                    .child(list)
                    .child(zed::Scrollbar::new("unstaged_scrollbar", scroll_handle).render(theme))
                    .into_any_element()
            }
            DiffArea::Staged => {
                let list = uniform_list("staged", count, cx.processor(Self::render_staged_rows))
                    .h(px(140.0))
                    .track_scroll(self.staged_scroll.clone());
                let scroll_handle = self.staged_scroll.0.borrow().base_handle.clone();
                div()
                    .id("staged_scroll_container")
                    .relative()
                    .h(px(140.0))
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

        let (title, body): (SharedString, AnyElement) = match self.history_tab {
            HistoryTab::Log => {
                self.update_history_search_debounce(cx);
                self.ensure_history_cache(cx);
                let repo = self.active_repo();
                let count = self
                    .history_cache
                    .as_ref()
                    .map(|c| c.visible_indices.len())
                    .unwrap_or(0);

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
                        let current: SharedString = self
                            .history_branch_filter
                            .clone()
                            .unwrap_or_else(|| "All branches".to_string())
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
                            .on_click(cx.listener(|this, e: &ClickEvent, window, cx| {
                                if let Some(repo_id) = this.active_repo_id() {
                                    let _ =
                                        this.ensure_history_branch_picker_search_input(window, cx);
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
                    .child(self.history_column_headers())
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
            .gap_3()
            .flex_1()
            .w_full()
            .h_full()
            .min_h(px(0.0))
            .child(
                zed::panel(
                    theme,
                    title,
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

        let subtitle: Option<SharedString> = if cfg!(debug_assertions) {
            fn loadable_tag<T>(l: &Loadable<T>) -> &'static str {
                match l {
                    Loadable::NotLoaded => "notloaded",
                    Loadable::Loading => "loading",
                    Loadable::Ready(_) => "ready",
                    Loadable::Error(_) => "error",
                }
            }
            self.active_repo().map(|r| {
                format!(
                    "patch={} patch_cache={} patch_visible={}",
                    loadable_tag(&r.diff),
                    self.diff_cache.len(),
                    self.diff_visible_indices.len(),
                )
                .into()
            })
        } else {
            None
        };

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

        let repo = self.active_repo();

        let mut controls = div().flex().items_center().gap_1();
        if !is_file_preview {
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
                            cx.notify();
                        }),
                )
                .child(
                    zed::Button::new("diff_prev_hunk", "Prev")
                        .style(zed::ButtonStyle::Outlined)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            let current = this.diff_selection_anchor.unwrap_or(0);
                            let hunks = this
                                .patch_hunk_entries()
                                .into_iter()
                                .map(|(visible_ix, _)| visible_ix)
                                .collect::<Vec<_>>();
                            if let Some(&target) = hunks
                                .iter()
                                .rev()
                                .find(|&&ix| ix < current)
                                .or_else(|| hunks.last())
                            {
                                this.diff_scroll
                                    .scroll_to_item(target, gpui::ScrollStrategy::Top);
                                this.diff_selection_anchor = Some(target);
                                this.diff_selection_range = Some((target, target));
                                cx.notify();
                            }
                        }),
                )
                .child(
                    zed::Button::new("diff_next_hunk", "Next")
                        .style(zed::ButtonStyle::Outlined)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            let current = this.diff_selection_anchor.unwrap_or(0);
                            let hunks = this
                                .patch_hunk_entries()
                                .into_iter()
                                .map(|(visible_ix, _)| visible_ix)
                                .collect::<Vec<_>>();
                            if let Some(&target) = hunks
                                .iter()
                                .find(|&&ix| ix > current)
                                .or_else(|| hunks.first())
                            {
                                this.diff_scroll
                                    .scroll_to_item(target, gpui::ScrollStrategy::Top);
                                this.diff_selection_anchor = Some(target);
                                this.diff_selection_range = Some((target, target));
                                cx.notify();
                            }
                        }),
                )
                .child(
                    zed::Button::new("diff_hunks", "Hunks…")
                        .style(zed::ButtonStyle::Outlined)
                        .on_click(theme, cx, |this, e, window, cx| {
                            let _ = this.ensure_diff_hunk_picker_search_input(window, cx);
                            this.popover = Some(PopoverKind::DiffHunks);
                            this.popover_anchor = Some(e.position());
                            cx.notify();
                        }),
                )
                .child(
                    zed::Button::new("diff_blame", "Blame")
                        .style(zed::ButtonStyle::Outlined)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            if let Some(repo_id) = this.active_repo_id()
                                && let Some(repo) = this.active_repo()
                            {
                                let path = repo.diff_target.as_ref().and_then(|t| match t {
                                    DiffTarget::WorkingTree { path, .. } => Some(path.clone()),
                                    DiffTarget::Commit {
                                        path: Some(path), ..
                                    } => Some(path.clone()),
                                    _ => None,
                                });
                                if let Some(path) = path {
                                    this.store.dispatch(Msg::LoadBlame {
                                        repo_id,
                                        path,
                                        rev: None,
                                    });
                                    this.show_diagnostics_view = true;
                                    this.tools_tab = ToolsTab::Blame;
                                }
                            }
                            cx.notify();
                        }),
                )
                .child(
                    zed::Button::new("diff_copy", "Copy")
                        .style(zed::ButtonStyle::Outlined)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            let text = this.diff_selected_text_for_clipboard();
                            if !text.is_empty() {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                            }
                            cx.notify();
                        }),
                );
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
                Loadable::Error(e) => zed::empty_state(theme, "File", e.clone()).into_any_element(),
                Loadable::Ready(lines) => {
                    if lines.is_empty() {
                        zed::empty_state(theme, "File", "Empty file.").into_any_element()
                    } else {
                        let list = uniform_list(
                            "worktree_preview",
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
                Loadable::Loading => zed::empty_state(theme, "Diff", "Loading…").into_any_element(),
                Loadable::Error(e) => zed::empty_state(theme, "Diff", e.clone()).into_any_element(),
                Loadable::Ready(diff) => {
                    if self.diff_cache_repo_id != Some(repo.id)
                        || self.diff_cache_rev != repo.diff_rev
                        || self.diff_cache_target != repo.diff_target
                        || self.diff_cache.len() != diff.lines.len()
                    {
                        self.rebuild_diff_cache();
                    }

                    self.update_diff_search_debounce(cx);
                    self.ensure_diff_visible_indices();
                    if self.diff_cache.is_empty() {
                        zed::empty_state(theme, "Diff", "No differences.").into_any_element()
                    } else if !self.diff_visible_query.is_empty() && self.diff_query_match_count == 0 {
                        zed::empty_state(theme, "Diff", "No matches.").into_any_element()
                    } else if self.diff_visible_indices.is_empty() {
                        zed::empty_state(theme, "Diff", "Nothing to render.").into_any_element()
                    } else {
                        let scroll_handle = self.diff_scroll.0.borrow().base_handle.clone();
                        let markers = self.diff_scrollbar_markers_patch();
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
            },
        }
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .flex_1()
            .w_full()
            .h_full()
            .min_h(px(0.0))
            .child(
                zed::panel(
                    theme,
                    "Diff",
                    subtitle,
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .h_full()
                        .child(header)
                        .child(
                            div()
                                .flex_1()
                                .min_h(px(120.0))
                                .w_full()
                                .h_full()
                                .child(body),
                        ),
                )
                .flex_1(),
            )
    }
}
