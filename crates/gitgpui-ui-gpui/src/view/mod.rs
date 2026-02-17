use crate::{theme::AppTheme, zed_port as zed};
use gitgpui_core::diff::{AnnotatedDiffLine, annotate_unified};
use gitgpui_core::domain::{
    Branch, Commit, CommitId, DiffArea, DiffTarget, FileStatus, FileStatusKind, RepoStatus, Tag,
    UpstreamDivergence,
};
use gitgpui_core::file_diff::FileDiffRow;
use gitgpui_core::services::{PullMode, RemoteUrlKind, ResetMode};
use gitgpui_state::model::{
    AppNotificationKind, AppState, CloneOpState, CloneOpStatus, DiagnosticKind, Loadable, RepoId,
    RepoState,
};
use gitgpui_state::msg::{Msg, RepoExternalChange, StoreEvent};
use gitgpui_state::session;
use gitgpui_state::store::AppStore;
use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt, AnyElement, App, Bounds, ClickEvent, Corner, CursorStyle, Decorations,
    Element, ElementId, Entity, FocusHandle, FontWeight, GlobalElementId, InspectorElementId,
    IsZero, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    Render, ResizeEdge, ScrollHandle, ShapedLine, SharedString, Size, Style, TextRun, Tiling,
    Timer, UniformListScrollHandle, WeakEntity, Window, WindowControlArea, anchored, div, fill,
    point, px, relative, size, uniform_list,
};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

mod app_model;
mod branch_sidebar;
mod caches;
mod chrome;
mod color;
mod conflict_resolver;
mod date_time;
mod diff_navigation;
mod diff_preview;
mod diff_text_model;
mod diff_text_selection;
mod diff_utils;
mod history_graph;
mod panels;
mod panes;
mod patch_split;
mod poller;
pub(crate) mod rows;
mod state_apply;
mod toast_host;
mod tooltip;
mod tooltip_host;
mod word_diff;

use app_model::AppUiModel;
use branch_sidebar::{BranchSection, BranchSidebarRow};
use caches::{
    BranchSidebarCache, HistoryCache, HistoryCacheRequest, HistoryStashIdsCache,
    HistoryWorktreeSummaryCache,
};
use chrome::{
    CLIENT_SIDE_DECORATION_INSET, TitleBarView, cursor_style_for_resize_edge, resize_edge,
};
use conflict_resolver::{
    ConflictDiffMode, ConflictInlineRow, ConflictPickSide, ConflictResolverViewMode,
};
use date_time::{DateTimeFormat, format_datetime_utc};
use diff_preview::{build_deleted_file_preview_from_diff, build_new_file_preview_from_diff};
use patch_split::build_patch_split_rows;
use poller::Poller;
use word_diff::capped_word_diff_ranges;

use diff_text_model::{CachedDiffStyledText, CachedDiffTextSegment, SyntaxTokenKind};
use diff_text_selection::{DiffTextSelectionOverlay, DiffTextSelectionTracker};
use diff_utils::{
    build_unified_patch_for_hunks, build_unified_patch_for_selected_lines_across_hunks,
    build_unified_patch_for_selected_lines_across_hunks_for_worktree_discard,
    compute_diff_file_for_src_ix, compute_diff_file_stats,
    context_menu_selection_range_from_diff_text, diff_content_text, enclosing_hunk_src_ix,
    parse_diff_git_header_path, parse_unified_hunk_header_for_display,
    scrollbar_markers_from_flags,
};
use panels::{ActionBarView, PopoverHost, RepoTabsBarView};
use panes::{DetailsPaneView, MainPaneView, SidebarPaneView};
use toast_host::ToastHost;
use tooltip_host::TooltipHost;

pub(crate) use chrome::window_frame;
use color::with_alpha;

const HISTORY_COL_BRANCH_PX: f32 = 130.0;
const HISTORY_COL_GRAPH_PX: f32 = 80.0;
const HISTORY_COL_GRAPH_MAX_PX: f32 = 240.0;
const HISTORY_COL_AUTHOR_PX: f32 = 140.0;
const HISTORY_COL_DATE_PX: f32 = 160.0;
const HISTORY_COL_SHA_PX: f32 = 88.0;
const HISTORY_COL_HANDLE_PX: f32 = 8.0;

const HISTORY_COL_BRANCH_MIN_PX: f32 = 60.0;
const HISTORY_COL_GRAPH_MIN_PX: f32 = 44.0;
const HISTORY_COL_AUTHOR_MIN_PX: f32 = 80.0;
const HISTORY_COL_DATE_MIN_PX: f32 = 110.0;
const HISTORY_COL_SHA_MIN_PX: f32 = 60.0;

const HISTORY_GRAPH_COL_GAP_PX: f32 = 16.0;
const HISTORY_GRAPH_MARGIN_X_PX: f32 = 10.0;

const PANE_RESIZE_HANDLE_PX: f32 = 8.0;
const SIDEBAR_MIN_PX: f32 = 200.0;
const DETAILS_MIN_PX: f32 = 280.0;
const MAIN_MIN_PX: f32 = 280.0;

const DIFF_SPLIT_COL_MIN_PX: f32 = 160.0;

const DIFF_TEXT_LAYOUT_CACHE_MAX_ENTRIES: usize = 4000;
const DIFF_TEXT_LAYOUT_CACHE_PRUNE_OVERAGE: usize = 256;
const TOAST_FADE_IN_MS: u64 = 180;
const TOAST_FADE_OUT_MS: u64 = 220;
const TOAST_SLIDE_PX: f32 = 12.0;

fn toast_fade_in_duration() -> Duration {
    Duration::from_millis(TOAST_FADE_IN_MS)
}

fn toast_fade_out_duration() -> Duration {
    Duration::from_millis(TOAST_FADE_OUT_MS)
}

fn toast_total_lifetime(ttl: Duration) -> Duration {
    toast_fade_in_duration() + ttl + toast_fade_out_duration()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HistoryColResizeHandle {
    Branch,
    Graph,
    Author,
    Date,
    Sha,
}

#[derive(Clone, Copy, Debug)]
struct HistoryColResizeState {
    handle: HistoryColResizeHandle,
    start_x: Pixels,
    start_branch: Pixels,
    start_graph: Pixels,
    start_author: Pixels,
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

fn is_svg_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
}

fn should_bypass_text_file_preview_for_path(path: &std::path::Path) -> bool {
    let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
        return false;
    };
    match ext.to_ascii_lowercase().as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "svg" => true,
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffViewMode {
    Inline,
    Split,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SvgDiffViewMode {
    Image,
    Code,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffSplitResizeHandle {
    Divider,
}

#[derive(Clone, Copy, Debug)]
struct DiffSplitResizeState {
    handle: DiffSplitResizeHandle,
    start_x: Pixels,
    start_ratio: f32,
}

struct DiffSplitResizeDragGhost;

impl Render for DiffSplitResizeDragGhost {
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
    ttl: Option<Duration>,
}

#[derive(Clone, Debug)]
struct CommitDetailsDelayState {
    repo_id: RepoId,
    commit_id: CommitId,
    show_loading: bool,
}

#[derive(Clone, Debug, Default)]
struct StatusMultiSelection {
    unstaged: Vec<std::path::PathBuf>,
    unstaged_anchor: Option<std::path::PathBuf>,
    staged: Vec<std::path::PathBuf>,
    staged_anchor: Option<std::path::PathBuf>,
}

fn status_entries_contain_path(entries: &[FileStatus], path: &std::path::PathBuf) -> bool {
    entries.iter().any(|e| &e.path == path)
}

fn reconcile_status_multi_selection(selection: &mut StatusMultiSelection, status: &RepoStatus) {
    selection
        .unstaged
        .retain(|p| status_entries_contain_path(&status.unstaged, p));
    if selection
        .unstaged_anchor
        .as_ref()
        .is_some_and(|a| !status_entries_contain_path(&status.unstaged, a))
    {
        selection.unstaged_anchor = None;
    }

    selection
        .staged
        .retain(|p| status_entries_contain_path(&status.staged, p));
    if selection
        .staged_anchor
        .as_ref()
        .is_some_and(|a| !status_entries_contain_path(&status.staged, a))
    {
        selection.staged_anchor = None;
    }
}

#[derive(Clone, Debug)]
struct ConflictResolverUiState {
    repo_id: Option<RepoId>,
    path: Option<std::path::PathBuf>,
    source_hash: Option<u64>,
    current: Option<String>,
    marker_segments: Vec<conflict_resolver::ConflictSegment>,
    active_conflict: usize,
    view_mode: ConflictResolverViewMode,
    diff_rows: Vec<FileDiffRow>,
    inline_rows: Vec<ConflictInlineRow>,
    three_way_base_lines: Vec<SharedString>,
    three_way_ours_lines: Vec<SharedString>,
    three_way_theirs_lines: Vec<SharedString>,
    three_way_len: usize,
    diff_mode: ConflictDiffMode,
    nav_anchor: Option<usize>,
    split_selected: std::collections::BTreeSet<(usize, ConflictPickSide)>,
    inline_selected: std::collections::BTreeSet<usize>,
}

impl Default for ConflictResolverUiState {
    fn default() -> Self {
        Self {
            repo_id: None,
            path: None,
            source_hash: None,
            current: None,
            marker_segments: Vec::new(),
            active_conflict: 0,
            view_mode: ConflictResolverViewMode::TwoWayDiff,
            diff_rows: Vec::new(),
            inline_rows: Vec::new(),
            three_way_base_lines: Vec::new(),
            three_way_ours_lines: Vec::new(),
            three_way_theirs_lines: Vec::new(),
            three_way_len: 0,
            diff_mode: ConflictDiffMode::Split,
            nav_anchor: None,
            split_selected: std::collections::BTreeSet::new(),
            inline_selected: std::collections::BTreeSet::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PopoverKind {
    RepoPicker,
    BranchPicker,
    CreateBranch,
    StashPrompt,
    CloneRepo,
    Settings,
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
    RemoteBranchDeletePicker {
        repo_id: RepoId,
        remote: Option<String>,
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
    RemoteMenu {
        repo_id: RepoId,
        name: String,
    },
    WorktreeSectionMenu {
        repo_id: RepoId,
    },
    WorktreeMenu {
        repo_id: RepoId,
        path: std::path::PathBuf,
    },
    SubmoduleSectionMenu {
        repo_id: RepoId,
    },
    SubmoduleMenu {
        repo_id: RepoId,
        path: std::path::PathBuf,
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
    ForceDeleteBranchConfirm {
        repo_id: RepoId,
        name: String,
    },
    DeleteRemoteBranchConfirm {
        repo_id: RepoId,
        remote: String,
        branch: String,
    },
    DiscardChangesConfirm {
        repo_id: RepoId,
        area: DiffArea,
        path: Option<std::path::PathBuf>,
    },
    PullReconcilePrompt {
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
    DiffEditorMenu {
        repo_id: RepoId,
        area: DiffArea,
        path: Option<std::path::PathBuf>,
        hunk_patch: Option<String>,
        hunks_count: usize,
        lines_patch: Option<String>,
        discard_lines_patch: Option<String>,
        lines_count: usize,
        copy_text: Option<String>,
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
    TagMenu {
        repo_id: RepoId,
        commit_id: CommitId,
    },
    HistoryBranchFilter {
        repo_id: RepoId,
    },
    HistoryColumnSettings,
}

#[cfg(test)]
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
    state: Arc<AppState>,
    _ui_model: Entity<AppUiModel>,
    _poller: Poller,
    _ui_model_subscription: gpui::Subscription,
    _activation_subscription: gpui::Subscription,
    _appearance_subscription: gpui::Subscription,
    theme: AppTheme,
    title_bar: Entity<TitleBarView>,
    sidebar_pane: Entity<SidebarPaneView>,
    main_pane: Entity<MainPaneView>,
    details_pane: Entity<DetailsPaneView>,
    repo_tabs_bar: Entity<RepoTabsBarView>,
    action_bar: Entity<ActionBarView>,
    tooltip_host: Entity<TooltipHost>,
    toast_host: Entity<ToastHost>,
    popover_host: Entity<PopoverHost>,

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
    history_cache: Option<HistoryCache>,
    history_worktree_summary_cache: Option<HistoryWorktreeSummaryCache>,
    history_stash_ids_cache: Option<HistoryStashIdsCache>,

    date_time_format: DateTimeFormat,
    terminal_program: Option<String>,

    open_repo_panel: bool,
    open_repo_input: Entity<zed::TextInput>,
    commit_message_input: Entity<zed::TextInput>,
    conflict_resolver_input: Entity<zed::TextInput>,
    conflict_resolver: ConflictResolverUiState,
    conflict_resolved_preview_path: Option<std::path::PathBuf>,
    conflict_resolved_preview_source_hash: Option<u64>,
    conflict_resolved_preview_lines: Vec<String>,
    conflict_resolved_preview_segments_cache: HashMap<usize, CachedDiffStyledText>,

    hover_resize_edge: Option<ResizeEdge>,

    history_scroll: UniformListScrollHandle,
    unstaged_scroll: UniformListScrollHandle,
    staged_scroll: UniformListScrollHandle,
    diff_scroll: UniformListScrollHandle,
    conflict_resolver_diff_scroll: UniformListScrollHandle,
    conflict_resolved_preview_scroll: UniformListScrollHandle,
    commit_files_scroll: UniformListScrollHandle,
    commit_scroll: ScrollHandle,

    sidebar_width: Pixels,
    details_width: Pixels,
    pane_resize: Option<PaneResizeState>,

    last_mouse_pos: Point<Pixels>,
    pending_pull_reconcile_prompt: Option<RepoId>,
    pending_force_delete_branch_prompt: Option<(RepoId, String)>,

    status_multi_selection: HashMap<RepoId, StatusMultiSelection>,
    status_multi_selection_last_status: HashMap<RepoId, Arc<RepoStatus>>,

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

    fn set_tooltip_text_if_changed(
        &mut self,
        next: Option<SharedString>,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        self.tooltip_host.update(cx, |tooltip, cx| {
            tooltip.set_tooltip_text_if_changed(next, cx)
        })
    }

    fn clear_tooltip_if_matches(
        &mut self,
        tooltip: &SharedString,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        let tooltip = tooltip.clone();
        self.tooltip_host.update(cx, |tooltip_host, cx| {
            tooltip_host.clear_tooltip_if_matches(&tooltip, cx)
        })
    }

    pub(in crate::view) fn close_popover(&mut self, cx: &mut gpui::Context<Self>) {
        self.popover_host
            .update(cx, |host, cx| host.close_popover(cx));
    }

    pub(in crate::view) fn open_popover_at(
        &mut self,
        kind: PopoverKind,
        anchor: Point<Pixels>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        self.popover_host.update(cx, |host, cx| {
            host.open_popover_at(kind, anchor, window, cx)
        });
    }

    fn is_file_preview_active(&self) -> bool {
        self.untracked_worktree_preview_path().is_some()
            || self.added_file_preview_abs_path().is_some()
            || self.deleted_file_preview_abs_path().is_some()
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
        if self.diff_text_layout_cache.len()
            <= DIFF_TEXT_LAYOUT_CACHE_MAX_ENTRIES + DIFF_TEXT_LAYOUT_CACHE_PRUNE_OVERAGE
        {
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
            && (self.file_image_diff_cache_old.is_some()
                || self.file_image_diff_cache_new.is_some())
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
        events: smol::channel::Receiver<StoreEvent>,
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
        let date_time_format = ui_session
            .date_time_format
            .as_deref()
            .and_then(DateTimeFormat::from_key)
            .unwrap_or(DateTimeFormat::YmdHm);
        let terminal_program = ui_session.terminal_program.and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        let history_show_author = ui_session.history_show_author.unwrap_or(true);
        let history_show_date = ui_session.history_show_date.unwrap_or(true);
        let history_show_sha = ui_session.history_show_sha.unwrap_or(false);

        if !ui_session.open_repos.is_empty() {
            store.dispatch(Msg::RestoreSession {
                open_repos: ui_session.open_repos,
                active_repo: ui_session.active_repo,
            });
        } else if let Ok(path) = std::env::current_dir() {
            store.dispatch(Msg::OpenRepo(path));
        }

        let initial_state = Arc::new(store.snapshot());
        let ui_model = cx.new(|_cx| AppUiModel::new(Arc::clone(&initial_state)));

        let ui_model_subscription = cx.observe(&ui_model, |this, model, cx| {
            let next = Arc::clone(&model.read(cx).state);
            let should_notify = this.apply_state_snapshot(next, cx);
            if should_notify {
                cx.notify();
            }
        });

        let weak_view = cx.weak_entity();
        let poller = Poller::start(Arc::clone(&store), events, ui_model.downgrade(), window, cx);

        let title_bar = cx.new(|_cx| TitleBarView::new(initial_theme, weak_view.clone()));
        let tooltip_host = cx.new(|_cx| TooltipHost::new(initial_theme));
        let toast_host = cx.new(|_cx| ToastHost::new(initial_theme, tooltip_host.downgrade()));
        let repo_tabs_bar = cx.new(|cx| {
            RepoTabsBarView::new(
                Arc::clone(&store),
                ui_model.clone(),
                initial_theme,
                weak_view.clone(),
                tooltip_host.downgrade(),
                cx,
            )
        });
        let action_bar = cx.new(|cx| {
            ActionBarView::new(
                Arc::clone(&store),
                ui_model.clone(),
                initial_theme,
                weak_view.clone(),
                tooltip_host.downgrade(),
                cx,
            )
        });

        let sidebar_pane = cx.new(|cx| {
            SidebarPaneView::new(
                Arc::clone(&store),
                ui_model.clone(),
                initial_theme,
                weak_view.clone(),
                tooltip_host.downgrade(),
                cx,
            )
        });
        let main_pane = cx.new(|cx| {
            MainPaneView::new(
                Arc::clone(&store),
                ui_model.clone(),
                initial_theme,
                date_time_format,
                history_show_author,
                history_show_date,
                history_show_sha,
                terminal_program.clone(),
                weak_view.clone(),
                tooltip_host.downgrade(),
                window,
                cx,
            )
        });
        let details_pane = cx.new(|cx| {
            DetailsPaneView::new(
                Arc::clone(&store),
                ui_model.clone(),
                initial_theme,
                weak_view.clone(),
                tooltip_host.downgrade(),
                window,
                cx,
            )
        });

        let popover_host = cx.new(|cx| {
            PopoverHost::new(
                Arc::clone(&store),
                ui_model.clone(),
                initial_theme,
                date_time_format,
                terminal_program.clone(),
                weak_view.clone(),
                toast_host.downgrade(),
                main_pane.clone(),
                details_pane.clone(),
                window,
                cx,
            )
        });

        let activation_subscription = cx.observe_window_activation(window, |this, window, _cx| {
            if !window.is_window_active() {
                return;
            }
            if let Some(repo) = this.active_repo()
                && matches!(repo.open, Loadable::Ready(_))
            {
                this.store.dispatch(Msg::RepoExternallyChanged {
                    repo_id: repo.id,
                    change: RepoExternalChange::GitState,
                });
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

        let conflict_resolver_input = cx.new(|cx| {
            zed::TextInput::new(
                zed::TextInputOptions {
                    placeholder: "Resolve file contents…".into(),
                    multiline: true,
                    read_only: false,
                    chromeless: true,
                    soft_wrap: true,
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

        let mut view = Self {
            state: Arc::clone(&initial_state),
            _ui_model: ui_model,
            store,
            _poller: poller,
            _ui_model_subscription: ui_model_subscription,
            _activation_subscription: activation_subscription,
            _appearance_subscription: appearance_subscription,
            theme: initial_theme,
            title_bar,
            sidebar_pane,
            main_pane,
            details_pane,
            repo_tabs_bar,
            action_bar,
            tooltip_host,
            toast_host,
            popover_host,
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
            conflict_resolver_input,
            conflict_resolver: ConflictResolverUiState::default(),
            conflict_resolved_preview_path: None,
            conflict_resolved_preview_source_hash: None,
            conflict_resolved_preview_lines: Vec::new(),
            conflict_resolved_preview_segments_cache: HashMap::default(),
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
            diff_text_hitboxes: HashMap::default(),
            diff_text_layout_cache_epoch: 0,
            diff_text_layout_cache: HashMap::default(),
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
            worktree_preview_segments_cache: HashMap::default(),
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
            history_cache: None,
            history_worktree_summary_cache: None,
            history_stash_ids_cache: None,
            date_time_format,
            terminal_program,
            open_repo_panel: false,
            open_repo_input,
            commit_message_input,
            hover_resize_edge: None,
            history_scroll: UniformListScrollHandle::default(),
            unstaged_scroll: UniformListScrollHandle::default(),
            staged_scroll: UniformListScrollHandle::default(),
            diff_scroll: UniformListScrollHandle::default(),
            conflict_resolver_diff_scroll: UniformListScrollHandle::default(),
            conflict_resolved_preview_scroll: UniformListScrollHandle::default(),
            commit_files_scroll: UniformListScrollHandle::default(),
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
            pending_pull_reconcile_prompt: None,
            pending_force_delete_branch_prompt: None,
            status_multi_selection: HashMap::default(),
            status_multi_selection_last_status: HashMap::default(),
            commit_details_message_input,
            error_banner_input,
            commit_details_delay: None,
            commit_details_delay_seq: 0,
            path_display_cache: std::cell::RefCell::new(HashMap::default()),
        };

        view.set_theme(initial_theme, cx);

        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        view.maybe_auto_install_linux_desktop_integration(cx);

        view
    }

    fn set_theme(&mut self, theme: AppTheme, cx: &mut gpui::Context<Self>) {
        self.theme = theme;
        self.title_bar
            .update(cx, |bar, cx| bar.set_theme(theme, cx));
        self.sidebar_pane
            .update(cx, |pane, cx| pane.set_theme(theme, cx));
        self.main_pane
            .update(cx, |pane, cx| pane.set_theme(theme, cx));
        self.details_pane
            .update(cx, |pane, cx| pane.set_theme(theme, cx));
        self.repo_tabs_bar
            .update(cx, |bar, cx| bar.set_theme(theme, cx));
        self.action_bar
            .update(cx, |bar, cx| bar.set_theme(theme, cx));
        self.tooltip_host
            .update(cx, |host, cx| host.set_theme(theme, cx));
        self.toast_host
            .update(cx, |host, cx| host.set_theme(theme, cx));
        self.popover_host
            .update(cx, |host, cx| host.set_theme(theme, cx));
        self.diff_text_segments_cache.clear();
        self.worktree_preview_segments_cache_path = None;
        self.worktree_preview_segments_cache.clear();
        self.open_repo_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.commit_message_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.diff_raw_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.conflict_resolver_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.commit_details_message_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
        self.error_banner_input
            .update(cx, |input, cx| input.set_theme(theme, cx));
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
            .then_some(self.diff_text_anchor)
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
                    .unwrap_or_else(|| line.text.as_ref().to_string())
            } else if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
                && line.text.starts_with("diff --git ")
            {
                parse_diff_git_header_path(line.text.as_ref())
                    .unwrap_or_else(|| line.text.as_ref().to_string())
            } else {
                line.text.as_ref().to_string()
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
                            .unwrap_or_else(|| line.text.as_ref().to_string())
                    }
                    DiffClickKind::FileHeader => parse_diff_git_header_path(line.text.as_ref())
                        .unwrap_or_else(|| line.text.as_ref().to_string()),
                    DiffClickKind::Line => line.text.as_ref().to_string(),
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

                let dot_git = path.join(".git");
                let is_repo_root = dot_git.is_dir()
                    || (dot_git.is_file()
                        && std::fs::read_to_string(&dot_git)
                            .ok()
                            .and_then(|contents| {
                                let line = contents.lines().next()?.trim();
                                let gitdir = line.strip_prefix("gitdir:")?.trim();
                                if gitdir.is_empty() {
                                    return None;
                                }
                                let gitdir_path = std::path::Path::new(gitdir);
                                let resolved = if gitdir_path.is_absolute() {
                                    gitdir_path.to_path_buf()
                                } else {
                                    path.join(gitdir_path)
                                };
                                Some(resolved.is_dir())
                            })
                            .unwrap_or(false));

                if is_repo_root {
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

    fn deleted_file_preview_abs_path(&self) -> Option<std::path::PathBuf> {
        let repo = self.active_repo()?;
        let workdir = repo.spec.workdir.clone();
        let target = repo.diff_target.as_ref()?;

        match target {
            DiffTarget::WorkingTree { path, area } => {
                let status = match &repo.status {
                    Loadable::Ready(s) => s,
                    _ => return None,
                };
                let entries = match area {
                    DiffArea::Unstaged => status.unstaged.as_slice(),
                    DiffArea::Staged => status.staged.as_slice(),
                };
                let is_deleted = entries
                    .iter()
                    .any(|e| e.kind == FileStatusKind::Deleted && &e.path == path);
                if !is_deleted {
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
                let is_deleted = details
                    .files
                    .iter()
                    .any(|f| f.kind == FileStatusKind::Deleted && &f.path == path);
                if !is_deleted {
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
                if meta.is_dir() {
                    return Err("Selected path is a directory. Select a file inside to preview, or stage the directory to add its contents.".to_string());
                }
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

            let prefer_old = match repo.diff_target.as_ref()? {
                DiffTarget::WorkingTree { path, area } => match &repo.status {
                    Loadable::Ready(status) => {
                        let entries = match area {
                            DiffArea::Unstaged => status.unstaged.as_slice(),
                            DiffArea::Staged => status.staged.as_slice(),
                        };
                        entries
                            .iter()
                            .any(|e| e.kind == FileStatusKind::Deleted && &e.path == path)
                    }
                    _ => false,
                },
                DiffTarget::Commit {
                    commit_id,
                    path: Some(path),
                } => match &repo.commit_details {
                    Loadable::Ready(details) if &details.id == commit_id => details
                        .files
                        .iter()
                        .any(|f| f.kind == FileStatusKind::Deleted && &f.path == path),
                    _ => false,
                },
                _ => false,
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
                    let text = if prefer_old {
                        file.old.as_deref()
                    } else {
                        file.new.as_deref()
                    };
                    text.map(|text| {
                        let lines = text.lines().map(|s| s.to_string()).collect::<Vec<_>>();
                        Ok(Arc::new(lines))
                    })
                }),
            };

            if preview_result.is_none() {
                match &repo.diff {
                    Loadable::Ready(diff) => {
                        let annotated = annotate_unified(diff);
                        if prefer_old {
                            if let Some((_abs_path, lines)) = build_deleted_file_preview_from_diff(
                                &annotated,
                                &repo.spec.workdir,
                                repo.diff_target.as_ref(),
                            ) {
                                preview_result = Some(Ok(Arc::new(lines)));
                            }
                        } else if let Some((_abs_path, lines)) = build_new_file_preview_from_diff(
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
                    let (old_ranges, new_ranges) = capped_word_diff_ranges(old, new);
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
                            text: format!(" {}", row.old.as_deref().unwrap_or("")).into(),
                            old_line: row.old_line,
                            new_line: row.new_line,
                        });
                        inline_word_highlights.push(None);
                    }
                    K::Add => {
                        inline_rows.push(AnnotatedDiffLine {
                            kind: gitgpui_core::domain::DiffLineKind::Add,
                            text: format!("+{}", row.new.as_deref().unwrap_or("")).into(),
                            old_line: None,
                            new_line: row.new_line,
                        });
                        inline_word_highlights.push(None);
                    }
                    K::Remove => {
                        inline_rows.push(AnnotatedDiffLine {
                            kind: gitgpui_core::domain::DiffLineKind::Remove,
                            text: format!("-{}", row.old.as_deref().unwrap_or("")).into(),
                            old_line: row.old_line,
                            new_line: None,
                        });
                        inline_word_highlights.push(None);
                    }
                    K::Modify => {
                        let old = row.old.as_deref().unwrap_or("");
                        let new = row.new.as_deref().unwrap_or("");
                        let (old_ranges, new_ranges) = capped_word_diff_ranges(old, new);

                        inline_rows.push(AnnotatedDiffLine {
                            kind: gitgpui_core::domain::DiffLineKind::Remove,
                            text: format!("-{}", old).into(),
                            old_line: row.old_line,
                            new_line: None,
                        });
                        inline_word_highlights.push((!old_ranges.is_empty()).then_some(old_ranges));

                        inline_rows.push(AnnotatedDiffLine {
                            kind: gitgpui_core::domain::DiffLineKind::Add,
                            text: format!("+{}", new).into(),
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
            let old = file.old.as_ref().and_then(|bytes| {
                format.map(|format| Arc::new(gpui::Image::from_bytes(format, bytes.clone())))
            });
            let new = file.new.as_ref().and_then(|bytes| {
                format.map(|format| Arc::new(gpui::Image::from_bytes(format, bytes.clone())))
            });

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

    fn push_toast(&mut self, kind: zed::ToastKind, message: String, cx: &mut gpui::Context<Self>) {
        self.toast_host
            .update(cx, |host, cx| host.push_toast(kind, message, cx));
    }

    fn push_persistent_toast(
        &mut self,
        kind: zed::ToastKind,
        message: String,
        cx: &mut gpui::Context<Self>,
    ) -> u64 {
        self.toast_host
            .update(cx, |host, cx| host.push_persistent_toast(kind, message, cx))
    }

    fn update_toast_text(&mut self, id: u64, message: String, cx: &mut gpui::Context<Self>) {
        self.toast_host
            .update(cx, |host, cx| host.update_toast_text(id, message, cx));
    }

    fn remove_toast(&mut self, id: u64, cx: &mut gpui::Context<Self>) {
        self.toast_host
            .update(cx, |host, cx| host.remove_toast(id, cx));
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
                let (old_ranges, new_ranges) = capped_word_diff_ranges(old_text, new_text);
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

    fn conflict_nav_entries(&self) -> Vec<usize> {
        match self.conflict_resolver.diff_mode {
            ConflictDiffMode::Split => {
                diff_navigation::conflict_nav_entries_for_split(&self.conflict_resolver.diff_rows)
            }
            ConflictDiffMode::Inline => diff_navigation::conflict_nav_entries_for_inline(
                &self.conflict_resolver.inline_rows,
            ),
        }
    }

    fn conflict_jump_prev(&mut self) {
        let entries = self.conflict_nav_entries();
        if entries.is_empty() {
            return;
        }

        let current = self.conflict_resolver.nav_anchor.unwrap_or(0);
        let Some(target) = diff_navigation::diff_nav_prev_target(&entries, current) else {
            return;
        };

        match self.conflict_resolver.view_mode {
            ConflictResolverViewMode::TwoWayDiff => self
                .conflict_resolver_diff_scroll
                .scroll_to_item_strict(target, gpui::ScrollStrategy::Center),
            ConflictResolverViewMode::ThreeWay => {
                let line_ix = match self.conflict_resolver.diff_mode {
                    ConflictDiffMode::Split => self
                        .conflict_resolver
                        .diff_rows
                        .get(target)
                        .and_then(|r| r.old_line.or(r.new_line))
                        .and_then(|n| usize::try_from(n.saturating_sub(1)).ok())
                        .unwrap_or(0),
                    ConflictDiffMode::Inline => self
                        .conflict_resolver
                        .inline_rows
                        .get(target)
                        .and_then(|r| r.old_line.or(r.new_line))
                        .and_then(|n| usize::try_from(n.saturating_sub(1)).ok())
                        .unwrap_or(0),
                };
                self.conflict_resolver_diff_scroll
                    .scroll_to_item_strict(line_ix, gpui::ScrollStrategy::Center);
            }
        }
        self.conflict_resolver.nav_anchor = Some(target);
    }

    fn conflict_jump_next(&mut self) {
        let entries = self.conflict_nav_entries();
        if entries.is_empty() {
            return;
        }

        let current = self.conflict_resolver.nav_anchor.unwrap_or(0);
        let Some(target) = diff_navigation::diff_nav_next_target(&entries, current) else {
            return;
        };

        match self.conflict_resolver.view_mode {
            ConflictResolverViewMode::TwoWayDiff => self
                .conflict_resolver_diff_scroll
                .scroll_to_item_strict(target, gpui::ScrollStrategy::Center),
            ConflictResolverViewMode::ThreeWay => {
                let line_ix = match self.conflict_resolver.diff_mode {
                    ConflictDiffMode::Split => self
                        .conflict_resolver
                        .diff_rows
                        .get(target)
                        .and_then(|r| r.old_line.or(r.new_line))
                        .and_then(|n| usize::try_from(n.saturating_sub(1)).ok())
                        .unwrap_or(0),
                    ConflictDiffMode::Inline => self
                        .conflict_resolver
                        .inline_rows
                        .get(target)
                        .and_then(|r| r.old_line.or(r.new_line))
                        .and_then(|n| usize::try_from(n.saturating_sub(1)).ok())
                        .unwrap_or(0),
                };
                self.conflict_resolver_diff_scroll
                    .scroll_to_item_strict(line_ix, gpui::ScrollStrategy::Center);
            }
        }
        self.conflict_resolver.nav_anchor = Some(target);
    }

    fn diff_jump_prev(&mut self) {
        let entries = self.diff_nav_entries();
        if entries.is_empty() {
            return;
        }

        let current = self.diff_selection_anchor.unwrap_or(0);
        let Some(target) = diff_navigation::diff_nav_prev_target(&entries, current) else {
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
        let Some(target) = diff_navigation::diff_nav_next_target(&entries, current) else {
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

    #[cfg(test)]
    pub(crate) fn is_popover_open(&self, app: &App) -> bool {
        self.popover_host.read(app).is_open()
    }

    fn sync_conflict_resolver(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(repo_id) = self.active_repo_id() else {
            self.conflict_resolver = ConflictResolverUiState::default();
            return;
        };

        let Some(repo) = self.state.repos.iter().find(|r| r.id == repo_id) else {
            self.conflict_resolver = ConflictResolverUiState::default();
            return;
        };

        let Some(DiffTarget::WorkingTree { path, area }) = repo.diff_target.as_ref() else {
            self.conflict_resolver = ConflictResolverUiState::default();
            return;
        };
        if *area != DiffArea::Unstaged {
            self.conflict_resolver = ConflictResolverUiState::default();
            return;
        }

        let is_conflicted = match &repo.status {
            Loadable::Ready(status) => status.unstaged.iter().any(|e| {
                e.path == *path && e.kind == gitgpui_core::domain::FileStatusKind::Conflicted
            }),
            _ => false,
        };
        if !is_conflicted {
            self.conflict_resolver = ConflictResolverUiState::default();
            return;
        }

        let path = path.clone();

        let should_load = repo.conflict_file_path.as_ref() != Some(&path)
            && !matches!(repo.conflict_file, Loadable::Loading);
        if should_load {
            self.conflict_resolver = ConflictResolverUiState::default();
            let theme = self.theme;
            self.conflict_resolver_input.update(cx, |input, cx| {
                input.set_theme(theme, cx);
                input.set_text("", cx);
            });
            self.store.dispatch(Msg::LoadConflictFile { repo_id, path });
            return;
        }

        let Loadable::Ready(Some(file)) = &repo.conflict_file else {
            return;
        };
        if file.path != path {
            return;
        }

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        file.base.hash(&mut hasher);
        file.ours.hash(&mut hasher);
        file.theirs.hash(&mut hasher);
        file.current.hash(&mut hasher);
        let source_hash = hasher.finish();

        let needs_rebuild = self.conflict_resolver.repo_id != Some(repo_id)
            || self.conflict_resolver.path.as_ref() != Some(&path)
            || self.conflict_resolver.source_hash != Some(source_hash);

        if !needs_rebuild {
            return;
        }

        let (marker_segments, resolved) = if let Some(cur) = file.current.as_deref() {
            let segments = conflict_resolver::parse_conflict_markers(cur);
            if conflict_resolver::conflict_count(&segments) > 0 {
                let resolved = conflict_resolver::generate_resolved_text(&segments);
                (segments, resolved)
            } else {
                (Vec::new(), cur.to_string())
            }
        } else if let Some(ours) = file.ours.as_deref() {
            (Vec::new(), ours.to_string())
        } else if let Some(theirs) = file.theirs.as_deref() {
            (Vec::new(), theirs.to_string())
        } else {
            (Vec::new(), String::new())
        };
        let ours_text = file.ours.as_deref().unwrap_or("");
        let theirs_text = file.theirs.as_deref().unwrap_or("");
        let base_text = file.base.as_deref().unwrap_or("");
        let diff_rows = gitgpui_core::file_diff::side_by_side_rows(ours_text, theirs_text);
        let inline_rows = conflict_resolver::build_inline_rows(&diff_rows);

        fn split_lines_shared(text: &str) -> Vec<SharedString> {
            if text.is_empty() {
                return Vec::new();
            }
            let mut out =
                Vec::with_capacity(text.as_bytes().iter().filter(|&&b| b == b'\n').count() + 1);
            out.extend(text.lines().map(|line| line.to_string().into()));
            out
        }

        let three_way_base_lines = split_lines_shared(base_text);
        let three_way_ours_lines = split_lines_shared(ours_text);
        let three_way_theirs_lines = split_lines_shared(theirs_text);
        let three_way_len = three_way_base_lines
            .len()
            .max(three_way_ours_lines.len())
            .max(three_way_theirs_lines.len());

        let view_mode = if self.conflict_resolver.repo_id == Some(repo_id)
            && self.conflict_resolver.path.as_ref() == Some(&path)
        {
            self.conflict_resolver.view_mode
        } else if file.base.is_some() {
            ConflictResolverViewMode::ThreeWay
        } else {
            ConflictResolverViewMode::TwoWayDiff
        };

        let diff_mode = if self.conflict_resolver.repo_id == Some(repo_id)
            && self.conflict_resolver.path.as_ref() == Some(&path)
        {
            self.conflict_resolver.diff_mode
        } else {
            ConflictDiffMode::Split
        };
        let nav_anchor = if self.conflict_resolver.repo_id == Some(repo_id)
            && self.conflict_resolver.path.as_ref() == Some(&path)
        {
            self.conflict_resolver.nav_anchor
        } else {
            None
        };
        let active_conflict = if self.conflict_resolver.repo_id == Some(repo_id)
            && self.conflict_resolver.path.as_ref() == Some(&path)
        {
            let total = conflict_resolver::conflict_count(&marker_segments);
            if total == 0 {
                0
            } else {
                self.conflict_resolver.active_conflict.min(total - 1)
            }
        } else {
            0
        };

        let theme = self.theme;
        self.conflict_resolver_input.update(cx, |input, cx| {
            input.set_theme(theme, cx);
            input.set_text(resolved, cx);
        });

        self.conflict_resolver = ConflictResolverUiState {
            repo_id: Some(repo_id),
            path: Some(path),
            source_hash: Some(source_hash),
            current: file.current.clone(),
            marker_segments,
            active_conflict,
            view_mode,
            diff_rows,
            inline_rows,
            three_way_base_lines,
            three_way_ours_lines,
            three_way_theirs_lines,
            three_way_len,
            diff_mode,
            nav_anchor,
            split_selected: std::collections::BTreeSet::new(),
            inline_selected: std::collections::BTreeSet::new(),
        };
    }

    fn conflict_resolver_set_mode(&mut self, mode: ConflictDiffMode, cx: &mut gpui::Context<Self>) {
        if self.conflict_resolver.diff_mode == mode {
            return;
        }
        self.conflict_resolver.diff_mode = mode;
        self.conflict_resolver.nav_anchor = None;
        self.conflict_resolver.split_selected.clear();
        self.conflict_resolver.inline_selected.clear();
        cx.notify();
    }

    fn conflict_resolver_selection_is_empty(&self) -> bool {
        match self.conflict_resolver.diff_mode {
            ConflictDiffMode::Split => self.conflict_resolver.split_selected.is_empty(),
            ConflictDiffMode::Inline => self.conflict_resolver.inline_selected.is_empty(),
        }
    }

    fn conflict_resolver_clear_selection(&mut self, cx: &mut gpui::Context<Self>) {
        self.conflict_resolver.split_selected.clear();
        self.conflict_resolver.inline_selected.clear();
        cx.notify();
    }

    fn conflict_resolver_toggle_split_selected(
        &mut self,
        row_ix: usize,
        side: ConflictPickSide,
        cx: &mut gpui::Context<Self>,
    ) {
        self.conflict_resolver.nav_anchor = Some(row_ix);
        let key = (row_ix, side);
        if self.conflict_resolver.split_selected.contains(&key) {
            self.conflict_resolver.split_selected.remove(&key);
        } else {
            self.conflict_resolver.split_selected.insert(key);
        }
        cx.notify();
    }

    fn conflict_resolver_toggle_inline_selected(
        &mut self,
        ix: usize,
        cx: &mut gpui::Context<Self>,
    ) {
        self.conflict_resolver.nav_anchor = Some(ix);
        if self.conflict_resolver.inline_selected.contains(&ix) {
            self.conflict_resolver.inline_selected.remove(&ix);
        } else {
            self.conflict_resolver.inline_selected.insert(ix);
        }
        cx.notify();
    }

    fn conflict_resolver_append_selection_to_output(&mut self, cx: &mut gpui::Context<Self>) {
        let lines = match self.conflict_resolver.diff_mode {
            ConflictDiffMode::Split => conflict_resolver::collect_split_selection(
                &self.conflict_resolver.diff_rows,
                &self.conflict_resolver.split_selected,
            ),
            ConflictDiffMode::Inline => conflict_resolver::collect_inline_selection(
                &self.conflict_resolver.inline_rows,
                &self.conflict_resolver.inline_selected,
            ),
        };
        if lines.is_empty() {
            return;
        }

        let current = self
            .conflict_resolver_input
            .read_with(cx, |i, _| i.text().to_string());
        let next = conflict_resolver::append_lines_to_output(&current, &lines);
        let theme = self.theme;
        self.conflict_resolver_input.update(cx, |input, cx| {
            input.set_theme(theme, cx);
            input.set_text(next, cx);
        });
    }

    fn conflict_resolver_set_output(&mut self, text: String, cx: &mut gpui::Context<Self>) {
        let theme = self.theme;
        self.conflict_resolver_input.update(cx, |input, cx| {
            input.set_theme(theme, cx);
            input.set_text(text, cx);
        });
    }

    fn conflict_resolver_reset_output_from_markers(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(current) = self.conflict_resolver.current.as_deref() else {
            return;
        };
        let segments = conflict_resolver::parse_conflict_markers(current);
        if conflict_resolver::conflict_count(&segments) == 0 {
            return;
        }
        let resolved = conflict_resolver::generate_resolved_text(&segments);
        self.conflict_resolver_set_output(resolved, cx);
    }
}

impl Render for GitGpuiView {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        self.last_window_size = window.window_bounds().get_bounds().size;
        self.clamp_pane_widths_to_window();
        if self.last_window_size != self.ui_window_size_last_seen {
            self.ui_window_size_last_seen = self.last_window_size;
            self.schedule_ui_settings_persist(cx);
        }

        if let Some(repo_id) = self.pending_pull_reconcile_prompt.take() {
            if self.active_repo_id() == Some(repo_id) {
                self.open_popover_at(
                    PopoverKind::PullReconcilePrompt { repo_id },
                    self.last_mouse_pos,
                    window,
                    cx,
                );
            }
        }

        if let Some((repo_id, name)) = self.pending_force_delete_branch_prompt.take() {
            if self.active_repo_id() == Some(repo_id) {
                self.open_popover_at(
                    PopoverKind::ForceDeleteBranchConfirm { repo_id, name },
                    self.last_mouse_pos,
                    window,
                    cx,
                );
            }
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

        let mut body = div()
            .flex()
            .flex_col()
            .size_full()
            .text_color(theme.colors.text)
            .child(self.title_bar.clone())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(self.repo_tabs_bar.clone())
                    .child(self.open_repo_panel(cx))
                    .child(self.action_bar.clone())
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
                                    .child(self.sidebar_pane.clone()),
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
                                    .child(self.main_pane.clone()),
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
                                            .child(self.details_pane.clone()),
                                    ),
                            ),
                    ),
            );

        if let Some(repo_id) = self.active_repo_id()
            && let Some(repo) = self.active_repo()
            && let Some(err) = repo.last_error.as_ref()
        {
            self.error_banner_input.update(cx, |input, cx| {
                input.set_theme(theme, cx);
                input.set_text(err.clone(), cx);
                input.set_read_only(true, cx);
            });

            let dismiss = zed::Button::new("repo_error_banner_close", "✕")
                .style(zed::ButtonStyle::Transparent)
                .on_click(theme, cx, move |this, _e, _w, cx| {
                    this.store.dispatch(Msg::DismissRepoError { repo_id });
                    cx.notify();
                });

            body = body.child(
                div()
                    .relative()
                    .px_2()
                    .py_1()
                    .pr(px(40.0))
                    .bg(with_alpha(theme.colors.danger, 0.15))
                    .border_1()
                    .border_color(with_alpha(theme.colors.danger, 0.3))
                    .rounded(px(theme.radii.panel))
                    .child(
                        div()
                            .id("repo_error_banner_scroll")
                            .max_h(px(140.0))
                            .overflow_y_scroll()
                            .child(self.error_banner_input.clone()),
                    )
                    .child(div().absolute().top(px(6.0)).right(px(6.0)).child(dismiss)),
            );
        }

        let mut root = div()
            .size_full()
            .cursor(cursor)
            .text_color(theme.colors.text);
        root = root.relative();

        root = root.on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, window, cx| {
            this.last_mouse_pos = e.position;
            this.tooltip_host
                .update(cx, |tooltip, cx| tooltip.on_mouse_moved(e.position, cx));

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

        root = root.child(self.toast_host.clone());

        root = root.child(self.popover_host.clone());

        root = root.child(self.tooltip_host.clone());

        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitgpui_core::domain::{Branch, CommitId, RemoteBranch, RepoSpec, Upstream};
    use std::path::PathBuf;

    #[test]
    fn toast_total_lifetime_includes_fade_in_and_out() {
        let ttl = Duration::from_secs(6);
        assert_eq!(
            toast_total_lifetime(ttl),
            ttl + Duration::from_millis(TOAST_FADE_IN_MS + TOAST_FADE_OUT_MS)
        );
    }

    #[test]
    fn reconcile_status_multi_selection_prunes_missing_paths_and_anchors() {
        let a = PathBuf::from("a.txt");
        let b = PathBuf::from("b.txt");
        let c = PathBuf::from("c.txt");

        let status = RepoStatus {
            staged: vec![],
            unstaged: vec![FileStatus {
                path: a.clone(),
                kind: FileStatusKind::Modified,
                conflict: None,
            }],
        };

        let mut selection = StatusMultiSelection {
            unstaged: vec![a.clone(), b.clone()],
            unstaged_anchor: Some(b),
            staged: vec![c.clone()],
            staged_anchor: Some(c),
        };

        reconcile_status_multi_selection(&mut selection, &status);

        assert_eq!(selection.unstaged, vec![a]);
        assert!(selection.unstaged_anchor.is_none());
        assert!(selection.staged.is_empty());
        assert!(selection.staged_anchor.is_none());
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
                target: CommitId("b0".to_string()),
            },
            RemoteBranch {
                remote: "origin".to_string(),
                name: "a".to_string(),
                target: CommitId("a0".to_string()),
            },
            RemoteBranch {
                remote: "upstream".to_string(),
                name: "main".to_string(),
                target: CommitId("c0".to_string()),
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
            target: CommitId("deadbeef".to_string()),
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
}
