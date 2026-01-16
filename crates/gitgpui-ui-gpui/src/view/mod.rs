use crate::{theme::AppTheme, zed_port as zed};
use gitgpui_core::diff::{AnnotatedDiffLine, annotate_unified};
use gitgpui_core::domain::{
    Commit, CommitId, DiffArea, DiffTarget, FileStatus, FileStatusKind, RepoStatus,
};
use gitgpui_core::file_diff::FileDiffRow;
use gitgpui_core::services::PullMode;
use gitgpui_state::model::{AppState, DiagnosticKind, Loadable, RepoId, RepoState};
use gitgpui_state::msg::{Msg, StoreEvent};
use gitgpui_state::session;
use gitgpui_state::store::AppStore;
use gpui::prelude::*;
use gpui::{
    AnyElement, Bounds, ClickEvent, Corner, CursorStyle, Decorations, Entity, FocusHandle,
    FontWeight,
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

fn should_hide_unified_diff_header_line(line: &AnnotatedDiffLine) -> bool {
    matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
        && (line.text.starts_with("index ")
            || line.text.starts_with("--- ")
            || line.text.starts_with("+++ "))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffViewMode {
    Inline,
    Split,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HistoryTab {
    Log,
    Stash,
    Reflog,
}

#[derive(Clone, Debug)]
struct ToastState {
    id: u64,
    kind: zed::ToastKind,
    message: String,
}

#[derive(Clone, Debug)]
struct CachedDiffTextSegment {
    text: SharedString,
    in_word: bool,
    in_query: bool,
    syntax: SyntaxTokenKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SyntaxTokenKind {
    None,
    Comment,
    String,
    Keyword,
    Number,
    Function,
    Type,
    Property,
    Constant,
    Punctuation,
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

#[derive(Clone, Debug)]
enum PatchSplitRow {
    Raw { src_ix: usize, click_kind: DiffClickKind },
    Aligned {
        row: FileDiffRow,
        old_src_ix: Option<usize>,
        new_src_ix: Option<usize>,
    },
}

pub struct GitGpuiView {
    store: Arc<AppStore>,
    state: AppState,
    _poller: Poller,
    _appearance_subscription: gpui::Subscription,
    theme: AppTheme,

    diff_view: DiffViewMode,
    diff_cache_repo_id: Option<RepoId>,
    diff_cache_rev: u64,
    diff_cache_target: Option<DiffTarget>,
    diff_cache: Vec<AnnotatedDiffLine>,
    diff_file_for_src_ix: Vec<Option<String>>,
    diff_split_cache: Vec<PatchSplitRow>,
    diff_split_cache_len: usize,
    diff_search_input: Entity<zed::TextInput>,
    diff_search_raw: String,
    diff_search_debounced: String,
    diff_search_seq: u64,
    diff_panel_focus_handle: FocusHandle,
    diff_autoscroll_pending: bool,
    diff_visible_indices: Vec<usize>,
    diff_visible_cache_len: usize,
    diff_visible_query: String,
    diff_visible_view: DiffViewMode,
    diff_visible_is_file_view: bool,
    diff_query_match_count: usize,
    diff_scrollbar_markers_cache: Vec<zed::ScrollbarMarker>,
    diff_word_highlights: HashMap<usize, Vec<Range<usize>>>,
    diff_file_stats: HashMap<usize, (usize, usize)>,
    diff_text_segments_cache_query: String,
    diff_text_segments_cache: HashMap<usize, Vec<CachedDiffTextSegment>>,
    diff_selection_anchor: Option<usize>,
    diff_selection_range: Option<(usize, usize)>,
    diff_hunk_picker_search_input: Option<Entity<zed::TextInput>>,

    file_diff_cache_repo_id: Option<RepoId>,
    file_diff_cache_rev: u64,
    file_diff_cache_target: Option<DiffTarget>,
    file_diff_cache_path: Option<std::path::PathBuf>,
    file_diff_cache_rows: Vec<FileDiffRow>,
    file_diff_inline_cache: Vec<AnnotatedDiffLine>,
    file_diff_inline_word_highlights: HashMap<usize, Vec<Range<usize>>>,
    file_diff_split_word_highlights_old: HashMap<usize, Vec<Range<usize>>>,
    file_diff_split_word_highlights_new: HashMap<usize, Vec<Range<usize>>>,

    worktree_preview_path: Option<std::path::PathBuf>,
    worktree_preview: Loadable<Arc<Vec<String>>>,
    worktree_preview_scroll: UniformListScrollHandle,
    worktree_preview_segments_cache_path: Option<std::path::PathBuf>,
    worktree_preview_segments_cache: HashMap<usize, Vec<CachedDiffTextSegment>>,
    diff_preview_is_new_file: bool,
    diff_preview_new_file_lines: Arc<Vec<String>>,

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
    commit_files_scroll: UniformListScrollHandle,
    sidebar_scroll: ScrollHandle,
    commit_scroll: ScrollHandle,

    toasts: Vec<ToastState>,

    hovered_status_row: Option<(RepoId, DiffArea, std::path::PathBuf)>,
}

impl GitGpuiView {
    fn is_file_diff_view_active(&self) -> bool {
        let Some(repo) = self.active_repo() else {
            return false;
        };
        self.file_diff_cache_repo_id == Some(repo.id)
            && self.file_diff_cache_rev == repo.diff_file_rev
            && self.file_diff_cache_target == repo.diff_target
            && self.file_diff_cache_path.is_some()
    }

    fn handle_file_diff_row_click(&mut self, clicked_visible_ix: usize, shift: bool) {
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

        self.diff_selection_anchor = Some(clicked_visible_ix);
        self.diff_selection_range = Some((clicked_visible_ix, clicked_visible_ix));
    }

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

        let diff_panel_focus_handle = cx.focus_handle().tab_index(0).tab_stop(false);

        let mut view = Self {
            state: store.snapshot(),
            store,
            _poller: poller,
            _appearance_subscription: appearance_subscription,
            theme: initial_theme,
            diff_view: DiffViewMode::Split,
            diff_cache_repo_id: None,
            diff_cache_rev: 0,
            diff_cache_target: None,
            diff_cache: Vec::new(),
            diff_file_for_src_ix: Vec::new(),
            diff_split_cache: Vec::new(),
            diff_split_cache_len: 0,
            diff_search_input,
            diff_search_raw: String::new(),
            diff_search_debounced: String::new(),
            diff_search_seq: 0,
            diff_panel_focus_handle,
            diff_autoscroll_pending: false,
            diff_visible_indices: Vec::new(),
            diff_visible_cache_len: 0,
            diff_visible_query: String::new(),
            diff_visible_view: DiffViewMode::Split,
            diff_visible_is_file_view: false,
            diff_query_match_count: 0,
            diff_scrollbar_markers_cache: Vec::new(),
            diff_word_highlights: HashMap::new(),
            diff_file_stats: HashMap::new(),
            diff_text_segments_cache_query: String::new(),
            diff_text_segments_cache: HashMap::new(),
            diff_selection_anchor: None,
            diff_selection_range: None,
            diff_hunk_picker_search_input: None,
            file_diff_cache_repo_id: None,
            file_diff_cache_rev: 0,
            file_diff_cache_target: None,
            file_diff_cache_path: None,
            file_diff_cache_rows: Vec::new(),
            file_diff_inline_cache: Vec::new(),
            file_diff_inline_word_highlights: HashMap::new(),
            file_diff_split_word_highlights_old: HashMap::new(),
            file_diff_split_word_highlights_new: HashMap::new(),
            worktree_preview_path: None,
            worktree_preview: Loadable::NotLoaded,
            worktree_preview_scroll: UniformListScrollHandle::default(),
            worktree_preview_segments_cache_path: None,
            worktree_preview_segments_cache: HashMap::new(),
            diff_preview_is_new_file: false,
            diff_preview_new_file_lines: Arc::new(Vec::new()),
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
            commit_files_scroll: UniformListScrollHandle::default(),
            sidebar_scroll: ScrollHandle::new(),
            commit_scroll: ScrollHandle::new(),
            toasts: Vec::new(),
            hovered_status_row: None,
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
        let input = self
            .history_branch_picker_search_input
            .get_or_insert_with(|| {
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
        let store = Arc::clone(&self.store);
        let view = cx.weak_entity();

        let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open Git Repository".into()),
        });

        window
            .spawn(cx, async move |cx| {
                let result = rx.await;
                let paths = match result {
                    Ok(Ok(Some(paths))) => paths,
                    Ok(Ok(None)) => return,
                    Ok(Err(_)) | Err(_) => {
                        let _ = view.update(cx, |this, cx| {
                            this.open_repo_panel = true;
                            cx.notify();
                        });
                        return;
                    }
                };

                let Some(path) = paths.into_iter().next() else {
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
    }

    fn untracked_worktree_preview_path(&self) -> Option<std::path::PathBuf> {
        let repo = self.active_repo()?;
        let status = match &repo.status {
            Loadable::Ready(s) => s,
            _ => return None,
        };
        let workdir = repo.spec.workdir.clone();
        let DiffTarget::WorkingTree { path, area } = repo.diff_target.as_ref()? else {
            return None;
        };
        if *area != DiffArea::Unstaged {
            return None;
        }
        let is_untracked = status
            .unstaged
            .iter()
            .any(|e| e.kind == FileStatusKind::Untracked && &e.path == path);
        is_untracked.then(|| {
            if path.is_absolute() {
                path.clone()
            } else {
                workdir.join(path)
            }
        })
    }

    fn added_file_preview_abs_path(&self) -> Option<std::path::PathBuf> {
        let repo = self.active_repo()?;
        let workdir = repo.spec.workdir.clone();
        let target = repo.diff_target.as_ref()?;

        match target {
            DiffTarget::WorkingTree { path, area } => {
                if *area != DiffArea::Staged {
                    return None;
                }
                let status = match &repo.status {
                    Loadable::Ready(s) => s,
                    _ => return None,
                };
                let is_added = status
                    .staged
                    .iter()
                    .any(|e| e.kind == FileStatusKind::Added && &e.path == path);
                if !is_added {
                    return None;
                }
                Some(if path.is_absolute() {
                    path.clone()
                } else {
                    workdir.join(path)
                })
            }
            DiffTarget::Commit {
                commit_id,
                path: Some(path),
            } => {
                let details = match &repo.commit_details {
                    Loadable::Ready(d) => d,
                    _ => return None,
                };
                if &details.id != commit_id {
                    return None;
                }
                let is_added = details
                    .files
                    .iter()
                    .any(|f| f.kind == FileStatusKind::Added && &f.path == path);
                if !is_added {
                    return None;
                }
                Some(workdir.join(path))
            }
            _ => None,
        }
    }

    fn ensure_preview_loading(&mut self, path: std::path::PathBuf) {
        let should_reset = match self.worktree_preview_path.as_ref() {
            Some(p) => p != &path,
            None => true,
        };
        if should_reset {
            self.worktree_preview_path = Some(path);
            self.worktree_preview = Loadable::Loading;
            self.worktree_preview_segments_cache_path = None;
            self.worktree_preview_segments_cache.clear();
        } else if matches!(self.worktree_preview, Loadable::NotLoaded | Loadable::Error(_)) {
            self.worktree_preview = Loadable::Loading;
        }
    }

    fn ensure_worktree_preview_loaded(
        &mut self,
        path: std::path::PathBuf,
        cx: &mut gpui::Context<Self>,
    ) {
        let should_reload = match self.worktree_preview_path.as_ref() {
            Some(p) => p != &path,
            None => true,
        } || matches!(self.worktree_preview, Loadable::Error(_) | Loadable::NotLoaded);
        if !should_reload {
            return;
        }

        self.worktree_preview_path = Some(path.clone());
        self.worktree_preview = Loadable::Loading;
        self.worktree_preview_segments_cache_path = None;
        self.worktree_preview_segments_cache.clear();

        cx.spawn(async move |view, cx| {
            const MAX_BYTES: u64 = 2 * 1024 * 1024;
            let path_for_task = path.clone();
            let task = cx.background_executor().spawn(async move {
                let meta = std::fs::metadata(&path_for_task).map_err(|e| e.to_string())?;
                if meta.len() > MAX_BYTES {
                    return Err(format!(
                        "File is too large to preview ({} bytes).",
                        meta.len()
                    ));
                }

                let bytes = std::fs::read(&path_for_task).map_err(|e| e.to_string())?;
                let text = String::from_utf8(bytes).map_err(|_| {
                    "File is not valid UTF-8; binary preview is not supported.".to_string()
                })?;

                let lines = text.lines().map(|s| s.to_string()).collect::<Vec<_>>();
                Ok::<Arc<Vec<String>>, String>(Arc::new(lines))
            });

            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                if this.worktree_preview_path.as_ref() != Some(&path) {
                    return;
                }
                this.worktree_preview = match result {
                    Ok(lines) => Loadable::Ready(lines),
                    Err(e) => Loadable::Error(e),
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn try_populate_worktree_preview_from_diff_file(&mut self) {
        let Some((abs_path, preview_result)) = (|| {
            let repo = self.active_repo()?;
            let path_from_target = match repo.diff_target.as_ref()? {
                DiffTarget::WorkingTree { path, .. } => Some(path),
                DiffTarget::Commit {
                    path: Some(path), ..
                } => Some(path),
                _ => None,
            }?;

            let abs_path = if path_from_target.is_absolute() {
                path_from_target.clone()
            } else {
                repo.spec.workdir.join(path_from_target)
            };

            let mut diff_file_error: Option<String> = None;
            let mut preview_result: Option<Result<Arc<Vec<String>>, String>> = match &repo.diff_file
            {
                Loadable::NotLoaded | Loadable::Loading => None,
                Loadable::Error(e) => {
                    diff_file_error = Some(e.clone());
                    None
                }
                Loadable::Ready(file) => file.as_ref().and_then(|file| {
                    file.new.as_deref().map(|text| {
                        let lines = text.lines().map(|s| s.to_string()).collect::<Vec<_>>();
                        Ok(Arc::new(lines))
                    })
                }),
            };

            if preview_result.is_none() {
                match &repo.diff {
                    Loadable::Ready(diff) => {
                        let annotated = annotate_unified(diff);
                        if let Some((_abs_path, lines)) = build_new_file_preview_from_diff(
                            &annotated,
                            &repo.spec.workdir,
                            repo.diff_target.as_ref(),
                        ) {
                            preview_result = Some(Ok(Arc::new(lines)));
                        } else if let Some(e) = diff_file_error {
                            preview_result = Some(Err(e));
                        } else {
                            preview_result =
                                Some(Err("No text preview available for this file.".to_string()));
                        }
                    }
                    Loadable::Error(e) => preview_result = Some(Err(e.clone())),
                    Loadable::NotLoaded | Loadable::Loading => {}
                }
            }

            Some((abs_path, preview_result))
        })() else {
            return;
        };

        if matches!(self.worktree_preview, Loadable::Ready(_))
            && self.worktree_preview_path.as_ref() == Some(&abs_path)
        {
            return;
        }

        let Some(preview_result) = preview_result else {
            return;
        };

        match preview_result {
            Ok(lines) => {
                self.worktree_preview_path = Some(abs_path);
                self.worktree_preview = Loadable::Ready(lines);
                self.worktree_preview_segments_cache_path = None;
                self.worktree_preview_segments_cache.clear();
            }
            Err(e) => {
                if self.worktree_preview_path.as_ref() != Some(&abs_path)
                    || matches!(self.worktree_preview, Loadable::NotLoaded | Loadable::Loading)
                {
                    self.worktree_preview_path = Some(abs_path);
                    self.worktree_preview = Loadable::Error(e);
                    self.worktree_preview_segments_cache_path = None;
                    self.worktree_preview_segments_cache.clear();
                }
            }
        }
    }

    fn is_file_diff_target(target: Option<&DiffTarget>) -> bool {
        matches!(
            target,
            Some(DiffTarget::WorkingTree { .. } | DiffTarget::Commit { path: Some(_), .. })
        )
    }

    fn ensure_file_diff_cache(&mut self) {
        let Some((repo_id, diff_file_rev, diff_target, workdir, diff_file)) = self
            .active_repo()
            .map(|repo| {
                (
                    repo.id,
                    repo.diff_file_rev,
                    repo.diff_target.clone(),
                    repo.spec.workdir.clone(),
                    repo.diff_file.clone(),
                )
            })
        else {
            self.file_diff_cache_repo_id = None;
            self.file_diff_cache_target = None;
            self.file_diff_cache_rev = 0;
            self.file_diff_cache_path = None;
            self.file_diff_cache_rows.clear();
            self.file_diff_inline_cache.clear();
            self.file_diff_inline_word_highlights.clear();
            self.file_diff_split_word_highlights_old.clear();
            self.file_diff_split_word_highlights_new.clear();
            return;
        };

        if !Self::is_file_diff_target(diff_target.as_ref()) {
            self.file_diff_cache_repo_id = None;
            self.file_diff_cache_target = None;
            self.file_diff_cache_rev = 0;
            self.file_diff_cache_path = None;
            self.file_diff_cache_rows.clear();
            self.file_diff_inline_cache.clear();
            self.file_diff_inline_word_highlights.clear();
            self.file_diff_split_word_highlights_old.clear();
            self.file_diff_split_word_highlights_new.clear();
            return;
        }

        if self.file_diff_cache_repo_id == Some(repo_id)
            && self.file_diff_cache_rev == diff_file_rev
            && self.file_diff_cache_target == diff_target
        {
            return;
        }

        self.file_diff_cache_repo_id = Some(repo_id);
        self.file_diff_cache_rev = diff_file_rev;
        self.file_diff_cache_target = diff_target.clone();
        self.file_diff_cache_path = None;
        self.file_diff_cache_rows.clear();
        self.file_diff_inline_cache.clear();
        self.file_diff_inline_word_highlights.clear();
        self.file_diff_split_word_highlights_old.clear();
        self.file_diff_split_word_highlights_new.clear();

        let Loadable::Ready(file_opt) = diff_file else {
            return;
        };
        let Some(file) = file_opt.as_ref() else {
            return;
        };

        let old_text = file.old.as_deref().unwrap_or("");
        let new_text = file.new.as_deref().unwrap_or("");
        self.file_diff_cache_rows = gitgpui_core::file_diff::side_by_side_rows(old_text, new_text);

        // Store the file path for syntax highlighting.
        let path = if file.path.is_absolute() {
            file.path.clone()
        } else {
            workdir.join(&file.path)
        };
        self.file_diff_cache_path = Some(path);

        // Precompute word highlights and inline rows.
        for (row_ix, row) in self.file_diff_cache_rows.iter().enumerate() {
            if matches!(row.kind, gitgpui_core::file_diff::FileDiffRowKind::Modify) {
                let old = row.old.as_deref().unwrap_or("");
                let new = row.new.as_deref().unwrap_or("");
                let (old_ranges, new_ranges) = word_diff_ranges(old, new);
                if !old_ranges.is_empty() {
                    self.file_diff_split_word_highlights_old
                        .insert(row_ix, old_ranges.clone());
                }
                if !new_ranges.is_empty() {
                    self.file_diff_split_word_highlights_new
                        .insert(row_ix, new_ranges.clone());
                }
            }
        }

        let mut inline_ix = 0usize;
        for row in &self.file_diff_cache_rows {
            use gitgpui_core::file_diff::FileDiffRowKind as K;
            match row.kind {
                K::Context => {
                    self.file_diff_inline_cache.push(AnnotatedDiffLine {
                        kind: gitgpui_core::domain::DiffLineKind::Context,
                        text: format!(" {}", row.old.as_deref().unwrap_or("")),
                        old_line: row.old_line,
                        new_line: row.new_line,
                    });
                    inline_ix += 1;
                }
                K::Add => {
                    self.file_diff_inline_cache.push(AnnotatedDiffLine {
                        kind: gitgpui_core::domain::DiffLineKind::Add,
                        text: format!("+{}", row.new.as_deref().unwrap_or("")),
                        old_line: None,
                        new_line: row.new_line,
                    });
                    inline_ix += 1;
                }
                K::Remove => {
                    self.file_diff_inline_cache.push(AnnotatedDiffLine {
                        kind: gitgpui_core::domain::DiffLineKind::Remove,
                        text: format!("-{}", row.old.as_deref().unwrap_or("")),
                        old_line: row.old_line,
                        new_line: None,
                    });
                    inline_ix += 1;
                }
                K::Modify => {
                    let old = row.old.as_deref().unwrap_or("");
                    let new = row.new.as_deref().unwrap_or("");
                    let (old_ranges, new_ranges) = word_diff_ranges(old, new);

                    self.file_diff_inline_cache.push(AnnotatedDiffLine {
                        kind: gitgpui_core::domain::DiffLineKind::Remove,
                        text: format!("-{}", old),
                        old_line: row.old_line,
                        new_line: None,
                    });
                    if !old_ranges.is_empty() {
                        self.file_diff_inline_word_highlights
                            .insert(inline_ix, old_ranges);
                    }
                    inline_ix += 1;

                    self.file_diff_inline_cache.push(AnnotatedDiffLine {
                        kind: gitgpui_core::domain::DiffLineKind::Add,
                        text: format!("+{}", new),
                        old_line: None,
                        new_line: row.new_line,
                    });
                    if !new_ranges.is_empty() {
                        self.file_diff_inline_word_highlights
                            .insert(inline_ix, new_ranges);
                    }
                    inline_ix += 1;
                }
            }
        }

        // Reset the segment cache to avoid mixing patch/file indices.
        self.diff_text_segments_cache_query.clear();
        self.diff_text_segments_cache.clear();
    }

    fn rebuild_diff_cache(&mut self) {
        self.diff_cache.clear();
        self.diff_cache_repo_id = None;
        self.diff_cache_rev = 0;
        self.diff_cache_target = None;
        self.diff_file_for_src_ix.clear();
        self.diff_split_cache.clear();
        self.diff_split_cache_len = 0;
        self.diff_visible_indices.clear();
        self.diff_visible_cache_len = 0;
        self.diff_visible_is_file_view = false;
        self.diff_query_match_count = 0;
        self.diff_scrollbar_markers_cache.clear();
        self.diff_word_highlights.clear();
        self.diff_file_stats.clear();
        self.diff_text_segments_cache_query.clear();
        self.diff_text_segments_cache.clear();
        self.diff_selection_anchor = None;
        self.diff_selection_range = None;
        self.diff_preview_is_new_file = false;
        self.diff_preview_new_file_lines = Arc::new(Vec::new());

        let (repo_id, diff_rev, diff_target, workdir, annotated) = {
            let Some(repo) = self.active_repo() else {
                return;
            };
            let workdir = repo.spec.workdir.clone();
            let annotated = match &repo.diff {
                Loadable::Ready(diff) => Some(annotate_unified(diff)),
                _ => None,
            };
            (repo.id, repo.diff_rev, repo.diff_target.clone(), workdir, annotated)
        };

        self.diff_cache_repo_id = Some(repo_id);
        self.diff_cache_rev = diff_rev;
        self.diff_cache_target = diff_target;

        let Some(annotated) = annotated else {
            return;
        };

        self.diff_cache = annotated;
        self.diff_file_for_src_ix = compute_diff_file_for_src_ix(&self.diff_cache);
        self.diff_file_stats = compute_diff_file_stats(&self.diff_cache);
        self.rebuild_diff_word_highlights();

        if let Some((abs_path, lines)) = build_new_file_preview_from_diff(&self.diff_cache, &workdir, self.diff_cache_target.as_ref()) {
            self.diff_preview_is_new_file = true;
            self.diff_preview_new_file_lines = Arc::new(lines);
            self.worktree_preview_path = Some(abs_path);
            self.worktree_preview = Loadable::Ready(self.diff_preview_new_file_lines.clone());
            self.worktree_preview_segments_cache_path = None;
            self.worktree_preview_segments_cache.clear();
        }
    }

    fn ensure_diff_split_cache(&mut self) {
        if self.diff_split_cache_len == self.diff_cache.len() && !self.diff_split_cache.is_empty() {
            return;
        }
        self.diff_split_cache_len = self.diff_cache.len();
        self.diff_split_cache = build_patch_split_rows(&self.diff_cache);
    }

    fn diff_scrollbar_markers_patch(&self) -> Vec<zed::ScrollbarMarker> {
        match self.diff_view {
            DiffViewMode::Inline => scrollbar_markers_from_flags(
                self.diff_visible_indices.len(),
                |visible_ix| {
                    let Some(&src_ix) = self.diff_visible_indices.get(visible_ix) else {
                        return 0;
                    };
                    let Some(line) = self.diff_cache.get(src_ix) else {
                        return 0;
                    };
                    match line.kind {
                        gitgpui_core::domain::DiffLineKind::Add => 1,
                        gitgpui_core::domain::DiffLineKind::Remove => 2,
                        _ => 0,
                    }
                },
            ),
            DiffViewMode::Split => scrollbar_markers_from_flags(
                self.diff_visible_indices.len(),
                |visible_ix| {
                    let Some(&row_ix) = self.diff_visible_indices.get(visible_ix) else {
                        return 0;
                    };
                    let Some(row) = self.diff_split_cache.get(row_ix) else {
                        return 0;
                    };
                    match row {
                        PatchSplitRow::Aligned { row, .. } => match row.kind {
                            gitgpui_core::file_diff::FileDiffRowKind::Add => 1,
                            gitgpui_core::file_diff::FileDiffRowKind::Remove => 2,
                            gitgpui_core::file_diff::FileDiffRowKind::Modify => 3,
                            gitgpui_core::file_diff::FileDiffRowKind::Context => 0,
                        },
                        PatchSplitRow::Raw { .. } => 0,
                    }
                },
            ),
        }
    }

    fn compute_diff_scrollbar_markers(&self) -> Vec<zed::ScrollbarMarker> {
        if !self.is_file_diff_view_active() {
            return self.diff_scrollbar_markers_patch();
        }

        match self.diff_view {
            DiffViewMode::Inline => scrollbar_markers_from_flags(
                self.diff_visible_indices.len(),
                |visible_ix| {
                    let Some(&inline_ix) = self.diff_visible_indices.get(visible_ix) else {
                        return 0;
                    };
                    let Some(line) = self.file_diff_inline_cache.get(inline_ix) else {
                        return 0;
                    };
                    match line.kind {
                        gitgpui_core::domain::DiffLineKind::Add => 1,
                        gitgpui_core::domain::DiffLineKind::Remove => 2,
                        _ => 0,
                    }
                },
            ),
            DiffViewMode::Split => scrollbar_markers_from_flags(
                self.diff_visible_indices.len(),
                |visible_ix| {
                    let Some(&row_ix) = self.diff_visible_indices.get(visible_ix) else {
                        return 0;
                    };
                    let Some(row) = self.file_diff_cache_rows.get(row_ix) else {
                        return 0;
                    };
                    match row.kind {
                        gitgpui_core::file_diff::FileDiffRowKind::Add => 1,
                        gitgpui_core::file_diff::FileDiffRowKind::Remove => 2,
                        gitgpui_core::file_diff::FileDiffRowKind::Modify => 3,
                        gitgpui_core::file_diff::FileDiffRowKind::Context => 0,
                    }
                },
            ),
        }
    }

    fn apply_state_snapshot(&mut self, next: AppState, cx: &mut gpui::Context<Self>) {
        let next_repo_id = next.active_repo;
        let next_repo = next_repo_id.and_then(|id| next.repos.iter().find(|r| r.id == id));
        let next_diff_target = next_repo.and_then(|r| r.diff_target.as_ref()).cloned();
        let next_diff_rev = next_repo.map(|r| r.diff_rev).unwrap_or(0);

        let prev_diff_target = self
            .active_repo()
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
            self.diff_selection_anchor = None;
            self.diff_selection_range = None;
            self.diff_autoscroll_pending = next_diff_target.is_some();
        }

        self.state = next;

        let should_rebuild_diff_cache = self.diff_cache_repo_id != next_repo_id
            || self.diff_cache_rev != next_diff_rev
            || self.diff_cache_target != next_diff_target;
        if should_rebuild_diff_cache {
            self.rebuild_diff_cache();
        }
    }

    fn push_toast(&mut self, kind: zed::ToastKind, message: String, cx: &mut gpui::Context<Self>) {
        let id = self
            .toasts
            .last()
            .map(|t| t.id.wrapping_add(1))
            .unwrap_or(1);
        self.toasts.push(ToastState { id, kind, message });

        cx.spawn(
            async move |view: WeakEntity<GitGpuiView>, cx: &mut gpui::AsyncApp| {
                Timer::after(Duration::from_secs(5)).await;
                let _ = view.update(cx, |this, cx| {
                    this.toasts.retain(|t| t.id != id);
                    cx.notify();
                });
            },
        )
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
                    self.diff_word_highlights
                        .insert(old_ix, vec![0..old_text.len()]);
                }
            }
            for (new_ix, new_text) in added.into_iter().skip(pairs) {
                if !new_text.is_empty() {
                    self.diff_word_highlights
                        .insert(new_ix, vec![0..new_text.len()]);
                }
            }
        }
    }

    fn update_diff_search_debounce(&mut self, cx: &mut gpui::Context<Self>) {
        let mut raw: Option<String> = None;
        self.diff_search_input.read_with(cx, |i, _| {
            if i.text() != self.diff_search_raw {
                raw = Some(i.text().to_string());
            }
        });
        let Some(raw) = raw else { return; };

        self.diff_search_raw = raw.clone();
        self.diff_search_seq = self.diff_search_seq.wrapping_add(1);
        let seq = self.diff_search_seq;

        cx.spawn(
            async move |view: WeakEntity<GitGpuiView>, cx: &mut gpui::AsyncApp| {
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
                    this.diff_scrollbar_markers_cache.clear();
                    cx.notify();
                });
            },
        )
        .detach();
    }

    fn ensure_diff_visible_indices(&mut self) {
        let query_trimmed = self.diff_search_debounced.trim();
        let is_file_view = self.is_file_diff_view_active();
        let current_len = if is_file_view {
            match self.diff_view {
                DiffViewMode::Inline => self.file_diff_inline_cache.len(),
                DiffViewMode::Split => self.file_diff_cache_rows.len(),
            }
        } else {
            self.diff_cache.len()
        };

        if self.diff_visible_cache_len == current_len
            && self.diff_visible_query == query_trimmed
            && self.diff_visible_view == self.diff_view
            && self.diff_visible_is_file_view == is_file_view
        {
            return;
        }

        let query = query_trimmed.to_string();
        self.diff_visible_cache_len = current_len;
        self.diff_visible_query = query.clone();
        self.diff_visible_view = self.diff_view;
        self.diff_visible_is_file_view = is_file_view;
        self.diff_query_match_count = 0;

        if is_file_view {
            if query.is_empty() {
                self.diff_visible_indices = (0..current_len).collect();
                self.diff_scrollbar_markers_cache = self.compute_diff_scrollbar_markers();
                return;
            }

            let q = query.to_ascii_lowercase();
            let mut match_count = 0usize;
            let mut indices = Vec::new();
            match self.diff_view {
                DiffViewMode::Inline => {
                    for (ix, line) in self.file_diff_inline_cache.iter().enumerate() {
                        let text = diff_content_text(line);
                        if text.to_ascii_lowercase().contains(&q) {
                            match_count += 1;
                            indices.push(ix);
                        }
                    }
                }
                DiffViewMode::Split => {
                    for (ix, row) in self.file_diff_cache_rows.iter().enumerate() {
                        let old_match = row
                            .old
                            .as_deref()
                            .is_some_and(|s| s.to_ascii_lowercase().contains(&q));
                        let new_match = row
                            .new
                            .as_deref()
                            .is_some_and(|s| s.to_ascii_lowercase().contains(&q));
                        if old_match || new_match {
                            match_count += 1;
                            indices.push(ix);
                        }
                    }
                }
            }
            self.diff_query_match_count = match_count;
            self.diff_visible_indices = indices;
            self.diff_scrollbar_markers_cache = self.compute_diff_scrollbar_markers();
            return;
        }

        match self.diff_view {
            DiffViewMode::Inline => {
                if query.is_empty() {
                    self.diff_visible_indices = self
                        .diff_cache
                        .iter()
                        .enumerate()
                        .filter_map(|(ix, line)| {
                            (!should_hide_unified_diff_header_line(line)).then_some(ix)
                        })
                        .collect();
                    self.diff_scrollbar_markers_cache = self.compute_diff_scrollbar_markers();
                    return;
                }

                let q = query.to_ascii_lowercase();
                let mut match_count = 0usize;
                let mut indices = Vec::new();
                for (ix, line) in self.diff_cache.iter().enumerate() {
                    if should_hide_unified_diff_header_line(line) {
                        continue;
                    }
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
            DiffViewMode::Split => {
                self.ensure_diff_split_cache();

                if query.is_empty() {
                    self.diff_visible_indices = self
                        .diff_split_cache
                        .iter()
                        .enumerate()
                        .filter_map(|(ix, row)| match row {
                            PatchSplitRow::Raw { src_ix, .. } => self
                                .diff_cache
                                .get(*src_ix)
                                .is_some_and(|line| !should_hide_unified_diff_header_line(line))
                                .then_some(ix),
                            PatchSplitRow::Aligned { .. } => Some(ix),
                        })
                        .collect();
                    self.diff_scrollbar_markers_cache = self.compute_diff_scrollbar_markers();
                    return;
                }

                let q = query.to_ascii_lowercase();
                let mut match_count = 0usize;
                let mut indices = Vec::new();
                for (ix, row) in self.diff_split_cache.iter().enumerate() {
                    match row {
                        PatchSplitRow::Raw { src_ix, click_kind } => {
                            if matches!(click_kind, DiffClickKind::HunkHeader | DiffClickKind::FileHeader)
                            {
                                indices.push(ix);
                                continue;
                            }

                            let Some(line) = self.diff_cache.get(*src_ix) else {
                                continue;
                            };
                            if should_hide_unified_diff_header_line(line) {
                                continue;
                            }
                            if line.text.to_ascii_lowercase().contains(&q) {
                                match_count += 1;
                                indices.push(ix);
                            }
                        }
                        PatchSplitRow::Aligned { row, .. } => {
                            let old_match = row
                                .old
                                .as_deref()
                                .is_some_and(|s| s.to_ascii_lowercase().contains(&q));
                            let new_match = row
                                .new
                                .as_deref()
                                .is_some_and(|s| s.to_ascii_lowercase().contains(&q));
                            if old_match || new_match {
                                match_count += 1;
                                indices.push(ix);
                            }
                        }
                    }
                }
                self.diff_query_match_count = match_count;
                self.diff_visible_indices = indices;
            }
        }

        self.diff_scrollbar_markers_cache = self.compute_diff_scrollbar_markers();
    }

    fn handle_patch_row_click(
        &mut self,
        clicked_visible_ix: usize,
        kind: DiffClickKind,
        shift: bool,
    ) {
        if self.is_file_diff_view_active() {
            self.handle_file_diff_row_click(clicked_visible_ix, shift);
            return;
        }
        match self.diff_view {
            DiffViewMode::Inline => self.handle_diff_row_click(clicked_visible_ix, kind, shift),
            DiffViewMode::Split => self.handle_split_row_click(clicked_visible_ix, kind, shift),
        }
    }

    fn handle_split_row_click(&mut self, clicked_visible_ix: usize, kind: DiffClickKind, shift: bool) {
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
                .split_next_boundary_visible_ix(clicked_visible_ix, |row| {
                    matches!(
                        row,
                        PatchSplitRow::Raw {
                            click_kind: DiffClickKind::HunkHeader | DiffClickKind::FileHeader,
                            ..
                        }
                    )
                })
                .unwrap_or(list_len - 1),
            DiffClickKind::FileHeader => self
                .split_next_boundary_visible_ix(clicked_visible_ix, |row| {
                    matches!(
                        row,
                        PatchSplitRow::Raw {
                            click_kind: DiffClickKind::FileHeader,
                            ..
                        }
                    )
                })
                .unwrap_or(list_len - 1),
        };

        self.diff_selection_anchor = Some(clicked_visible_ix);
        self.diff_selection_range = Some((clicked_visible_ix, end));
    }

    fn split_next_boundary_visible_ix(
        &self,
        from_visible_ix: usize,
        is_boundary: impl Fn(&PatchSplitRow) -> bool,
    ) -> Option<usize> {
        let from_visible_ix =
            from_visible_ix.min(self.diff_visible_indices.len().saturating_sub(1));
        for visible_ix in (from_visible_ix + 1)..self.diff_visible_indices.len() {
            let row_ix = *self.diff_visible_indices.get(visible_ix)?;
            let row = self.diff_split_cache.get(row_ix)?;
            if is_boundary(row) {
                return Some(visible_ix.saturating_sub(1));
            }
        }
        None
    }

    fn patch_hunk_entries(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for (visible_ix, &ix) in self.diff_visible_indices.iter().enumerate() {
            match self.diff_view {
                DiffViewMode::Inline => {
                    let Some(line) = self.diff_cache.get(ix) else {
                        continue;
                    };
                    if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk) {
                        out.push((visible_ix, ix));
                    }
                }
                DiffViewMode::Split => {
                    let Some(row) = self.diff_split_cache.get(ix) else {
                        continue;
                    };
                    if let PatchSplitRow::Raw {
                        src_ix,
                        click_kind: DiffClickKind::HunkHeader,
                    } = row
                    {
                        out.push((visible_ix, *src_ix));
                    }
                }
            }
        }
        out
    }

    fn handle_diff_row_click(
        &mut self,
        clicked_visible_ix: usize,
        kind: DiffClickKind,
        shift: bool,
    ) {
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

    fn diff_next_boundary_visible_ix(
        &self,
        from_visible_ix: usize,
        is_boundary: impl Fn(usize) -> bool,
    ) -> Option<usize> {
        let from_visible_ix =
            from_visible_ix.min(self.diff_visible_indices.len().saturating_sub(1));
        for visible_ix in (from_visible_ix + 1)..self.diff_visible_indices.len() {
            let src_ix = *self.diff_visible_indices.get(visible_ix)?;
            if is_boundary(src_ix) {
                return Some(visible_ix.saturating_sub(1));
            }
        }
        None
    }

    fn file_change_visible_indices(&self) -> Vec<usize> {
        if !self.is_file_diff_view_active() {
            return Vec::new();
        }
        let mut out: Vec<usize> = Vec::new();
        match self.diff_view {
            DiffViewMode::Inline => {
                let mut prev_changed = false;
                for visible_ix in 0..self.diff_visible_indices.len() {
                    let Some(&inline_ix) = self.diff_visible_indices.get(visible_ix) else {
                        continue;
                    };
                    let changed = self
                        .file_diff_inline_cache
                        .get(inline_ix)
                        .is_some_and(|l| {
                            matches!(
                                l.kind,
                                gitgpui_core::domain::DiffLineKind::Add
                                    | gitgpui_core::domain::DiffLineKind::Remove
                            )
                        });
                    if changed && !prev_changed {
                        out.push(visible_ix);
                    }
                    prev_changed = changed;
                }
            }
            DiffViewMode::Split => {
                let mut prev_changed = false;
                for visible_ix in 0..self.diff_visible_indices.len() {
                    let Some(&row_ix) = self.diff_visible_indices.get(visible_ix) else {
                        continue;
                    };
                    let changed = self.file_diff_cache_rows.get(row_ix).is_some_and(|row| {
                        !matches!(row.kind, gitgpui_core::file_diff::FileDiffRowKind::Context)
                    });
                    if changed && !prev_changed {
                        out.push(visible_ix);
                    }
                    prev_changed = changed;
                }
            }
        }
        out
    }

    fn diff_nav_entries(&self) -> Vec<usize> {
        if self.is_file_diff_view_active() {
            return self.file_change_visible_indices();
        }
        self.patch_hunk_entries()
            .into_iter()
            .map(|(visible_ix, _)| visible_ix)
            .collect()
    }

    fn diff_jump_prev(&mut self) {
        let entries = self.diff_nav_entries();
        if entries.is_empty() {
            return;
        }

        let current = self.diff_selection_anchor.unwrap_or(0);
        let target = entries
            .iter()
            .rev()
            .find(|&&ix| ix < current)
            .copied()
            .unwrap_or_else(|| *entries.last().unwrap_or(&0));

        self.diff_scroll
            .scroll_to_item(target, gpui::ScrollStrategy::Center);
        self.diff_selection_anchor = Some(target);
        self.diff_selection_range = Some((target, target));
    }

    fn diff_jump_next(&mut self) {
        let entries = self.diff_nav_entries();
        if entries.is_empty() {
            return;
        }

        let current = self.diff_selection_anchor.unwrap_or(0);
        let target = entries
            .iter()
            .find(|&&ix| ix > current)
            .copied()
            .unwrap_or_else(|| *entries.first().unwrap_or(&0));

        self.diff_scroll
            .scroll_to_item(target, gpui::ScrollStrategy::Center);
        self.diff_selection_anchor = Some(target);
        self.diff_selection_range = Some((target, target));
    }

    fn maybe_autoscroll_diff_to_first_change(&mut self) {
        if !self.diff_autoscroll_pending {
            return;
        }
        if self.diff_visible_indices.is_empty() {
            return;
        }

        let entries = self.diff_nav_entries();
        let target = entries.first().copied().unwrap_or(0);

        self.diff_scroll
            .scroll_to_item(target, gpui::ScrollStrategy::Top);
        self.diff_selection_anchor = Some(target);
        self.diff_selection_range = Some((target, target));
        self.diff_autoscroll_pending = false;
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

        cx.spawn(
            async move |view: WeakEntity<GitGpuiView>, cx: &mut gpui::AsyncApp| {
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
            },
        )
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

fn build_patch_split_rows(diff: &[AnnotatedDiffLine]) -> Vec<PatchSplitRow> {
    use gitgpui_core::file_diff::FileDiffRowKind as K;
    use gitgpui_core::domain::DiffLineKind as DK;

    let mut out: Vec<PatchSplitRow> = Vec::with_capacity(diff.len());
    let mut ix = 0usize;

    let mut pending_removes: Vec<usize> = Vec::new();
    let mut pending_adds: Vec<usize> = Vec::new();

    fn flush_pending(
        out: &mut Vec<PatchSplitRow>,
        diff: &[AnnotatedDiffLine],
        pending_removes: &mut Vec<usize>,
        pending_adds: &mut Vec<usize>,
    ) {
        let pairs = pending_removes.len().max(pending_adds.len());
        for i in 0..pairs {
            let left_ix = pending_removes.get(i).copied();
            let right_ix = pending_adds.get(i).copied();
            let left = left_ix.and_then(|ix| diff.get(ix));
            let right = right_ix.and_then(|ix| diff.get(ix));

            let kind = match (left_ix.is_some(), right_ix.is_some()) {
                (true, true) => gitgpui_core::file_diff::FileDiffRowKind::Modify,
                (true, false) => gitgpui_core::file_diff::FileDiffRowKind::Remove,
                (false, true) => gitgpui_core::file_diff::FileDiffRowKind::Add,
                (false, false) => gitgpui_core::file_diff::FileDiffRowKind::Context,
            };
            let row = FileDiffRow {
                kind,
                old_line: left.and_then(|l| l.old_line),
                new_line: right.and_then(|l| l.new_line),
                old: left.map(|l| diff_content_text(l).to_string()),
                new: right.map(|l| diff_content_text(l).to_string()),
            };
            out.push(PatchSplitRow::Aligned {
                row,
                old_src_ix: left_ix,
                new_src_ix: right_ix,
            });
        }
        pending_removes.clear();
        pending_adds.clear();
    }

    while ix < diff.len() {
        let line = &diff[ix];
        let is_file_header = matches!(line.kind, DK::Header) && line.text.starts_with("diff --git ");

        if is_file_header {
            flush_pending(&mut out, diff, &mut pending_removes, &mut pending_adds);
            out.push(PatchSplitRow::Raw {
                src_ix: ix,
                click_kind: DiffClickKind::FileHeader,
            });
            ix += 1;
            continue;
        }

        if matches!(line.kind, DK::Hunk) {
            flush_pending(&mut out, diff, &mut pending_removes, &mut pending_adds);
            out.push(PatchSplitRow::Raw {
                src_ix: ix,
                click_kind: DiffClickKind::HunkHeader,
            });
            ix += 1;

            while ix < diff.len() {
                let line = &diff[ix];
                let is_next_file_header =
                    matches!(line.kind, DK::Header) && line.text.starts_with("diff --git ");
                if is_next_file_header || matches!(line.kind, DK::Hunk) {
                    break;
                }

                match line.kind {
                    DK::Context => {
                        flush_pending(&mut out, diff, &mut pending_removes, &mut pending_adds);
                        let text = diff_content_text(line).to_string();
                        out.push(PatchSplitRow::Aligned {
                            row: FileDiffRow {
                                kind: K::Context,
                                old_line: line.old_line,
                                new_line: line.new_line,
                                old: Some(text.clone()),
                                new: Some(text),
                            },
                            old_src_ix: Some(ix),
                            new_src_ix: Some(ix),
                        });
                    }
                    DK::Remove => pending_removes.push(ix),
                    DK::Add => pending_adds.push(ix),
                    DK::Header | DK::Hunk => {
                        flush_pending(&mut out, diff, &mut pending_removes, &mut pending_adds);
                        out.push(PatchSplitRow::Raw {
                            src_ix: ix,
                            click_kind: DiffClickKind::Line,
                        });
                    }
                }
                ix += 1;
            }

            flush_pending(&mut out, diff, &mut pending_removes, &mut pending_adds);
            continue;
        }

        // Headers outside hunks, e.g. `index`, `---`, `+++`, etc.
        out.push(PatchSplitRow::Raw {
            src_ix: ix,
            click_kind: DiffClickKind::Line,
        });
        ix += 1;
    }

    flush_pending(&mut out, diff, &mut pending_removes, &mut pending_adds);
    out
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
        let main_view = if show_diff {
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

fn scrollbar_markers_from_flags(
    len: usize,
    mut flag_at_index: impl FnMut(usize) -> u8,
) -> Vec<zed::ScrollbarMarker> {
    if len == 0 {
        return Vec::new();
    }

    let bucket_count = 240usize.min(len).max(1);
    let mut buckets = vec![0u8; bucket_count];
    for ix in 0..len {
        let flag = flag_at_index(ix);
        if flag == 0 {
            continue;
        }
        let b = (ix * bucket_count) / len;
        if let Some(cell) = buckets.get_mut(b) {
            *cell |= flag;
        }
    }

    let mut out = Vec::new();
    let mut ix = 0usize;
    while ix < bucket_count {
        let flag = buckets[ix];
        if flag == 0 {
            ix += 1;
            continue;
        }

        let start = ix;
        ix += 1;
        while ix < bucket_count && buckets[ix] == flag {
            ix += 1;
        }
        let end = ix; // exclusive

        let kind = match flag {
            1 => zed::ScrollbarMarkerKind::Add,
            2 => zed::ScrollbarMarkerKind::Remove,
            _ => zed::ScrollbarMarkerKind::Modify,
        };

        out.push(zed::ScrollbarMarker {
            start: start as f32 / bucket_count as f32,
            end: end as f32 / bucket_count as f32,
            kind,
        });
    }

    out
}

fn diff_content_text(line: &AnnotatedDiffLine) -> &str {
    match line.kind {
        gitgpui_core::domain::DiffLineKind::Add => {
            line.text.strip_prefix('+').unwrap_or(&line.text)
        }
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

fn compute_diff_file_for_src_ix(diff: &[AnnotatedDiffLine]) -> Vec<Option<String>> {
    let mut out: Vec<Option<String>> = Vec::with_capacity(diff.len());
    let mut current_file: Option<String> = None;

    for line in diff {
        let is_file_header = matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
            && line.text.starts_with("diff --git ");
        if is_file_header {
            current_file = parse_diff_git_header_path(&line.text);
        }
        out.push(current_file.clone());
    }

    out
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

fn build_new_file_preview_from_diff(
    diff: &[AnnotatedDiffLine],
    workdir: &std::path::Path,
    target: Option<&DiffTarget>,
) -> Option<(std::path::PathBuf, Vec<String>)> {
    let mut file_header_count = 0usize;
    let mut is_new_file = false;
    let mut has_remove = false;

    for line in diff {
        if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
            && line.text.starts_with("diff --git ")
        {
            file_header_count += 1;
        }
        if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
            && (line.text.starts_with("new file mode ") || line.text == "--- /dev/null")
        {
            is_new_file = true;
        }
        if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Remove) {
            has_remove = true;
        }
    }

    if file_header_count != 1 || !is_new_file || has_remove {
        return None;
    }

    let rel_path = match target? {
        DiffTarget::WorkingTree { path, .. } => path.clone(),
        DiffTarget::Commit { path: Some(path), .. } => path.clone(),
        _ => return None,
    };

    let abs_path = if rel_path.is_absolute() {
        rel_path
    } else {
        workdir.join(rel_path)
    };

    let lines = diff
        .iter()
        .filter(|l| matches!(l.kind, gitgpui_core::domain::DiffLineKind::Add))
        .map(|l| l.text.strip_prefix('+').unwrap_or(&l.text).to_string())
        .collect::<Vec<_>>();

    Some((abs_path, lines))
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

        let toasts = self
            .toasts
            .iter()
            .rev()
            .take(3)
            .cloned()
            .collect::<Vec<_>>();
        let children = toasts
            .into_iter()
            .map(|t| zed::toast(theme, t.kind, t.message).id(("toast", t.id)));

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
                let mut events = events;
                loop {
                    let (snapshot, next_events) = cx
                        .background_spawn({
                            let store = Arc::clone(&store);
                            async move {
                                let events = events;
                                if events.recv().is_err() {
                                    return (None, events);
                                }
                                while events.try_recv().is_ok() {}
                                (Some(store.snapshot()), events)
                            }
                        })
                        .await;
                    events = next_events;

                    let Some(snapshot) = snapshot else {
                        break;
                    };

                    let _ = view.update(cx, |view, cx| {
                        view.apply_state_snapshot(snapshot, cx);
                        cx.notify();
                    });
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
