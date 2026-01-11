use crate::{components, kit, theme::AppTheme, zed_port as zed};
use gitgpui_core::diff::{AnnotatedDiffLine, annotate_unified};
use gitgpui_core::domain::{
    Commit, CommitId, DiffArea, DiffTarget, FileStatus, FileStatusKind, RepoStatus,
};
use gitgpui_core::services::PullMode;
use gitgpui_state::model::{AppState, DiagnosticKind, Loadable, RepoId, RepoState};
use gitgpui_state::msg::{Msg, StoreEvent};
use gitgpui_state::store::AppStore;
use gpui::prelude::*;
use gpui::{
    AnyElement, Bounds, ClickEvent, Corner, CursorStyle, Decorations, Entity, FontWeight,
    MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, Render, ResizeEdge, ScrollHandle,
    SharedString, Size, Tiling, Timer, UniformListScrollHandle, WeakEntity, Window,
    WindowControlArea, anchored, div, point, px, size, uniform_list,
};
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::{Arc, mpsc};
use std::time::Duration;

mod chrome;
mod panels;
mod rows;

use chrome::{CLIENT_SIDE_DECORATION_INSET, cursor_style_for_resize_edge, resize_edge};

pub(crate) use chrome::window_frame;

const HISTORY_COL_BRANCH_PX: f32 = 160.0;
const HISTORY_COL_GRAPH_PX: f32 = 56.0;
const HISTORY_COL_DATE_PX: f32 = 160.0;
const HISTORY_COL_SHA_PX: f32 = 88.0;

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
    AppMenu,
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
    _appearance_subscription: gpui::Subscription,
    theme: AppTheme,

    diff_view: DiffViewMode,
    diff_cache: Vec<AnnotatedDiffLine>,

    open_repo_panel: bool,
    show_diagnostics_view: bool,
    open_repo_input: Entity<kit::TextInput>,
    commit_message_input: Entity<kit::TextInput>,
    create_branch_input: Entity<kit::TextInput>,

    popover: Option<PopoverKind>,
    popover_anchor: Option<Point<Pixels>>,

    title_should_move: bool,
    hover_resize_edge: Option<ResizeEdge>,

    branches_scroll: UniformListScrollHandle,
    remotes_scroll: UniformListScrollHandle,
    history_scroll: UniformListScrollHandle,
    unstaged_scroll: UniformListScrollHandle,
    staged_scroll: UniformListScrollHandle,
    diff_scroll: UniformListScrollHandle,
    diagnostics_scroll: UniformListScrollHandle,
    sidebar_scroll: ScrollHandle,
    commit_scroll: ScrollHandle,
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
        let initial_theme = AppTheme::default_for_window_appearance(window.appearance());

        if let Some(path) = initial_path.or_else(|| std::env::current_dir().ok()) {
            store.dispatch(Msg::OpenRepo(path));
        }

        let weak_view = cx.weak_entity();
        let poller = Poller::start(Arc::clone(&store), events, weak_view, window, cx);

        let appearance_subscription = {
            let view = cx.weak_entity();
            let mut first = true;
            window.observe_window_appearance(move |window, app| {
                if first {
                    first = false;
                    return;
                }
                let theme = AppTheme::default_for_window_appearance(window.appearance());
                let _ = view.update(app, |this, cx| {
                    this.set_theme(theme, cx);
                    cx.notify();
                });
            })
        };

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
            _appearance_subscription: appearance_subscription,
            theme: initial_theme,
            diff_view: DiffViewMode::Inline,
            diff_cache: Vec::new(),
            open_repo_panel: false,
            show_diagnostics_view: false,
            open_repo_input,
            commit_message_input,
            create_branch_input,
            popover: None,
            popover_anchor: None,
            title_should_move: false,
            hover_resize_edge: None,
            branches_scroll: UniformListScrollHandle::default(),
            remotes_scroll: UniformListScrollHandle::default(),
            history_scroll: UniformListScrollHandle::default(),
            unstaged_scroll: UniformListScrollHandle::default(),
            staged_scroll: UniformListScrollHandle::default(),
            diff_scroll: UniformListScrollHandle::default(),
            diagnostics_scroll: UniformListScrollHandle::default(),
            sidebar_scroll: ScrollHandle::new(),
            commit_scroll: ScrollHandle::new(),
        };

        view.set_theme(initial_theme, cx);
        view.rebuild_diff_cache();
        view
    }

    fn set_theme(&mut self, theme: AppTheme, cx: &mut gpui::Context<Self>) {
        self.theme = theme;
        self.open_repo_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.commit_message_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.create_branch_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
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

    #[cfg(test)]
    pub(crate) fn is_popover_open(&self) -> bool {
        self.popover.is_some()
    }
}

impl Render for GitGpuiView {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = self.theme;

        let decorations = window.window_decorations();
        let (tiling, client_inset) = match decorations {
            Decorations::Client { tiling } => (Some(tiling), CLIENT_SIDE_DECORATION_INSET),
            Decorations::Server => (None, px(0.0)),
        };
        window.set_client_inset(client_inset);

        let cursor = self
            .hover_resize_edge
            .map(cursor_style_for_resize_edge)
            .unwrap_or(CursorStyle::Arrow);

        let show_diff = self
            .active_repo()
            .and_then(|r| r.diff_target.as_ref())
            .is_some();
        let main_view = if self.show_diagnostics_view {
            self.diagnostics_view(cx)
        } else if show_diff {
            self.diff_view(cx)
        } else {
            self.history_view(cx)
        };

        let mut body = div()
            .flex()
            .flex_col()
            .size_full()
            .text_color(theme.colors.text)
            .child(self.title_bar(window, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap_2()
                    .child(div().px_3().pt_3().child(self.repo_tabs_bar(cx)))
                    .child(div().px_3().child(self.open_repo_panel(cx)))
                    .child(div().px_3().child(self.action_bar(cx)))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_3()
                            .flex_1()
                            .px_3()
                            .pb_3()
                            .child(
                                div()
                                    .id("sidebar_scroll")
                                    .w(px(280.0))
                                    .overflow_y_scroll()
                                    .track_scroll(&self.sidebar_scroll)
                                    .child(self.sidebar(cx))
                                    .child(
                                        kit::Scrollbar::new(
                                            "sidebar_scrollbar",
                                            self.sidebar_scroll.clone(),
                                        )
                                        .render(theme),
                                    ),
                            )
                            .child(main_view)
                            .child(
                                div()
                                    .id("commit_scroll")
                                    .w(px(420.0))
                                    .overflow_y_scroll()
                                    .track_scroll(&self.commit_scroll)
                                    .child(self.commit_details_view(cx))
                                    .child(
                                        kit::Scrollbar::new(
                                            "commit_scrollbar",
                                            self.commit_scroll.clone(),
                                        )
                                        .render(theme),
                                    ),
                            ),
                    ),
            );

        if let Some(repo) = self.active_repo()
            && let Some(err) = repo.last_error.as_ref()
        {
            body = body.child(
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

        let mut root = div().size_full().cursor(cursor);
        root = root.relative();

        if tiling.is_some() {
            root = root
                .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, window, cx| {
                    let Decorations::Client { tiling } = window.window_decorations() else {
                        if this.hover_resize_edge.is_some() {
                            this.hover_resize_edge = None;
                            cx.notify();
                        }
                        return;
                    };

                    let size = window.window_bounds().get_bounds().size;
                    let next = resize_edge(e.position, CLIENT_SIDE_DECORATION_INSET, size, tiling);
                    if next != this.hover_resize_edge {
                        this.hover_resize_edge = next;
                        cx.notify();
                    }
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_this, e: &MouseDownEvent, window, cx| {
                        let Decorations::Client { tiling } = window.window_decorations() else {
                            return;
                        };

                        let size = window.window_bounds().get_bounds().size;
                        let edge =
                            resize_edge(e.position, CLIENT_SIDE_DECORATION_INSET, size, tiling);
                        let Some(edge) = edge else {
                            return;
                        };

                        cx.stop_propagation();
                        window.start_window_resize(edge);
                    }),
                );
        } else if self.hover_resize_edge.is_some() {
            self.hover_resize_edge = None;
        }

        root = root.child(window_frame(theme, decorations, body.into_any_element()));

        if self.popover.is_some() {
            root = root.child(self.popover_layer(cx));
        }

        root
    }
}

pub(super) fn with_alpha(mut color: gpui::Rgba, alpha: f32) -> gpui::Rgba {
    color.a = alpha;
    color
}

impl GitGpuiView {
    fn popover_layer(&mut self, cx: &mut gpui::Context<Self>) -> AnyElement {
        let close = cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
            this.popover = None;
            this.popover_anchor = None;
            cx.notify();
        });

        let scrim = div()
            .id("popover_scrim")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(gpui::rgba(0x00000000))
            .occlude()
            .on_any_mouse_down(close);

        let popover = self
            .popover
            .and_then(|kind| Some(self.popover_view(kind, cx).into_any_element()))
            .unwrap_or_else(|| div().into_any_element());

        div()
            .id("popover_layer")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(scrim)
            .child(popover)
            .into_any_element()
    }
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
                let events = events;
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
        let mut repo = RepoState::new_opening(
            RepoId(1),
            RepoSpec {
                workdir: PathBuf::new(),
            },
        );
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

    #[test]
    fn resize_edge_detects_edges_and_corners() {
        let window_size = size(px(100.0), px(100.0));
        let tiling = Tiling::default();
        let inset = px(10.0);

        assert_eq!(
            resize_edge(point(px(0.0), px(0.0)), inset, window_size, tiling),
            Some(ResizeEdge::TopLeft)
        );
        assert_eq!(
            resize_edge(point(px(99.0), px(0.0)), inset, window_size, tiling),
            Some(ResizeEdge::TopRight)
        );
        assert_eq!(
            resize_edge(point(px(0.0), px(99.0)), inset, window_size, tiling),
            Some(ResizeEdge::BottomLeft)
        );
        assert_eq!(
            resize_edge(point(px(99.0), px(99.0)), inset, window_size, tiling),
            Some(ResizeEdge::BottomRight)
        );

        assert_eq!(
            resize_edge(point(px(50.0), px(0.0)), inset, window_size, tiling),
            Some(ResizeEdge::Top)
        );
        assert_eq!(
            resize_edge(point(px(50.0), px(99.0)), inset, window_size, tiling),
            Some(ResizeEdge::Bottom)
        );
        assert_eq!(
            resize_edge(point(px(0.0), px(50.0)), inset, window_size, tiling),
            Some(ResizeEdge::Left)
        );
        assert_eq!(
            resize_edge(point(px(99.0), px(50.0)), inset, window_size, tiling),
            Some(ResizeEdge::Right)
        );

        assert_eq!(
            resize_edge(point(px(50.0), px(50.0)), inset, window_size, tiling),
            None
        );
    }

    #[test]
    fn resize_edge_respects_tiling() {
        let window_size = size(px(100.0), px(100.0));
        let inset = px(10.0);
        let tiling = Tiling {
            top: true,
            left: false,
            right: false,
            bottom: false,
        };

        assert_eq!(
            resize_edge(point(px(0.0), px(0.0)), inset, window_size, tiling),
            Some(ResizeEdge::Left)
        );
        assert_eq!(
            resize_edge(point(px(50.0), px(0.0)), inset, window_size, tiling),
            None
        );
        assert_eq!(
            resize_edge(point(px(0.0), px(50.0)), inset, window_size, tiling),
            Some(ResizeEdge::Left)
        );
    }

    #[test]
    fn cursor_style_matches_resize_edge() {
        assert_eq!(
            cursor_style_for_resize_edge(ResizeEdge::Left),
            CursorStyle::ResizeLeftRight
        );
        assert_eq!(
            cursor_style_for_resize_edge(ResizeEdge::Top),
            CursorStyle::ResizeUpDown
        );
        assert_eq!(
            cursor_style_for_resize_edge(ResizeEdge::TopLeft),
            CursorStyle::ResizeUpLeftDownRight
        );
        assert_eq!(
            cursor_style_for_resize_edge(ResizeEdge::TopRight),
            CursorStyle::ResizeUpRightDownLeft
        );
    }
}
