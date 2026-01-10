use gitgpui_core::services::GitBackend;
use gitgpui_state::msg::Msg;
use gitgpui_state::store::AppStore;
use gpui::{
    App, Application, Bounds, Context, SharedString, Timer, WeakEntity, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size, uniform_list,
};
use std::sync::{mpsc, Arc};
use std::time::Duration;

pub fn run(backend: Arc<dyn GitBackend>) {
    let initial_path = std::env::args_os().nth(1).map(std::path::PathBuf::from);

    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1100.0), px(700.0)), cx);
        let backend = Arc::clone(&backend);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            move |window, cx| {
                let (store, events) = AppStore::new(Arc::clone(&backend));
                cx.new(|cx| GitGpuiView::new(store, events, initial_path.clone(), window, cx))
            },
        )
        .unwrap();

        cx.activate(true);
    });
}

struct GitGpuiView {
    _store: Arc<AppStore>,
    state: gitgpui_state::model::AppState,
    _poller: Poller,
    commits_scroll: gpui::UniformListScrollHandle,
    status_scroll: gpui::UniformListScrollHandle,
}

impl GitGpuiView {
    fn new(
        store: AppStore,
        events: mpsc::Receiver<gitgpui_state::msg::StoreEvent>,
        initial_path: Option<std::path::PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let store = Arc::new(store);

        if let Some(path) = initial_path.or_else(|| std::env::current_dir().ok()) {
            store.dispatch(Msg::OpenRepo(path));
        }

        let weak_view = cx.weak_entity();
        let poller = Poller::start(Arc::clone(&store), events, weak_view, window, cx);

        Self {
            state: store.snapshot(),
            _store: store,
            _poller: poller,
            commits_scroll: gpui::UniformListScrollHandle::default(),
            status_scroll: gpui::UniformListScrollHandle::default(),
        }
    }
}

impl Render for GitGpuiView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let repo_line: SharedString = match &self.state.current_repo {
            None => "No repository".into(),
            Some(repo) => format!("Repo: {}", repo.spec.workdir.display()).into(),
        };

        let mut open_state = "n/a".to_string();
        let mut branches_state = "n/a".to_string();
        let mut status_state = "n/a".to_string();
        let mut remotes_state = "n/a".to_string();
        let mut log_state = "n/a".to_string();
        let mut status_count = 0usize;
        if let Some(repo) = &self.state.current_repo {
            open_state = format_loadable_unit(&repo.open);
            branches_state = format_loadable_len("branches", &repo.branches);
            remotes_state = format_loadable_len("remotes", &repo.remotes);
            status_state = format_loadable_len("changes", &repo.status);
            log_state = match &repo.log {
                gitgpui_state::model::Loadable::NotLoaded => "log: not loaded".to_string(),
                gitgpui_state::model::Loadable::Loading => "log: loading".to_string(),
                gitgpui_state::model::Loadable::Error(e) => format!("log: error: {e}"),
                gitgpui_state::model::Loadable::Ready(page) => {
                    format!("log: {} commits", page.commits.len())
                }
            };
            if let gitgpui_state::model::Loadable::Ready(changes) = &repo.status {
                status_count = changes.len();
            }
        }

        let commit_count = self
            .state
            .current_repo
            .as_ref()
            .and_then(|r| match &r.log {
                gitgpui_state::model::Loadable::Ready(page) => Some(page.commits.len()),
                _ => None,
            })
            .unwrap_or(0);

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x101418))
            .text_color(rgb(0xE6E6E6))
            .p_3()
            .gap_3()
            .child(div().text_lg().child("GitGpui (MVP shell)"))
            .child(div().child(repo_line))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .text_sm()
                    .child(format!("open={open_state}"))
                    .child(format!("branches={branches_state}"))
                    .child(format!("remotes={remotes_state}"))
                    .child(format!("status={status_state}"))
                    .child(format!("log={log_state}")),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_3()
                    .h_full()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .flex_1()
                            .child(div().text_sm().child(format!("Commits: {commit_count}")))
                            .child(
                                uniform_list(
                                    "commits",
                                    commit_count,
                                    cx.processor(|this, range, _window, _cx| {
                                        let Some(repo) = this.state.current_repo.as_ref() else {
                                            return Vec::new();
                                        };
                                        let gitgpui_state::model::Loadable::Ready(page) = &repo.log
                                        else {
                                            return Vec::new();
                                        };

                                        let mut items = Vec::new();
                                        for ix in range {
                                            if let Some(commit) = page.commits.get(ix) {
                                                let commit: &gitgpui_core::domain::Commit = commit;
                                                let id: &str = <gitgpui_core::domain::CommitId as AsRef<
                                                    str,
                                                >>::as_ref(&commit.id);
                                                let short = id.get(0..8).unwrap_or(id);
                                                items.push(
                                                    div()
                                                        .id(ix)
                                                        .px_2()
                                                        .py_1()
                                                        .border_1()
                                                        .border_color(rgb(0x1F2A33))
                                                        .child(format!(
                                                            "{short}  {}  —  {}",
                                                            commit.summary, commit.author
                                                        )),
                                                );
                                            }
                                        }
                                        items
                                    }),
                                )
                                .h_full()
                                .track_scroll(self.commits_scroll.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .w(px(380.0))
                            .child(div().text_sm().child(format!("Changes: {status_count}")))
                            .child(
                                uniform_list(
                                    "status",
                                    status_count,
                                    cx.processor(|this, range, _window, _cx| {
                                        let Some(repo) = this.state.current_repo.as_ref() else {
                                            return Vec::new();
                                        };
                                        let gitgpui_state::model::Loadable::Ready(changes) =
                                            &repo.status
                                        else {
                                            return Vec::new();
                                        };

                                        let mut items = Vec::new();
                                        for ix in range {
                                            if let Some(entry) = changes.get(ix) {
                                                let entry: &gitgpui_core::domain::FileStatus = entry;
                                                items.push(
                                                    div()
                                                        .id(ix)
                                                        .px_2()
                                                        .py_1()
                                                        .border_1()
                                                        .border_color(rgb(0x1F2A33))
                                                        .child(format!(
                                                            "{:?}  {}",
                                                            entry.kind,
                                                            entry.path.display()
                                                        )),
                                                );
                                            }
                                        }
                                        items
                                    }),
                                )
                                .h_full()
                                .track_scroll(self.status_scroll.clone()),
                            ),
                    ),
            )
    }
}

fn format_loadable_unit(v: &gitgpui_state::model::Loadable<()>) -> String {
    match v {
        gitgpui_state::model::Loadable::NotLoaded => "open: not loaded".to_string(),
        gitgpui_state::model::Loadable::Loading => "open: loading".to_string(),
        gitgpui_state::model::Loadable::Ready(()) => "open: ready".to_string(),
        gitgpui_state::model::Loadable::Error(e) => format!("open: error: {e}"),
    }
}

fn format_loadable_len<T>(label: &str, v: &gitgpui_state::model::Loadable<Vec<T>>) -> String {
    match v {
        gitgpui_state::model::Loadable::NotLoaded => format!("{label}: not loaded"),
        gitgpui_state::model::Loadable::Loading => format!("{label}: loading"),
        gitgpui_state::model::Loadable::Ready(items) => format!("{label}: {}", items.len()),
        gitgpui_state::model::Loadable::Error(e) => format!("{label}: error: {e}"),
    }
}

struct Poller;

impl Poller {
    fn start(
        store: Arc<AppStore>,
        events: mpsc::Receiver<gitgpui_state::msg::StoreEvent>,
        view: WeakEntity<GitGpuiView>,
        window: &mut Window,
        cx: &mut Context<GitGpuiView>,
    ) -> Poller {
        let events = std::sync::Arc::new(std::sync::Mutex::new(events));

        window
            .spawn(cx, async move |cx| {
                loop {
                    let mut changed = false;
                    {
                        let events = events.lock().expect("events lock poisoned");
                        while events.try_recv().is_ok() {
                            changed = true;
                        }
                    }

                    if changed {
                        let snapshot = store.snapshot();
                        let _ = view.update(cx, |view, cx| {
                            view.state = snapshot;
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
