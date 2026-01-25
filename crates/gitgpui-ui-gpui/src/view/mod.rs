use crate::{theme::AppTheme, zed_port as zed};
use gitgpui_core::diff::{AnnotatedDiffLine, annotate_unified};
use gitgpui_core::domain::{
    Commit, CommitId, DiffArea, DiffTarget, FileStatus, FileStatusKind, RepoStatus,
    UpstreamDivergence,
};
use gitgpui_core::file_diff::FileDiffRow;
use gitgpui_core::services::{PullMode, RemoteUrlKind, ResetMode};
use gitgpui_state::model::{AppState, CloneOpStatus, DiagnosticKind, Loadable, RepoId, RepoState};
use gitgpui_state::msg::{Msg, StoreEvent};
use gitgpui_state::session;
use gitgpui_state::store::AppStore;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Bounds, ClickEvent, Corner, CursorStyle, Decorations, Element, ElementId,
    Entity, FocusHandle, FontWeight, GlobalElementId, InspectorElementId, IsZero, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Render, ResizeEdge,
    ScrollHandle, ShapedLine, SharedString, Size, Style, TextRun, Tiling, Timer,
    UniformListScrollHandle, WeakEntity, Window, WindowControlArea, anchored, div, fill, point, px,
    relative, size, uniform_list,
};
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::{Arc, mpsc};
use std::time::Duration;

mod chrome;
mod diff_text_model;
mod diff_text_selection;
mod diff_utils;
mod history_graph;
mod panels;
pub(crate) mod rows;

use chrome::{CLIENT_SIDE_DECORATION_INSET, cursor_style_for_resize_edge, resize_edge};

use diff_text_model::{CachedDiffStyledText, CachedDiffTextSegment, SyntaxTokenKind};
use diff_text_selection::{DiffTextSelectionOverlay, DiffTextSelectionTracker};
use diff_utils::{
    compute_diff_file_for_src_ix, compute_diff_file_stats, diff_content_text,
    parse_diff_git_header_path, parse_unified_hunk_header_for_display,
    scrollbar_markers_from_flags,
};

pub(crate) use chrome::window_frame;

const HISTORY_COL_BRANCH_PX: f32 = 130.0;
const HISTORY_COL_GRAPH_PX: f32 = 80.0;
const HISTORY_COL_GRAPH_MAX_PX: f32 = 240.0;
const HISTORY_COL_DATE_PX: f32 = 160.0;
const HISTORY_COL_SHA_PX: f32 = 88.0;
const HISTORY_COL_HANDLE_PX: f32 = 8.0;

const HISTORY_COL_BRANCH_MIN_PX: f32 = 60.0;
const HISTORY_COL_GRAPH_MIN_PX: f32 = 44.0;
const HISTORY_COL_DATE_MIN_PX: f32 = 110.0;
const HISTORY_COL_SHA_MIN_PX: f32 = 60.0;

const HISTORY_GRAPH_COL_GAP_PX: f32 = 16.0;
const HISTORY_GRAPH_MARGIN_X_PX: f32 = 10.0;

const PANE_RESIZE_HANDLE_PX: f32 = 8.0;
const SIDEBAR_MIN_PX: f32 = 200.0;
const DETAILS_MIN_PX: f32 = 280.0;
const MAIN_MIN_PX: f32 = 280.0;

const DIFF_TEXT_LAYOUT_CACHE_MAX_ENTRIES: usize = 4000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HistoryColResizeHandle {
    Branch,
    Graph,
    Message,
    Date,
}

#[derive(Clone, Copy, Debug)]
struct HistoryColResizeState {
    handle: HistoryColResizeHandle,
    start_x: Pixels,
    start_branch: Pixels,
    start_graph: Pixels,
    start_date: Pixels,
    start_sha: Pixels,
}

fn should_hide_unified_diff_header_line(line: &AnnotatedDiffLine) -> bool {
    matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
        && (line.text.starts_with("index ")
            || line.text.starts_with("--- ")
            || line.text.starts_with("+++ "))
}

fn absolute_scroll_y(handle: &ScrollHandle) -> Pixels {
    let raw = handle.offset().y;
    if raw < px(0.0) { -raw } else { raw }
}

fn scroll_is_near_bottom(handle: &ScrollHandle, threshold: Pixels) -> bool {
    let max_offset = handle.max_offset().height.max(px(0.0));
    if max_offset <= px(0.0) {
        return true;
    }

    let scroll_y = absolute_scroll_y(handle).max(px(0.0)).min(max_offset);
    (max_offset - scroll_y) <= threshold
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffViewMode {
    Inline,
    Split,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PaneResizeHandle {
    Sidebar,
    Details,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PaneResizeState {
    handle: PaneResizeHandle,
    start_x: Pixels,
    start_sidebar: Pixels,
    start_details: Pixels,
}

struct PaneResizeDragGhost;

impl Render for PaneResizeDragGhost {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div().w(px(0.0)).h(px(0.0))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum DiffTextRegion {
    Inline,
    SplitLeft,
    SplitRight,
}

impl DiffTextRegion {
    fn order(self) -> u8 {
        match self {
            DiffTextRegion::Inline | DiffTextRegion::SplitLeft => 0,
            DiffTextRegion::SplitRight => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DiffTextPos {
    visible_ix: usize,
    region: DiffTextRegion,
    offset: usize,
}

impl DiffTextPos {
    fn cmp_key(self) -> (usize, u8, usize) {
        (self.visible_ix, self.region.order(), self.offset)
    }
}

struct DiffTextHitbox {
    bounds: Bounds<Pixels>,
    layout_key: u64,
    text_len: usize,
}

#[derive(Clone)]
struct ToastState {
    id: u64,
    kind: zed::ToastKind,
    input: Entity<zed::TextInput>,
}

#[derive(Clone, Debug)]
struct CommitDetailsDelayState {
    repo_id: RepoId,
    commit_id: CommitId,
    show_loading: bool,
}

#[derive(Clone, Debug)]
struct HistoryCache {
    repo_id: RepoId,
    log_fingerprint: u64,
    visible_indices: Vec<usize>,
    graph_rows: Vec<history_graph::GraphRow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HistoryCacheRequest {
    repo_id: RepoId,
    log_fingerprint: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PopoverKind {
    RepoPicker,
    BranchPicker,
    CreateBranch,
    StashPrompt,
    CloneRepo,
    ResetPrompt {
        repo_id: RepoId,
        target: String,
        mode: ResetMode,
    },
    RebasePrompt {
        repo_id: RepoId,
    },
    CreateTagPrompt {
        repo_id: RepoId,
        target: String,
    },
    TagDeletePicker {
        repo_id: RepoId,
    },
    RemoteAddPrompt {
        repo_id: RepoId,
    },
    RemoteUrlPicker {
        repo_id: RepoId,
        kind: RemoteUrlKind,
    },
    RemoteRemovePicker {
        repo_id: RepoId,
    },
    RemoteEditUrlPrompt {
        repo_id: RepoId,
        name: String,
        kind: RemoteUrlKind,
    },
    RemoteRemoveConfirm {
        repo_id: RepoId,
        name: String,
    },
    WorktreeAddPrompt {
        repo_id: RepoId,
    },
    WorktreeOpenPicker {
        repo_id: RepoId,
    },
    WorktreeRemovePicker {
        repo_id: RepoId,
    },
    WorktreeRemoveConfirm {
        repo_id: RepoId,
        path: std::path::PathBuf,
    },
    SubmoduleAddPrompt {
        repo_id: RepoId,
    },
    SubmoduleOpenPicker {
        repo_id: RepoId,
    },
    SubmoduleRemovePicker {
        repo_id: RepoId,
    },
    SubmoduleRemoveConfirm {
        repo_id: RepoId,
        path: std::path::PathBuf,
    },
    FileHistory {
        repo_id: RepoId,
        path: std::path::PathBuf,
    },
    Blame {
        repo_id: RepoId,
        path: std::path::PathBuf,
        rev: Option<String>,
    },
    PushSetUpstreamPrompt {
        repo_id: RepoId,
        remote: String,
    },
    ForcePushConfirm {
        repo_id: RepoId,
    },
    PullPicker,
    PushPicker,
    AppMenu,
    DiffHunks,
    DiffHunkMenu {
        repo_id: RepoId,
        src_ix: usize,
    },
    CommitMenu {
        repo_id: RepoId,
        commit_id: CommitId,
    },
    StatusFileMenu {
        repo_id: RepoId,
        area: DiffArea,
        path: std::path::PathBuf,
    },
    BranchMenu {
        repo_id: RepoId,
        section: BranchSection,
        name: String,
    },
    BranchSectionMenu {
        repo_id: RepoId,
        section: BranchSection,
    },
    CommitFileMenu {
        repo_id: RepoId,
        commit_id: CommitId,
        path: std::path::PathBuf,
    },
    HistoryBranchFilter {
        repo_id: RepoId,
    },
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
enum RemoteRow {
    Header(String),
    Branch { remote: String, name: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BranchSection {
    Local,
    Remote,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum BranchSidebarRow {
    SectionHeader {
        section: BranchSection,
        top_border: bool,
    },
    SectionSpacer,
    Placeholder {
        section: BranchSection,
        message: SharedString,
    },
    RemoteHeader {
        name: SharedString,
    },
    GroupHeader {
        label: SharedString,
        depth: usize,
    },
    Branch {
        label: SharedString,
        name: SharedString,
        section: BranchSection,
        depth: usize,
        muted: bool,
        divergence: Option<UpstreamDivergence>,
        is_head: bool,
        is_upstream: bool,
    },
    StashHeader {
        top_border: bool,
    },
    StashPlaceholder {
        message: SharedString,
    },
    StashItem {
        index: usize,
        message: SharedString,
        created_at: Option<std::time::SystemTime>,
    },
}

#[derive(Default)]
struct SlashTree {
    is_leaf: bool,
    children: BTreeMap<String, SlashTree>,
}

impl SlashTree {
    fn insert(&mut self, name: &str) {
        let mut node = self;
        for part in name.split('/').filter(|p| !p.is_empty()) {
            node = node.children.entry(part.to_string()).or_default();
        }
        node.is_leaf = true;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffClickKind {
    Line,
    HunkHeader,
    FileHeader,
}

#[derive(Clone, Debug)]
enum PatchSplitRow {
    Raw {
        src_ix: usize,
        click_kind: DiffClickKind,
    },
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
    _activation_subscription: gpui::Subscription,
    _appearance_subscription: gpui::Subscription,
    theme: AppTheme,

    diff_view: DiffViewMode,
    diff_cache_repo_id: Option<RepoId>,
    diff_cache_rev: u64,
    diff_cache_target: Option<DiffTarget>,
    diff_cache: Vec<AnnotatedDiffLine>,
    diff_file_for_src_ix: Vec<Option<Arc<str>>>,
    diff_split_cache: Vec<PatchSplitRow>,
    diff_split_cache_len: usize,
    diff_panel_focus_handle: FocusHandle,
    diff_autoscroll_pending: bool,
    diff_raw_input: Entity<zed::TextInput>,
    diff_visible_indices: Vec<usize>,
    diff_visible_cache_len: usize,
    diff_visible_view: DiffViewMode,
    diff_visible_is_file_view: bool,
    diff_scrollbar_markers_cache: Vec<zed::ScrollbarMarker>,
    diff_word_highlights: Vec<Option<Vec<Range<usize>>>>,
    diff_file_stats: Vec<Option<(usize, usize)>>,
    diff_text_segments_cache: Vec<Option<CachedDiffStyledText>>,
    diff_selection_anchor: Option<usize>,
    diff_selection_range: Option<(usize, usize)>,
    diff_text_selecting: bool,
    diff_text_anchor: Option<DiffTextPos>,
    diff_text_head: Option<DiffTextPos>,
    diff_suppress_clicks_remaining: u8,
    diff_text_hitboxes: HashMap<(usize, DiffTextRegion), DiffTextHitbox>,
    diff_text_layout_cache_epoch: u64,
    diff_text_layout_cache: HashMap<u64, DiffTextLayoutCacheEntry>,
    diff_hunk_picker_search_input: Option<Entity<zed::TextInput>>,

    file_diff_cache_repo_id: Option<RepoId>,
    file_diff_cache_rev: u64,
    file_diff_cache_target: Option<DiffTarget>,
    file_diff_cache_path: Option<std::path::PathBuf>,
    file_diff_cache_rows: Vec<FileDiffRow>,
    file_diff_inline_cache: Vec<AnnotatedDiffLine>,
    file_diff_inline_word_highlights: Vec<Option<Vec<Range<usize>>>>,
    file_diff_split_word_highlights_old: Vec<Option<Vec<Range<usize>>>>,
    file_diff_split_word_highlights_new: Vec<Option<Vec<Range<usize>>>>,

    file_image_diff_cache_repo_id: Option<RepoId>,
    file_image_diff_cache_rev: u64,
    file_image_diff_cache_target: Option<DiffTarget>,
    file_image_diff_cache_path: Option<std::path::PathBuf>,
    file_image_diff_cache_old: Option<Arc<gpui::Image>>,
    file_image_diff_cache_new: Option<Arc<gpui::Image>>,

    worktree_preview_path: Option<std::path::PathBuf>,
    worktree_preview: Loadable<Arc<Vec<String>>>,
    worktree_preview_scroll: UniformListScrollHandle,
    worktree_preview_segments_cache_path: Option<std::path::PathBuf>,
    worktree_preview_segments_cache: HashMap<usize, CachedDiffStyledText>,
    diff_preview_is_new_file: bool,
    diff_preview_new_file_lines: Arc<Vec<String>>,

    history_cache_seq: u64,
    history_cache_inflight: Option<HistoryCacheRequest>,
    last_window_size: Size<Pixels>,
    ui_window_size_last_seen: Size<Pixels>,
    ui_settings_persist_seq: u64,
    history_col_branch: Pixels,
    history_col_graph: Pixels,
    history_col_date: Pixels,
    history_col_sha: Pixels,
    history_col_graph_auto: bool,
    history_col_resize: Option<HistoryColResizeState>,
    repo_picker_search_input: Option<Entity<zed::TextInput>>,
    branch_picker_search_input: Option<Entity<zed::TextInput>>,
    tag_picker_search_input: Option<Entity<zed::TextInput>>,
    remote_picker_search_input: Option<Entity<zed::TextInput>>,
    file_history_search_input: Option<Entity<zed::TextInput>>,
    worktree_picker_search_input: Option<Entity<zed::TextInput>>,
    submodule_picker_search_input: Option<Entity<zed::TextInput>>,
    history_cache: Option<HistoryCache>,

    open_repo_panel: bool,
    open_repo_input: Entity<zed::TextInput>,
    clone_repo_url_input: Entity<zed::TextInput>,
    clone_repo_parent_dir_input: Entity<zed::TextInput>,
    commit_message_input: Entity<zed::TextInput>,
    rebase_onto_input: Entity<zed::TextInput>,
    create_tag_input: Entity<zed::TextInput>,
    remote_name_input: Entity<zed::TextInput>,
    remote_url_input: Entity<zed::TextInput>,
    remote_url_edit_input: Entity<zed::TextInput>,
    create_branch_input: Entity<zed::TextInput>,
    stash_message_input: Entity<zed::TextInput>,
    push_upstream_branch_input: Entity<zed::TextInput>,
    worktree_path_input: Entity<zed::TextInput>,
    worktree_ref_input: Entity<zed::TextInput>,
    submodule_url_input: Entity<zed::TextInput>,
    submodule_path_input: Entity<zed::TextInput>,

    popover: Option<PopoverKind>,
    popover_anchor: Option<Point<Pixels>>,
    context_menu_focus_handle: FocusHandle,
    context_menu_selected_ix: Option<usize>,

    title_should_move: bool,
    hover_resize_edge: Option<ResizeEdge>,

    branches_scroll: UniformListScrollHandle,
    history_scroll: UniformListScrollHandle,
    unstaged_scroll: UniformListScrollHandle,
    staged_scroll: UniformListScrollHandle,
    diff_scroll: UniformListScrollHandle,
    commit_files_scroll: UniformListScrollHandle,
    blame_scroll: UniformListScrollHandle,
    commit_scroll: ScrollHandle,

    sidebar_width: Pixels,
    details_width: Pixels,
    pane_resize: Option<PaneResizeState>,

    last_mouse_pos: Point<Pixels>,
    tooltip_text: Option<SharedString>,
    tooltip_visible_text: Option<SharedString>,
    tooltip_candidate_last: Option<SharedString>,
    tooltip_pending_pos: Option<Point<Pixels>>,
    tooltip_visible_pos: Option<Point<Pixels>>,
    tooltip_delay_seq: u64,

    toasts: Vec<ToastState>,
    clone_progress_toast_id: Option<u64>,
    clone_progress_last_seq: u64,
    clone_progress_dest: Option<std::path::PathBuf>,

    hovered_repo_tab: Option<RepoId>,

    commit_details_message_input: Entity<zed::TextInput>,
    error_banner_input: Entity<zed::TextInput>,

    commit_details_delay: Option<CommitDetailsDelayState>,
    commit_details_delay_seq: u64,

    path_display_cache: std::cell::RefCell<HashMap<std::path::PathBuf, SharedString>>,
}

struct DiffTextLayoutCacheEntry {
    layout: ShapedLine,
    last_used_epoch: u64,
}

impl GitGpuiView {
    fn cached_path_display(&self, path: &std::path::PathBuf) -> SharedString {
        const MAX_ENTRIES: usize = 8_192;
        let mut cache = self.path_display_cache.borrow_mut();
        if cache.len() > MAX_ENTRIES {
            cache.clear();
        }
        if let Some(s) = cache.get(path) {
            return s.clone();
        }
        let s: SharedString = path.display().to_string().into();
        cache.insert(path.clone(), s.clone());
        s
    }

    fn set_tooltip_text_if_changed(&mut self, next: Option<SharedString>) -> bool {
        if self.tooltip_text == next {
            return false;
        }
        self.tooltip_text = next;
        true
    }

    fn is_file_preview_active(&self) -> bool {
        self.untracked_worktree_preview_path().is_some()
            || self.added_file_preview_abs_path().is_some()
    }

    fn worktree_preview_line_count(&self) -> Option<usize> {
        match &self.worktree_preview {
            Loadable::Ready(lines) => Some(lines.len()),
            _ => None,
        }
    }

    fn touch_diff_text_layout_cache(&mut self, key: u64, layout: Option<ShapedLine>) {
        let epoch = self.diff_text_layout_cache_epoch;
        match layout {
            Some(layout) => {
                self.diff_text_layout_cache.insert(
                    key,
                    DiffTextLayoutCacheEntry {
                        layout,
                        last_used_epoch: epoch,
                    },
                );
            }
            None => {
                if let Some(entry) = self.diff_text_layout_cache.get_mut(&key) {
                    entry.last_used_epoch = epoch;
                }
            }
        }

        self.prune_diff_text_layout_cache();
    }

    fn prune_diff_text_layout_cache(&mut self) {
        if self.diff_text_layout_cache.len() <= DIFF_TEXT_LAYOUT_CACHE_MAX_ENTRIES {
            return;
        }

        let over_by = self
            .diff_text_layout_cache
            .len()
            .saturating_sub(DIFF_TEXT_LAYOUT_CACHE_MAX_ENTRIES);
        if over_by == 0 {
            return;
        }

        let mut by_age: Vec<(u64, u64)> = self
            .diff_text_layout_cache
            .iter()
            .map(|(k, v)| (*k, v.last_used_epoch))
            .collect();
        by_age.sort_by_key(|(_, last_used)| *last_used);

        for (key, _) in by_age.into_iter().take(over_by) {
            self.diff_text_layout_cache.remove(&key);
        }
    }

    fn diff_text_segments_cache_get(&self, key: usize) -> Option<&CachedDiffStyledText> {
        self.diff_text_segments_cache
            .get(key)
            .and_then(Option::as_ref)
    }

    fn diff_text_segments_cache_set(
        &mut self,
        key: usize,
        value: CachedDiffStyledText,
    ) -> &CachedDiffStyledText {
        if self.diff_text_segments_cache.len() <= key {
            self.diff_text_segments_cache.resize_with(key + 1, || None);
        }
        self.diff_text_segments_cache[key] = Some(value);
        self.diff_text_segments_cache[key]
            .as_ref()
            .expect("just set")
    }

    fn log_fingerprint(commits: &[Commit]) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        commits.len().hash(&mut hasher);
        // This runs in the render path; keep it O(1) even for huge histories.
        for id in commits.iter().take(3).map(|c| c.id.as_ref()) {
            id.hash(&mut hasher);
        }
        for id in commits.iter().rev().take(3).map(|c| c.id.as_ref()) {
            id.hash(&mut hasher);
        }
        hasher.finish()
    }

    fn is_file_diff_view_active(&self) -> bool {
        let Some(repo) = self.active_repo() else {
            return false;
        };
        self.file_diff_cache_repo_id == Some(repo.id)
            && self.file_diff_cache_rev == repo.diff_file_rev
            && self.file_diff_cache_target == repo.diff_target
            && self.file_diff_cache_path.is_some()
    }

    fn is_file_image_diff_view_active(&self) -> bool {
        let Some(repo) = self.active_repo() else {
            return false;
        };
        self.file_image_diff_cache_repo_id == Some(repo.id)
            && self.file_image_diff_cache_rev == repo.diff_file_rev
            && self.file_image_diff_cache_target == repo.diff_target
            && self.file_image_diff_cache_path.is_some()
            && (self.file_image_diff_cache_old.is_some() || self.file_image_diff_cache_new.is_some())
    }

    fn handle_file_diff_row_click(&mut self, clicked_visible_ix: usize, shift: bool) {
        let list_len = self.diff_visible_indices.len();
        if list_len == 0 {
            self.diff_selection_anchor = None;
            self.diff_selection_range = None;
            return;
        }

        let clicked_visible_ix = clicked_visible_ix.min(list_len - 1);
        if shift && let Some(anchor) = self.diff_selection_anchor {
            let a = anchor.min(clicked_visible_ix);
            let b = anchor.max(clicked_visible_ix);
            self.diff_selection_range = Some((a, b));
            return;
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

        let restored_sidebar_width = ui_session.sidebar_width;
        let restored_details_width = ui_session.details_width;

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

        let activation_subscription = cx.observe_window_activation(window, |this, window, _cx| {
            if !window.is_window_active() {
                return;
            }
            if let Some(repo_id) = this.active_repo_id() {
                this.store.dispatch(Msg::SetActiveRepo { repo_id });
            }
        });

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
                    read_only: false,
                    chromeless: false,
                    soft_wrap: false,
                },
                window,
                cx,
            )
        });

        let clone_repo_url_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "https://example.com/org/repo.git".into(),
                    multiline: false,
                    read_only: false,
                    chromeless: false,
                    soft_wrap: false,
                },
                window,
                cx,
            )
        });

        let clone_repo_parent_dir_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "/path/to/parent/folder".into(),
                    multiline: false,
                    read_only: false,
                    chromeless: false,
                    soft_wrap: false,
                },
                window,
                cx,
            )
        });

        let commit_message_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "Enter commit message".into(),
                    multiline: false,
                    read_only: false,
                    chromeless: false,
                    soft_wrap: false,
                },
                window,
                cx,
            )
        });

        let rebase_onto_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "onto branch / tag / SHA (e.g. origin/main)".into(),
                    multiline: false,
                    read_only: false,
                    chromeless: false,
                    soft_wrap: false,
                },
                window,
                cx,
            )
        });

        let create_tag_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "tag-name".into(),
                    multiline: false,
                    read_only: false,
                    chromeless: false,
                    soft_wrap: false,
                },
                window,
                cx,
            )
        });

        let remote_name_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "remote name (e.g. origin)".into(),
                    multiline: false,
                    read_only: false,
                    chromeless: false,
                    soft_wrap: false,
                },
                window,
                cx,
            )
        });

        let remote_url_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "remote URL".into(),
                    multiline: false,
                    read_only: false,
                    chromeless: false,
                    soft_wrap: false,
                },
                window,
                cx,
            )
        });

        let remote_url_edit_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "new remote URL".into(),
                    multiline: false,
                    read_only: false,
                    chromeless: false,
                    soft_wrap: false,
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
                    read_only: false,
                    chromeless: false,
                    soft_wrap: false,
                },
                window,
                cx,
            )
        });

        let stash_message_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "Stash message".into(),
                    multiline: false,
                    read_only: false,
                    chromeless: false,
                    soft_wrap: false,
                },
                window,
                cx,
            )
        });

        let push_upstream_branch_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "Remote branch name".into(),
                    multiline: false,
                    read_only: false,
                    chromeless: false,
                    soft_wrap: false,
                },
                window,
                cx,
            )
        });

        let worktree_path_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "Worktree folder".into(),
                    multiline: false,
                    read_only: false,
                    chromeless: false,
                    soft_wrap: false,
                },
                window,
                cx,
            )
        });

        let worktree_ref_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "Branch / commit (optional)".into(),
                    multiline: false,
                    read_only: false,
                    chromeless: false,
                    soft_wrap: false,
                },
                window,
                cx,
            )
        });

        let submodule_url_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "Submodule URL".into(),
                    multiline: false,
                    read_only: false,
                    chromeless: false,
                    soft_wrap: false,
                },
                window,
                cx,
            )
        });

        let submodule_path_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "Submodule path (relative)".into(),
                    multiline: false,
                    read_only: false,
                    chromeless: false,
                    soft_wrap: false,
                },
                window,
                cx,
            )
        });

        let diff_raw_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "".into(),
                    multiline: true,
                    read_only: true,
                    chromeless: false,
                    soft_wrap: false,
                },
                window,
                cx,
            )
        });

        let error_banner_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "".into(),
                    multiline: true,
                    read_only: true,
                    chromeless: true,
                    soft_wrap: false,
                },
                window,
                cx,
            )
        });

        let commit_details_message_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "".into(),
                    multiline: true,
                    read_only: true,
                    chromeless: true,
                    soft_wrap: true,
                },
                window,
                cx,
            )
        });

        let diff_panel_focus_handle = cx.focus_handle().tab_index(0).tab_stop(false);
        let context_menu_focus_handle = cx.focus_handle().tab_index(0).tab_stop(false);

        let mut view = Self {
            state: store.snapshot(),
            store,
            _poller: poller,
            _activation_subscription: activation_subscription,
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
            diff_panel_focus_handle,
            diff_autoscroll_pending: false,
            diff_raw_input,
            diff_visible_indices: Vec::new(),
            diff_visible_cache_len: 0,
            diff_visible_view: DiffViewMode::Split,
            diff_visible_is_file_view: false,
            diff_scrollbar_markers_cache: Vec::new(),
            diff_word_highlights: Vec::new(),
            diff_file_stats: Vec::new(),
            diff_text_segments_cache: Vec::new(),
            diff_selection_anchor: None,
            diff_selection_range: None,
            diff_text_selecting: false,
            diff_text_anchor: None,
            diff_text_head: None,
            diff_suppress_clicks_remaining: 0,
            diff_text_hitboxes: HashMap::new(),
            diff_text_layout_cache_epoch: 0,
            diff_text_layout_cache: HashMap::new(),
            diff_hunk_picker_search_input: None,
            file_diff_cache_repo_id: None,
            file_diff_cache_rev: 0,
            file_diff_cache_target: None,
            file_diff_cache_path: None,
            file_diff_cache_rows: Vec::new(),
            file_diff_inline_cache: Vec::new(),
            file_diff_inline_word_highlights: Vec::new(),
            file_diff_split_word_highlights_old: Vec::new(),
            file_diff_split_word_highlights_new: Vec::new(),
            file_image_diff_cache_repo_id: None,
            file_image_diff_cache_rev: 0,
            file_image_diff_cache_target: None,
            file_image_diff_cache_path: None,
            file_image_diff_cache_old: None,
            file_image_diff_cache_new: None,
            worktree_preview_path: None,
            worktree_preview: Loadable::NotLoaded,
            worktree_preview_scroll: UniformListScrollHandle::default(),
            worktree_preview_segments_cache_path: None,
            worktree_preview_segments_cache: HashMap::new(),
            diff_preview_is_new_file: false,
            diff_preview_new_file_lines: Arc::new(Vec::new()),
            history_cache_seq: 0,
            history_cache_inflight: None,
            last_window_size: size(px(0.0), px(0.0)),
            ui_window_size_last_seen: size(px(0.0), px(0.0)),
            ui_settings_persist_seq: 0,
            history_col_branch: px(HISTORY_COL_BRANCH_PX),
            history_col_graph: px(HISTORY_COL_GRAPH_PX),
            history_col_date: px(HISTORY_COL_DATE_PX),
            history_col_sha: px(HISTORY_COL_SHA_PX),
            history_col_graph_auto: true,
            history_col_resize: None,
            repo_picker_search_input: None,
            branch_picker_search_input: None,
            tag_picker_search_input: None,
            remote_picker_search_input: None,
            file_history_search_input: None,
            worktree_picker_search_input: None,
            submodule_picker_search_input: None,
            history_cache: None,
            open_repo_panel: false,
            open_repo_input,
            clone_repo_url_input,
            clone_repo_parent_dir_input,
            commit_message_input,
            rebase_onto_input,
            create_tag_input,
            remote_name_input,
            remote_url_input,
            remote_url_edit_input,
            create_branch_input,
            stash_message_input,
            push_upstream_branch_input,
            worktree_path_input,
            worktree_ref_input,
            submodule_url_input,
            submodule_path_input,
            popover: None,
            popover_anchor: None,
            context_menu_focus_handle,
            context_menu_selected_ix: None,
            title_should_move: false,
            hover_resize_edge: None,
            branches_scroll: UniformListScrollHandle::default(),
            history_scroll: UniformListScrollHandle::default(),
            unstaged_scroll: UniformListScrollHandle::default(),
            staged_scroll: UniformListScrollHandle::default(),
            diff_scroll: UniformListScrollHandle::default(),
            commit_files_scroll: UniformListScrollHandle::default(),
            blame_scroll: UniformListScrollHandle::default(),
            commit_scroll: ScrollHandle::new(),
            sidebar_width: restored_sidebar_width
                .map(|w| px(w as f32))
                .unwrap_or(px(280.0))
                .max(px(SIDEBAR_MIN_PX)),
            details_width: restored_details_width
                .map(|w| px(w as f32))
                .unwrap_or(px(420.0))
                .max(px(DETAILS_MIN_PX)),
            pane_resize: None,
            last_mouse_pos: point(px(0.0), px(0.0)),
            tooltip_text: None,
            tooltip_visible_text: None,
            tooltip_candidate_last: None,
            tooltip_pending_pos: None,
            tooltip_visible_pos: None,
            tooltip_delay_seq: 0,
            toasts: Vec::new(),
            clone_progress_toast_id: None,
            clone_progress_last_seq: 0,
            clone_progress_dest: None,
            hovered_repo_tab: None,
            commit_details_message_input,
            error_banner_input,
            commit_details_delay: None,
            commit_details_delay_seq: 0,
            path_display_cache: std::cell::RefCell::new(HashMap::new()),
        };

        view.set_theme(initial_theme, cx);
        view.rebuild_diff_cache();

        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        view.maybe_auto_install_linux_desktop_integration(cx);

        view
    }

    fn set_theme(&mut self, theme: AppTheme, cx: &mut gpui::Context<Self>) {
        self.theme = theme;
        self.diff_text_segments_cache.clear();
        self.worktree_preview_segments_cache_path = None;
        self.worktree_preview_segments_cache.clear();
        self.open_repo_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.clone_repo_url_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.clone_repo_parent_dir_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.commit_message_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.rebase_onto_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.create_tag_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.remote_name_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.remote_url_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.remote_url_edit_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.create_branch_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.stash_message_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.push_upstream_branch_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.worktree_path_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.worktree_ref_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.submodule_url_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.submodule_path_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.diff_raw_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.commit_details_message_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.error_banner_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        if let Some(input) = &self.repo_picker_search_input {
            input.update(cx, |input, cx| input.set_theme(theme, cx));
        }
        if let Some(input) = &self.branch_picker_search_input {
            input.update(cx, |input, cx| input.set_theme(theme, cx));
        }
        if let Some(input) = &self.tag_picker_search_input {
            input.update(cx, |input, cx| input.set_theme(theme, cx));
        }
        if let Some(input) = &self.remote_picker_search_input {
            input.update(cx, |input, cx| input.set_theme(theme, cx));
        }
        if let Some(input) = &self.file_history_search_input {
            input.update(cx, |input, cx| input.set_theme(theme, cx));
        }
        if let Some(input) = &self.worktree_picker_search_input {
            input.update(cx, |input, cx| input.set_theme(theme, cx));
        }
        if let Some(input) = &self.submodule_picker_search_input {
            input.update(cx, |input, cx| input.set_theme(theme, cx));
        }
        if let Some(input) = &self.diff_hunk_picker_search_input {
            input.update(cx, |input, cx| input.set_theme(theme, cx));
        }
    }

    fn pane_resize_handle(
        &self,
        theme: AppTheme,
        id: &'static str,
        handle: PaneResizeHandle,
        cx: &gpui::Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .w(px(PANE_RESIZE_HANDLE_PX))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .cursor(CursorStyle::ResizeLeftRight)
            .hover(move |s| s.bg(with_alpha(theme.colors.hover, 0.65)))
            .active(move |s| s.bg(theme.colors.active))
            .child(div().w(px(1.0)).h_full().bg(theme.colors.border))
            .on_drag(handle, |_handle, _offset, _window, cx| {
                cx.new(|_cx| PaneResizeDragGhost)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, _w, cx| {
                    cx.stop_propagation();
                    this.pane_resize = Some(PaneResizeState {
                        handle,
                        start_x: e.position.x,
                        start_sidebar: this.sidebar_width,
                        start_details: this.details_width,
                    });
                    cx.notify();
                }),
            )
            .on_drag_move(cx.listener(
                move |this, e: &gpui::DragMoveEvent<PaneResizeHandle>, _w, cx| {
                    let Some(state) = this.pane_resize else {
                        return;
                    };
                    if state.handle != *e.drag(cx) {
                        return;
                    }

                    let total_w = this.last_window_size.width;
                    let handles_w = px(PANE_RESIZE_HANDLE_PX) * 2.0;
                    let main_min = px(MAIN_MIN_PX);
                    let sidebar_min = px(SIDEBAR_MIN_PX);
                    let details_min = px(DETAILS_MIN_PX);

                    let dx = e.event.position.x - state.start_x;
                    match state.handle {
                        PaneResizeHandle::Sidebar => {
                            let max_sidebar =
                                (total_w - state.start_details - main_min - handles_w)
                                    .max(sidebar_min);
                            this.sidebar_width =
                                (state.start_sidebar + dx).max(sidebar_min).min(max_sidebar);
                        }
                        PaneResizeHandle::Details => {
                            let max_details =
                                (total_w - state.start_sidebar - main_min - handles_w)
                                    .max(details_min);
                            this.details_width =
                                (state.start_details - dx).max(details_min).min(max_details);
                        }
                    }
                    cx.notify();
                },
            ))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _e, _w, cx| {
                    this.pane_resize = None;
                    this.schedule_ui_settings_persist(cx);
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _e, _w, cx| {
                    this.pane_resize = None;
                    this.schedule_ui_settings_persist(cx);
                    cx.notify();
                }),
            )
    }

    fn active_repo_id(&self) -> Option<RepoId> {
        self.state.active_repo
    }

    fn active_repo(&self) -> Option<&RepoState> {
        let repo_id = self.active_repo_id()?;
        self.state.repos.iter().find(|r| r.id == repo_id)
    }

    fn consume_suppress_click_after_drag(&mut self) -> bool {
        if self.diff_suppress_clicks_remaining > 0 {
            self.diff_suppress_clicks_remaining =
                self.diff_suppress_clicks_remaining.saturating_sub(1);
            return true;
        }
        false
    }

    fn diff_text_normalized_selection(&self) -> Option<(DiffTextPos, DiffTextPos)> {
        let a = self.diff_text_anchor?;
        let b = self.diff_text_head?;
        Some(if a.cmp_key() <= b.cmp_key() {
            (a, b)
        } else {
            (b, a)
        })
    }

    fn diff_text_selection_color(&self) -> gpui::Rgba {
        with_alpha(
            self.theme.colors.accent,
            if self.theme.is_dark { 0.28 } else { 0.18 },
        )
    }

    fn set_diff_text_hitbox(
        &mut self,
        visible_ix: usize,
        region: DiffTextRegion,
        hitbox: DiffTextHitbox,
    ) {
        self.diff_text_hitboxes.insert((visible_ix, region), hitbox);
    }

    fn diff_text_pos_from_hitbox(
        &self,
        visible_ix: usize,
        region: DiffTextRegion,
        position: Point<Pixels>,
    ) -> Option<DiffTextPos> {
        let hitbox = self.diff_text_hitboxes.get(&(visible_ix, region))?;
        let layout = &self.diff_text_layout_cache.get(&hitbox.layout_key)?.layout;
        let local = hitbox.bounds.localize(&position)?;
        let x = local.x.max(px(0.0));
        let offset = layout
            .closest_index_for_x(x)
            .min(layout.len())
            .min(hitbox.text_len);
        Some(DiffTextPos {
            visible_ix,
            region,
            offset,
        })
    }

    fn diff_text_pos_for_mouse(&self, position: Point<Pixels>) -> Option<DiffTextPos> {
        if self.diff_text_hitboxes.is_empty() {
            return None;
        }

        let restrict_region = self
            .diff_text_selecting
            .then(|| self.diff_text_anchor)
            .flatten()
            .map(|p| p.region)
            .filter(|r| matches!(r, DiffTextRegion::SplitLeft | DiffTextRegion::SplitRight));

        for ((visible_ix, region), hitbox) in &self.diff_text_hitboxes {
            if restrict_region.is_some_and(|restrict| restrict != *region) {
                continue;
            }
            if hitbox.bounds.contains(&position) {
                return self.diff_text_pos_from_hitbox(*visible_ix, *region, position);
            }
        }

        let mut best: Option<((usize, DiffTextRegion), Pixels)> = None;
        for (key, hitbox) in &self.diff_text_hitboxes {
            if restrict_region.is_some_and(|restrict| restrict != key.1) {
                continue;
            }
            let dy = if position.y < hitbox.bounds.top() {
                hitbox.bounds.top() - position.y
            } else if position.y > hitbox.bounds.bottom() {
                position.y - hitbox.bounds.bottom()
            } else {
                px(0.0)
            };
            let dx = if position.x < hitbox.bounds.left() {
                hitbox.bounds.left() - position.x
            } else if position.x > hitbox.bounds.right() {
                position.x - hitbox.bounds.right()
            } else {
                px(0.0)
            };
            let score = dy + dx;
            if best.is_none() || score < best.unwrap().1 {
                best = Some((*key, score));
            }
        }
        let ((visible_ix, region), _) = best?;
        self.diff_text_pos_from_hitbox(visible_ix, region, position)
    }

    fn begin_diff_text_selection(
        &mut self,
        visible_ix: usize,
        region: DiffTextRegion,
        position: Point<Pixels>,
    ) {
        let Some(pos) = self.diff_text_pos_from_hitbox(visible_ix, region, position) else {
            return;
        };
        self.diff_text_selecting = true;
        self.diff_text_anchor = Some(pos);
        self.diff_text_head = Some(pos);
        self.diff_suppress_clicks_remaining = 0;
    }

    fn update_diff_text_selection_from_mouse(&mut self, position: Point<Pixels>) {
        if !self.diff_text_selecting {
            return;
        }
        let Some(pos) = self.diff_text_pos_for_mouse(position) else {
            return;
        };
        if self.diff_text_head != Some(pos) {
            self.diff_text_head = Some(pos);
            if self
                .diff_text_normalized_selection()
                .is_some_and(|(a, b)| a != b)
            {
                self.diff_suppress_clicks_remaining = 1;
            }
        }
    }

    fn end_diff_text_selection(&mut self) {
        self.diff_text_selecting = false;
    }

    fn diff_text_has_selection(&self) -> bool {
        self.diff_text_normalized_selection()
            .is_some_and(|(a, b)| a != b)
    }

    fn diff_text_local_selection_range(
        &self,
        visible_ix: usize,
        region: DiffTextRegion,
        text_len: usize,
    ) -> Option<Range<usize>> {
        let (start, end) = self.diff_text_normalized_selection()?;
        if start == end {
            return None;
        }
        if visible_ix < start.visible_ix || visible_ix > end.visible_ix {
            return None;
        }

        let split_region = (self.diff_view == DiffViewMode::Split
            && start.region == end.region
            && matches!(
                start.region,
                DiffTextRegion::SplitLeft | DiffTextRegion::SplitRight
            ))
        .then_some(start.region);
        if split_region.is_some_and(|r| r != region) {
            return None;
        }

        let region_order = region.order();
        let start_order = start.region.order();
        let end_order = end.region.order();

        let mut a = 0usize;
        let mut b = text_len;

        if start.visible_ix == end.visible_ix && visible_ix == start.visible_ix {
            if region_order < start_order || region_order > end_order {
                return None;
            }
            if region == start.region {
                a = start.offset.min(text_len);
            }
            if region == end.region {
                b = end.offset.min(text_len);
            }
        } else if visible_ix == start.visible_ix {
            if region_order < start_order {
                return None;
            }
            if region == start.region {
                a = start.offset.min(text_len);
            }
        } else if visible_ix == end.visible_ix {
            if region_order > end_order {
                return None;
            }
            if region == end.region {
                b = end.offset.min(text_len);
            }
        }

        if a >= b {
            return None;
        }
        Some(a..b)
    }

    fn diff_text_line_for_region(&self, visible_ix: usize, region: DiffTextRegion) -> SharedString {
        let fallback = SharedString::default();
        let expand_tabs = |s: &str| -> SharedString {
            if !s.contains('\t') {
                return s.to_string().into();
            }
            let mut out = String::with_capacity(s.len());
            for ch in s.chars() {
                match ch {
                    '\t' => out.push_str("    "),
                    _ => out.push(ch),
                }
            }
            out.into()
        };

        if self.is_file_preview_active() {
            if region != DiffTextRegion::Inline {
                return fallback;
            }
            let Loadable::Ready(lines) = &self.worktree_preview else {
                return fallback;
            };
            return lines
                .get(visible_ix)
                .map(|l| expand_tabs(l))
                .unwrap_or(fallback);
        }

        let Some(&mapped_ix) = self.diff_visible_indices.get(visible_ix) else {
            return fallback;
        };

        if self.diff_view == DiffViewMode::Inline {
            if region != DiffTextRegion::Inline {
                return fallback;
            }
            if self.is_file_diff_view_active() {
                if let Some(styled) = self.diff_text_segments_cache_get(mapped_ix) {
                    return styled.text.clone();
                }
                return self
                    .file_diff_inline_cache
                    .get(mapped_ix)
                    .map(|l| expand_tabs(diff_content_text(l)))
                    .unwrap_or(fallback);
            }

            if let Some(styled) = self.diff_text_segments_cache_get(mapped_ix) {
                return styled.text.clone();
            }
            let Some(line) = self.diff_cache.get(mapped_ix) else {
                return fallback;
            };
            let display = if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk) {
                parse_unified_hunk_header_for_display(line.text.as_ref())
                    .map(|p| {
                        let heading = p.heading.unwrap_or_default();
                        if heading.is_empty() {
                            format!("{} {}", p.old, p.new)
                        } else {
                            format!("{} {}  {heading}", p.old, p.new)
                        }
                    })
                    .unwrap_or_else(|| line.text.clone())
            } else if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
                && line.text.starts_with("diff --git ")
            {
                parse_diff_git_header_path(line.text.as_ref()).unwrap_or_else(|| line.text.clone())
            } else {
                line.text.clone()
            };
            return expand_tabs(display.as_str());
        }

        match region {
            DiffTextRegion::SplitLeft | DiffTextRegion::SplitRight => {}
            DiffTextRegion::Inline => return fallback,
        }

        if self.is_file_diff_view_active() {
            let key = match region {
                DiffTextRegion::SplitLeft => mapped_ix * 2,
                DiffTextRegion::SplitRight => mapped_ix * 2 + 1,
                DiffTextRegion::Inline => unreachable!(),
            };
            if let Some(styled) = self.diff_text_segments_cache_get(key) {
                return styled.text.clone();
            }
            let Some(row) = self.file_diff_cache_rows.get(mapped_ix) else {
                return fallback;
            };
            let text = match region {
                DiffTextRegion::SplitLeft => row.old.as_deref().unwrap_or(""),
                DiffTextRegion::SplitRight => row.new.as_deref().unwrap_or(""),
                DiffTextRegion::Inline => unreachable!(),
            };
            return expand_tabs(text);
        }

        let Some(split_row) = self.diff_split_cache.get(mapped_ix) else {
            return fallback;
        };
        match split_row {
            PatchSplitRow::Raw { src_ix, click_kind } => {
                let Some(line) = self.diff_cache.get(*src_ix) else {
                    return fallback;
                };
                let display = match click_kind {
                    DiffClickKind::HunkHeader => {
                        parse_unified_hunk_header_for_display(line.text.as_ref())
                            .map(|p| {
                                let heading = p.heading.unwrap_or_default();
                                if heading.is_empty() {
                                    format!("{} {}", p.old, p.new)
                                } else {
                                    format!("{} {}  {heading}", p.old, p.new)
                                }
                            })
                            .unwrap_or_else(|| line.text.clone())
                    }
                    DiffClickKind::FileHeader => parse_diff_git_header_path(line.text.as_ref())
                        .unwrap_or_else(|| line.text.clone()),
                    DiffClickKind::Line => line.text.clone(),
                };
                expand_tabs(display.as_str())
            }
            PatchSplitRow::Aligned { row, .. } => {
                let text = match region {
                    DiffTextRegion::SplitLeft => row.old.as_deref().unwrap_or(""),
                    DiffTextRegion::SplitRight => row.new.as_deref().unwrap_or(""),
                    DiffTextRegion::Inline => unreachable!(),
                };
                expand_tabs(text)
            }
        }
    }

    fn diff_text_combined_offset(&self, pos: DiffTextPos, left_len: usize) -> usize {
        match self.diff_view {
            DiffViewMode::Inline => pos.offset,
            DiffViewMode::Split => match pos.region {
                DiffTextRegion::SplitLeft => pos.offset,
                DiffTextRegion::SplitRight => left_len.saturating_add(1).saturating_add(pos.offset),
                DiffTextRegion::Inline => pos.offset,
            },
        }
    }

    fn selected_diff_text_string(&self) -> Option<String> {
        let (start, end) = self.diff_text_normalized_selection()?;
        if start == end {
            return None;
        }

        let force_inline = self.is_file_preview_active();

        let mut out = String::new();
        for visible_ix in start.visible_ix..=end.visible_ix {
            if force_inline || self.diff_view == DiffViewMode::Inline {
                let text = self.diff_text_line_for_region(visible_ix, DiffTextRegion::Inline);
                let line_len = text.len();
                let a = if visible_ix == start.visible_ix {
                    start.offset.min(line_len)
                } else {
                    0
                };
                let b = if visible_ix == end.visible_ix {
                    end.offset.min(line_len)
                } else {
                    line_len
                };
                if !out.is_empty() {
                    out.push('\n');
                }
                if a < b {
                    out.push_str(&text[a..b]);
                }
                continue;
            }

            let split_region = (start.region == end.region
                && matches!(
                    start.region,
                    DiffTextRegion::SplitLeft | DiffTextRegion::SplitRight
                ))
            .then_some(start.region);

            if let Some(region) = split_region {
                let text = self.diff_text_line_for_region(visible_ix, region);
                let line_len = text.len();
                let a = if visible_ix == start.visible_ix {
                    start.offset.min(line_len)
                } else {
                    0
                };
                let b = if visible_ix == end.visible_ix {
                    end.offset.min(line_len)
                } else {
                    line_len
                };
                if !out.is_empty() {
                    out.push('\n');
                }
                if a < b {
                    out.push_str(&text[a..b]);
                }
            } else {
                let left = self.diff_text_line_for_region(visible_ix, DiffTextRegion::SplitLeft);
                let right = self.diff_text_line_for_region(visible_ix, DiffTextRegion::SplitRight);
                let combined = format!("{}\t{}", left.as_ref(), right.as_ref());
                let left_len = left.len();
                let combined_len = combined.len();

                let a = if visible_ix == start.visible_ix {
                    self.diff_text_combined_offset(start, left_len)
                        .min(combined_len)
                } else {
                    0
                };
                let b = if visible_ix == end.visible_ix {
                    self.diff_text_combined_offset(end, left_len)
                        .min(combined_len)
                } else {
                    combined_len
                };

                if !out.is_empty() {
                    out.push('\n');
                }
                if a < b {
                    out.push_str(&combined[a..b]);
                }
            }
        }

        if out.is_empty() { None } else { Some(out) }
    }

    fn copy_selected_diff_text_to_clipboard(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(text) = self.selected_diff_text_string() else {
            return;
        };
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
    }

    fn copy_diff_text_selection_or_region_line_to_clipboard(
        &mut self,
        visible_ix: usize,
        region: DiffTextRegion,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.diff_text_has_selection() {
            self.copy_selected_diff_text_to_clipboard(cx);
            return;
        }
        let text = self.diff_text_line_for_region(visible_ix, region);
        if text.is_empty() {
            return;
        }
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.to_string()));
    }

    fn select_all_diff_text(&mut self) {
        if self.is_file_preview_active() {
            let Some(count) = self.worktree_preview_line_count() else {
                return;
            };
            if count == 0 {
                return;
            }
            let end_visible_ix = count - 1;
            let end_text = self.diff_text_line_for_region(end_visible_ix, DiffTextRegion::Inline);

            self.diff_text_selecting = false;
            self.diff_text_anchor = Some(DiffTextPos {
                visible_ix: 0,
                region: DiffTextRegion::Inline,
                offset: 0,
            });
            self.diff_text_head = Some(DiffTextPos {
                visible_ix: end_visible_ix,
                region: DiffTextRegion::Inline,
                offset: end_text.len(),
            });
            return;
        }

        if self.diff_visible_indices.is_empty() {
            return;
        }

        let start_region = match self.diff_view {
            DiffViewMode::Inline => DiffTextRegion::Inline,
            DiffViewMode::Split => self
                .diff_text_head
                .or(self.diff_text_anchor)
                .map(|p| p.region)
                .filter(|r| matches!(r, DiffTextRegion::SplitLeft | DiffTextRegion::SplitRight))
                .unwrap_or(DiffTextRegion::SplitLeft),
        };

        let end_visible_ix = self.diff_visible_indices.len() - 1;
        let end_region = start_region;
        let end_text = self.diff_text_line_for_region(end_visible_ix, end_region);

        self.diff_text_selecting = false;
        self.diff_text_anchor = Some(DiffTextPos {
            visible_ix: 0,
            region: start_region,
            offset: 0,
        });
        self.diff_text_head = Some(DiffTextPos {
            visible_ix: end_visible_ix,
            region: end_region,
            offset: end_text.len(),
        });
    }

    fn select_diff_text_rows_range(
        &mut self,
        start_visible_ix: usize,
        end_visible_ix: usize,
        region: DiffTextRegion,
    ) {
        let list_len = self.diff_visible_indices.len();
        if list_len == 0 {
            return;
        }

        let a = start_visible_ix.min(list_len - 1);
        let b = end_visible_ix.min(list_len - 1);
        let (a, b) = if a <= b { (a, b) } else { (b, a) };

        let region = match self.diff_view {
            DiffViewMode::Inline => DiffTextRegion::Inline,
            DiffViewMode::Split => match region {
                DiffTextRegion::SplitRight => DiffTextRegion::SplitRight,
                _ => DiffTextRegion::SplitLeft,
            },
        };
        let start_region = region;
        let end_region = region;

        let end_text = self.diff_text_line_for_region(b, end_region);

        self.diff_text_selecting = false;
        self.diff_text_anchor = Some(DiffTextPos {
            visible_ix: a,
            region: start_region,
            offset: 0,
        });
        self.diff_text_head = Some(DiffTextPos {
            visible_ix: b,
            region: end_region,
            offset: end_text.len(),
        });

        // Double-click produces two click events; suppress both.
        self.diff_suppress_clicks_remaining = 2;
    }

    fn double_click_select_diff_text(
        &mut self,
        visible_ix: usize,
        region: DiffTextRegion,
        kind: DiffClickKind,
    ) {
        if self.is_file_preview_active() {
            let Some(count) = self.worktree_preview_line_count() else {
                return;
            };
            if count == 0 {
                return;
            }
            let visible_ix = visible_ix.min(count - 1);
            let end_text = self.diff_text_line_for_region(visible_ix, DiffTextRegion::Inline);
            self.diff_text_selecting = false;
            self.diff_text_anchor = Some(DiffTextPos {
                visible_ix,
                region: DiffTextRegion::Inline,
                offset: 0,
            });
            self.diff_text_head = Some(DiffTextPos {
                visible_ix,
                region: DiffTextRegion::Inline,
                offset: end_text.len(),
            });

            // Double-click produces two click events; suppress both.
            self.diff_suppress_clicks_remaining = 2;
            return;
        }

        let list_len = self.diff_visible_indices.len();
        if list_len == 0 {
            return;
        }
        let visible_ix = visible_ix.min(list_len - 1);

        // File-diff view doesn't have file/hunk header blocks; treat as row selection.
        if self.is_file_diff_view_active() {
            self.select_diff_text_rows_range(visible_ix, visible_ix, region);
            return;
        }

        let end = match self.diff_view {
            DiffViewMode::Inline => match kind {
                DiffClickKind::Line => visible_ix,
                DiffClickKind::HunkHeader => self
                    .diff_next_boundary_visible_ix(visible_ix, |src_ix| {
                        let line = &self.diff_cache[src_ix];
                        matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk)
                            || (matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
                                && line.text.starts_with("diff --git "))
                    })
                    .unwrap_or(list_len - 1),
                DiffClickKind::FileHeader => self
                    .diff_next_boundary_visible_ix(visible_ix, |src_ix| {
                        let line = &self.diff_cache[src_ix];
                        matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
                            && line.text.starts_with("diff --git ")
                    })
                    .unwrap_or(list_len - 1),
            },
            DiffViewMode::Split => match kind {
                DiffClickKind::Line => visible_ix,
                DiffClickKind::HunkHeader => self
                    .split_next_boundary_visible_ix(visible_ix, |row| {
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
                    .split_next_boundary_visible_ix(visible_ix, |row| {
                        matches!(
                            row,
                            PatchSplitRow::Raw {
                                click_kind: DiffClickKind::FileHeader,
                                ..
                            }
                        )
                    })
                    .unwrap_or(list_len - 1),
            },
        };

        self.select_diff_text_rows_range(visible_ix, end, region);
    }

    fn history_visible_columns(&self) -> (bool, bool) {
        // Prefer keeping commit message visible. Hide SHA first, then date.
        let mut available = self.last_window_size.width;
        available -= px(280.0);
        available -= px(420.0);
        available -= px(64.0);
        if available <= px(0.0) {
            return (false, false);
        }

        let min_message = px(220.0);

        // Always show Branch + Graph; Message is flex.
        let fixed_base = self.history_col_branch + self.history_col_graph;

        // Show both by default.
        let mut show_date = true;
        let mut show_sha = true;
        let mut fixed = fixed_base + self.history_col_date + self.history_col_sha;

        if available - fixed < min_message {
            show_sha = false;
            fixed -= self.history_col_sha;
        }
        if available - fixed < min_message {
            show_date = false;
            show_sha = false;
        }

        (show_date, show_sha)
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
                        placeholder: "Filter repositories".into(),
                        multiline: false,
                        read_only: false,
                        chromeless: false,
                        soft_wrap: false,
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
                        placeholder: "Filter branches".into(),
                        multiline: false,
                        read_only: false,
                        chromeless: false,
                        soft_wrap: false,
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

    fn ensure_tag_picker_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Entity<zed::TextInput> {
        let theme = self.theme;
        let input = self.tag_picker_search_input.get_or_insert_with(|| {
            cx.new(|cx| {
                zed::TextInput::new(
                    zed::TextInputOptions {
                        placeholder: "Filter tags".into(),
                        multiline: false,
                        read_only: false,
                        chromeless: false,
                        soft_wrap: false,
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

    fn ensure_remote_picker_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Entity<zed::TextInput> {
        let theme = self.theme;
        let input = self.remote_picker_search_input.get_or_insert_with(|| {
            cx.new(|cx| {
                zed::TextInput::new(
                    zed::TextInputOptions {
                        placeholder: "Filter remotes".into(),
                        multiline: false,
                        read_only: false,
                        chromeless: false,
                        soft_wrap: false,
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

    fn ensure_worktree_picker_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Entity<zed::TextInput> {
        let theme = self.theme;
        let input = self.worktree_picker_search_input.get_or_insert_with(|| {
            cx.new(|cx| {
                zed::TextInput::new(
                    zed::TextInputOptions {
                        placeholder: "Filter worktrees".into(),
                        multiline: false,
                        read_only: false,
                        chromeless: false,
                        soft_wrap: false,
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

    fn ensure_submodule_picker_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Entity<zed::TextInput> {
        let theme = self.theme;
        let input = self.submodule_picker_search_input.get_or_insert_with(|| {
            cx.new(|cx| {
                zed::TextInput::new(
                    zed::TextInputOptions {
                        placeholder: "Filter submodules".into(),
                        multiline: false,
                        read_only: false,
                        chromeless: false,
                        soft_wrap: false,
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

    fn ensure_file_history_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Entity<zed::TextInput> {
        let theme = self.theme;
        let input = self.file_history_search_input.get_or_insert_with(|| {
            cx.new(|cx| {
                zed::TextInput::new(
                    zed::TextInputOptions {
                        placeholder: "Filter commits".into(),
                        multiline: false,
                        read_only: false,
                        chromeless: false,
                        soft_wrap: false,
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
                        placeholder: "Filter hunks".into(),
                        multiline: false,
                        read_only: false,
                        chromeless: false,
                        soft_wrap: false,
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

    #[cfg(test)]
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

        if grouped.is_empty()
            && let Loadable::Ready(remotes) = &repo.remotes
        {
            for remote in remotes {
                grouped.entry(remote.name.clone()).or_default();
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

    fn branch_sidebar_rows(repo: &RepoState) -> Vec<BranchSidebarRow> {
        let mut rows = Vec::new();
        let head_upstream_full = match (&repo.branches, &repo.head_branch) {
            (Loadable::Ready(branches), Loadable::Ready(head)) => branches
                .iter()
                .find(|b| b.name == *head)
                .and_then(|b| b.upstream.as_ref())
                .map(|u| format!("{}/{}", u.remote, u.branch)),
            _ => None,
        };

        rows.push(BranchSidebarRow::SectionHeader {
            section: BranchSection::Local,
            top_border: false,
        });

        match &repo.branches {
            Loadable::Ready(branches) if branches.is_empty() => {
                rows.push(BranchSidebarRow::Placeholder {
                    section: BranchSection::Local,
                    message: "No branches".into(),
                });
            }
            Loadable::Ready(branches) => {
                let head = match &repo.head_branch {
                    Loadable::Ready(h) => Some(h.as_str()),
                    _ => None,
                };
                let mut local_meta: std::collections::HashMap<
                    String,
                    (Option<UpstreamDivergence>, bool),
                > = std::collections::HashMap::new();
                for b in branches {
                    local_meta.insert(
                        b.name.clone(),
                        (b.divergence, head.is_some_and(|h| h == b.name)),
                    );
                }

                let mut tree = SlashTree::default();
                for branch in branches {
                    tree.insert(&branch.name);
                }
                push_slash_tree_rows(
                    &tree,
                    &mut rows,
                    Some(&local_meta),
                    head_upstream_full.as_deref(),
                    0,
                    false,
                    BranchSection::Local,
                    "",
                );
            }
            Loadable::Loading => rows.push(BranchSidebarRow::Placeholder {
                section: BranchSection::Local,
                message: "Loading".into(),
            }),
            Loadable::NotLoaded => rows.push(BranchSidebarRow::Placeholder {
                section: BranchSection::Local,
                message: "Not loaded".into(),
            }),
            Loadable::Error(e) => rows.push(BranchSidebarRow::Placeholder {
                section: BranchSection::Local,
                message: e.clone().into(),
            }),
        }

        rows.push(BranchSidebarRow::SectionSpacer);
        rows.push(BranchSidebarRow::SectionHeader {
            section: BranchSection::Remote,
            top_border: true,
        });

        let mut remotes: BTreeMap<String, Vec<String>> = BTreeMap::new();
        match &repo.remote_branches {
            Loadable::Ready(branches) => {
                for branch in branches {
                    remotes
                        .entry(branch.remote.clone())
                        .or_default()
                        .push(branch.name.clone());
                }
            }
            Loadable::Loading => {
                rows.push(BranchSidebarRow::Placeholder {
                    section: BranchSection::Remote,
                    message: "Loading".into(),
                });
                return rows;
            }
            Loadable::Error(e) => {
                rows.push(BranchSidebarRow::Placeholder {
                    section: BranchSection::Remote,
                    message: e.clone().into(),
                });
                return rows;
            }
            Loadable::NotLoaded => {
                if let Loadable::Ready(known) = &repo.remotes {
                    for remote in known {
                        remotes.entry(remote.name.clone()).or_default();
                    }
                }
            }
        }

        if remotes.is_empty() {
            rows.push(BranchSidebarRow::Placeholder {
                section: BranchSection::Remote,
                message: "No remotes".into(),
            });
            return rows;
        }

        for (remote, mut branches) in remotes {
            branches.sort();
            branches.dedup();
            rows.push(BranchSidebarRow::RemoteHeader {
                name: remote.clone().into(),
            });
            if branches.is_empty() {
                continue;
            }

            let mut tree = SlashTree::default();
            for branch in branches {
                tree.insert(&branch);
            }
            let name_prefix = format!("{remote}/");
            push_slash_tree_rows(
                &tree,
                &mut rows,
                None,
                head_upstream_full.as_deref(),
                1,
                true,
                BranchSection::Remote,
                &name_prefix,
            );
        }

        rows.push(BranchSidebarRow::SectionSpacer);
        rows.push(BranchSidebarRow::StashHeader { top_border: true });
        match &repo.stashes {
            Loadable::Ready(stashes) if stashes.is_empty() => {
                rows.push(BranchSidebarRow::StashPlaceholder {
                    message: "No stashes".into(),
                });
            }
            Loadable::Ready(stashes) => {
                for stash in stashes {
                    rows.push(BranchSidebarRow::StashItem {
                        index: stash.index,
                        message: stash.message.clone().into(),
                        created_at: stash.created_at,
                    });
                }
            }
            Loadable::Loading => rows.push(BranchSidebarRow::StashPlaceholder {
                message: "Loading".into(),
            }),
            Loadable::NotLoaded => rows.push(BranchSidebarRow::StashPlaceholder {
                message: "Not loaded".into(),
            }),
            Loadable::Error(e) => rows.push(BranchSidebarRow::StashPlaceholder {
                message: e.clone().into(),
            }),
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
            self.worktree_preview_scroll
                .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
            self.worktree_preview_path = Some(path);
            self.worktree_preview = Loadable::Loading;
            self.worktree_preview_segments_cache_path = None;
            self.worktree_preview_segments_cache.clear();
        } else if matches!(
            self.worktree_preview,
            Loadable::NotLoaded | Loadable::Error(_)
        ) {
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
        } || matches!(
            self.worktree_preview,
            Loadable::Error(_) | Loadable::NotLoaded
        );
        if !should_reload {
            return;
        }

        self.worktree_preview_path = Some(path.clone());
        self.worktree_preview = Loadable::Loading;
        self.worktree_preview_segments_cache_path = None;
        self.worktree_preview_segments_cache.clear();
        self.worktree_preview_scroll
            .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);

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
                this.worktree_preview_scroll
                    .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
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
                self.worktree_preview_scroll
                    .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
                self.worktree_preview_path = Some(abs_path);
                self.worktree_preview = Loadable::Ready(lines);
                self.worktree_preview_segments_cache_path = None;
                self.worktree_preview_segments_cache.clear();
            }
            Err(e) => {
                if self.worktree_preview_path.as_ref() != Some(&abs_path)
                    || matches!(
                        self.worktree_preview,
                        Loadable::NotLoaded | Loadable::Loading
                    )
                {
                    self.worktree_preview_scroll
                        .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
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
        struct Rebuild {
            repo_id: RepoId,
            diff_file_rev: u64,
            diff_target: Option<DiffTarget>,
            file_path: Option<std::path::PathBuf>,
            rows: Vec<FileDiffRow>,
            inline_rows: Vec<AnnotatedDiffLine>,
            inline_word_highlights: Vec<Option<Vec<Range<usize>>>>,
            split_word_highlights_old: Vec<Option<Vec<Range<usize>>>>,
            split_word_highlights_new: Vec<Option<Vec<Range<usize>>>>,
        }

        enum Action {
            Clear,
            Noop,
            Reset {
                repo_id: RepoId,
                diff_file_rev: u64,
                diff_target: Option<DiffTarget>,
            },
            Rebuild(Rebuild),
        }

        let action = (|| {
            let Some(repo) = self.active_repo() else {
                return Action::Clear;
            };

            if !Self::is_file_diff_target(repo.diff_target.as_ref()) {
                return Action::Clear;
            }

            if self.file_diff_cache_repo_id == Some(repo.id)
                && self.file_diff_cache_rev == repo.diff_file_rev
                && self.file_diff_cache_target.as_ref() == repo.diff_target.as_ref()
            {
                return Action::Noop;
            }

            let repo_id = repo.id;
            let diff_file_rev = repo.diff_file_rev;
            let diff_target = repo.diff_target.clone();

            let Loadable::Ready(file_opt) = &repo.diff_file else {
                return Action::Reset {
                    repo_id,
                    diff_file_rev,
                    diff_target,
                };
            };
            let Some(file) = file_opt.as_ref() else {
                return Action::Reset {
                    repo_id,
                    diff_file_rev,
                    diff_target,
                };
            };

            let old_text = file.old.as_deref().unwrap_or("");
            let new_text = file.new.as_deref().unwrap_or("");
            let rows = gitgpui_core::file_diff::side_by_side_rows(old_text, new_text);

            // Store the file path for syntax highlighting.
            let workdir = &repo.spec.workdir;
            let file_path = Some(if file.path.is_absolute() {
                file.path.clone()
            } else {
                workdir.join(&file.path)
            });

            // Precompute word highlights and inline rows.
            let mut split_word_highlights_old: Vec<Option<Vec<Range<usize>>>> =
                vec![None; rows.len()];
            let mut split_word_highlights_new: Vec<Option<Vec<Range<usize>>>> =
                vec![None; rows.len()];
            for (row_ix, row) in rows.iter().enumerate() {
                if matches!(row.kind, gitgpui_core::file_diff::FileDiffRowKind::Modify) {
                    let old = row.old.as_deref().unwrap_or("");
                    let new = row.new.as_deref().unwrap_or("");
                    let (old_ranges, new_ranges) = word_diff_ranges(old, new);
                    if !old_ranges.is_empty() {
                        split_word_highlights_old[row_ix] = Some(old_ranges);
                    }
                    if !new_ranges.is_empty() {
                        split_word_highlights_new[row_ix] = Some(new_ranges);
                    }
                }
            }

            let mut inline_rows: Vec<AnnotatedDiffLine> = Vec::new();
            let mut inline_word_highlights: Vec<Option<Vec<Range<usize>>>> = Vec::new();
            for row in &rows {
                use gitgpui_core::file_diff::FileDiffRowKind as K;
                match row.kind {
                    K::Context => {
                        inline_rows.push(AnnotatedDiffLine {
                            kind: gitgpui_core::domain::DiffLineKind::Context,
                            text: format!(" {}", row.old.as_deref().unwrap_or("")),
                            old_line: row.old_line,
                            new_line: row.new_line,
                        });
                        inline_word_highlights.push(None);
                    }
                    K::Add => {
                        inline_rows.push(AnnotatedDiffLine {
                            kind: gitgpui_core::domain::DiffLineKind::Add,
                            text: format!("+{}", row.new.as_deref().unwrap_or("")),
                            old_line: None,
                            new_line: row.new_line,
                        });
                        inline_word_highlights.push(None);
                    }
                    K::Remove => {
                        inline_rows.push(AnnotatedDiffLine {
                            kind: gitgpui_core::domain::DiffLineKind::Remove,
                            text: format!("-{}", row.old.as_deref().unwrap_or("")),
                            old_line: row.old_line,
                            new_line: None,
                        });
                        inline_word_highlights.push(None);
                    }
                    K::Modify => {
                        let old = row.old.as_deref().unwrap_or("");
                        let new = row.new.as_deref().unwrap_or("");
                        let (old_ranges, new_ranges) = word_diff_ranges(old, new);

                        inline_rows.push(AnnotatedDiffLine {
                            kind: gitgpui_core::domain::DiffLineKind::Remove,
                            text: format!("-{}", old),
                            old_line: row.old_line,
                            new_line: None,
                        });
                        inline_word_highlights.push((!old_ranges.is_empty()).then_some(old_ranges));

                        inline_rows.push(AnnotatedDiffLine {
                            kind: gitgpui_core::domain::DiffLineKind::Add,
                            text: format!("+{}", new),
                            old_line: None,
                            new_line: row.new_line,
                        });
                        inline_word_highlights.push((!new_ranges.is_empty()).then_some(new_ranges));
                    }
                }
            }

            Action::Rebuild(Rebuild {
                repo_id,
                diff_file_rev,
                diff_target,
                file_path,
                rows,
                inline_rows,
                inline_word_highlights,
                split_word_highlights_old,
                split_word_highlights_new,
            })
        })();

        match action {
            Action::Noop => {}
            Action::Clear => {
                self.file_diff_cache_repo_id = None;
                self.file_diff_cache_target = None;
                self.file_diff_cache_rev = 0;
                self.file_diff_cache_path = None;
                self.file_diff_cache_rows.clear();
                self.file_diff_inline_cache.clear();
                self.file_diff_inline_word_highlights.clear();
                self.file_diff_split_word_highlights_old.clear();
                self.file_diff_split_word_highlights_new.clear();
            }
            Action::Reset {
                repo_id,
                diff_file_rev,
                diff_target,
            } => {
                self.file_diff_cache_repo_id = Some(repo_id);
                self.file_diff_cache_rev = diff_file_rev;
                self.file_diff_cache_target = diff_target;
                self.file_diff_cache_path = None;
                self.file_diff_cache_rows.clear();
                self.file_diff_inline_cache.clear();
                self.file_diff_inline_word_highlights.clear();
                self.file_diff_split_word_highlights_old.clear();
                self.file_diff_split_word_highlights_new.clear();
            }
            Action::Rebuild(rebuild) => {
                self.file_diff_cache_repo_id = Some(rebuild.repo_id);
                self.file_diff_cache_rev = rebuild.diff_file_rev;
                self.file_diff_cache_target = rebuild.diff_target;
                self.file_diff_cache_path = rebuild.file_path;
                self.file_diff_cache_rows = rebuild.rows;
                self.file_diff_inline_cache = rebuild.inline_rows;
                self.file_diff_inline_word_highlights = rebuild.inline_word_highlights;
                self.file_diff_split_word_highlights_old = rebuild.split_word_highlights_old;
                self.file_diff_split_word_highlights_new = rebuild.split_word_highlights_new;

                // Reset the segment cache to avoid mixing patch/file indices.
                self.diff_text_segments_cache.clear();
            }
        }
    }

    fn image_format_for_path(path: &std::path::Path) -> Option<gpui::ImageFormat> {
        let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
        match ext.as_str() {
            "png" => Some(gpui::ImageFormat::Png),
            "jpg" | "jpeg" => Some(gpui::ImageFormat::Jpeg),
            "gif" => Some(gpui::ImageFormat::Gif),
            "webp" => Some(gpui::ImageFormat::Webp),
            "bmp" => Some(gpui::ImageFormat::Bmp),
            "svg" => Some(gpui::ImageFormat::Svg),
            "tif" | "tiff" => Some(gpui::ImageFormat::Tiff),
            _ => None,
        }
    }

    fn ensure_file_image_diff_cache(&mut self) {
        struct Rebuild {
            repo_id: RepoId,
            diff_file_rev: u64,
            diff_target: Option<DiffTarget>,
            file_path: Option<std::path::PathBuf>,
            old: Option<Arc<gpui::Image>>,
            new: Option<Arc<gpui::Image>>,
        }

        enum Action {
            Clear,
            Noop,
            Reset {
                repo_id: RepoId,
                diff_file_rev: u64,
                diff_target: Option<DiffTarget>,
            },
            Rebuild(Rebuild),
        }

        let action = (|| {
            let Some(repo) = self.active_repo() else {
                return Action::Clear;
            };

            if !Self::is_file_diff_target(repo.diff_target.as_ref()) {
                return Action::Clear;
            }

            if self.file_image_diff_cache_repo_id == Some(repo.id)
                && self.file_image_diff_cache_rev == repo.diff_file_rev
                && self.file_image_diff_cache_target.as_ref() == repo.diff_target.as_ref()
            {
                return Action::Noop;
            }

            let repo_id = repo.id;
            let diff_file_rev = repo.diff_file_rev;
            let diff_target = repo.diff_target.clone();

            let Loadable::Ready(file_opt) = &repo.diff_file_image else {
                return Action::Reset {
                    repo_id,
                    diff_file_rev,
                    diff_target,
                };
            };
            let Some(file) = file_opt.as_ref() else {
                return Action::Reset {
                    repo_id,
                    diff_file_rev,
                    diff_target,
                };
            };

            let format = Self::image_format_for_path(&file.path);
            let old = file
                .old
                .as_ref()
                .and_then(|bytes| format.map(|format| Arc::new(gpui::Image::from_bytes(format, bytes.clone()))));
            let new = file
                .new
                .as_ref()
                .and_then(|bytes| format.map(|format| Arc::new(gpui::Image::from_bytes(format, bytes.clone()))));

            let workdir = &repo.spec.workdir;
            let file_path = Some(if file.path.is_absolute() {
                file.path.clone()
            } else {
                workdir.join(&file.path)
            });

            Action::Rebuild(Rebuild {
                repo_id,
                diff_file_rev,
                diff_target,
                file_path,
                old,
                new,
            })
        })();

        match action {
            Action::Noop => {}
            Action::Clear => {
                self.file_image_diff_cache_repo_id = None;
                self.file_image_diff_cache_target = None;
                self.file_image_diff_cache_rev = 0;
                self.file_image_diff_cache_path = None;
                self.file_image_diff_cache_old = None;
                self.file_image_diff_cache_new = None;
            }
            Action::Reset {
                repo_id,
                diff_file_rev,
                diff_target,
            } => {
                self.file_image_diff_cache_repo_id = Some(repo_id);
                self.file_image_diff_cache_rev = diff_file_rev;
                self.file_image_diff_cache_target = diff_target;
                self.file_image_diff_cache_path = None;
                self.file_image_diff_cache_old = None;
                self.file_image_diff_cache_new = None;
            }
            Action::Rebuild(rebuild) => {
                self.file_image_diff_cache_repo_id = Some(rebuild.repo_id);
                self.file_image_diff_cache_rev = rebuild.diff_file_rev;
                self.file_image_diff_cache_target = rebuild.diff_target;
                self.file_image_diff_cache_path = rebuild.file_path;
                self.file_image_diff_cache_old = rebuild.old;
                self.file_image_diff_cache_new = rebuild.new;
            }
        }
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
        self.diff_scrollbar_markers_cache.clear();
        self.diff_word_highlights.clear();
        self.diff_file_stats.clear();
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
            (
                repo.id,
                repo.diff_rev,
                repo.diff_target.clone(),
                workdir,
                annotated,
            )
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

        if let Some((abs_path, lines)) = build_new_file_preview_from_diff(
            &self.diff_cache,
            &workdir,
            self.diff_cache_target.as_ref(),
        ) {
            self.diff_preview_is_new_file = true;
            self.diff_preview_new_file_lines = Arc::new(lines);
            self.worktree_preview_path = Some(abs_path);
            self.worktree_preview = Loadable::Ready(self.diff_preview_new_file_lines.clone());
            self.worktree_preview_segments_cache_path = None;
            self.worktree_preview_segments_cache.clear();
            self.worktree_preview_scroll
                .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
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
            DiffViewMode::Inline => {
                scrollbar_markers_from_flags(self.diff_visible_indices.len(), |visible_ix| {
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
                })
            }
            DiffViewMode::Split => {
                scrollbar_markers_from_flags(self.diff_visible_indices.len(), |visible_ix| {
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
                })
            }
        }
    }

    fn compute_diff_scrollbar_markers(&self) -> Vec<zed::ScrollbarMarker> {
        if !self.is_file_diff_view_active() {
            return self.diff_scrollbar_markers_patch();
        }

        match self.diff_view {
            DiffViewMode::Inline => {
                scrollbar_markers_from_flags(self.diff_visible_indices.len(), |visible_ix| {
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
                })
            }
            DiffViewMode::Split => {
                scrollbar_markers_from_flags(self.diff_visible_indices.len(), |visible_ix| {
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
                })
            }
        }
    }

    fn apply_state_snapshot(&mut self, next: AppState, cx: &mut gpui::Context<Self>) {
        let prev_active_repo_id = self.state.active_repo;
        let prev_selected_commit = prev_active_repo_id.and_then(|repo_id| {
            self.state
                .repos
                .iter()
                .find(|r| r.id == repo_id)
                .and_then(|r| r.selected_commit.clone())
        });

        let next_repo_id = next.active_repo;
        let next_repo = next_repo_id.and_then(|id| next.repos.iter().find(|r| r.id == id));
        let next_diff_target = next_repo.and_then(|r| r.diff_target.as_ref()).cloned();
        let next_diff_rev = next_repo.map(|r| r.diff_rev).unwrap_or(0);
        let next_selected_commit = next_repo.and_then(|r| r.selected_commit.clone());

        let prev_diff_target = self
            .active_repo()
            .and_then(|r| r.diff_target.as_ref())
            .cloned();

        let next_clone = next.clone.clone();

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

        match next_clone.as_ref() {
            Some(op) => match &op.status {
                CloneOpStatus::Running => {
                    let needs_reset = self.clone_progress_toast_id.is_none()
                        || self.clone_progress_dest.as_ref() != Some(&op.dest);
                    if needs_reset {
                        if let Some(id) = self.clone_progress_toast_id.take() {
                            self.remove_toast(id, cx);
                        }
                        self.clone_progress_last_seq = 0;
                        self.clone_progress_dest = Some(op.dest.clone());

                        let id = self.push_persistent_toast(
                            zed::ToastKind::Success,
                            format!(
                                "Cloning repository…\n{}\n→ {}",
                                op.url,
                                op.dest.display()
                            ),
                            cx,
                        );
                        self.clone_progress_toast_id = Some(id);
                    }

                    if let Some(id) = self.clone_progress_toast_id
                        && self.clone_progress_last_seq != op.seq
                    {
                        self.clone_progress_last_seq = op.seq;
                        let tail_lines = op.output_tail.iter().rev().take(12).rev().cloned();
                        let tail = tail_lines.collect::<Vec<_>>().join("\n");
                        let message = if tail.is_empty() {
                            format!(
                                "Cloning repository…\n{}\n→ {}",
                                op.url,
                                op.dest.display()
                            )
                        } else {
                            format!(
                                "Cloning repository…\n{}\n→ {}\n\n{}",
                                op.url,
                                op.dest.display(),
                                tail
                            )
                        };
                        self.update_toast_text(id, message, cx);
                    }
                }
                CloneOpStatus::FinishedOk => {
                    if self.clone_progress_last_seq != op.seq {
                        if let Some(id) = self.clone_progress_toast_id.take() {
                            self.remove_toast(id, cx);
                        }
                        self.clone_progress_dest = None;
                        self.clone_progress_last_seq = op.seq;
                        self.push_toast(
                            zed::ToastKind::Success,
                            format!("Clone finished: {}", op.dest.display()),
                            cx,
                        );
                    }
                }
                CloneOpStatus::FinishedErr(err) => {
                    if self.clone_progress_last_seq != op.seq {
                        if let Some(id) = self.clone_progress_toast_id.take() {
                            self.remove_toast(id, cx);
                        }
                        self.clone_progress_dest = None;
                        self.clone_progress_last_seq = op.seq;
                        self.push_toast(zed::ToastKind::Error, format!("Clone failed: {err}"), cx);
                    }
                }
            },
            None => {
                if let Some(id) = self.clone_progress_toast_id.take() {
                    self.remove_toast(id, cx);
                }
                self.clone_progress_last_seq = 0;
                self.clone_progress_dest = None;
            }
        }

        if prev_diff_target != next_diff_target {
            self.diff_selection_anchor = None;
            self.diff_selection_range = None;
            self.diff_autoscroll_pending = next_diff_target.is_some();
        }

        self.state = next;

        if prev_active_repo_id != next_repo_id {
            self.history_scroll
                .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
            self.commit_scroll.set_offset(point(px(0.0), px(0.0)));
            self.commit_files_scroll
                .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
        } else if prev_selected_commit != next_selected_commit {
            self.commit_scroll.set_offset(point(px(0.0), px(0.0)));
            self.commit_files_scroll
                .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
        }

        self.update_commit_details_delay(cx);

        let should_rebuild_diff_cache = self.diff_cache_repo_id != next_repo_id
            || self.diff_cache_rev != next_diff_rev
            || self.diff_cache_target != next_diff_target;
        if should_rebuild_diff_cache {
            self.rebuild_diff_cache();
        }
    }

    fn update_commit_details_delay(&mut self, cx: &mut gpui::Context<Self>) {
        let Some((repo_id, selected_id, ready_for_selected, is_error)) = (|| {
            let repo = self.active_repo()?;
            let selected_id = repo.selected_commit.clone()?;
            let ready_for_selected = matches!(
                &repo.commit_details,
                Loadable::Ready(details) if details.id == selected_id
            );
            let is_error = matches!(&repo.commit_details, Loadable::Error(_));
            Some((repo.id, selected_id, ready_for_selected, is_error))
        })() else {
            self.commit_details_delay = None;
            return;
        };

        if ready_for_selected || is_error {
            self.commit_details_delay = None;
            return;
        }

        let same_selection = self
            .commit_details_delay
            .as_ref()
            .is_some_and(|s| s.repo_id == repo_id && s.commit_id == selected_id);
        if same_selection {
            return;
        }

        self.commit_details_delay_seq = self.commit_details_delay_seq.wrapping_add(1);
        let seq = self.commit_details_delay_seq;
        self.commit_details_delay = Some(CommitDetailsDelayState {
            repo_id,
            commit_id: selected_id.clone(),
            show_loading: false,
        });

        cx.spawn(
            async move |view: WeakEntity<GitGpuiView>, cx: &mut gpui::AsyncApp| {
                Timer::after(Duration::from_millis(100)).await;
                let _ = view.update(cx, |this, cx| {
                    if this.commit_details_delay_seq != seq {
                        return;
                    }
                    let Some(repo) = this.active_repo() else {
                        return;
                    };
                    let Some(selected_id) = repo.selected_commit.clone() else {
                        return;
                    };
                    if repo.id != repo_id {
                        return;
                    }

                    let ready_for_selected = matches!(
                        &repo.commit_details,
                        Loadable::Ready(details) if details.id == selected_id
                    );
                    if ready_for_selected || matches!(&repo.commit_details, Loadable::Error(_)) {
                        return;
                    }

                    if let Some(state) = this.commit_details_delay.as_mut()
                        && state.repo_id == repo_id
                        && state.commit_id == selected_id
                        && !state.show_loading
                    {
                        state.show_loading = true;
                        cx.notify();
                    }
                });
            },
        )
        .detach();
    }

    fn push_toast(&mut self, kind: zed::ToastKind, message: String, cx: &mut gpui::Context<Self>) {
        let id = self.push_toast_inner(kind, message, Some(match kind {
            zed::ToastKind::Error => Duration::from_secs(15),
            zed::ToastKind::Success => Duration::from_secs(6),
        }), cx);

        let ttl = match kind {
            zed::ToastKind::Error => Duration::from_secs(15),
            zed::ToastKind::Success => Duration::from_secs(6),
        };

        cx.spawn(
            async move |view: WeakEntity<GitGpuiView>, cx: &mut gpui::AsyncApp| {
                Timer::after(ttl).await;
                let _ = view.update(cx, |this, cx| {
                    this.toasts.retain(|t| t.id != id);
                    cx.notify();
                });
            },
        )
        .detach();
    }

    fn push_persistent_toast(
        &mut self,
        kind: zed::ToastKind,
        message: String,
        cx: &mut gpui::Context<Self>,
    ) -> u64 {
        self.push_toast_inner(kind, message, None, cx)
    }

    fn push_toast_inner(
        &mut self,
        kind: zed::ToastKind,
        message: String,
        _ttl: Option<Duration>,
        cx: &mut gpui::Context<Self>,
    ) -> u64 {
        let id = self
            .toasts
            .last()
            .map(|t| t.id.wrapping_add(1))
            .unwrap_or(1);
        let theme = self.theme;
        let input = cx.new(|cx| {
            zed::TextInput::new_inert(
                zed::TextInputOptions {
                    placeholder: "".into(),
                    multiline: true,
                    read_only: true,
                    chromeless: true,
                    soft_wrap: true,
                },
                cx,
            )
        });
        input.update(cx, |input, cx| {
            input.set_theme(theme, cx);
            input.set_text(message, cx);
            input.set_read_only(true, cx);
        });

        self.toasts.push(ToastState { id, kind, input });
        id
    }

    fn update_toast_text(
        &mut self,
        id: u64,
        message: String,
        cx: &mut gpui::Context<Self>,
    ) {
        let Some(toast) = self.toasts.iter().find(|t| t.id == id).cloned() else {
            return;
        };
        let theme = self.theme;
        toast.input.update(cx, |input, cx| {
            input.set_theme(theme, cx);
            input.set_text(message, cx);
            input.set_read_only(true, cx);
        });
    }

    fn remove_toast(&mut self, id: u64, cx: &mut gpui::Context<Self>) {
        let before = self.toasts.len();
        self.toasts.retain(|t| t.id != id);
        if self.toasts.len() != before {
            cx.notify();
        }
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn maybe_auto_install_linux_desktop_integration(&mut self, cx: &mut gpui::Context<Self>) {
        use std::path::PathBuf;

        if std::env::var_os("GITGPUI_NO_DESKTOP_INSTALL").is_some() {
            return;
        }

        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        if !desktop.to_ascii_lowercase().contains("gnome") {
            return;
        }

        let home = std::env::var_os("HOME").map(PathBuf::from);
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|h| h.join(".local/share")));
        let Some(data_home) = data_home else {
            return;
        };

        let desktop_path = data_home.join("applications/gitgpui.desktop");
        let icon_path = data_home.join("icons/hicolor/scalable/apps/gitgpui.svg");
        if desktop_path.exists() && icon_path.exists() {
            return;
        }

        self.install_linux_desktop_integration(cx);
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn install_linux_desktop_integration(&mut self, cx: &mut gpui::Context<Self>) {
        use std::fs;
        use std::path::PathBuf;
        use std::process::Command;

        const DESKTOP_TEMPLATE: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/linux/gitgpui.desktop"
        ));
        const ICON_SVG: &[u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/gitgpui_logo.svg"
        ));

        let Ok(exe) = std::env::current_exe() else {
            self.push_toast(
                zed::ToastKind::Error,
                "Desktop install failed: could not resolve executable path".to_string(),
                cx,
            );
            return;
        };

        let home = std::env::var_os("HOME").map(PathBuf::from);
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|h| h.join(".local/share")));
        let Some(data_home) = data_home else {
            self.push_toast(
                zed::ToastKind::Error,
                "Desktop install failed: HOME/XDG_DATA_HOME not set".to_string(),
                cx,
            );
            return;
        };

        let applications_dir = data_home.join("applications");
        let icons_dir = data_home.join("icons/hicolor/scalable/apps");
        let desktop_path = applications_dir.join("gitgpui.desktop");
        let icon_path = icons_dir.join("gitgpui.svg");

        if let Err(e) =
            fs::create_dir_all(&applications_dir).and_then(|_| fs::create_dir_all(&icons_dir))
        {
            self.push_toast(
                zed::ToastKind::Error,
                format!("Desktop install failed: {e}"),
                cx,
            );
            return;
        }

        let mut desktop_out = String::new();
        for line in DESKTOP_TEMPLATE.lines() {
            if line.starts_with("Exec=") {
                desktop_out.push_str("Exec=");
                desktop_out.push_str(&exe.display().to_string());
                desktop_out.push('\n');
            } else {
                desktop_out.push_str(line);
                desktop_out.push('\n');
            }
        }

        if let Err(e) = fs::write(&desktop_path, desktop_out.as_bytes())
            .and_then(|_| fs::write(&icon_path, ICON_SVG))
        {
            self.push_toast(
                zed::ToastKind::Error,
                format!("Desktop install failed: {e}"),
                cx,
            );
            return;
        }

        let _ = Command::new("update-desktop-database")
            .arg(&applications_dir)
            .output();
        let _ = Command::new("gtk-update-icon-cache")
            .arg(data_home.join("icons/hicolor"))
            .output();

        self.push_toast(
            zed::ToastKind::Success,
            format!(
                "Installed desktop entry + icon to:\n{}\n{}\n\nIf GNOME still shows a generic icon, log out/in (or restart GNOME Shell).",
                desktop_path.display(),
                icon_path.display()
            ),
            cx,
        );
    }

    fn rebuild_diff_word_highlights(&mut self) {
        self.diff_word_highlights.clear();
        self.diff_word_highlights
            .resize_with(self.diff_cache.len(), || None);

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

            let mut removed: Vec<(usize, &str)> = Vec::new();
            while ix < self.diff_cache.len()
                && matches!(
                    self.diff_cache[ix].kind,
                    gitgpui_core::domain::DiffLineKind::Remove
                )
            {
                let text = diff_content_text(&self.diff_cache[ix]);
                removed.push((ix, text));
                ix += 1;
            }

            let mut added: Vec<(usize, &str)> = Vec::new();
            while ix < self.diff_cache.len()
                && matches!(
                    self.diff_cache[ix].kind,
                    gitgpui_core::domain::DiffLineKind::Add
                )
            {
                let text = diff_content_text(&self.diff_cache[ix]);
                added.push((ix, text));
                ix += 1;
            }

            let pairs = removed.len().min(added.len());
            for i in 0..pairs {
                let (old_ix, old_text) = removed[i];
                let (new_ix, new_text) = added[i];
                let (old_ranges, new_ranges) = word_diff_ranges(old_text, new_text);
                if !old_ranges.is_empty() {
                    self.diff_word_highlights[old_ix] = Some(old_ranges);
                }
                if !new_ranges.is_empty() {
                    self.diff_word_highlights[new_ix] = Some(new_ranges);
                }
            }

            for (old_ix, old_text) in removed.into_iter().skip(pairs) {
                if !old_text.is_empty() {
                    self.diff_word_highlights[old_ix] = Some(vec![0..old_text.len()]);
                }
            }
            for (new_ix, new_text) in added.into_iter().skip(pairs) {
                if !new_text.is_empty() {
                    self.diff_word_highlights[new_ix] = Some(vec![0..new_text.len()]);
                }
            }
        }
    }

    fn ensure_diff_visible_indices(&mut self) {
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
            && self.diff_visible_view == self.diff_view
            && self.diff_visible_is_file_view == is_file_view
        {
            return;
        }

        self.diff_visible_cache_len = current_len;
        self.diff_visible_view = self.diff_view;
        self.diff_visible_is_file_view = is_file_view;

        if is_file_view {
            self.diff_visible_indices = (0..current_len).collect();
            self.diff_scrollbar_markers_cache = self.compute_diff_scrollbar_markers();
            return;
        }

        match self.diff_view {
            DiffViewMode::Inline => {
                self.diff_visible_indices = self
                    .diff_cache
                    .iter()
                    .enumerate()
                    .filter_map(|(ix, line)| {
                        (!should_hide_unified_diff_header_line(line)).then_some(ix)
                    })
                    .collect();
            }
            DiffViewMode::Split => {
                self.ensure_diff_split_cache();

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

    fn handle_split_row_click(
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

        if shift && let Some(anchor) = self.diff_selection_anchor {
            let a = anchor.min(clicked_visible_ix);
            let b = anchor.max(clicked_visible_ix);
            self.diff_selection_range = Some((a, b));
            return;
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

        if shift && let Some(anchor) = self.diff_selection_anchor {
            let a = anchor.min(clicked_visible_ix);
            let b = anchor.max(clicked_visible_ix);
            self.diff_selection_range = Some((a, b));
            return;
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
                    let changed = self.file_diff_inline_cache.get(inline_ix).is_some_and(|l| {
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

    fn diff_nav_prev_target(entries: &[usize], current: usize) -> Option<usize> {
        entries.iter().rev().find(|&&ix| ix < current).copied()
    }

    fn diff_nav_next_target(entries: &[usize], current: usize) -> Option<usize> {
        entries.iter().find(|&&ix| ix > current).copied()
    }

    fn diff_jump_prev(&mut self) {
        let entries = self.diff_nav_entries();
        if entries.is_empty() {
            return;
        }

        let current = self.diff_selection_anchor.unwrap_or(0);
        let Some(target) = Self::diff_nav_prev_target(&entries, current) else {
            return;
        };

        self.diff_scroll
            .scroll_to_item_strict(target, gpui::ScrollStrategy::Center);
        self.diff_selection_anchor = Some(target);
        self.diff_selection_range = Some((target, target));
    }

    fn diff_jump_next(&mut self) {
        let entries = self.diff_nav_entries();
        if entries.is_empty() {
            return;
        }

        let current = self.diff_selection_anchor.unwrap_or(0);
        let Some(target) = Self::diff_nav_next_target(&entries, current) else {
            return;
        };

        self.diff_scroll
            .scroll_to_item_strict(target, gpui::ScrollStrategy::Center);
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

    fn ensure_history_cache(&mut self, cx: &mut gpui::Context<Self>) {
        enum Next {
            Clear,
            CacheOk,
            Inflight,
            Build {
                request: HistoryCacheRequest,
                commits: Vec<Commit>,
            },
        }

        let next = if let Some(repo) = self.active_repo() {
            if let Loadable::Ready(page) = &repo.log {
                let request = HistoryCacheRequest {
                    repo_id: repo.id,
                    log_fingerprint: Self::log_fingerprint(&page.commits),
                };

                let cache_ok = self.history_cache.as_ref().is_some_and(|c| {
                    c.repo_id == request.repo_id
                        && c.log_fingerprint == request.log_fingerprint
                });
                if cache_ok {
                    Next::CacheOk
                } else if self.history_cache_inflight.as_ref() == Some(&request) {
                    Next::Inflight
                } else {
                    Next::Build {
                        request,
                        commits: page.commits.clone(),
                    }
                }
            } else {
                Next::Clear
            }
        } else {
            Next::Clear
        };

        let (request_for_task, commits) = match next {
            Next::Clear => {
                self.history_cache_inflight = None;
                self.history_cache = None;
                return;
            }
            Next::CacheOk => {
                self.history_cache_inflight = None;
                return;
            }
            Next::Inflight => {
                return;
            }
            Next::Build { request, commits } => (request, commits),
        };

        self.history_cache_seq = self.history_cache_seq.wrapping_add(1);
        let seq = self.history_cache_seq;
        self.history_cache_inflight = Some(request_for_task.clone());

        let theme = self.theme;

        cx.spawn(
            async move |view: WeakEntity<GitGpuiView>, cx: &mut gpui::AsyncApp| {
                let visible_indices = (0..commits.len()).collect::<Vec<_>>();

                let visible_commits = visible_indices
                    .iter()
                    .filter_map(|ix| commits.get(*ix))
                    .collect::<Vec<_>>();

                let graph_rows = history_graph::compute_graph(&visible_commits, theme);
                let max_lanes = graph_rows
                    .iter()
                    .map(|r| r.lanes_now.len().max(r.lanes_next.len()))
                    .max()
                    .unwrap_or(1);

                let _ = view.update(cx, |this, cx| {
                    if this.history_cache_seq != seq {
                        return;
                    }
                    if this.history_cache_inflight.as_ref() != Some(&request_for_task) {
                        return;
                    }
                    if this.active_repo_id() != Some(request_for_task.repo_id) {
                        return;
                    }

                    if this.history_col_graph_auto && this.history_col_resize.is_none() {
                        let required = px(HISTORY_GRAPH_MARGIN_X_PX * 2.0
                            + HISTORY_GRAPH_COL_GAP_PX * (max_lanes as f32));
                        this.history_col_graph = required
                            .min(px(HISTORY_COL_GRAPH_MAX_PX))
                            .max(px(HISTORY_COL_GRAPH_MIN_PX));
                    }

                    this.history_cache_inflight = None;
                    this.history_cache = Some(HistoryCache {
                        repo_id: request_for_task.repo_id,
                        log_fingerprint: request_for_task.log_fingerprint,
                        visible_indices,
                        graph_rows,
                    });
                    cx.notify();
                });
            },
        )
        .detach();
    }

    #[cfg(test)]
    pub(crate) fn is_popover_open(&self) -> bool {
        self.popover.is_some()
    }
}

fn push_slash_tree_rows(
    tree: &SlashTree,
    out: &mut Vec<BranchSidebarRow>,
    local_meta: Option<&std::collections::HashMap<String, (Option<UpstreamDivergence>, bool)>>,
    upstream_full: Option<&str>,
    depth: usize,
    muted: bool,
    section: BranchSection,
    name_prefix: &str,
) {
    for (label, node) in &tree.children {
        if node.children.is_empty() {
            if node.is_leaf {
                let full = format!("{name_prefix}{label}");
                let is_upstream = upstream_full.is_some_and(|u| u == full.as_str());
                let (divergence, is_head) = local_meta
                    .and_then(|m| m.get(&full))
                    .copied()
                    .unwrap_or((None, false));
                out.push(BranchSidebarRow::Branch {
                    label: label.clone().into(),
                    name: full.into(),
                    section,
                    depth,
                    muted,
                    divergence,
                    is_head,
                    is_upstream,
                });
            }
            continue;
        }

        out.push(BranchSidebarRow::GroupHeader {
            label: format!("{label}/").into(),
            depth,
        });

        if node.is_leaf {
            let full = format!("{name_prefix}{label}");
            let is_upstream = upstream_full.is_some_and(|u| u == full.as_str());
            let (divergence, is_head) = local_meta
                .and_then(|m| m.get(&full))
                .copied()
                .unwrap_or((None, false));
            out.push(BranchSidebarRow::Branch {
                label: label.clone().into(),
                name: full.into(),
                section,
                depth: depth + 1,
                muted,
                divergence,
                is_head,
                is_upstream,
            });
        }

        let next_prefix = format!("{name_prefix}{label}/");
        push_slash_tree_rows(
            node,
            out,
            local_meta,
            upstream_full,
            depth + 1,
            muted,
            section,
            &next_prefix,
        );
    }
}

fn build_patch_split_rows(diff: &[AnnotatedDiffLine]) -> Vec<PatchSplitRow> {
    use gitgpui_core::domain::DiffLineKind as DK;
    use gitgpui_core::file_diff::FileDiffRowKind as K;

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
        let is_file_header =
            matches!(line.kind, DK::Header) && line.text.starts_with("diff --git ");

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
        self.last_window_size = window.window_bounds().get_bounds().size;
        self.sync_tooltip_state(cx);
        self.clamp_pane_widths_to_window();
        if self.last_window_size != self.ui_window_size_last_seen {
            self.ui_window_size_last_seen = self.last_window_size;
            self.schedule_ui_settings_persist(cx);
        }

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
                    .child(self.repo_tabs_bar(cx))
                    .child(self.open_repo_panel(cx))
                    .child(self.action_bar(cx))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_1()
                            .min_h(px(0.0))
                            .child(
                                div()
                                    .id("sidebar_pane")
                                    .w(self.sidebar_width)
                                    .min_h(px(0.0))
                                    .bg(theme.colors.surface_bg)
                                    .child(self.sidebar(cx)),
                            )
                            .child(self.pane_resize_handle(
                                theme,
                                "pane_resize_sidebar",
                                PaneResizeHandle::Sidebar,
                                cx,
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .min_h(px(0.0))
                                    .child(main_view),
                            )
                            .child(self.pane_resize_handle(
                                theme,
                                "pane_resize_details",
                                PaneResizeHandle::Details,
                                cx,
                            ))
                            .child(
                                div()
                                    .id("details_pane")
                                    .w(self.details_width)
                                    .min_h(px(0.0))
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_h(px(0.0))
                                            .child(self.commit_details_view(cx)),
                                    ),
                            ),
                    ),
            );

        if let Some(repo) = self.active_repo()
            && let Some(err) = repo.last_error.as_ref()
        {
            self.error_banner_input.update(cx, |input, cx| {
                input.set_theme(theme, cx);
                input.set_text(err.clone(), cx);
                input.set_read_only(true, cx);
            });
            body = body.child(
                div()
                    .px_2()
                    .py_1()
                    .bg(with_alpha(theme.colors.danger, 0.15))
                    .border_1()
                    .border_color(with_alpha(theme.colors.danger, 0.3))
                    .rounded(px(theme.radii.panel))
                    .child(self.error_banner_input.clone()),
            );
        }

        let mut root = div()
            .size_full()
            .cursor(cursor)
            .text_color(theme.colors.text);
        root = root.relative();

        root = root.on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, window, cx| {
            this.last_mouse_pos = e.position;
            this.maybe_restart_tooltip_delay(cx);

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
        }));
        if tiling.is_some() {
            root = root.on_mouse_down(
                MouseButton::Left,
                cx.listener(|_this, e: &MouseDownEvent, window, cx| {
                    let Decorations::Client { tiling } = window.window_decorations() else {
                        return;
                    };

                    let size = window.window_bounds().get_bounds().size;
                    let edge = resize_edge(e.position, CLIENT_SIDE_DECORATION_INSET, size, tiling);
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

        if let Some(text) = self.tooltip_visible_text.clone() {
            let tooltip_bg = gpui::rgba(0x000000ff);
            let tooltip_text_color = gpui::rgba(0xffffffff);
            let anchor = self.tooltip_visible_pos.unwrap_or(self.last_mouse_pos);
            let pos = point(anchor.x + px(12.0), anchor.y + px(18.0));
            root = root.child(
                anchored()
                    .position(pos)
                    .anchor(Corner::TopLeft)
                    .offset(point(px(0.0), px(0.0)))
                    .child(
                        div()
                            .occlude()
                            .px_2()
                            .py_1()
                            .bg(tooltip_bg)
                            .rounded(px(theme.radii.row))
                            .shadow_sm()
                            .text_xs()
                            .text_color(tooltip_text_color)
                            .child(text),
                    ),
            );
        }

        root
    }
}

impl GitGpuiView {
    fn sync_tooltip_state(&mut self, cx: &mut gpui::Context<Self>) {
        if self.tooltip_text == self.tooltip_candidate_last {
            return;
        }

        self.tooltip_candidate_last = self.tooltip_text.clone();
        self.tooltip_visible_text = None;
        self.tooltip_visible_pos = None;
        self.tooltip_pending_pos = None;
        self.tooltip_delay_seq = self.tooltip_delay_seq.wrapping_add(1);

        let Some(text) = self.tooltip_text.clone() else {
            return;
        };

        let anchor = self.last_mouse_pos;
        self.tooltip_pending_pos = Some(anchor);
        let seq = self.tooltip_delay_seq;

        cx.spawn(
            async move |view: WeakEntity<GitGpuiView>, cx: &mut gpui::AsyncApp| {
                Timer::after(Duration::from_millis(500)).await;
                let _ = view.update(cx, |this, cx| {
                    if this.tooltip_delay_seq != seq {
                        return;
                    }
                    if this.tooltip_text.as_ref() != Some(&text) {
                        return;
                    }
                    let Some(pending_pos) = this.tooltip_pending_pos else {
                        return;
                    };
                    let dx = (this.last_mouse_pos.x - pending_pos.x).abs();
                    let dy = (this.last_mouse_pos.y - pending_pos.y).abs();
                    if dx > px(2.0) || dy > px(2.0) {
                        return;
                    }
                    this.tooltip_visible_text = Some(text.clone());
                    this.tooltip_visible_pos = Some(pending_pos);
                    cx.notify();
                });
            },
        )
        .detach();
    }

    fn maybe_restart_tooltip_delay(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(candidate) = self.tooltip_text.clone() else {
            if self.tooltip_visible_text.is_some() {
                self.tooltip_visible_text = None;
                self.tooltip_visible_pos = None;
                cx.notify();
            }
            return;
        };

        if let Some(visible_anchor) = self.tooltip_visible_pos {
            let dx = (self.last_mouse_pos.x - visible_anchor.x).abs();
            let dy = (self.last_mouse_pos.y - visible_anchor.y).abs();
            if dx <= px(6.0) && dy <= px(6.0) {
                return;
            }
        }

        let should_restart = match self.tooltip_pending_pos {
            None => true,
            Some(pending_anchor) => {
                let dx = (self.last_mouse_pos.x - pending_anchor.x).abs();
                let dy = (self.last_mouse_pos.y - pending_anchor.y).abs();
                dx > px(2.0) || dy > px(2.0)
            }
        };

        if !should_restart {
            return;
        }

        self.tooltip_visible_text = None;
        self.tooltip_visible_pos = None;
        self.tooltip_pending_pos = Some(self.last_mouse_pos);
        self.tooltip_delay_seq = self.tooltip_delay_seq.wrapping_add(1);
        let seq = self.tooltip_delay_seq;

        cx.spawn(
            async move |view: WeakEntity<GitGpuiView>, cx: &mut gpui::AsyncApp| {
                Timer::after(Duration::from_millis(500)).await;
                let _ = view.update(cx, |this, cx| {
                    if this.tooltip_delay_seq != seq {
                        return;
                    }
                    if this.tooltip_text.as_ref() != Some(&candidate) {
                        return;
                    }
                    let Some(pending_pos) = this.tooltip_pending_pos else {
                        return;
                    };
                    let dx = (this.last_mouse_pos.x - pending_pos.x).abs();
                    let dy = (this.last_mouse_pos.y - pending_pos.y).abs();
                    if dx > px(2.0) || dy > px(2.0) {
                        return;
                    }
                    this.tooltip_visible_text = Some(candidate.clone());
                    this.tooltip_visible_pos = Some(pending_pos);
                    cx.notify();
                });
            },
        )
        .detach();
    }

    fn schedule_ui_settings_persist(&mut self, cx: &mut gpui::Context<Self>) {
        self.ui_settings_persist_seq = self.ui_settings_persist_seq.wrapping_add(1);
        let seq = self.ui_settings_persist_seq;

        cx.spawn(
            async move |view: WeakEntity<GitGpuiView>, cx: &mut gpui::AsyncApp| {
                Timer::after(Duration::from_millis(250)).await;
                let _ = view.update(cx, |this, _cx| {
                    if this.ui_settings_persist_seq != seq {
                        return;
                    }

                    let ww: f32 = this.last_window_size.width.round().into();
                    let wh: f32 = this.last_window_size.height.round().into();
                    let window_width = (ww.is_finite() && ww >= 1.0).then_some(ww as u32);
                    let window_height = (wh.is_finite() && wh >= 1.0).then_some(wh as u32);

                    let sidebar_width: f32 = this.sidebar_width.round().into();
                    let details_width: f32 = this.details_width.round().into();

                    let settings = session::UiSettings {
                        window_width,
                        window_height,
                        sidebar_width: (sidebar_width.is_finite() && sidebar_width >= 1.0)
                            .then_some(sidebar_width as u32),
                        details_width: (details_width.is_finite() && details_width >= 1.0)
                            .then_some(details_width as u32),
                    };

                    let _ = session::persist_ui_settings(settings);
                });
            },
        )
        .detach();
    }

    fn clamp_pane_widths_to_window(&mut self) {
        let total_w = self.last_window_size.width;
        if total_w.is_zero() {
            return;
        }

        let handles_w = px(PANE_RESIZE_HANDLE_PX) * 2.0;
        let main_min = px(MAIN_MIN_PX);
        let sidebar_min = px(SIDEBAR_MIN_PX);
        let details_min = px(DETAILS_MIN_PX);

        let max_sidebar = (total_w - self.details_width - main_min - handles_w).max(sidebar_min);
        self.sidebar_width = self.sidebar_width.max(sidebar_min).min(max_sidebar);

        let max_details = (total_w - self.sidebar_width - main_min - handles_w).max(details_min);
        self.details_width = self.details_width.max(details_min).min(max_details);
    }
}

pub(super) fn with_alpha(mut color: gpui::Rgba, alpha: f32) -> gpui::Rgba {
    color.a = alpha;
    color
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
        if let Some(last) = out.last_mut()
            && r.start <= last.end
        {
            last.end = last.end.max(r.end);
            continue;
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
    if old_tokens.is_empty() || new_tokens.is_empty() {
        return fallback_affix_diff_ranges(old, new);
    }

    let old_slices: Vec<&str> = old_tokens
        .iter()
        .map(|t| &old[t.range.clone()])
        .collect::<Vec<_>>();
    let new_slices: Vec<&str> = new_tokens
        .iter()
        .map(|t| &new[t.range.clone()])
        .collect::<Vec<_>>();

    // Compute the longest common subsequence via Myers' diff algorithm, marking matching tokens
    // as "kept". This is substantially faster than O(n*m) DP for typical lines.
    let n = old_slices.len() as isize;
    let m = new_slices.len() as isize;
    let max = (n + m) as usize;
    let offset = max as isize;

    let mut v: Vec<isize> = vec![0; 2 * max + 1];
    let mut trace: Vec<Vec<isize>> = Vec::new();

    let mut done = false;
    for d in 0..=max {
        for k in (-(d as isize)..=(d as isize)).step_by(2) {
            let k_ix = (k + offset) as usize;
            let x = if k == -(d as isize)
                || (k != d as isize && v[(k - 1 + offset) as usize] < v[(k + 1 + offset) as usize])
            {
                v[(k + 1 + offset) as usize]
            } else {
                v[(k - 1 + offset) as usize] + 1
            };

            let mut x = x;
            let mut y = x - k;
            while x < n && y < m && old_slices[x as usize] == new_slices[y as usize] {
                x += 1;
                y += 1;
            }

            v[k_ix] = x;
            if x >= n && y >= m {
                done = true;
                break;
            }
        }

        trace.push(v.clone());
        if done {
            break;
        }
    }

    let mut keep_old = vec![false; old_tokens.len()];
    let mut keep_new = vec![false; new_tokens.len()];

    let mut x = n;
    let mut y = m;

    for d in (1..trace.len()).rev() {
        let d_isize = d as isize;
        let v = &trace[d - 1];
        let k = x - y;
        let prev_k = if k == -d_isize
            || (k != d_isize && v[(k - 1 + offset) as usize] < v[(k + 1 + offset) as usize])
        {
            k + 1
        } else {
            k - 1
        };

        let prev_x = v[(prev_k + offset) as usize];
        let prev_y = prev_x - prev_k;

        while x > prev_x && y > prev_y {
            keep_old[(x - 1) as usize] = true;
            keep_new[(y - 1) as usize] = true;
            x -= 1;
            y -= 1;
        }

        // Step to the previous edit.
        if x == prev_x {
            y -= 1;
        } else {
            x -= 1;
        }
    }

    while x > 0 && y > 0 {
        if old_slices[(x - 1) as usize] != new_slices[(y - 1) as usize] {
            break;
        }
        keep_old[(x - 1) as usize] = true;
        keep_new[(y - 1) as usize] = true;
        x -= 1;
        y -= 1;
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
        DiffTarget::Commit {
            path: Some(path), ..
        } => path.clone(),
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
        let close = cx.listener(|this, _e: &MouseDownEvent, _w, cx| this.close_popover(cx));

        let scrim = div()
            .id("popover_scrim")
            .debug_selector(|| "repo_popover_close".to_string())
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
            .map(|kind| self.popover_view(kind, cx).into_any_element())
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

        let progress_id = self.clone_progress_toast_id;
        let max_other = if progress_id.is_some() { 2 } else { 3 };
        let mut displayed = self
            .toasts
            .iter()
            .rev()
            .filter(|t| Some(t.id) != progress_id)
            .take(max_other)
            .cloned()
            .collect::<Vec<_>>();
        if let Some(id) = progress_id
            && let Some(progress) = self.toasts.iter().find(|t| t.id == id).cloned()
        {
            displayed.push(progress);
        }

        let children = displayed
            .into_iter()
            .map(|t| zed::toast(theme, t.kind, t.input.clone()).id(("toast", t.id)));

        div()
            .id("toast_layer")
            .absolute()
            .right_0()
            .bottom_0()
            .p_2()
            .flex()
            .flex_col()
            .items_end()
            .gap_2()
            .children(children)
            .into_any_element()
    }
}

struct Poller {
    _task: gpui::Task<()>,
}

impl Poller {
    fn start(
        store: Arc<AppStore>,
        events: mpsc::Receiver<StoreEvent>,
        view: WeakEntity<GitGpuiView>,
        window: &mut Window,
        cx: &mut gpui::Context<GitGpuiView>,
    ) -> Poller {
        let task = window.spawn(cx, async move |cx| {
            let events = events;
            loop {
                match events.try_recv() {
                    Ok(_) => while events.try_recv().is_ok() {},
                    Err(mpsc::TryRecvError::Empty) => {
                        Timer::after(Duration::from_millis(10)).await;
                        continue;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => break,
                }

                let snapshot = cx
                    .background_spawn({
                        let store = Arc::clone(&store);
                        async move { store.snapshot() }
                    })
                    .await;

                let _ = view.update(cx, |view, cx| {
                    view.apply_state_snapshot(snapshot, cx);
                    cx.notify();
                });
            }
        });

        Poller { _task: task }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitgpui_core::domain::{Branch, CommitId, RemoteBranch, RepoSpec, Upstream};
    use std::path::PathBuf;
    use std::time::Instant;

    #[test]
    fn diff_nav_prev_next_targets_do_not_wrap() {
        let entries = vec![10, 20, 30];

        assert_eq!(GitGpuiView::diff_nav_prev_target(&entries, 10), None);
        assert_eq!(GitGpuiView::diff_nav_next_target(&entries, 30), None);

        assert_eq!(GitGpuiView::diff_nav_prev_target(&entries, 25), Some(20));
        assert_eq!(GitGpuiView::diff_nav_next_target(&entries, 25), Some(30));

        assert_eq!(GitGpuiView::diff_nav_next_target(&entries, 0), Some(10));
        assert_eq!(GitGpuiView::diff_nav_prev_target(&entries, 100), Some(30));
    }

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
    fn remote_upstream_branch_is_marked() {
        let mut repo = RepoState::new_opening(
            RepoId(1),
            RepoSpec {
                workdir: PathBuf::new(),
            },
        );

        repo.head_branch = Loadable::Ready("main".to_string());
        repo.branches = Loadable::Ready(vec![Branch {
            name: "main".to_string(),
            target: CommitId("deadbeef".to_string()),
            upstream: Some(Upstream {
                remote: "origin".to_string(),
                branch: "main".to_string(),
            }),
            divergence: None,
        }]);
        repo.remote_branches = Loadable::Ready(vec![RemoteBranch {
            remote: "origin".to_string(),
            name: "main".to_string(),
        }]);

        let rows = GitGpuiView::branch_sidebar_rows(&repo);
        let upstream_row = rows.iter().find(|r| {
            matches!(
                r,
                BranchSidebarRow::Branch {
                    section: BranchSection::Remote,
                    name,
                    is_upstream: true,
                    ..
                } if name.as_ref() == "origin/main"
            )
        });
        assert!(
            upstream_row.is_some(),
            "expected origin/main to be marked as upstream"
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

    #[test]
    fn word_diff_ranges_highlights_changed_tokens() {
        let (old, new) = ("let x = 1;", "let x = 2;");
        let (old_ranges, new_ranges) = word_diff_ranges(old, new);
        assert_eq!(
            old_ranges
                .iter()
                .map(|r| &old[r.clone()])
                .collect::<Vec<_>>(),
            vec!["1"]
        );
        assert_eq!(
            new_ranges
                .iter()
                .map(|r| &new[r.clone()])
                .collect::<Vec<_>>(),
            vec!["2"]
        );
    }

    #[test]
    fn word_diff_ranges_handles_unicode_safely() {
        let (old, new) = ("aé", "aê");
        let (old_ranges, new_ranges) = word_diff_ranges(old, new);
        assert_eq!(
            old_ranges
                .iter()
                .map(|r| &old[r.clone()])
                .collect::<Vec<_>>(),
            vec!["aé"]
        );
        assert_eq!(
            new_ranges
                .iter()
                .map(|r| &new[r.clone()])
                .collect::<Vec<_>>(),
            vec!["aê"]
        );
    }

    #[test]
    fn word_diff_ranges_falls_back_for_large_inputs() {
        let old = "a".repeat(2048);
        let new = format!("{old}x");
        let (old_ranges, new_ranges) = word_diff_ranges(&old, &new);
        assert!(old_ranges.len() <= 1);
        assert!(new_ranges.len() <= 1);
    }

    #[test]
    fn word_diff_ranges_outputs_are_ordered_and_utf8_safe() {
        let (old, new) = ("aé b", "aê  b");
        let (old_ranges, new_ranges) = word_diff_ranges(old, new);

        for r in &old_ranges {
            assert!(r.start <= r.end);
            assert!(r.end <= old.len());
            assert!(old.is_char_boundary(r.start));
            assert!(old.is_char_boundary(r.end));
        }
        for w in old_ranges.windows(2) {
            assert!(w[0].end <= w[1].start);
        }

        for r in &new_ranges {
            assert!(r.start <= r.end);
            assert!(r.end <= new.len());
            assert!(new.is_char_boundary(r.start));
            assert!(new.is_char_boundary(r.end));
        }
        for w in new_ranges.windows(2) {
            assert!(w[0].end <= w[1].start);
        }
    }

    #[test]
    fn word_diff_ranges_empty_inputs_do_not_panic() {
        let (old_ranges, new_ranges) = word_diff_ranges("", "");
        assert!(old_ranges.is_empty());
        assert!(new_ranges.is_empty());
    }

    #[test]
    #[ignore]
    fn perf_word_diff_ranges_smoke() {
        let old = "fn foo(a: i32, b: i32) -> i32 { a + b }";
        let new = "fn foo(a: i32, b: i32) -> i32 { a - b }";
        let start = Instant::now();
        for _ in 0..200_000 {
            let _ = word_diff_ranges(old, new);
        }
        eprintln!("word_diff_ranges: {:?}", start.elapsed());
    }
}
