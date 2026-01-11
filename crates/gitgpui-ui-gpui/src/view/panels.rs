use super::*;

impl GitGpuiView {
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
                .on_click(theme, cx, |this, _e, window, cx| this.prompt_open_repo(window, cx)),
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
            .child(div().text_sm().text_color(theme.colors.text_muted).child("Path"))
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
            .child(
                zed::Button::new("open_repo_cancel", "Cancel").on_click(
                    theme,
                    cx,
                    |this, _e, _w, cx| {
                        this.open_repo_panel = false;
                        cx.notify();
                    },
                ),
            )
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
            .child(div().text_sm().font_weight(FontWeight::BOLD).child("Repository"))
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
            .child(div().text_sm().font_weight(FontWeight::BOLD).child("Branch"))
            .child(div().text_sm().text_color(theme.colors.text_muted).child(branch))
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
        let anchor = self.popover_anchor.unwrap_or_else(|| point(px(64.0), px(64.0)));

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
                                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                            this.store.dispatch(Msg::CheckoutBranch { repo_id, name: name.clone() });
                                            this.popover = None;
                                            this.popover_anchor = None;
                                            cx.notify();
                                        })),
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
                                if let Some(repo_id) = this.active_repo_id() && !name.is_empty() {
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
            PopoverKind::AppMenu => {
                div()
                    .flex()
                    .flex_col()
                    .min_w(px(200.0))
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
                    )
            }
        };

        let offset_y = match kind {
            PopoverKind::AppMenu => px(40.0),
            _ => px(8.0),
        };

        anchored()
            .position(anchor)
            .anchor(Corner::TopLeft)
            .offset(point(px(0.0), offset_y))
            .child(
                div()
                    .id("app_popover")
                    .debug_selector(|| "app_popover".to_string())
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

        let remotes_count = repo.map(Self::remote_rows).map(|rows| rows.len()).unwrap_or(0);

        let commits_count = repo
            .and_then(|r| match &r.log {
                Loadable::Ready(v) => Some(v.commits.len()),
                _ => None,
            })
            .unwrap_or(0);

        let diagnostics_count = repo.map(|r| r.diagnostics.len()).unwrap_or(0);

        let (staged_count, unstaged_count) = repo
            .and_then(|r| match &r.status {
                Loadable::Ready(s) => Some((s.staged.len(), s.unstaged.len())),
                _ => None,
            })
            .unwrap_or((0, 0));

        let branches_list: AnyElement = if branches_count == 0 {
            components::empty_state(theme, "Local", "No branches loaded.").into_any_element()
        } else {
            uniform_list("branches", branches_count, cx.processor(Self::render_branch_rows))
                .h(px(140.0))
                .track_scroll(self.branches_scroll.clone())
                .into_any_element()
        };

        let remotes_list: AnyElement = if remotes_count == 0 {
            components::empty_state(theme, "Remote", "No remotes loaded.").into_any_element()
        } else {
            uniform_list("remotes", remotes_count, cx.processor(Self::render_remote_rows))
                .h(px(160.0))
                .track_scroll(self.remotes_scroll.clone())
                .into_any_element()
        };

        let commits_list: AnyElement = if commits_count == 0 {
            components::empty_state(theme, "History", "No commits loaded.").into_any_element()
        } else {
            uniform_list("history", commits_count, cx.processor(Self::render_commit_rows))
                .h(px(240.0))
                .track_scroll(self.commits_scroll.clone())
                .into_any_element()
        };

        let diagnostics_list: AnyElement = if diagnostics_count == 0 {
            components::empty_state(theme, "Diagnostics", "No issues.").into_any_element()
        } else {
            uniform_list(
                "diagnostics",
                diagnostics_count,
                cx.processor(Self::render_diagnostic_rows),
            )
            .h(px(140.0))
            .track_scroll(self.diagnostics_scroll.clone())
            .into_any_element()
        };

        let unstaged_list = self.status_list(cx, DiffArea::Unstaged, unstaged_count);
        let staged_list = self.status_list(cx, DiffArea::Staged, staged_count);

        let commit_box = self.commit_box(cx);

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(components::panel(theme, "Local", None, branches_list))
            .child(components::panel(theme, "Remote", None, remotes_list))
            .child(components::panel(theme, "History", None, commits_list))
            .child(components::panel(
                theme,
                "Diagnostics",
                Some(format!("{diagnostics_count}").into()),
                diagnostics_list,
            ))
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
            DiffArea::Unstaged => uniform_list("unstaged", count, cx.processor(Self::render_unstaged_rows))
                .h(px(140.0))
                .track_scroll(self.unstaged_scroll.clone())
                .into_any_element(),
            DiffArea::Staged => uniform_list("staged", count, cx.processor(Self::render_staged_rows))
                .h(px(140.0))
                .track_scroll(self.staged_scroll.clone())
                .into_any_element(),
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
                                if let Some(repo_id) = this.active_repo_id() && !message.is_empty()
                                {
                                    this.store.dispatch(Msg::Commit { repo_id, message });
                                }
                                cx.notify();
                            }),
                    ),
            )
    }

    pub(super) fn diff_view(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let repo = self.active_repo();

        let title = repo
            .and_then(|r| r.diff_target.as_ref())
            .map(|t| {
                format!(
                    "{}: {}",
                    if t.area == DiffArea::Staged {
                        "staged"
                    } else {
                        "unstaged"
                    },
                    t.path.display()
                )
            })
            .unwrap_or_else(|| "Select a file to view diff".to_string());

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(div().text_sm().font_weight(FontWeight::BOLD).child(title))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
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
                    ),
            );

        let body: AnyElement = match repo.map(|r| &r.diff) {
            None => components::empty_state(theme, "Diff", "No repository.").into_any_element(),
            Some(Loadable::NotLoaded) => {
                components::empty_state(theme, "Diff", "Select a file.").into_any_element()
            }
            Some(Loadable::Loading) => components::empty_state(theme, "Diff", "Loading…").into_any_element(),
            Some(Loadable::Error(e)) => components::empty_state(theme, "Diff", e.clone()).into_any_element(),
            Some(Loadable::Ready(_)) => {
                if self.diff_cache.is_empty() {
                    components::empty_state(theme, "Diff", "No differences.").into_any_element()
                } else {
                    uniform_list("diff", self.diff_cache.len(), cx.processor(Self::render_diff_rows))
                        .h_full()
                        .track_scroll(self.diff_scroll.clone())
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
