use crate::{theme::AppTheme, zed_port as zed};
use gitgpui_core::diff::{AnnotatedDiffLine, annotate_unified};
use gitgpui_core::domain::{
    Commit, CommitId, DiffArea, DiffTarget, FileStatus, FileStatusKind, RepoStatus,
};
use gitgpui_core::file_diff::{FileDiffRow, side_by_side_rows};
use gitgpui_core::services::PullMode;
use gitgpui_state::model::{AppState, DiagnosticKind, Loadable, RepoId, RepoState};
use gitgpui_state::msg::{Msg, StoreEvent};
use gitgpui_state::session;
use gitgpui_state::store::AppStore;
use gpui::prelude::*;
use gpui::{
    AnyElement, Bounds, ClickEvent, Corner, CursorStyle, Decorations, Entity, FontWeight,
    MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, Render, ResizeEdge, ScrollHandle,
    SharedString, Size, Tiling, Timer, UniformListScrollHandle, WeakEntity, Window,
    WindowControlArea, anchored, div, point, px, size, uniform_list,
};
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::sync::{Arc, mpsc};
use std::time::Duration;

mod chrome;
mod history_graph;
mod panels;
mod rows;

use chrome::{CLIENT_SIDE_DECORATION_INSET, cursor_style_for_resize_edge, resize_edge};

pub(crate) use chrome::window_frame;

const HISTORY_COL_BRANCH_PX: f32 = 160.0;
const HISTORY_COL_GRAPH_PX: f32 = 180.0;
const HISTORY_COL_DATE_PX: f32 = 160.0;
const HISTORY_COL_SHA_PX: f32 = 88.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffViewMode {
    Inline,
    Split,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffDisplayMode {
    File,
    Patch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffSelectionScope {
    File,
    Patch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HistoryTab {
    Log,
    Stash,
    Reflog,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ToolsTab {
    Diagnostics,
    Output,
    Conflicts,
    Blame,
}

#[derive(Clone, Debug)]
struct ToastState {
    id: u64,
    kind: zed::ToastKind,
    message: String,
}

#[derive(Clone, Debug)]
struct HistoryCache {
    repo_id: RepoId,
    query: String,
    branch_filter: Option<String>,
    commit_ids: Vec<CommitId>,
    visible_indices: Vec<usize>,
    graph_rows: Vec<history_graph::GraphRow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PopoverKind {
    RepoPicker,
    BranchPicker,
    PullPicker,
    AppMenu,
    DiffHunks,
    CommandLogDetails {
        repo_id: RepoId,
        index: usize,
    },
    CommitModal {
        repo_id: RepoId,
    },
    CommitMenu {
        repo_id: RepoId,
        commit_id: CommitId,
    },
    HistoryBranchFilter {
        repo_id: RepoId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RemoteRow {
    Header(String),
    Branch { remote: String, name: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffClickKind {
    Line,
    HunkHeader,
    FileHeader,
}

pub struct GitGpuiView {
    store: Arc<AppStore>,
    state: AppState,
    _poller: Poller,
    _appearance_subscription: gpui::Subscription,
    theme: AppTheme,

    diff_view: DiffViewMode,
    diff_display: DiffDisplayMode,
    diff_cache: Vec<AnnotatedDiffLine>,
    file_diff_cache: Vec<FileDiffRow>,
    diff_search_input: Entity<zed::TextInput>,
    diff_search_raw: String,
    diff_search_debounced: String,
    diff_search_seq: u64,
    diff_visible_indices: Vec<usize>,
    diff_visible_cache_len: usize,
    diff_visible_query: String,
    diff_query_match_count: usize,
    diff_word_highlights: HashMap<usize, Vec<Range<usize>>>,
    diff_file_stats: HashMap<usize, (usize, usize)>,
    diff_selection_anchor: Option<usize>,
    diff_selection_range: Option<(usize, usize)>,
    diff_selection_scope: DiffSelectionScope,
    diff_hunk_picker_search_input: Option<Entity<zed::TextInput>>,

    history_tab: HistoryTab,
    history_search_input: Entity<zed::TextInput>,
    history_search_raw: String,
    history_search_debounced: String,
    history_search_seq: u64,
    history_branch_filter: Option<String>,
    repo_picker_search_input: Option<Entity<zed::TextInput>>,
    branch_picker_search_input: Option<Entity<zed::TextInput>>,
    history_branch_picker_search_input: Option<Entity<zed::TextInput>>,
    history_cache: Option<HistoryCache>,

    open_repo_panel: bool,
    show_diagnostics_view: bool,
    tools_tab: ToolsTab,
    open_repo_input: Entity<zed::TextInput>,
    commit_message_input: Entity<zed::TextInput>,
    commit_modal_input: Option<Entity<zed::TextInput>>,
    create_branch_input: Entity<zed::TextInput>,

    popover: Option<PopoverKind>,
    popover_anchor: Option<Point<Pixels>>,

    title_should_move: bool,
    hover_resize_edge: Option<ResizeEdge>,

    branches_scroll: UniformListScrollHandle,
    remotes_scroll: UniformListScrollHandle,
    history_scroll: UniformListScrollHandle,
    stashes_scroll: UniformListScrollHandle,
    reflog_scroll: UniformListScrollHandle,
    unstaged_scroll: UniformListScrollHandle,
    staged_scroll: UniformListScrollHandle,
    diff_scroll: UniformListScrollHandle,
    diagnostics_scroll: UniformListScrollHandle,
    output_scroll: UniformListScrollHandle,
    conflicts_scroll: UniformListScrollHandle,
    blame_scroll: UniformListScrollHandle,
    sidebar_scroll: ScrollHandle,
    commit_scroll: ScrollHandle,

    toasts: Vec<ToastState>,
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

        let mut ui_session = session::load();
        if let Some(path) = initial_path {
            if !ui_session.open_repos.iter().any(|p| p == &path) {
                ui_session.open_repos.push(path.clone());
            }
            ui_session.active_repo = Some(path);
        }

        if !ui_session.open_repos.is_empty() {
            store.dispatch(Msg::RestoreSession {
                open_repos: ui_session.open_repos,
                active_repo: ui_session.active_repo,
            });
        } else if let Ok(path) = std::env::current_dir() {
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
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "/path/to/repo".into(),
                    multiline: false,
                },
                window,
                cx,
            )
        });

        let commit_message_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "Enter commit message…".into(),
                    multiline: false,
                },
                window,
                cx,
            )
        });

        let create_branch_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "new-branch-name".into(),
                    multiline: false,
                },
                window,
                cx,
            )
        });

        let history_search_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "Search commits…".into(),
                    multiline: false,
                },
                window,
                cx,
            )
        });

        let diff_search_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "Search diff…".into(),
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
            diff_display: DiffDisplayMode::File,
            diff_cache: Vec::new(),
            file_diff_cache: Vec::new(),
            diff_search_input,
            diff_search_raw: String::new(),
            diff_search_debounced: String::new(),
            diff_search_seq: 0,
            diff_visible_indices: Vec::new(),
            diff_visible_cache_len: 0,
            diff_visible_query: String::new(),
            diff_query_match_count: 0,
            diff_word_highlights: HashMap::new(),
            diff_file_stats: HashMap::new(),
            diff_selection_anchor: None,
            diff_selection_range: None,
            diff_selection_scope: DiffSelectionScope::Patch,
            diff_hunk_picker_search_input: None,
            history_tab: HistoryTab::Log,
            history_search_input,
            history_search_raw: String::new(),
            history_search_debounced: String::new(),
            history_search_seq: 0,
            history_branch_filter: None,
            repo_picker_search_input: None,
            branch_picker_search_input: None,
            history_branch_picker_search_input: None,
            history_cache: None,
            open_repo_panel: false,
            show_diagnostics_view: false,
            tools_tab: ToolsTab::Diagnostics,
            open_repo_input,
            commit_message_input,
            commit_modal_input: None,
            create_branch_input,
            popover: None,
            popover_anchor: None,
            title_should_move: false,
            hover_resize_edge: None,
            branches_scroll: UniformListScrollHandle::default(),
            remotes_scroll: UniformListScrollHandle::default(),
            history_scroll: UniformListScrollHandle::default(),
            stashes_scroll: UniformListScrollHandle::default(),
            reflog_scroll: UniformListScrollHandle::default(),
            unstaged_scroll: UniformListScrollHandle::default(),
            staged_scroll: UniformListScrollHandle::default(),
            diff_scroll: UniformListScrollHandle::default(),
            diagnostics_scroll: UniformListScrollHandle::default(),
            output_scroll: UniformListScrollHandle::default(),
            conflicts_scroll: UniformListScrollHandle::default(),
            blame_scroll: UniformListScrollHandle::default(),
            sidebar_scroll: ScrollHandle::new(),
            commit_scroll: ScrollHandle::new(),
            toasts: Vec::new(),
        };

        view.set_theme(initial_theme, cx);
        view.rebuild_diff_cache();
        view.rebuild_file_diff_cache();
        view
    }

    fn set_theme(&mut self, theme: AppTheme, cx: &mut gpui::Context<Self>) {
        self.theme = theme;
        self.open_repo_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.commit_message_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        if let Some(input) = &self.commit_modal_input {
            input.update(cx, |input, cx| input.set_theme(theme, cx));
        }
        self.create_branch_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.history_search_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.diff_search_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        if let Some(input) = &self.repo_picker_search_input {
            input.update(cx, |input, cx| input.set_theme(theme, cx));
        }
        if let Some(input) = &self.branch_picker_search_input {
            input.update(cx, |input, cx| input.set_theme(theme, cx));
        }
        if let Some(input) = &self.history_branch_picker_search_input {
            input.update(cx, |input, cx| input.set_theme(theme, cx));
        }
        if let Some(input) = &self.diff_hunk_picker_search_input {
            input.update(cx, |input, cx| input.set_theme(theme, cx));
        }
    }

    fn active_repo_id(&self) -> Option<RepoId> {
        self.state.active_repo
    }

    fn active_repo(&self) -> Option<&RepoState> {
        let repo_id = self.active_repo_id()?;
        self.state.repos.iter().find(|r| r.id == repo_id)
    }

    fn ensure_repo_picker_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Entity<zed::TextInput> {
        let theme = self.theme;
        let input = self.repo_picker_search_input.get_or_insert_with(|| {
            cx.new(|cx| {
                zed::TextInput::new(
                    zed::TextInputOptions {
                        placeholder: "Filter repositories…".into(),
                        multiline: false,
                    },
                    window,
                    cx,
                )
            })
        });
        input.update(cx, |input, cx| {
            input.set_theme(theme, cx);
            input.set_text("", cx);
        });
        let focus_handle = input.read_with(cx, |input, _| input.focus_handle());
        window.focus(&focus_handle);
        input.clone()
    }

    fn ensure_commit_modal_input(
        &mut self,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Entity<zed::TextInput> {
        let theme = self.theme;
        let input = self.commit_modal_input.get_or_insert_with(|| {
            cx.new(|cx| {
                zed::TextInput::new(
                    zed::TextInputOptions {
                        placeholder: "Commit message…".into(),
                        multiline: true,
                    },
                    window,
                    cx,
                )
            })
        });
        input.update(cx, |input, cx| input.set_theme(theme, cx));
        input.clone()
    }

    fn ensure_branch_picker_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Entity<zed::TextInput> {
        let theme = self.theme;
        let input = self.branch_picker_search_input.get_or_insert_with(|| {
            cx.new(|cx| {
                zed::TextInput::new(
                    zed::TextInputOptions {
                        placeholder: "Filter branches…".into(),
                        multiline: false,
                    },
                    window,
                    cx,
                )
            })
        });
        input.update(cx, |input, cx| {
            input.set_theme(theme, cx);
            input.set_text("", cx);
        });
        let focus_handle = input.read_with(cx, |input, _| input.focus_handle());
        window.focus(&focus_handle);
        input.clone()
    }

    fn ensure_history_branch_picker_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Entity<zed::TextInput> {
        let theme = self.theme;
        let input = self.history_branch_picker_search_input.get_or_insert_with(|| {
            cx.new(|cx| {
                zed::TextInput::new(
                    zed::TextInputOptions {
                        placeholder: "Filter branches…".into(),
                        multiline: false,
                    },
                    window,
                    cx,
                )
            })
        });
        input.update(cx, |input, cx| {
            input.set_theme(theme, cx);
            input.set_text("", cx);
        });
        let focus_handle = input.read_with(cx, |input, _| input.focus_handle());
        window.focus(&focus_handle);
        input.clone()
    }

    fn ensure_diff_hunk_picker_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Entity<zed::TextInput> {
        let theme = self.theme;
        let input = self.diff_hunk_picker_search_input.get_or_insert_with(|| {
            cx.new(|cx| {
                zed::TextInput::new(
                    zed::TextInputOptions {
                        placeholder: "Filter hunks…".into(),
                        multiline: false,
                    },
                    window,
                    cx,
                )
            })
        });
        input.update(cx, |input, cx| {
            input.set_theme(theme, cx);
            input.set_text("", cx);
        });
        let focus_handle = input.read_with(cx, |input, _| input.focus_handle());
        window.focus(&focus_handle);
        input.clone()
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
        self.diff_visible_indices.clear();
        self.diff_visible_cache_len = 0;
        self.diff_query_match_count = 0;
        self.diff_word_highlights.clear();
        self.diff_file_stats.clear();
        self.diff_selection_anchor = None;
        self.diff_selection_range = None;
        self.diff_selection_scope = DiffSelectionScope::Patch;
        let Some(repo) = self.active_repo() else {
            return;
        };
        let Loadable::Ready(diff) = &repo.diff else {
            return;
        };
        self.diff_cache = annotate_unified(diff);
        self.diff_file_stats = compute_diff_file_stats(&self.diff_cache);
        self.rebuild_diff_word_highlights();
    }

    fn rebuild_file_diff_cache(&mut self) {
        self.file_diff_cache.clear();

        let Some(repo) = self.active_repo() else {
            return;
        };
        let Loadable::Ready(Some(file)) = &repo.diff_file else {
            let supported = repo.diff_target.as_ref().is_some_and(|t| {
                matches!(
                    t,
                    DiffTarget::WorkingTree { .. } | DiffTarget::Commit { path: Some(_), .. }
                )
            });
            if !supported && self.diff_display == DiffDisplayMode::File {
                self.diff_display = DiffDisplayMode::Patch;
            }
            return;
        };

        let old = file.old.as_deref().unwrap_or("");
        let new = file.new.as_deref().unwrap_or("");

        let old_len = old.lines().count();
        let new_len = new.lines().count();
        let too_large = old_len.saturating_add(new_len) > 20_000;
        if too_large {
            return;
        }

        self.file_diff_cache = side_by_side_rows(old, new);
    }

    fn apply_state_snapshot(&mut self, next: AppState, cx: &mut gpui::Context<Self>) {
        let prev_diff_target = self
            .active_repo()
            .and_then(|r| r.diff_target.as_ref())
            .cloned();
        let next_diff_target = next
            .active_repo
            .and_then(|id| next.repos.iter().find(|r| r.id == id))
            .and_then(|r| r.diff_target.as_ref())
            .cloned();

        for next_repo in &next.repos {
            let (old_diag_len, old_cmd_len) = self
                .state
                .repos
                .iter()
                .find(|r| r.id == next_repo.id)
                .map(|r| (r.diagnostics.len(), r.command_log.len()))
                .unwrap_or((0, 0));

            let new_diag_messages = next_repo
                .diagnostics
                .iter()
                .skip(old_diag_len.min(next_repo.diagnostics.len()))
                .filter(|d| d.kind == DiagnosticKind::Error)
                .map(|d| d.message.clone())
                .collect::<Vec<_>>();
            for msg in new_diag_messages {
                self.push_toast(zed::ToastKind::Error, msg, cx);
            }

            let new_command_summaries = next_repo
                .command_log
                .iter()
                .skip(old_cmd_len.min(next_repo.command_log.len()))
                .map(|e| (e.ok, e.summary.clone()))
                .collect::<Vec<_>>();
            for (ok, summary) in new_command_summaries {
                self.push_toast(
                    if ok {
                        zed::ToastKind::Success
                    } else {
                        zed::ToastKind::Error
                    },
                    summary,
                    cx,
                );
            }
        }

        if prev_diff_target != next_diff_target {
            let supported = next_diff_target.as_ref().is_some_and(|t| {
                matches!(
                    t,
                    DiffTarget::WorkingTree { .. } | DiffTarget::Commit { path: Some(_), .. }
                )
            });
            self.diff_display = if supported {
                DiffDisplayMode::File
            } else {
                DiffDisplayMode::Patch
            };
            self.diff_selection_anchor = None;
            self.diff_selection_range = None;
            self.diff_selection_scope = if supported {
                DiffSelectionScope::File
            } else {
                DiffSelectionScope::Patch
            };
        }

        self.state = next;
        self.rebuild_diff_cache();
        self.rebuild_file_diff_cache();
    }

    fn push_toast(&mut self, kind: zed::ToastKind, message: String, cx: &mut gpui::Context<Self>) {
        let id = self.toasts.last().map(|t| t.id.wrapping_add(1)).unwrap_or(1);
        self.toasts.push(ToastState { id, kind, message });

        cx.spawn(async move |view: WeakEntity<GitGpuiView>, cx: &mut gpui::AsyncApp| {
            Timer::after(Duration::from_secs(5)).await;
            let _ = view.update(cx, |this, cx| {
                this.toasts.retain(|t| t.id != id);
                cx.notify();
            });
        })
        .detach();
    }

    fn rebuild_diff_word_highlights(&mut self) {
        self.diff_word_highlights.clear();

        let mut ix = 0usize;
        while ix < self.diff_cache.len() {
            let kind = self.diff_cache[ix].kind;
            if matches!(kind, gitgpui_core::domain::DiffLineKind::Hunk) {
                ix += 1;
                continue;
            }

            if !matches!(kind, gitgpui_core::domain::DiffLineKind::Remove) {
                ix += 1;
                continue;
            }

            let mut removed: Vec<(usize, String)> = Vec::new();
            while ix < self.diff_cache.len()
                && matches!(
                    self.diff_cache[ix].kind,
                    gitgpui_core::domain::DiffLineKind::Remove
                )
            {
                let text = diff_content_text(&self.diff_cache[ix]).to_string();
                removed.push((ix, text));
                ix += 1;
            }

            let mut added: Vec<(usize, String)> = Vec::new();
            while ix < self.diff_cache.len()
                && matches!(
                    self.diff_cache[ix].kind,
                    gitgpui_core::domain::DiffLineKind::Add
                )
            {
                let text = diff_content_text(&self.diff_cache[ix]).to_string();
                added.push((ix, text));
                ix += 1;
            }

            let pairs = removed.len().min(added.len());
            for i in 0..pairs {
                let (old_ix, ref old_text) = removed[i];
                let (new_ix, ref new_text) = added[i];
                let (old_ranges, new_ranges) = word_diff_ranges(old_text, new_text);
                if !old_ranges.is_empty() {
                    self.diff_word_highlights.insert(old_ix, old_ranges);
                }
                if !new_ranges.is_empty() {
                    self.diff_word_highlights.insert(new_ix, new_ranges);
                }
            }

            for (old_ix, old_text) in removed.into_iter().skip(pairs) {
                if !old_text.is_empty() {
                    self.diff_word_highlights.insert(old_ix, vec![0..old_text.len()]);
                }
            }
            for (new_ix, new_text) in added.into_iter().skip(pairs) {
                if !new_text.is_empty() {
                    self.diff_word_highlights.insert(new_ix, vec![0..new_text.len()]);
                }
            }
        }
    }

    fn update_diff_search_debounce(&mut self, cx: &mut gpui::Context<Self>) {
        let raw = self
            .diff_search_input
            .read_with(cx, |i, _| i.text().to_string());

        if raw == self.diff_search_raw {
            return;
        }

        self.diff_search_raw = raw.clone();
        self.diff_search_seq = self.diff_search_seq.wrapping_add(1);
        let seq = self.diff_search_seq;

        cx.spawn(async move |view: WeakEntity<GitGpuiView>, cx: &mut gpui::AsyncApp| {
            Timer::after(Duration::from_millis(150)).await;
            let _ = view.update(cx, |this, cx| {
                if this.diff_search_seq != seq {
                    return;
                }
                if this.diff_search_raw != raw {
                    return;
                }
                this.diff_search_debounced = raw;
                this.diff_visible_indices.clear();
                this.diff_visible_cache_len = 0;
                cx.notify();
            });
        })
        .detach();
    }

    fn ensure_diff_visible_indices(&mut self) {
        let query = self.diff_search_debounced.trim().to_string();
        if self.diff_visible_cache_len == self.diff_cache.len() && self.diff_visible_query == query {
            return;
        }

        self.diff_visible_cache_len = self.diff_cache.len();
        self.diff_visible_query = query.clone();
        self.diff_query_match_count = 0;

        if query.is_empty() {
            self.diff_visible_indices = (0..self.diff_cache.len()).collect();
            return;
        }

        let q = query.to_ascii_lowercase();
        let mut match_count = 0usize;
        let mut indices = Vec::new();
        for (ix, line) in self.diff_cache.iter().enumerate() {
            if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk) {
                indices.push(ix);
                continue;
            }

            if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
                && line.text.starts_with("diff --git ")
            {
                indices.push(ix);
                continue;
            }

            let text = match line.kind {
                gitgpui_core::domain::DiffLineKind::Add => {
                    line.text.strip_prefix('+').unwrap_or(&line.text)
                }
                gitgpui_core::domain::DiffLineKind::Remove => {
                    line.text.strip_prefix('-').unwrap_or(&line.text)
                }
                gitgpui_core::domain::DiffLineKind::Context => {
                    line.text.strip_prefix(' ').unwrap_or(&line.text)
                }
                gitgpui_core::domain::DiffLineKind::Header => &line.text,
                gitgpui_core::domain::DiffLineKind::Hunk => &line.text,
            };

            if text.to_ascii_lowercase().contains(&q) {
                match_count += 1;
                indices.push(ix);
            }
        }

        self.diff_query_match_count = match_count;
        self.diff_visible_indices = indices;
    }

    fn handle_diff_row_click(
        &mut self,
        clicked_visible_ix: usize,
        kind: DiffClickKind,
        shift: bool,
    ) {
        self.diff_selection_scope = DiffSelectionScope::Patch;

        let list_len = self.diff_visible_indices.len();
        if list_len == 0 {
            self.diff_selection_anchor = None;
            self.diff_selection_range = None;
            return;
        }

        let clicked_visible_ix = clicked_visible_ix.min(list_len - 1);

        if shift {
            if let Some(anchor) = self.diff_selection_anchor {
                let a = anchor.min(clicked_visible_ix);
                let b = anchor.max(clicked_visible_ix);
                self.diff_selection_range = Some((a, b));
                return;
            }
        }

        let end = match kind {
            DiffClickKind::Line => clicked_visible_ix,
            DiffClickKind::HunkHeader => self
                .diff_next_boundary_visible_ix(clicked_visible_ix, |src_ix| {
                    let line = &self.diff_cache[src_ix];
                    matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk)
                        || (matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
                            && line.text.starts_with("diff --git "))
                })
                .unwrap_or(list_len - 1),
            DiffClickKind::FileHeader => self
                .diff_next_boundary_visible_ix(clicked_visible_ix, |src_ix| {
                    let line = &self.diff_cache[src_ix];
                    matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
                        && line.text.starts_with("diff --git ")
                })
                .unwrap_or(list_len - 1),
        };

        self.diff_selection_anchor = Some(clicked_visible_ix);
        self.diff_selection_range = Some((clicked_visible_ix, end));
    }

    fn handle_file_diff_row_click(&mut self, clicked_ix: usize, shift: bool) {
        self.diff_selection_scope = DiffSelectionScope::File;

        let list_len = self.file_diff_cache.len();
        if list_len == 0 {
            self.diff_selection_anchor = None;
            self.diff_selection_range = None;
            return;
        }

        let clicked_ix = clicked_ix.min(list_len - 1);

        if shift {
            if let Some(anchor) = self.diff_selection_anchor {
                let a = anchor.min(clicked_ix);
                let b = anchor.max(clicked_ix);
                self.diff_selection_range = Some((a, b));
                return;
            }
        }

        self.diff_selection_anchor = Some(clicked_ix);
        self.diff_selection_range = Some((clicked_ix, clicked_ix));
    }

    fn diff_next_boundary_visible_ix(
        &self,
        from_visible_ix: usize,
        is_boundary: impl Fn(usize) -> bool,
    ) -> Option<usize> {
        let from_visible_ix = from_visible_ix.min(self.diff_visible_indices.len().saturating_sub(1));
        for visible_ix in (from_visible_ix + 1)..self.diff_visible_indices.len() {
            let src_ix = *self.diff_visible_indices.get(visible_ix)?;
            if is_boundary(src_ix) {
                return Some(visible_ix.saturating_sub(1));
            }
        }
        None
    }

    fn diff_selected_text_for_clipboard(&self) -> String {
        match self.diff_display {
            DiffDisplayMode::Patch => {
                if self.diff_cache.is_empty() || self.diff_visible_indices.is_empty() {
                    return String::new();
                }

                let (start, end) = if matches!(self.diff_selection_scope, DiffSelectionScope::Patch)
                {
                    self.diff_selection_range.unwrap_or((
                        0,
                        self.diff_visible_indices.len().saturating_sub(1),
                    ))
                } else {
                    (0, self.diff_visible_indices.len().saturating_sub(1))
                };

                let start = start.min(self.diff_visible_indices.len().saturating_sub(1));
                let end = end.min(self.diff_visible_indices.len().saturating_sub(1));
                let (start, end) = (start.min(end), start.max(end));

                let mut out = String::new();
                for visible_ix in start..=end {
                    let Some(&src_ix) = self.diff_visible_indices.get(visible_ix) else {
                        continue;
                    };
                    if let Some(line) = self.diff_cache.get(src_ix) {
                        out.push_str(&line.text);
                        out.push('\n');
                    }
                }
                out
            }
            DiffDisplayMode::File => {
                if self.file_diff_cache.is_empty() {
                    return String::new();
                }

                let (start, end) = if matches!(self.diff_selection_scope, DiffSelectionScope::File) {
                    self.diff_selection_range
                        .unwrap_or((0, self.file_diff_cache.len().saturating_sub(1)))
                } else {
                    (0, self.file_diff_cache.len().saturating_sub(1))
                };

                let start = start.min(self.file_diff_cache.len().saturating_sub(1));
                let end = end.min(self.file_diff_cache.len().saturating_sub(1));
                let (start, end) = (start.min(end), start.max(end));

                let mut out = String::new();
                for ix in start..=end {
                    let Some(row) = self.file_diff_cache.get(ix) else {
                        continue;
                    };
                    if let Some(s) = row.new.as_deref().or(row.old.as_deref()) {
                        out.push_str(s);
                        out.push('\n');
                    }
                }
                out
            }
        }
    }

    fn update_history_search_debounce(&mut self, cx: &mut gpui::Context<Self>) {
        let raw = self
            .history_search_input
            .read_with(cx, |i, _| i.text().to_string());

        if raw == self.history_search_raw {
            return;
        }

        self.history_search_raw = raw.clone();
        self.history_search_seq = self.history_search_seq.wrapping_add(1);
        let seq = self.history_search_seq;

        cx.spawn(async move |view: WeakEntity<GitGpuiView>, cx: &mut gpui::AsyncApp| {
            Timer::after(Duration::from_millis(150)).await;
            let _ = view.update(cx, |this, cx| {
                if this.history_search_seq != seq {
                    return;
                }
                if this.history_search_raw != raw {
                    return;
                }
                this.history_search_debounced = raw;
                this.history_cache = None;
                cx.notify();
            });
        })
        .detach();
    }

    fn ensure_history_cache(&mut self, _cx: &mut gpui::Context<Self>) {
        let Some(repo) = self.active_repo() else {
            self.history_cache = None;
            return;
        };
        let Loadable::Ready(page) = &repo.log else {
            self.history_cache = None;
            return;
        };

        let query = self.history_search_debounced.trim().to_string();
        let branch_filter = self.history_branch_filter.clone();

        let commit_ids = page
            .commits
            .iter()
            .map(|c| c.id.clone())
            .collect::<Vec<_>>();

        let cache_ok = self.history_cache.as_ref().is_some_and(|c| {
            c.repo_id == repo.id
                && c.query == query
                && c.branch_filter == branch_filter
                && c.commit_ids == commit_ids
        });
        if cache_ok {
            return;
        }

        let query_lc = query.to_lowercase();
        let mut visible_indices = if query_lc.is_empty() {
            (0..page.commits.len()).collect::<Vec<_>>()
        } else {
            page.commits
                .iter()
                .enumerate()
                .filter_map(|(ix, c)| {
                    let haystack =
                        format!("{} {} {}", c.summary, c.author, c.id.as_ref()).to_lowercase();
                    if haystack.contains(&query_lc) {
                        Some(ix)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };

        if let Some(branch_name) = &branch_filter {
            if let Loadable::Ready(branches) = &repo.branches {
                if let Some(branch) = branches.iter().find(|b| &b.name == branch_name) {
                    let mut by_id: std::collections::HashMap<&str, &Commit> =
                        std::collections::HashMap::new();
                    for c in &page.commits {
                        by_id.insert(c.id.as_ref(), c);
                    }

                    let mut reachable: std::collections::HashSet<&str> =
                        std::collections::HashSet::new();
                    let mut stack: Vec<&CommitId> = vec![&branch.target];
                    while let Some(id) = stack.pop() {
                        if !reachable.insert(id.as_ref()) {
                            continue;
                        }
                        if let Some(c) = by_id.get(id.as_ref()) {
                            for p in &c.parent_ids {
                                stack.push(p);
                            }
                        }
                    }

                    visible_indices.retain(|ix| {
                        page.commits
                            .get(*ix)
                            .is_some_and(|c| reachable.contains(c.id.as_ref()))
                    });
                }
            }
        }

        let visible_commits = visible_indices
            .iter()
            .filter_map(|ix| page.commits.get(*ix).cloned())
            .collect::<Vec<_>>();

        let graph_rows = history_graph::compute_graph(&visible_commits, self.theme);

        self.history_cache = Some(HistoryCache {
            repo_id: repo.id,
            query,
            branch_filter,
            commit_ids,
            visible_indices,
            graph_rows,
        });
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
                    .min_h(px(0.0))
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
                            .min_h(px(0.0))
                            .px_3()
                            .pb_3()
                            .child(
                                div()
                                    .id("sidebar_scroll")
                                    .w(px(280.0))
                                    .min_h(px(0.0))
                                    .overflow_y_scroll()
                                    .track_scroll(&self.sidebar_scroll)
                                    .child(self.sidebar(cx))
                                    .child(
                                        zed::Scrollbar::new(
                                            "sidebar_scrollbar",
                                            self.sidebar_scroll.clone(),
                                        )
                                        .render(theme),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .min_h(px(0.0))
                                    .child(main_view),
                            )
                            .child(
                                div()
                                    .id("commit_scroll")
                                    .w(px(420.0))
                                    .min_h(px(0.0))
                                    .overflow_y_scroll()
                                    .track_scroll(&self.commit_scroll)
                                    .child(self.commit_details_view(cx))
                                    .child(
                                        zed::Scrollbar::new(
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

        if !self.toasts.is_empty() {
            root = root.child(self.toast_layer());
        }

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

fn diff_content_text(line: &AnnotatedDiffLine) -> &str {
    match line.kind {
        gitgpui_core::domain::DiffLineKind::Add => line.text.strip_prefix('+').unwrap_or(&line.text),
        gitgpui_core::domain::DiffLineKind::Remove => {
            line.text.strip_prefix('-').unwrap_or(&line.text)
        }
        gitgpui_core::domain::DiffLineKind::Context => {
            line.text.strip_prefix(' ').unwrap_or(&line.text)
        }
        gitgpui_core::domain::DiffLineKind::Header | gitgpui_core::domain::DiffLineKind::Hunk => {
            &line.text
        }
    }
}

fn parse_diff_git_header_path(text: &str) -> Option<String> {
    let text = text.strip_prefix("diff --git ")?;
    let mut parts = text.split_whitespace();
    let a = parts.next()?;
    let b = parts.next().unwrap_or(a);
    let b = b.strip_prefix("b/").unwrap_or(b);
    Some(b.to_string())
}

#[derive(Clone, Debug)]
struct ParsedHunkHeader {
    old: String,
    new: String,
    heading: Option<String>,
}

fn parse_unified_hunk_header_for_display(text: &str) -> Option<ParsedHunkHeader> {
    let text = text.strip_prefix("@@")?.trim_start();
    let (ranges, rest) = text.split_once("@@")?;
    let ranges = ranges.trim();
    let heading = rest.trim();

    let mut it = ranges.split_whitespace();
    let old = it.next()?.trim().to_string();
    let new = it.next()?.trim().to_string();

    Some(ParsedHunkHeader {
        old,
        new,
        heading: (!heading.is_empty()).then_some(heading.to_string()),
    })
}

fn compute_diff_file_stats(diff: &[AnnotatedDiffLine]) -> HashMap<usize, (usize, usize)> {
    let mut stats: HashMap<usize, (usize, usize)> = HashMap::new();

    let mut current_file_header_ix: Option<usize> = None;
    let mut adds = 0usize;
    let mut removes = 0usize;

    for (ix, line) in diff.iter().enumerate() {
        let is_file_header = matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
            && line.text.starts_with("diff --git ");

        if is_file_header {
            if let Some(header_ix) = current_file_header_ix.take() {
                stats.insert(header_ix, (adds, removes));
            }
            current_file_header_ix = Some(ix);
            adds = 0;
            removes = 0;
            continue;
        }

        match line.kind {
            gitgpui_core::domain::DiffLineKind::Add => adds += 1,
            gitgpui_core::domain::DiffLineKind::Remove => removes += 1,
            _ => {}
        }
    }

    if let Some(header_ix) = current_file_header_ix {
        stats.insert(header_ix, (adds, removes));
    }

    stats
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TokenKind {
    Whitespace,
    Other,
}

#[derive(Clone, Debug)]
struct Token {
    range: Range<usize>,
    kind: TokenKind,
}

fn tokenize_for_word_diff(s: &str) -> Vec<Token> {
    fn classify(c: char) -> (u8, TokenKind) {
        if c.is_whitespace() {
            return (0, TokenKind::Whitespace);
        }
        if c.is_alphanumeric() || c == '_' {
            return (1, TokenKind::Other);
        }
        (2, TokenKind::Other)
    }

    let mut out = Vec::new();
    let mut it = s.char_indices().peekable();
    while let Some((start, ch)) = it.next() {
        let (class, kind) = classify(ch);
        let mut end = start + ch.len_utf8();
        while let Some(&(next_start, next_ch)) = it.peek() {
            let (next_class, _) = classify(next_ch);
            if next_class != class {
                break;
            }
            it.next();
            end = next_start + next_ch.len_utf8();
        }
        out.push(Token {
            range: start..end,
            kind,
        });
    }
    out
}

fn coalesce_ranges(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    if ranges.len() <= 1 {
        return ranges;
    }
    ranges.sort_by_key(|r| (r.start, r.end));
    let mut out: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for r in ranges {
        if let Some(last) = out.last_mut() {
            if r.start <= last.end {
                last.end = last.end.max(r.end);
                continue;
            }
        }
        out.push(r);
    }
    out
}

fn word_diff_ranges(old: &str, new: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    let old_tokens = tokenize_for_word_diff(old);
    let new_tokens = tokenize_for_word_diff(new);

    const MAX_TOKENS: usize = 128;
    if old_tokens.len() > MAX_TOKENS || new_tokens.len() > MAX_TOKENS {
        return fallback_affix_diff_ranges(old, new);
    }

    let n = old_tokens.len();
    let m = new_tokens.len();

    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in 0..n {
        for j in 0..m {
            let a = &old[old_tokens[i].range.clone()];
            let b = &new[new_tokens[j].range.clone()];
            dp[i + 1][j + 1] = if a == b {
                dp[i][j] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    let mut keep_old = vec![false; n];
    let mut keep_new = vec![false; m];
    let mut i = n;
    let mut j = m;
    while i > 0 && j > 0 {
        let a = &old[old_tokens[i - 1].range.clone()];
        let b = &new[new_tokens[j - 1].range.clone()];
        if a == b {
            keep_old[i - 1] = true;
            keep_new[j - 1] = true;
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] >= dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }

    let old_ranges = old_tokens
        .iter()
        .zip(keep_old.iter().copied())
        .filter_map(|(t, keep)| (!keep && t.kind == TokenKind::Other).then_some(t.range.clone()))
        .collect::<Vec<_>>();
    let new_ranges = new_tokens
        .iter()
        .zip(keep_new.iter().copied())
        .filter_map(|(t, keep)| (!keep && t.kind == TokenKind::Other).then_some(t.range.clone()))
        .collect::<Vec<_>>();

    (coalesce_ranges(old_ranges), coalesce_ranges(new_ranges))
}

fn fallback_affix_diff_ranges(old: &str, new: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    let mut prefix = 0usize;
    for ((old_ix, old_ch), (_new_ix, new_ch)) in old.char_indices().zip(new.char_indices()) {
        if old_ch != new_ch {
            break;
        }
        prefix = old_ix + old_ch.len_utf8();
    }

    let mut suffix = 0usize;
    let old_tail = &old[prefix.min(old.len())..];
    let new_tail = &new[prefix.min(new.len())..];
    for (old_ch, new_ch) in old_tail.chars().rev().zip(new_tail.chars().rev()) {
        if old_ch != new_ch {
            break;
        }
        suffix += old_ch.len_utf8();
    }

    let old_mid_start = prefix.min(old.len());
    let old_mid_end = old.len().saturating_sub(suffix).max(old_mid_start);
    let new_mid_start = prefix.min(new.len());
    let new_mid_end = new.len().saturating_sub(suffix).max(new_mid_start);

    let old_ranges = if old_mid_end > old_mid_start {
        vec![old_mid_start..old_mid_end]
    } else {
        Vec::new()
    };
    let new_ranges = if new_mid_end > new_mid_start {
        vec![new_mid_start..new_mid_end]
    } else {
        Vec::new()
    };
    (old_ranges, new_ranges)
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
            .clone()
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

    fn toast_layer(&self) -> AnyElement {
        if self.toasts.is_empty() {
            return div().into_any_element();
        }
        let theme = self.theme;

        let toasts = self.toasts.iter().rev().take(3).cloned().collect::<Vec<_>>();
        let children = toasts.into_iter().map(|t| {
            zed::toast(theme, t.kind, t.message)
                .id(("toast", t.id))
        });

        div()
            .id("toast_layer")
            .absolute()
            .right_0()
            .bottom_0()
            .p_3()
            .flex()
            .flex_col()
            .items_end()
            .gap_2()
            .children(children)
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
                            view.apply_state_snapshot(snapshot, cx);
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
