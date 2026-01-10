use crate::{components, kit, theme::AppTheme, zed_port as zed};
use gitgpui_core::diff::{AnnotatedDiffLine, annotate_unified};
use gitgpui_core::domain::{Commit, CommitId, DiffArea, DiffTarget, FileStatus, FileStatusKind, RepoStatus};
use gitgpui_core::services::PullMode;
use gitgpui_state::msg::{Msg, StoreEvent};
use gitgpui_state::model::{AppState, Loadable, RepoId, RepoState};
use gitgpui_state::store::AppStore;
use gpui::prelude::*;
use gpui::{
    AnyElement, ClickEvent, Corner, Entity, FontWeight, Pixels, Point, Render, SharedString, Timer,
    UniformListScrollHandle, WeakEntity, Window, anchored, div, point, px, uniform_list,
};
use std::ops::Range;
use std::sync::{Arc, mpsc};
use std::time::Duration;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffViewMode {
    Inline,
    Split,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PopoverKind {
    RepoPicker,
    BranchPicker,
    PullPicker,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RemoteRow {
    Header(String),
    Branch { remote: String, name: String },
}

pub struct GitGpuiView {
    store: Arc<AppStore>,
    state: AppState,
    _poller: Poller,
    theme: AppTheme,

    diff_view: DiffViewMode,
    diff_cache: Vec<AnnotatedDiffLine>,

    open_repo_panel: bool,
    open_repo_input: Entity<kit::TextInput>,
    commit_message_input: Entity<kit::TextInput>,
    create_branch_input: Entity<kit::TextInput>,

    popover: Option<PopoverKind>,
    popover_anchor: Option<Point<Pixels>>,

    branches_scroll: UniformListScrollHandle,
    remotes_scroll: UniformListScrollHandle,
    commits_scroll: UniformListScrollHandle,
    unstaged_scroll: UniformListScrollHandle,
    staged_scroll: UniformListScrollHandle,
    diff_scroll: UniformListScrollHandle,
}

impl GitGpuiView {
    pub fn new(
        store: AppStore,
        events: mpsc::Receiver<StoreEvent>,
        initial_path: Option<std::path::PathBuf>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let store = Arc::new(store);

        if let Some(path) = initial_path.or_else(|| std::env::current_dir().ok()) {
            store.dispatch(Msg::OpenRepo(path));
        }

        let weak_view = cx.weak_entity();
        let poller = Poller::start(Arc::clone(&store), events, weak_view, window, cx);

        let open_repo_input = cx.new(|cx| {
            kit::TextInput::new(
                kit::TextInputOptions {
                    placeholder: "/path/to/repo".into(),
                    multiline: false,
                },
                window,
                cx,
            )
        });

        let commit_message_input = cx.new(|cx| {
            kit::TextInput::new(
                kit::TextInputOptions {
                    placeholder: "Enter commit message…".into(),
                    multiline: false,
                },
                window,
                cx,
            )
        });

        let create_branch_input = cx.new(|cx| {
            kit::TextInput::new(
                kit::TextInputOptions {
                    placeholder: "new-branch-name".into(),
                    multiline: false,
                },
                window,
                cx,
            )
        });

        let mut view = Self {
            state: store.snapshot(),
            store,
            _poller: poller,
            theme: AppTheme::zed_one_dark(),
            diff_view: DiffViewMode::Inline,
            diff_cache: Vec::new(),
            open_repo_panel: false,
            open_repo_input,
            commit_message_input,
            create_branch_input,
            popover: None,
            popover_anchor: None,
            branches_scroll: UniformListScrollHandle::default(),
            remotes_scroll: UniformListScrollHandle::default(),
            commits_scroll: UniformListScrollHandle::default(),
            unstaged_scroll: UniformListScrollHandle::default(),
            staged_scroll: UniformListScrollHandle::default(),
            diff_scroll: UniformListScrollHandle::default(),
        };

        view.rebuild_diff_cache();
        view
    }

    fn active_repo_id(&self) -> Option<RepoId> {
        self.state.active_repo
    }

    fn active_repo(&self) -> Option<&RepoState> {
        let repo_id = self.active_repo_id()?;
        self.state.repos.iter().find(|r| r.id == repo_id)
    }

    fn remote_rows(repo: &RepoState) -> Vec<RemoteRow> {
        let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();

        if let Loadable::Ready(remote_branches) = &repo.remote_branches {
            for branch in remote_branches {
                grouped
                    .entry(branch.remote.clone())
                    .or_default()
                    .push(branch.name.clone());
            }
        }

        if grouped.is_empty() {
            if let Loadable::Ready(remotes) = &repo.remotes {
                for remote in remotes {
                    grouped.entry(remote.name.clone()).or_default();
                }
            }
        }

        let mut rows = Vec::new();
        for (remote, mut branches) in grouped {
            branches.sort();
            branches.dedup();
            rows.push(RemoteRow::Header(remote.clone()));
            for name in branches {
                rows.push(RemoteRow::Branch {
                    remote: remote.clone(),
                    name,
                });
            }
        }

        rows
    }

    fn prompt_open_repo(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
        #[cfg(target_os = "linux")]
        {
            let store = Arc::clone(&self.store);
            let view = cx.weak_entity();
            window
                .spawn(cx, async move |cx| {
                    use ashpd::desktop::file_chooser::OpenFileRequest;

                    let request = match OpenFileRequest::default()
                        .title("Open Git Repository")
                        .multiple(false)
                        .directory(true)
                        .send()
                        .await
                    {
                        Ok(request) => request,
                        Err(_) => {
                            let _ = view.update(cx, |this, cx| {
                                this.open_repo_panel = true;
                                cx.notify();
                            });
                            return;
                        }
                    };

                    let response = match request.response() {
                        Ok(response) => response,
                        Err(_) => return,
                    };

                    let Some(path) = response
                        .uris()
                        .first()
                        .and_then(|uri| uri.to_file_path().ok())
                    else {
                        return;
                    };

                    if path.join(".git").is_dir() {
                        store.dispatch(Msg::OpenRepo(path));
                        let _ = view.update(cx, |this, cx| {
                            this.open_repo_panel = false;
                            cx.notify();
                        });
                    } else {
                        let _ = view.update(cx, |this, cx| {
                            this.open_repo_panel = true;
                            this.open_repo_input.update(cx, |input, cx| {
                                input.set_text(path.display().to_string(), cx)
                            });
                            cx.notify();
                        });
                    }
                })
                .detach();
            return;
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = window;
            self.open_repo_panel = !self.open_repo_panel;
            self.popover = None;
            self.popover_anchor = None;
            cx.notify();
        }
    }

    fn rebuild_diff_cache(&mut self) {
        self.diff_cache.clear();
        let Some(repo) = self.active_repo() else {
            return;
        };
        let Loadable::Ready(diff) = &repo.diff else {
            return;
        };
        self.diff_cache = annotate_unified(diff);
    }

    fn repo_tabs_bar(&mut self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let active = self.active_repo_id();
        let repos_len = self.state.repos.len();

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
                zed::TabPosition::Middle(std::cmp::Ordering::Equal)
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
            zed::Button::new("add_repo", "Open Repo…")
                .style(zed::ButtonStyle::Subtle)
                .on_click(theme, cx, |this, _e, window, cx| this.prompt_open_repo(window, cx)),
        )
        .render(theme)
    }

    fn open_repo_panel(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
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

    fn action_bar(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
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
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .rounded(px(theme.radii.row))
            .hover(move |s| s.bg(theme.colors.hover))
            .child(div().text_sm().font_weight(FontWeight::BOLD).child("Repository"))
            .child(div().text_sm().text_color(theme.colors.text_muted).line_clamp(1).child(repo_title))
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

        let mut bar = div()
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
            .child(div().flex().items_center().gap_2().flex_1().child(repo_picker).child(branch_picker))
            .child(div().flex().items_center().gap_2().child(pull).child(push).child(create_branch).child(stash));

        if let Some(kind) = self.popover {
            bar = bar.child(self.popover_view(kind, cx));
        }

        bar
    }

    fn popover_view(&mut self, kind: PopoverKind, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let anchor = self
            .popover_anchor
            .unwrap_or_else(|| point(px(64.0), px(64.0)));

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
                        .id(("popover_close", 0usize))
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
                    .child(div().px_3().py_2().text_color(theme.colors.text_muted).child("Create branch"))
                    .child(self.create_branch_input.clone())
                    .child(
                        kit::Button::new("create_branch_go", "Create")
                            .style(kit::ButtonStyle::Primary)
                            .on_click(theme, cx, |this, _e, _w, cx| {
                                let name = this.create_branch_input.read_with(cx, |i, _| i.text().trim().to_string());
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
            PopoverKind::PullPicker => {
                let repo_id = self.active_repo_id();
                let mut menu = div().flex().flex_col().min_w(px(280.0));
                for (ix, (label, mode)) in [
                    ("Fetch all", None),
                    ("Pull (default)", Some(PullMode::Default)),
                    ("Pull (fast-forward if possible)", Some(PullMode::FastForwardIfPossible)),
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
        };

        anchored()
            .position(anchor)
            .anchor(Corner::TopLeft)
            .offset(point(px(0.0), px(8.0)))
            .child(
                div()
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

    fn sidebar(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
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

        let commits_count = repo
            .and_then(|r| match &r.log {
                Loadable::Ready(v) => Some(v.commits.len()),
                _ => None,
            })
            .unwrap_or(0);

        let (staged_count, unstaged_count) = repo
            .and_then(|r| match &r.status {
                Loadable::Ready(s) => Some((s.staged.len(), s.unstaged.len())),
                _ => None,
            })
            .unwrap_or((0, 0));

        let branches_list: AnyElement = if branches_count == 0 {
            components::empty_state(theme, "Local", "No branches loaded.").into_any_element()
        } else {
            uniform_list(
                "branches",
                branches_count,
                cx.processor(Self::render_branch_rows),
            )
            .h(px(140.0))
            .track_scroll(self.branches_scroll.clone())
            .into_any_element()
        };

        let remotes_list: AnyElement = if remotes_count == 0 {
            components::empty_state(theme, "Remote", "No remotes loaded.").into_any_element()
        } else {
            uniform_list(
                "remotes",
                remotes_count,
                cx.processor(Self::render_remote_rows),
            )
            .h(px(160.0))
            .track_scroll(self.remotes_scroll.clone())
            .into_any_element()
        };

        let commits_list: AnyElement = if commits_count == 0 {
            components::empty_state(theme, "History", "No commits loaded.").into_any_element()
        } else {
            uniform_list(
                "history",
                commits_count,
                cx.processor(Self::render_commit_rows),
            )
            .h(px(240.0))
            .track_scroll(self.commits_scroll.clone())
            .into_any_element()
        };

        let unstaged_list = self.status_list(cx, DiffArea::Unstaged, unstaged_count);
        let staged_list = self.status_list(cx, DiffArea::Staged, staged_count);

        let commit_box = self.commit_box(cx);

        div()
            .flex()
            .flex_col()
            .gap_3()
            .w(px(420.0))
            .child(components::panel(theme, "Local", None, branches_list))
            .child(components::panel(theme, "Remote", None, remotes_list))
            .child(components::panel(theme, "History", None, commits_list))
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

    fn status_list(
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
            DiffArea::Unstaged => uniform_list(
                "unstaged",
                count,
                cx.processor(Self::render_unstaged_rows),
            )
            .h(px(140.0))
            .track_scroll(self.unstaged_scroll.clone())
            .into_any_element(),
            DiffArea::Staged => uniform_list(
                "staged",
                count,
                cx.processor(Self::render_staged_rows),
            )
            .h(px(140.0))
            .track_scroll(self.staged_scroll.clone())
            .into_any_element(),
        }
    }

    fn commit_box(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
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

    fn diff_view(&mut self, cx: &mut gpui::Context<Self>) -> gpui::Div {
        let theme = self.theme;
        let repo = self.active_repo();

        let title = repo
            .and_then(|r| r.diff_target.as_ref())
            .map(|t| format!("{}: {}", if t.area == DiffArea::Staged { "staged" } else { "unstaged" }, t.path.display()))
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
            Some(Loadable::NotLoaded) => components::empty_state(theme, "Diff", "Select a file.")
                .into_any_element(),
            Some(Loadable::Loading) => {
                components::empty_state(theme, "Diff", "Loading…").into_any_element()
            }
            Some(Loadable::Error(e)) => components::empty_state(theme, "Diff", e.clone()).into_any_element(),
            Some(Loadable::Ready(_)) => {
                if self.diff_cache.is_empty() {
                    components::empty_state(theme, "Diff", "No differences.").into_any_element()
                } else {
                    uniform_list(
                        "diff",
                        self.diff_cache.len(),
                        cx.processor(Self::render_diff_rows),
                    )
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
            .child(components::panel(theme, "Diff", None, div().flex().flex_col().gap_2().child(header).child(div().flex_1().child(body))))
    }

    fn render_branch_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let Loadable::Ready(branches) = &repo.branches else {
            return Vec::new();
        };
        let theme = this.theme;
        range
            .filter_map(|ix| branches.get(ix).map(|b| (ix, b)))
            .map(|(ix, branch)| {
                div()
                    .id(ix)
                    .px_2()
                    .py_1()
                    .rounded(px(theme.radii.row))
                    .hover(move |s| s.bg(theme.colors.hover))
                    .child(branch.name.clone())
                    .into_any_element()
            })
            .collect()
    }

    fn render_remote_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let rows = Self::remote_rows(repo);
        let theme = this.theme;
        range
            .filter_map(|ix| rows.get(ix).cloned().map(|r| (ix, r)))
            .map(|(ix, row)| match row {
                RemoteRow::Header(name) => div()
                    .id(("remote_hdr", ix))
                    .px_2()
                    .py_1()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.colors.text)
                    .child(name)
                    .into_any_element(),
                RemoteRow::Branch { remote: _, name } => div()
                    .id(("remote_branch", ix))
                    .px_2()
                    .py_1()
                    .pl_4()
                    .rounded(px(theme.radii.row))
                    .hover(move |s| s.bg(theme.colors.hover))
                    .text_sm()
                    .text_color(theme.colors.text_muted)
                    .child(name)
                    .into_any_element(),
            })
            .collect()
    }

    fn render_commit_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let Loadable::Ready(page) = &repo.log else {
            return Vec::new();
        };
        let theme = this.theme;
        range
            .filter_map(|ix| page.commits.get(ix).map(|c| (ix, c)))
            .map(|(ix, commit)| commit_row(theme, ix, commit))
            .collect()
    }

    fn render_unstaged_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let Loadable::Ready(RepoStatus { unstaged, .. }) = &repo.status else {
            return Vec::new();
        };
        let theme = this.theme;
        range
            .filter_map(|ix| unstaged.get(ix).map(|e| (ix, e)))
            .map(|(ix, entry)| status_row(theme, ix, entry, DiffArea::Unstaged, repo.id, cx))
            .collect()
    }

    fn render_staged_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let Loadable::Ready(RepoStatus { staged, .. }) = &repo.status else {
            return Vec::new();
        };
        let theme = this.theme;
        range
            .filter_map(|ix| staged.get(ix).map(|e| (ix, e)))
            .map(|(ix, entry)| status_row(theme, ix, entry, DiffArea::Staged, repo.id, cx))
            .collect()
    }

    fn render_diff_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let theme = this.theme;
        range
            .filter_map(|ix| this.diff_cache.get(ix).map(|l| (ix, l)))
            .map(|(ix, line)| diff_row(theme, ix, this.diff_view, line))
            .collect()
    }
}

impl Render for GitGpuiView {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = self.theme;

        let mut root = div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.colors.window_bg)
            .text_color(theme.colors.text)
            .p_3()
            .gap_3()
            .child(self.repo_tabs_bar(cx))
            .child(self.open_repo_panel(cx))
            .child(self.action_bar(cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_3()
                    .flex_1()
                    .child(self.sidebar(cx))
                    .child(self.diff_view(cx)),
            );

        if let Some(repo) = self.active_repo()
            && let Some(err) = repo.last_error.as_ref()
        {
            root = root.child(
                div()
                    .px_3()
                    .py_2()
                    .bg(with_alpha(theme.colors.danger, 0.15))
                    .border_1()
                    .border_color(with_alpha(theme.colors.danger, 0.3))
                    .rounded(px(theme.radii.panel))
                    .child(err.clone()),
            );
        }

        root
    }
}

fn commit_row(theme: AppTheme, ix: usize, commit: &Commit) -> AnyElement {
    let id: &str = <CommitId as AsRef<str>>::as_ref(&commit.id);
    let short = id.get(0..8).unwrap_or(id);
    div()
        .id(ix)
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .px_2()
        .py_1()
        .rounded(px(theme.radii.row))
        .hover(move |s| s.bg(theme.colors.hover))
        .child(div().text_sm().line_clamp(1).child(commit.summary.clone()))
        .child(div().text_xs().text_color(theme.colors.text_muted).child(short.to_string()))
        .into_any_element()
}

fn status_row(
    theme: AppTheme,
    ix: usize,
    entry: &FileStatus,
    area: DiffArea,
    repo_id: RepoId,
    cx: &mut gpui::Context<GitGpuiView>,
) -> AnyElement {
    let (label, color) = match entry.kind {
        FileStatusKind::Untracked => ("Untracked", theme.colors.warning),
        FileStatusKind::Modified => ("Modified", theme.colors.accent),
        FileStatusKind::Added => ("Added", theme.colors.success),
        FileStatusKind::Deleted => ("Deleted", theme.colors.danger),
        FileStatusKind::Renamed => ("Renamed", theme.colors.accent),
        FileStatusKind::Conflicted => ("Conflicted", theme.colors.danger),
    };

    let path = entry.path.clone();
    let path_for_stage = path.clone();
    let path_for_row = path.clone();
    let stage_label = match area {
        DiffArea::Unstaged => "Stage",
        DiffArea::Staged => "Unstage",
    };

    div()
        .id(ix)
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .px_2()
        .py_1()
        .rounded(px(theme.radii.row))
        .hover(move |s| s.bg(theme.colors.hover))
        .child(div().flex().items_center().gap_2().child(components::pill(theme, label, color)).child(div().text_sm().line_clamp(1).child(path.display().to_string())))
        .child(
            kit::Button::new(format!("stage_btn_{ix}"), stage_label)
                .style(kit::ButtonStyle::Secondary)
                .on_click(theme, cx, move |this, _e, _w, cx| {
                    this.store.dispatch(Msg::SelectDiff {
                        repo_id,
                        target: DiffTarget { path: path_for_stage.clone(), area },
                    });
                    match area {
                        DiffArea::Unstaged => this.store.dispatch(Msg::StagePath { repo_id, path: path_for_stage.clone() }),
                        DiffArea::Staged => this.store.dispatch(Msg::UnstagePath { repo_id, path: path_for_stage.clone() }),
                    }
                    cx.notify();
                }),
        )
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.store.dispatch(Msg::SelectDiff {
                repo_id,
                target: DiffTarget { path: path_for_row.clone(), area },
            });
            this.rebuild_diff_cache();
            cx.notify();
        }))
        .into_any_element()
}

fn diff_row(theme: AppTheme, ix: usize, mode: DiffViewMode, line: &AnnotatedDiffLine) -> AnyElement {
    let (bg, fg, gutter_fg) = match line.kind {
        gitgpui_core::domain::DiffLineKind::Header => (theme.colors.surface_bg, theme.colors.text_muted, theme.colors.text_muted),
        gitgpui_core::domain::DiffLineKind::Hunk => (theme.colors.surface_bg_elevated, theme.colors.accent, theme.colors.text_muted),
        gitgpui_core::domain::DiffLineKind::Add => (gpui::rgb(0x0B2E1C), gpui::rgb(0xBBF7D0), gpui::rgb(0x86EFAC)),
        gitgpui_core::domain::DiffLineKind::Remove => (gpui::rgb(0x3A0D13), gpui::rgb(0xFECACA), gpui::rgb(0xFCA5A5)),
        gitgpui_core::domain::DiffLineKind::Context => (theme.colors.surface_bg_elevated, theme.colors.text, theme.colors.text_muted),
    };

    let text = match line.kind {
        gitgpui_core::domain::DiffLineKind::Add => line.text.strip_prefix('+').unwrap_or(&line.text),
        gitgpui_core::domain::DiffLineKind::Remove => line.text.strip_prefix('-').unwrap_or(&line.text),
        gitgpui_core::domain::DiffLineKind::Context => line.text.strip_prefix(' ').unwrap_or(&line.text),
        _ => &line.text,
    };

    let old = line.old_line.map(|n| n.to_string()).unwrap_or_default();
    let new = line.new_line.map(|n| n.to_string()).unwrap_or_default();

    let (left_text, right_text) = match (mode, line.kind) {
        (DiffViewMode::Split, gitgpui_core::domain::DiffLineKind::Remove) => (text.to_string(), String::new()),
        (DiffViewMode::Split, gitgpui_core::domain::DiffLineKind::Add) => (String::new(), text.to_string()),
        (DiffViewMode::Split, gitgpui_core::domain::DiffLineKind::Context) => (text.to_string(), text.to_string()),
        (DiffViewMode::Split, _) => (text.to_string(), String::new()),
        (DiffViewMode::Inline, _) => (text.to_string(), String::new()),
    };

    let row = div()
        .id(ix)
        .h(px(20.0))
        .flex()
        .items_center()
        .bg(bg)
        .font_family("monospace")
        .text_xs()
        .child(div().w(px(44.0)).px_2().text_color(gutter_fg).whitespace_nowrap().child(old))
        .child(div().w(px(44.0)).px_2().text_color(gutter_fg).whitespace_nowrap().child(new));

    match mode {
        DiffViewMode::Inline => row
            .child(div().flex_1().px_2().text_color(fg).whitespace_nowrap().child(left_text))
            .into_any_element(),
        DiffViewMode::Split => row
            .child(div().flex_1().px_2().text_color(fg).whitespace_nowrap().child(left_text))
            .child(div().flex_1().px_2().text_color(fg).whitespace_nowrap().child(right_text))
            .into_any_element(),
    }
}

fn with_alpha(mut color: gpui::Rgba, alpha: f32) -> gpui::Rgba {
    color.a = alpha;
    color
}

struct Poller;

impl Poller {
    fn start(
        store: Arc<AppStore>,
        events: mpsc::Receiver<StoreEvent>,
        view: WeakEntity<GitGpuiView>,
        window: &mut Window,
        cx: &mut gpui::Context<GitGpuiView>,
    ) -> Poller {
        window
            .spawn(cx, async move |cx| {
                let mut events = events;
                loop {
                    let mut changed = false;
                    while events.try_recv().is_ok() {
                        changed = true;
                    }

                    if changed {
                        let snapshot = store.snapshot();
                        let _ = view.update(cx, |view, cx| {
                            view.state = snapshot;
                            view.rebuild_diff_cache();
                            cx.notify();
                        });
                    }

                    Timer::after(Duration::from_millis(33)).await;
                }
            })
            .detach();

        Poller
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitgpui_core::domain::{RemoteBranch, RepoSpec};
    use std::path::PathBuf;

    #[test]
    fn remote_rows_groups_and_sorts() {
        let mut repo = RepoState::new_opening(RepoId(1), RepoSpec { workdir: PathBuf::new() });
        repo.remote_branches = Loadable::Ready(vec![
            RemoteBranch {
                remote: "origin".to_string(),
                name: "b".to_string(),
            },
            RemoteBranch {
                remote: "origin".to_string(),
                name: "a".to_string(),
            },
            RemoteBranch {
                remote: "upstream".to_string(),
                name: "main".to_string(),
            },
        ]);

        let rows = GitGpuiView::remote_rows(&repo);
        assert_eq!(
            rows,
            vec![
                RemoteRow::Header("origin".to_string()),
                RemoteRow::Branch {
                    remote: "origin".to_string(),
                    name: "a".to_string()
                },
                RemoteRow::Branch {
                    remote: "origin".to_string(),
                    name: "b".to_string()
                },
                RemoteRow::Header("upstream".to_string()),
                RemoteRow::Branch {
                    remote: "upstream".to_string(),
                    name: "main".to_string()
                },
            ]
        );
    }
}
