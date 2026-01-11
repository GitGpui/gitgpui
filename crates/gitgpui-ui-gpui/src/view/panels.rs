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
            .on_click(cx.listener(|this, e: &ClickEvent, _w, cx| {
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
            .on_click(cx.listener(|this, e: &ClickEvent, _w, cx| {
                this.popover = Some(PopoverKind::BranchPicker);
                this.popover_anchor = Some(e.position());
                cx.notify();
            }));

        let pull = zed::SplitButton::new(
            zed::Button::new("pull_main", "Pull")
                .style(zed::ButtonStyle::Outlined)
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
                .style(zed::ButtonStyle::Outlined)
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
            .on_click(theme, cx, |this, _e, _w, cx| {
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
                        .child("Close")
                        .on_click(close),
                )
            }
            PopoverKind::BranchPicker => {
                let mut menu = div().flex().flex_col().min_w(px(260.0));
                if let Some(repo) = self.active_repo() {
                    match &repo.branches {
                        Loadable::Ready(branches) => {
                            for (ix, branch) in branches.iter().enumerate() {
                                let repo_id = repo.id;
                                let name = branch.name.clone();
                                menu = menu.child(
                                    div()
                                        .id(("branch_item", ix))
                                        .px_3()
                                        .py_2()
                                        .hover(move |s| s.bg(theme.colors.hover))
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

                menu = menu
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .text_color(theme.colors.text_muted)
                            .child("Create branch"),
                    )
                    .child(self.create_branch_input.clone())
                    .child(
                        kit::Button::new("create_branch_go", "Create")
                            .style(kit::ButtonStyle::Primary)
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
                            .child("Close")
                            .on_click(close),
                    );

                menu
            }
            PopoverKind::HistoryBranchFilter { repo_id } => {
                let mut menu = div().flex().flex_col().min_w(px(260.0));

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

                menu.child(
                    div()
                        .id("history_branch_close")
                        .px_3()
                        .py_2()
                        .hover(move |s| s.bg(theme.colors.hover))
                        .child("Close")
                        .on_click(close),
                )
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
            PopoverKind::CommitMenu { repo_id, commit_id } => {
                let sha = commit_id.as_ref().to_string();
                let short = sha.get(0..8).unwrap_or(&sha).to_string();
                let sha_for_clipboard = sha.clone();
                let commit_id_checkout = commit_id.clone();
                let commit_id_cherry_pick = commit_id.clone();
                let commit_id_revert = commit_id.clone();
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
                    .child(
                        div()
                            .id("commit_menu_copy_sha")
                            .px_3()
                            .py_2()
                            .hover(move |s| s.bg(theme.colors.hover))
                            .child("Copy SHA")
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
                            .child("Checkout (detached)")
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
                            .child("Cherry-pick")
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
                            .child("Revert")
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
                        .child("Diagnostics")
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.show_diagnostics_view = !this.show_diagnostics_view;
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
                    .bg(theme.colors.surface_bg)
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
            components::empty_state(theme, "Local", "No branches loaded.").into_any_element()
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
                .child(kit::Scrollbar::new("branches_scrollbar", scroll_handle).render(theme))
                .into_any_element()
        };

        let remotes_list: AnyElement = if remotes_count == 0 {
            components::empty_state(theme, "Remote", "No remotes loaded.").into_any_element()
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
                .child(kit::Scrollbar::new("remotes_scrollbar", scroll_handle).render(theme))
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(components::panel(theme, "Local", None, branches_list))
            .child(components::panel(theme, "Remote", None, remotes_list))
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
                        components::empty_state(theme, "Commit", "Loading…").into_any_element()
                    }
                    Loadable::Error(e) => {
                        components::empty_state(theme, "Commit", e.clone()).into_any_element()
                    }
                    Loadable::NotLoaded => {
                        components::empty_state(theme, "Commit", "Select a commit in History.")
                            .into_any_element()
                    }
                    Loadable::Ready(details) => {
                        if &details.id != selected_id {
                            components::empty_state(theme, "Commit", "Loading…").into_any_element()
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
                                let repo_id = repo.id;
                                let commit_id_for_list = details.id.clone();
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .children(details.files.iter().enumerate().map(|(ix, f)| {
                                        let (label, color) = match f.kind {
                                            FileStatusKind::Added => {
                                                ("Added", theme.colors.success)
                                            }
                                            FileStatusKind::Modified => {
                                                ("Modified", theme.colors.accent)
                                            }
                                            FileStatusKind::Deleted => {
                                                ("Deleted", theme.colors.danger)
                                            }
                                            FileStatusKind::Renamed => {
                                                ("Renamed", theme.colors.accent)
                                            }
                                            FileStatusKind::Untracked => {
                                                ("Untracked", theme.colors.warning)
                                            }
                                            FileStatusKind::Conflicted => {
                                                ("Conflicted", theme.colors.danger)
                                            }
                                        };

                                        let path = f.path.clone();
                                        let path_for_click = path.clone();
                                        let commit_id = commit_id_for_list.clone();
                                        div()
                                            .id(("commit_file", ix))
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .px_2()
                                            .py_1()
                                            .rounded(px(theme.radii.row))
                                            .hover(move |s| s.bg(theme.colors.hover))
                                            .child(components::pill(theme, label, color))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .line_clamp(1)
                                                    .child(path.display().to_string()),
                                            )
                                            .on_click(cx.listener(
                                                move |this, _e: &ClickEvent, _w, cx| {
                                                    this.store.dispatch(Msg::SelectDiff {
                                                        repo_id,
                                                        target: DiffTarget::Commit {
                                                            commit_id: commit_id.clone(),
                                                            path: Some(path_for_click.clone()),
                                                        },
                                                    });
                                                    cx.notify();
                                                },
                                            ))
                                    }))
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
                                .child(components::key_value(
                                    theme,
                                    "Commit SHA",
                                    details.id.as_ref().to_string(),
                                ))
                                .child(components::key_value(
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

                return div().flex().flex_col().gap_3().child(components::panel(
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
            .child(components::panel(
                theme,
                "Unstaged",
                Some(format!("{unstaged_count}").into()),
                unstaged_list,
            ))
            .child(components::panel(
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

        let diagnostics_count = repo.map(|r| r.diagnostics.len()).unwrap_or(0);

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child("Diagnostics"),
            )
            .child(
                zed::Button::new("diagnostics_close", "✕")
                    .style(zed::ButtonStyle::Transparent)
                    .on_click(theme, cx, |this, _e, _w, cx| {
                        this.show_diagnostics_view = false;
                        cx.notify();
                    }),
            );

        let body: AnyElement = if diagnostics_count == 0 {
            match repo {
                None => components::empty_state(theme, "Diagnostics", "No repository.")
                    .into_any_element(),
                Some(_) => {
                    components::empty_state(theme, "Diagnostics", "No issues.").into_any_element()
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
                    kit::Scrollbar::new("diagnostics_main_scrollbar", scroll_handle).render(theme),
                )
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .flex_1()
            .child(components::panel(
                theme,
                "Diagnostics",
                None,
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
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
            return components::empty_state(theme, "Status", "Clean.").into_any_element();
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
                    .child(kit::Scrollbar::new("unstaged_scrollbar", scroll_handle).render(theme))
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
                    .child(kit::Scrollbar::new("staged_scrollbar", scroll_handle).render(theme))
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
                                let message = this
                                    .commit_message_input
                                    .read_with(cx, |i, _| i.text().trim().to_string());
                                if let Some(repo_id) = this.active_repo_id()
                                    && !message.is_empty()
                                {
                                    this.store.dispatch(Msg::Commit { repo_id, message });
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
                            .on_click(cx.listener(|this, e: &ClickEvent, _w, cx| {
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
                        None => components::empty_state(theme, "History", "No repository.")
                            .into_any_element(),
                        Some(Loadable::Loading) => {
                            components::empty_state(theme, "History", "Loading…").into_any_element()
                        }
                        Some(Loadable::Error(e)) => {
                            components::empty_state(theme, "History", e.clone()).into_any_element()
                        }
                        Some(Loadable::NotLoaded) | Some(Loadable::Ready(_)) => {
                            components::empty_state(theme, "History", "No commits.")
                                .into_any_element()
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
                            kit::Scrollbar::new("history_main_scrollbar", scroll_handle)
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
                        None => components::empty_state(theme, "Stash", "No repository.")
                            .into_any_element(),
                        Some(Loadable::Loading) => {
                            components::empty_state(theme, "Stash", "Loading…").into_any_element()
                        }
                        Some(Loadable::Error(e)) => {
                            components::empty_state(theme, "Stash", e.clone()).into_any_element()
                        }
                        Some(Loadable::NotLoaded) | Some(Loadable::Ready(_)) => {
                            components::empty_state(theme, "Stash", "No stashes.")
                                .into_any_element()
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
                            kit::Scrollbar::new("stash_main_scrollbar", scroll_handle)
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
                        None => components::empty_state(theme, "Reflog", "No repository.")
                            .into_any_element(),
                        Some(Loadable::Loading) => {
                            components::empty_state(theme, "Reflog", "Loading…").into_any_element()
                        }
                        Some(Loadable::Error(e)) => {
                            components::empty_state(theme, "Reflog", e.clone()).into_any_element()
                        }
                        Some(Loadable::NotLoaded) | Some(Loadable::Ready(_)) => {
                            components::empty_state(theme, "Reflog", "No reflog.")
                                .into_any_element()
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
                            kit::Scrollbar::new("reflog_main_scrollbar", scroll_handle)
                                .render(theme),
                        )
                        .into_any_element()
                };

                ("Reflog".into(), body)
            }
        };

        div().flex().flex_col().gap_3().flex_1().child(
            components::panel(
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
        let repo = self.active_repo();
        let repo_id = self.active_repo_id();

        let title = repo
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

        let mut controls = div().flex().items_center().gap_1();
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
            );

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
            .child(div().text_sm().font_weight(FontWeight::BOLD).child(title))
            .child(controls);

        let body: AnyElement = match repo.map(|r| &r.diff) {
            None => components::empty_state(theme, "Diff", "No repository.").into_any_element(),
            Some(Loadable::NotLoaded) => {
                components::empty_state(theme, "Diff", "Select a file.").into_any_element()
            }
            Some(Loadable::Loading) => {
                components::empty_state(theme, "Diff", "Loading…").into_any_element()
            }
            Some(Loadable::Error(e)) => {
                components::empty_state(theme, "Diff", e.clone()).into_any_element()
            }
            Some(Loadable::Ready(_)) => {
                if self.diff_cache.is_empty() {
                    components::empty_state(theme, "Diff", "No differences.").into_any_element()
                } else {
                    let list = uniform_list(
                        "diff",
                        self.diff_cache.len(),
                        cx.processor(Self::render_diff_rows),
                    )
                    .h_full()
                    .track_scroll(self.diff_scroll.clone());
                    let scroll_handle = self.diff_scroll.0.borrow().base_handle.clone();
                    div()
                        .id("diff_scroll_container")
                        .relative()
                        .h_full()
                        .child(list)
                        .child(kit::Scrollbar::new("diff_scrollbar", scroll_handle).render(theme))
                        .into_any_element()
                }
            }
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .flex_1()
            .child(components::panel(
                theme,
                "Diff",
                None,
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(header)
                    .child(div().flex_1().child(body)),
            ))
    }
}
