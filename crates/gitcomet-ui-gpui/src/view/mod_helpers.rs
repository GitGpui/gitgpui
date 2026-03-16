use super::*;

pub(super) fn toast_fade_in_duration() -> Duration {
    Duration::from_millis(TOAST_FADE_IN_MS)
}

pub(super) fn toast_fade_out_duration() -> Duration {
    Duration::from_millis(TOAST_FADE_OUT_MS)
}

pub(super) fn toast_total_lifetime(ttl: Duration) -> Duration {
    toast_fade_in_duration() + ttl + toast_fade_out_duration()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HistoryColResizeHandle {
    Branch,
    Graph,
    Author,
    Date,
    Sha,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct HistoryColResizeState {
    pub(super) handle: HistoryColResizeHandle,
    pub(super) start_x: Pixels,
    pub(super) start_branch: Pixels,
    pub(super) start_graph: Pixels,
    pub(super) start_author: Pixels,
    pub(super) start_date: Pixels,
    pub(super) start_sha: Pixels,
}

pub(super) struct ResizeDragGhost;

impl Render for ResizeDragGhost {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div().w(px(0.0)).h(px(0.0))
    }
}

pub(super) use ResizeDragGhost as HistoryColResizeDragGhost;

pub(super) fn should_hide_unified_diff_header_line(line: &AnnotatedDiffLine) -> bool {
    matches!(line.kind, gitcomet_core::domain::DiffLineKind::Header)
        && (line.text.starts_with("index ")
            || line.text.starts_with("--- ")
            || line.text.starts_with("+++ "))
}

pub(super) fn absolute_scroll_y(handle: &ScrollHandle) -> Pixels {
    let raw = handle.offset().y;
    if raw < px(0.0) { -raw } else { raw }
}

pub(super) fn scroll_is_near_bottom(handle: &ScrollHandle, threshold: Pixels) -> bool {
    let max_offset = handle.max_offset().height.max(px(0.0));
    if max_offset <= px(0.0) {
        return true;
    }

    let scroll_y = absolute_scroll_y(handle).max(px(0.0)).min(max_offset);
    (max_offset - scroll_y) <= threshold
}

pub(super) fn is_svg_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
}

pub(super) fn is_existing_directory(path: &std::path::Path) -> bool {
    std::fs::metadata(path).is_ok_and(|meta| meta.is_dir())
}

pub(super) fn is_existing_regular_file(path: &std::path::Path) -> bool {
    std::fs::metadata(path).is_ok_and(|meta| meta.is_file())
}

pub(super) fn should_bypass_text_file_preview_for_path(path: &std::path::Path) -> bool {
    let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
        return false;
    };
    ext.eq_ignore_ascii_case("png")
        || ext.eq_ignore_ascii_case("jpg")
        || ext.eq_ignore_ascii_case("jpeg")
        || ext.eq_ignore_ascii_case("webp")
        || ext.eq_ignore_ascii_case("ico")
        || ext.eq_ignore_ascii_case("svg")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DiffViewMode {
    Inline,
    Split,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum RenderedPreviewKind {
    Svg,
    Markdown,
}

impl RenderedPreviewKind {
    pub(super) fn rendered_label(self) -> &'static str {
        match self {
            Self::Svg => "Image",
            Self::Markdown => "Preview",
        }
    }

    pub(super) fn source_label(self) -> &'static str {
        match self {
            Self::Svg => "Code",
            Self::Markdown => "Text",
        }
    }

    pub(super) fn rendered_button_id(self) -> &'static str {
        match self {
            Self::Svg => "svg_diff_view_image",
            Self::Markdown => "markdown_diff_view_preview",
        }
    }

    pub(super) fn toggle_id(self) -> &'static str {
        match self {
            Self::Svg => "svg_diff_view_toggle",
            Self::Markdown => "markdown_diff_view_toggle",
        }
    }

    pub(super) fn source_button_id(self) -> &'static str {
        match self {
            Self::Svg => "svg_diff_view_code",
            Self::Markdown => "markdown_diff_view_text",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RenderedPreviewMode {
    Rendered,
    Source,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RenderedPreviewModes {
    pub(super) svg: RenderedPreviewMode,
    pub(super) markdown: RenderedPreviewMode,
}

impl Default for RenderedPreviewModes {
    fn default() -> Self {
        Self {
            svg: RenderedPreviewMode::Rendered,
            markdown: RenderedPreviewMode::Rendered,
        }
    }
}

impl RenderedPreviewModes {
    pub(super) fn get(self, kind: RenderedPreviewKind) -> RenderedPreviewMode {
        match kind {
            RenderedPreviewKind::Svg => self.svg,
            RenderedPreviewKind::Markdown => self.markdown,
        }
    }

    pub(super) fn set(&mut self, kind: RenderedPreviewKind, mode: RenderedPreviewMode) {
        match kind {
            RenderedPreviewKind::Svg => self.svg = mode,
            RenderedPreviewKind::Markdown => self.markdown = mode,
        }
    }
}

/// Preview mode for the conflict resolver merge-input pane.
///
/// When the conflicted file supports a rendered preview (for example, SVG or
/// markdown), the user can toggle between the normal text diff view and a
/// rendered preview of each conflict side.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum ConflictResolverPreviewMode {
    /// Normal text/diff view with syntax highlighting.
    #[default]
    Text,
    /// Rendered preview (image for SVG files, rendered rows for markdown).
    Preview,
}

pub(super) fn is_markdown_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "md" | "markdown" | "mdown" | "mkd" | "mkdn" | "mdwn"
            )
        })
}

pub(super) fn preview_path_rendered_kind(path: &std::path::Path) -> Option<RenderedPreviewKind> {
    if is_svg_path(path) {
        Some(RenderedPreviewKind::Svg)
    } else if is_markdown_path(path) {
        Some(RenderedPreviewKind::Markdown)
    } else {
        None
    }
}

pub(super) fn diff_target_rendered_preview_kind(
    target: Option<&DiffTarget>,
) -> Option<RenderedPreviewKind> {
    let path = match target? {
        DiffTarget::WorkingTree { path, .. } => path.as_path(),
        DiffTarget::Commit {
            path: Some(path), ..
        } => path.as_path(),
        _ => return None,
    };
    preview_path_rendered_kind(path)
}

pub(super) fn main_diff_rendered_preview_toggle_kind(
    wants_file_diff: bool,
    is_file_preview: bool,
    preview_kind: Option<RenderedPreviewKind>,
) -> Option<RenderedPreviewKind> {
    match preview_kind? {
        RenderedPreviewKind::Svg if wants_file_diff => Some(RenderedPreviewKind::Svg),
        RenderedPreviewKind::Markdown if wants_file_diff || is_file_preview => {
            Some(RenderedPreviewKind::Markdown)
        }
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PaneResizeHandle {
    Sidebar,
    Details,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PaneResizeState {
    pub(super) handle: PaneResizeHandle,
    pub(super) start_x: Pixels,
    pub(super) start_sidebar: Pixels,
    pub(super) start_details: Pixels,
}

pub(super) use ResizeDragGhost as PaneResizeDragGhost;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DiffSplitResizeHandle {
    Divider,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DiffSplitResizeState {
    pub(super) handle: DiffSplitResizeHandle,
    pub(super) start_x: Pixels,
    pub(super) start_ratio: f32,
}

pub(super) use ResizeDragGhost as DiffSplitResizeDragGhost;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConflictVSplitResizeHandle {
    Divider,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ConflictVSplitResizeState {
    pub(super) start_y: Pixels,
    pub(super) start_ratio: f32,
}

pub(super) use ResizeDragGhost as ConflictVSplitResizeDragGhost;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConflictHSplitResizeHandle {
    First,
    Second,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ConflictHSplitResizeState {
    pub(super) handle: ConflictHSplitResizeHandle,
    pub(super) start_x: Pixels,
    pub(super) start_ratios: [f32; 2],
}

pub(super) use ResizeDragGhost as ConflictHSplitResizeDragGhost;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConflictDiffSplitResizeHandle {
    Divider,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ConflictDiffSplitResizeState {
    pub(super) start_x: Pixels,
    pub(super) start_ratio: f32,
}

pub(super) use ResizeDragGhost as ConflictDiffSplitResizeDragGhost;

#[cfg(test)]
mod resize_drag_ghost_tests {
    use super::{
        ConflictDiffSplitResizeDragGhost, ConflictHSplitResizeDragGhost,
        ConflictVSplitResizeDragGhost, DiffSplitResizeDragGhost, HistoryColResizeDragGhost,
        PaneResizeDragGhost, ResizeDragGhost,
    };
    use std::any::TypeId;

    #[test]
    fn all_resize_drag_ghost_aliases_use_shared_type() {
        let shared = TypeId::of::<ResizeDragGhost>();

        assert_eq!(TypeId::of::<HistoryColResizeDragGhost>(), shared);
        assert_eq!(TypeId::of::<PaneResizeDragGhost>(), shared);
        assert_eq!(TypeId::of::<DiffSplitResizeDragGhost>(), shared);
        assert_eq!(TypeId::of::<ConflictVSplitResizeDragGhost>(), shared);
        assert_eq!(TypeId::of::<ConflictHSplitResizeDragGhost>(), shared);
        assert_eq!(TypeId::of::<ConflictDiffSplitResizeDragGhost>(), shared);
    }
}

#[cfg(test)]
mod directory_path_tests {
    use super::is_existing_directory;

    #[test]
    fn detects_existing_directory_paths() {
        let tmp = std::env::temp_dir().join(format!("gitcomet_is_dir_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp directory");

        assert!(is_existing_directory(&tmp));
        assert!(!is_existing_directory(&tmp.join("missing")));

        std::fs::remove_dir_all(&tmp).expect("cleanup temp directory");
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(super) enum DiffTextRegion {
    Inline,
    SplitLeft,
    SplitRight,
}

impl DiffTextRegion {
    pub(super) fn order(self) -> u8 {
        match self {
            DiffTextRegion::Inline | DiffTextRegion::SplitLeft => 0,
            DiffTextRegion::SplitRight => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DiffTextPos {
    pub(super) visible_ix: usize,
    pub(super) region: DiffTextRegion,
    pub(super) offset: usize,
}

impl DiffTextPos {
    pub(super) fn cmp_key(self) -> (usize, u8, usize) {
        (self.visible_ix, self.region.order(), self.offset)
    }
}

pub(super) struct DiffTextHitbox {
    pub(super) bounds: Bounds<Pixels>,
    pub(super) layout_key: u64,
    pub(super) text_len: usize,
}

#[derive(Clone)]
pub(super) struct ToastState {
    pub(super) id: u64,
    pub(super) kind: components::ToastKind,
    pub(super) input: Entity<components::TextInput>,
    pub(super) is_code_message: bool,
    pub(super) action_url: Option<String>,
    pub(super) action_label: Option<String>,
    pub(super) ttl: Option<Duration>,
}

#[derive(Clone, Debug)]
pub(super) struct CommitDetailsDelayState {
    pub(super) repo_id: RepoId,
    pub(super) commit_id: CommitId,
    pub(super) show_loading: bool,
}

#[derive(Clone, Debug, Default)]
pub(super) struct StatusMultiSelection {
    pub(super) unstaged: Vec<std::path::PathBuf>,
    pub(super) unstaged_anchor: Option<std::path::PathBuf>,
    pub(super) staged: Vec<std::path::PathBuf>,
    pub(super) staged_anchor: Option<std::path::PathBuf>,
}

pub(super) fn reconcile_status_multi_selection(
    selection: &mut StatusMultiSelection,
    status: &RepoStatus,
) {
    let mut unstaged_paths: HashSet<&std::path::Path> =
        HashSet::with_capacity_and_hasher(status.unstaged.len(), Default::default());
    for entry in &status.unstaged {
        unstaged_paths.insert(entry.path.as_path());
    }

    selection
        .unstaged
        .retain(|p| unstaged_paths.contains(&p.as_path()));
    if selection
        .unstaged_anchor
        .as_ref()
        .is_some_and(|a| !unstaged_paths.contains(&a.as_path()))
    {
        selection.unstaged_anchor = None;
    }

    let mut staged_paths: HashSet<&std::path::Path> =
        HashSet::with_capacity_and_hasher(status.staged.len(), Default::default());
    for entry in &status.staged {
        staged_paths.insert(entry.path.as_path());
    }

    selection
        .staged
        .retain(|p| staged_paths.contains(&p.as_path()));
    if selection
        .staged_anchor
        .as_ref()
        .is_some_and(|a| !staged_paths.contains(&a.as_path()))
    {
        selection.staged_anchor = None;
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum ThreeWayColumn {
    Base,
    Ours,
    Theirs,
}

impl ThreeWayColumn {
    pub(super) const ALL: [ThreeWayColumn; 3] = [
        ThreeWayColumn::Base,
        ThreeWayColumn::Ours,
        ThreeWayColumn::Theirs,
    ];
}

#[derive(Clone, Debug, Default)]
pub(super) struct ThreeWaySides<T> {
    pub(super) base: T,
    pub(super) ours: T,
    pub(super) theirs: T,
}

impl<T> std::ops::Index<ThreeWayColumn> for ThreeWaySides<T> {
    type Output = T;
    fn index(&self, side: ThreeWayColumn) -> &T {
        match side {
            ThreeWayColumn::Base => &self.base,
            ThreeWayColumn::Ours => &self.ours,
            ThreeWayColumn::Theirs => &self.theirs,
        }
    }
}

impl<T> std::ops::IndexMut<ThreeWayColumn> for ThreeWaySides<T> {
    fn index_mut(&mut self, side: ThreeWayColumn) -> &mut T {
        match side {
            ThreeWayColumn::Base => &mut self.base,
            ThreeWayColumn::Ours => &mut self.ours,
            ThreeWayColumn::Theirs => &mut self.theirs,
        }
    }
}

pub(super) type LoadableMarkdownDoc =
    Loadable<Arc<crate::view::markdown_preview::MarkdownPreviewDocument>>;

pub(super) type LoadableMarkdownDiff =
    Loadable<Arc<crate::view::markdown_preview::MarkdownPreviewDiff>>;

#[derive(Clone, Debug)]
pub(super) struct ConflictResolverMarkdownPreviewState {
    pub(super) source_hash: Option<u64>,
    pub(super) documents: ThreeWaySides<LoadableMarkdownDoc>,
}

impl Default for ConflictResolverMarkdownPreviewState {
    fn default() -> Self {
        Self {
            source_hash: None,
            documents: ThreeWaySides {
                base: Loadable::NotLoaded,
                ours: Loadable::NotLoaded,
                theirs: Loadable::NotLoaded,
            },
        }
    }
}

impl ConflictResolverMarkdownPreviewState {
    pub(super) fn document(&self, side: ThreeWayColumn) -> &LoadableMarkdownDoc {
        &self.documents[side]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ResolvedOutputConflictMarker {
    pub(super) conflict_ix: usize,
    pub(super) range_start: usize,
    pub(super) range_end: usize,
    pub(super) is_start: bool,
    pub(super) is_end: bool,
    pub(super) unresolved: bool,
}

/// Resolved-output outline metadata: per-line provenance, conflict markers, and source index.
/// Shared between visible state (`ConflictResolverUiState`) and incremental-recompute stash.
#[derive(Clone, Debug, Default)]
pub(super) struct ResolvedOutlineData {
    /// Per-line provenance metadata.
    pub(super) meta: Vec<conflict_resolver::ResolvedLineMeta>,
    /// Per-line conflict marker metadata for gutter markers.
    pub(super) markers: Vec<Option<ResolvedOutputConflictMarker>>,
    /// Source line keys currently represented in resolved output (for dedupe/plus-icon).
    pub(super) sources_index: HashSet<conflict_resolver::SourceLineKey>,
}

/// Mode-specific state for eager (small-file) conflict resolution.
///
/// All fields here are only populated when the conflict is small enough
/// for full eager materialization.
#[derive(Clone, Debug, Default)]
pub(super) struct EagerConflictState {
    pub(super) diff_rows: Vec<FileDiffRow>,
    pub(super) inline_rows: Vec<ConflictInlineRow>,
    pub(super) three_way_line_conflict_map: ThreeWaySides<Vec<Option<usize>>>,
    pub(super) three_way_visible_map: Vec<conflict_resolver::ThreeWayVisibleItem>,
    pub(super) diff_row_conflict_map: Vec<Option<usize>>,
    pub(super) inline_row_conflict_map: Vec<Option<usize>>,
    pub(super) diff_visible_row_indices: Vec<usize>,
    pub(super) inline_visible_row_indices: Vec<usize>,
}

/// Mode-specific state for streamed (giant-file) conflict resolution.
///
/// Uses lazy paged access and span-based projections instead of
/// eagerly materializing all rows.
#[derive(Clone, Debug)]
pub(super) struct StreamedConflictState {
    pub(super) three_way_visible_projection: conflict_resolver::ThreeWayVisibleProjection,
    pub(super) split_row_index: conflict_resolver::ConflictSplitRowIndex,
    pub(super) two_way_split_projection: conflict_resolver::TwoWaySplitProjection,
}

impl Default for StreamedConflictState {
    fn default() -> Self {
        Self {
            three_way_visible_projection: conflict_resolver::ThreeWayVisibleProjection::default(),
            split_row_index: conflict_resolver::ConflictSplitRowIndex::default(),
            two_way_split_projection: conflict_resolver::TwoWaySplitProjection::default(),
        }
    }
}

/// Discriminated union of eager vs streamed conflict mode state.
///
/// Replaces the previous pattern of 13 mode-specific fields on
/// `ConflictResolverUiState` guarded by runtime `rendering_mode` checks.
/// The enum makes invalid states unrepresentable: eager-only fields
/// cannot exist in streamed mode and vice versa.
#[derive(Clone, Debug)]
pub(super) enum ConflictModeState {
    Eager(EagerConflictState),
    Streamed(StreamedConflictState),
}

impl Default for ConflictModeState {
    fn default() -> Self {
        Self::Eager(EagerConflictState::default())
    }
}

#[derive(Clone, Debug)]
pub(super) struct ConflictResolverUiState {
    pub(super) repo_id: Option<RepoId>,
    pub(super) path: Option<std::path::PathBuf>,
    pub(super) conflict_syntax_language: Option<rows::DiffSyntaxLanguage>,
    pub(super) source_hash: Option<u64>,
    pub(super) current: Option<std::sync::Arc<str>>,
    pub(super) marker_segments: Vec<conflict_resolver::ConflictSegment>,
    /// Mapping from visible block index to `ConflictSession` region index.
    pub(super) conflict_region_indices: Vec<usize>,
    pub(super) active_conflict: usize,
    pub(super) hovered_conflict: Option<(usize, ThreeWayColumn)>,
    /// Discriminated mode state — replaces the old `rendering_mode` field
    /// plus 13 mode-specific fields.
    pub(super) mode_state: ConflictModeState,
    pub(super) view_mode: ConflictResolverViewMode,
    /// Backing text for each three-way source side.
    pub(super) three_way_text: ThreeWaySides<SharedString>,
    /// Per-side line start offsets into `three_way_text`.
    pub(super) three_way_line_starts: ThreeWaySides<Arc<[usize]>>,
    pub(super) three_way_len: usize,
    /// Per-side conflict ranges for O(log n) binary-search lookups and
    /// conflict-to-visible mapping. The ours ranges remain the anchor space for
    /// legacy three-way visible projections.
    pub(super) three_way_conflict_ranges: ThreeWaySides<Vec<Range<usize>>>,
    pub(super) conflict_has_base: Vec<bool>,
    pub(super) three_way_word_highlights: ThreeWaySides<conflict_resolver::WordHighlights>,
    pub(super) diff_word_highlights_split: conflict_resolver::TwoWayWordHighlights,
    pub(super) diff_mode: ConflictDiffMode,
    pub(super) nav_anchor: Option<usize>,
    pub(super) hide_resolved: bool,
    /// True when any conflict side contains non-UTF8 binary data.
    pub(super) is_binary_conflict: bool,
    /// Byte sizes of the three conflict sides (for binary UI display).
    pub(super) binary_side_sizes: [Option<usize>; 3],
    /// The resolver strategy for the current conflict (set during sync).
    pub(super) strategy: Option<gitcomet_core::conflict_session::ConflictResolverStrategy>,
    /// The conflict kind for the current file (set during sync).
    pub(super) conflict_kind: Option<gitcomet_core::domain::FileConflictKind>,
    /// Last autosolve trace summary shown in resolver UI.
    pub(super) last_autosolve_summary: Option<SharedString>,
    /// Tracks the last-seen `conflict_rev` from state so we can detect
    /// state-side session changes (e.g. hide-resolved, bulk picks, autosolve)
    /// that don't change the underlying file content.
    pub(super) conflict_rev: u64,
    /// Sequence token for debounced resolved-output outline recompute tasks.
    pub(super) resolver_pending_recompute_seq: u64,
    /// Resolved-output outline metadata (provenance, conflict markers, source index).
    pub(super) resolved_outline: ResolvedOutlineData,
    /// Cached rendered markdown previews for the merge-input sides.
    pub(super) markdown_preview: ConflictResolverMarkdownPreviewState,
    /// Preview mode for the merge-input pane (Text vs rendered Preview).
    pub(super) resolver_preview_mode: ConflictResolverPreviewMode,
}

impl Default for ConflictResolverUiState {
    fn default() -> Self {
        Self {
            repo_id: None,
            path: None,
            conflict_syntax_language: None,
            source_hash: None,
            current: None,
            marker_segments: Vec::new(),
            conflict_region_indices: Vec::new(),
            active_conflict: 0,
            hovered_conflict: None,
            mode_state: ConflictModeState::default(),
            view_mode: ConflictResolverViewMode::TwoWayDiff,
            three_way_text: ThreeWaySides::default(),
            three_way_line_starts: ThreeWaySides::default(),
            three_way_len: 0,
            three_way_conflict_ranges: ThreeWaySides::default(),
            conflict_has_base: Vec::new(),
            three_way_word_highlights: ThreeWaySides::default(),
            diff_word_highlights_split: Vec::new(),
            diff_mode: ConflictDiffMode::Split,
            nav_anchor: None,
            hide_resolved: false,
            is_binary_conflict: false,
            binary_side_sizes: [None; 3],
            strategy: None,
            conflict_kind: None,
            last_autosolve_summary: None,
            conflict_rev: 0,
            resolver_pending_recompute_seq: 0,
            resolved_outline: ResolvedOutlineData::default(),
            markdown_preview: ConflictResolverMarkdownPreviewState::default(),
            resolver_preview_mode: ConflictResolverPreviewMode::default(),
        }
    }
}

fn indexed_line_count(text: &str, line_starts: &[usize]) -> usize {
    if text.is_empty() {
        0
    } else {
        line_starts.len()
    }
}

fn indexed_line_text<'a>(text: &'a str, line_starts: &[usize], line_ix: usize) -> Option<&'a str> {
    if text.is_empty() {
        return None;
    }
    let text_len = text.len();
    let start = line_starts.get(line_ix).copied().unwrap_or(text_len);
    if start >= text_len {
        return None;
    }
    let mut end = line_starts
        .get(line_ix.saturating_add(1))
        .copied()
        .unwrap_or(text_len)
        .min(text_len);
    if end > start && text.as_bytes().get(end.saturating_sub(1)) == Some(&b'\n') {
        end = end.saturating_sub(1);
    }
    Some(text.get(start..end).unwrap_or(""))
}

impl ConflictResolverUiState {
    // ----- Mode accessors -----

    /// True if in streamed large-file mode.
    pub(super) fn is_streamed_large_file(&self) -> bool {
        matches!(&self.mode_state, ConflictModeState::Streamed(_))
    }

    /// Return the rendering mode enum (for tracing / external APIs that expect it).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn rendering_mode(&self) -> conflict_resolver::ConflictRenderingMode {
        match &self.mode_state {
            ConflictModeState::Eager(_) => conflict_resolver::ConflictRenderingMode::EagerSmallFile,
            ConflictModeState::Streamed(_) => {
                conflict_resolver::ConflictRenderingMode::StreamedLargeFile
            }
        }
    }

    /// Access the eager mode state. Panics if not in eager mode.
    #[track_caller]
    pub(super) fn eager(&self) -> &EagerConflictState {
        match &self.mode_state {
            ConflictModeState::Eager(s) => s,
            ConflictModeState::Streamed(_) => {
                panic!("expected eager mode, got streamed")
            }
        }
    }

    /// Mutably access the eager mode state. Panics if not in eager mode.
    #[cfg_attr(not(test), allow(dead_code))]
    #[track_caller]
    pub(super) fn eager_mut(&mut self) -> &mut EagerConflictState {
        match &mut self.mode_state {
            ConflictModeState::Eager(s) => s,
            ConflictModeState::Streamed(_) => {
                panic!("expected eager mode, got streamed")
            }
        }
    }

    /// Access the streamed mode state. Panics if not in streamed mode.
    #[cfg_attr(not(test), allow(dead_code))]
    #[track_caller]
    pub(super) fn streamed(&self) -> &StreamedConflictState {
        match &self.mode_state {
            ConflictModeState::Streamed(s) => s,
            ConflictModeState::Eager(_) => {
                panic!("expected streamed mode, got eager")
            }
        }
    }

    /// Mutably access the streamed mode state. Panics if not in streamed mode.
    #[cfg_attr(not(test), allow(dead_code))]
    #[track_caller]
    pub(super) fn streamed_mut(&mut self) -> &mut StreamedConflictState {
        match &mut self.mode_state {
            ConflictModeState::Streamed(s) => s,
            ConflictModeState::Eager(_) => {
                panic!("expected streamed mode, got eager")
            }
        }
    }

    // ----- Field-level accessors for external code -----
    // These return empty slices / None for the "wrong" mode, matching
    // the old behavior where those fields were empty/None in the other mode.

    pub(super) fn diff_rows(&self) -> &[FileDiffRow] {
        match &self.mode_state {
            ConflictModeState::Eager(s) => &s.diff_rows,
            ConflictModeState::Streamed(_) => &[],
        }
    }

    pub(super) fn inline_rows(&self) -> &[ConflictInlineRow] {
        match &self.mode_state {
            ConflictModeState::Eager(s) => &s.inline_rows,
            ConflictModeState::Streamed(_) => &[],
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn diff_row_conflict_map(&self) -> &[Option<usize>] {
        match &self.mode_state {
            ConflictModeState::Eager(s) => &s.diff_row_conflict_map,
            ConflictModeState::Streamed(_) => &[],
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn diff_visible_row_indices(&self) -> &[usize] {
        match &self.mode_state {
            ConflictModeState::Eager(s) => &s.diff_visible_row_indices,
            ConflictModeState::Streamed(_) => &[],
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn inline_visible_row_indices(&self) -> &[usize] {
        match &self.mode_state {
            ConflictModeState::Eager(s) => &s.inline_visible_row_indices,
            ConflictModeState::Streamed(_) => &[],
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn three_way_visible_map(&self) -> &[conflict_resolver::ThreeWayVisibleItem] {
        match &self.mode_state {
            ConflictModeState::Eager(s) => &s.three_way_visible_map,
            ConflictModeState::Streamed(_) => &[],
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn three_way_line_conflict_map(&self) -> &ThreeWaySides<Vec<Option<usize>>> {
        static EMPTY: std::sync::LazyLock<ThreeWaySides<Vec<Option<usize>>>> =
            std::sync::LazyLock::new(ThreeWaySides::default);
        match &self.mode_state {
            ConflictModeState::Eager(s) => &s.three_way_line_conflict_map,
            ConflictModeState::Streamed(_) => &EMPTY,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn split_row_index(&self) -> Option<&conflict_resolver::ConflictSplitRowIndex> {
        match &self.mode_state {
            ConflictModeState::Streamed(s) => Some(&s.split_row_index),
            ConflictModeState::Eager(_) => None,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn two_way_split_projection(
        &self,
    ) -> Option<&conflict_resolver::TwoWaySplitProjection> {
        match &self.mode_state {
            ConflictModeState::Streamed(s) => Some(&s.two_way_split_projection),
            ConflictModeState::Eager(_) => None,
        }
    }

    #[allow(dead_code)]
    pub(super) fn three_way_visible_projection(
        &self,
    ) -> &conflict_resolver::ThreeWayVisibleProjection {
        static EMPTY: std::sync::LazyLock<conflict_resolver::ThreeWayVisibleProjection> =
            std::sync::LazyLock::new(conflict_resolver::ThreeWayVisibleProjection::default);
        match &self.mode_state {
            ConflictModeState::Streamed(s) => &s.three_way_visible_projection,
            ConflictModeState::Eager(_) => &EMPTY,
        }
    }

    #[track_caller]
    pub(super) fn debug_assert_rendering_mode_invariants(&self) {
        // The enum makes most invariants structural. The one remaining
        // runtime invariant is that streamed mode must stay in split diff mode.
        if self.is_streamed_large_file() {
            debug_assert_eq!(
                self.diff_mode,
                ConflictDiffMode::Split,
                "streamed large-file mode must stay in split diff mode"
            );
        }
    }

    pub(super) fn three_way_line_count(&self, side: ThreeWayColumn) -> usize {
        indexed_line_count(
            &self.three_way_text[side],
            &self.three_way_line_starts[side],
        )
    }

    pub(super) fn three_way_line_text(&self, side: ThreeWayColumn, line_ix: usize) -> Option<&str> {
        indexed_line_text(
            &self.three_way_text[side],
            &self.three_way_line_starts[side],
            line_ix,
        )
    }

    pub(super) fn three_way_has_line(&self, side: ThreeWayColumn, line_ix: usize) -> bool {
        self.three_way_line_text(side, line_ix).is_some()
    }

    /// Return source-pane text for a conflict pick choice at a global line index.
    ///
    /// This reads from the indexed merge-input texts directly so callers do not
    /// depend on eager diff rows or streamed page generation.
    pub(super) fn source_line_text_for_choice(
        &self,
        choice: conflict_resolver::ConflictChoice,
        line_ix: usize,
    ) -> Option<&str> {
        match choice {
            conflict_resolver::ConflictChoice::Base
                if self.view_mode == ConflictResolverViewMode::ThreeWay =>
            {
                self.three_way_line_text(ThreeWayColumn::Base, line_ix)
            }
            conflict_resolver::ConflictChoice::Ours => {
                self.three_way_line_text(ThreeWayColumn::Ours, line_ix)
            }
            conflict_resolver::ConflictChoice::Theirs => {
                self.three_way_line_text(ThreeWayColumn::Theirs, line_ix)
            }
            conflict_resolver::ConflictChoice::Base | conflict_resolver::ConflictChoice::Both => {
                None
            }
        }
    }

    /// Look up the visible item at `visible_ix`, dispatching between the eager
    /// map (small files) and the span-based projection (giant files).
    pub(super) fn three_way_visible_item(
        &self,
        visible_ix: usize,
    ) -> Option<conflict_resolver::ThreeWayVisibleItem> {
        match &self.mode_state {
            ConflictModeState::Streamed(s) => s.three_way_visible_projection.get(visible_ix),
            ConflictModeState::Eager(s) => s.three_way_visible_map.get(visible_ix).copied(),
        }
    }

    /// Number of visible rows in the three-way view.
    pub(super) fn three_way_visible_len(&self) -> usize {
        match &self.mode_state {
            ConflictModeState::Streamed(s) => s.three_way_visible_projection.len(),
            ConflictModeState::Eager(s) => s.three_way_visible_map.len(),
        }
    }

    /// Look up the conflict index for a given line on a given side.
    /// Uses binary search on per-side ranges in giant mode, O(1) array lookup otherwise.
    pub(super) fn conflict_index_for_side_line(
        &self,
        side: ThreeWayColumn,
        line_ix: usize,
    ) -> Option<usize> {
        match &self.mode_state {
            ConflictModeState::Streamed(_) => {
                let ranges = &self.three_way_conflict_ranges[side];
                conflict_resolver::conflict_index_for_line(ranges, line_ix)
            }
            ConflictModeState::Eager(s) => s.three_way_line_conflict_map[side]
                .get(line_ix)
                .copied()
                .flatten(),
        }
    }

    /// Find the visible index for a conflict range, using the projection in giant mode.
    pub(super) fn visible_index_for_conflict(&self, range_ix: usize) -> Option<usize> {
        match &self.mode_state {
            ConflictModeState::Streamed(s) => {
                s.three_way_visible_projection.visible_index_for_conflict(
                    &self.three_way_conflict_ranges[ThreeWayColumn::Ours],
                    range_ix,
                )
            }
            ConflictModeState::Eager(s) => conflict_resolver::visible_index_for_conflict(
                &s.three_way_visible_map,
                &self.three_way_conflict_ranges[ThreeWayColumn::Ours],
                range_ix,
            ),
        }
    }

    // ----- Two-way split dispatch (giant vs eager) -----

    /// Number of visible rows in the two-way split view.
    pub(super) fn two_way_split_visible_len(&self) -> usize {
        match &self.mode_state {
            ConflictModeState::Streamed(s) => s.two_way_split_projection.visible_len(),
            ConflictModeState::Eager(s) => s.diff_visible_row_indices.len(),
        }
    }

    /// Number of visible rows in the two-way inline view.
    pub(super) fn two_way_inline_visible_len(&self) -> usize {
        // Giant mode forces split; inline is only used in eager mode.
        match &self.mode_state {
            ConflictModeState::Eager(s) => s.inline_visible_row_indices.len(),
            ConflictModeState::Streamed(_) => 0,
        }
    }

    /// Retrieve a split row for the given visible index, dispatching between
    /// the paged index (giant) and the eager `diff_rows` array (small).
    ///
    /// Returns `(source_row_ix, row, conflict_ix)`.
    pub(super) fn two_way_split_visible_row(
        &self,
        visible_ix: usize,
    ) -> Option<(usize, gitcomet_core::file_diff::FileDiffRow, Option<usize>)> {
        match &self.mode_state {
            ConflictModeState::Streamed(s) => {
                let (source_ix, conflict_ix) = s.two_way_split_projection.get(visible_ix)?;
                let row = s.split_row_index.row_at(&self.marker_segments, source_ix)?;
                Some((source_ix, row, conflict_ix))
            }
            ConflictModeState::Eager(s) => {
                let &row_ix = s.diff_visible_row_indices.get(visible_ix)?;
                let row = s.diff_rows.get(row_ix)?.clone();
                let conflict_ix = s.diff_row_conflict_map.get(row_ix).copied().flatten();
                Some((row_ix, row, conflict_ix))
            }
        }
    }

    /// Retrieve a split row by source row index (not visible index).
    pub(super) fn two_way_split_row_by_source(
        &self,
        row_ix: usize,
    ) -> Option<gitcomet_core::file_diff::FileDiffRow> {
        match &self.mode_state {
            ConflictModeState::Streamed(s) => {
                s.split_row_index.row_at(&self.marker_segments, row_ix)
            }
            ConflictModeState::Eager(s) => s.diff_rows.get(row_ix).cloned(),
        }
    }

    /// Retrieve an inline row for the given visible index.
    ///
    /// Returns `(source_row_ix, row, conflict_ix)`. Streamed mode returns
    /// `None` because inline diff is disabled for giant files.
    pub(super) fn two_way_inline_visible_row(
        &self,
        visible_ix: usize,
    ) -> Option<(usize, ConflictInlineRow, Option<usize>)> {
        match &self.mode_state {
            ConflictModeState::Eager(s) => {
                let &row_ix = s.inline_visible_row_indices.get(visible_ix)?;
                let row = s.inline_rows.get(row_ix)?.clone();
                let conflict_ix = s.inline_row_conflict_map.get(row_ix).copied().flatten();
                Some((row_ix, row, conflict_ix))
            }
            ConflictModeState::Streamed(_) => None,
        }
    }

    /// Retrieve an inline row by source row index (not visible index).
    ///
    /// Streamed mode returns `None` because inline diff is disabled for giant
    /// files.
    pub(super) fn two_way_inline_row_by_source(&self, row_ix: usize) -> Option<ConflictInlineRow> {
        match &self.mode_state {
            ConflictModeState::Eager(s) => s.inline_rows.get(row_ix).cloned(),
            ConflictModeState::Streamed(_) => None,
        }
    }

    /// Find the first visible index for a conflict in two-way split view.
    pub(super) fn two_way_split_visible_ix_for_conflict(
        &self,
        conflict_ix: usize,
    ) -> Option<usize> {
        match &self.mode_state {
            ConflictModeState::Streamed(s) => s
                .two_way_split_projection
                .visible_index_for_conflict(conflict_ix),
            ConflictModeState::Eager(s) => conflict_resolver::visible_index_for_two_way_conflict(
                &s.diff_row_conflict_map,
                &s.diff_visible_row_indices,
                conflict_ix,
            ),
        }
    }

    /// Map a two-way split visible index back to its conflict index.
    pub(super) fn two_way_split_conflict_ix_for_visible(&self, visible_ix: usize) -> Option<usize> {
        match &self.mode_state {
            ConflictModeState::Streamed(s) => s
                .two_way_split_projection
                .get(visible_ix)
                .and_then(|(_, ci)| ci),
            ConflictModeState::Eager(s) => {
                conflict_resolver::two_way_conflict_index_for_visible_row(
                    &s.diff_row_conflict_map,
                    &s.diff_visible_row_indices,
                    visible_ix,
                )
            }
        }
    }

    /// Build unresolved conflict navigation entries for two-way split view.
    pub(super) fn two_way_split_nav_entries(&self) -> Vec<usize> {
        match &self.mode_state {
            ConflictModeState::Streamed(s) => {
                conflict_resolver::unresolved_conflict_indices(&self.marker_segments)
                    .into_iter()
                    .filter_map(|ci| s.two_way_split_projection.visible_index_for_conflict(ci))
                    .collect()
            }
            ConflictModeState::Eager(s) => {
                conflict_resolver::unresolved_visible_nav_entries_for_two_way(
                    &self.marker_segments,
                    &s.diff_row_conflict_map,
                    &s.diff_visible_row_indices,
                )
            }
        }
    }

    // ----- Unified two-way dispatch (Split + Inline, giant vs eager) -----

    /// Build unresolved conflict navigation entries for the current two-way
    /// diff mode (split or inline). Handles both giant and eager rendering modes.
    pub(super) fn two_way_nav_entries(&self) -> Vec<usize> {
        match self.diff_mode {
            conflict_resolver::ConflictDiffMode::Split => self.two_way_split_nav_entries(),
            conflict_resolver::ConflictDiffMode::Inline => {
                let eager = self.eager();
                conflict_resolver::unresolved_visible_nav_entries_for_two_way(
                    &self.marker_segments,
                    &eager.inline_row_conflict_map,
                    &eager.inline_visible_row_indices,
                )
            }
        }
    }

    /// Map a two-way visible index to its conflict index, dispatching on diff mode
    /// (split/inline) and rendering mode (giant/eager).
    pub(super) fn two_way_conflict_ix_for_visible(&self, visible_ix: usize) -> Option<usize> {
        match self.diff_mode {
            conflict_resolver::ConflictDiffMode::Split => {
                self.two_way_split_conflict_ix_for_visible(visible_ix)
            }
            conflict_resolver::ConflictDiffMode::Inline => {
                let eager = self.eager();
                conflict_resolver::two_way_conflict_index_for_visible_row(
                    &eager.inline_row_conflict_map,
                    &eager.inline_visible_row_indices,
                    visible_ix,
                )
            }
        }
    }

    /// Find the first visible index for a conflict in the current two-way diff
    /// mode (split/inline). Handles both giant and eager rendering modes.
    pub(super) fn two_way_visible_ix_for_conflict(&self, conflict_ix: usize) -> Option<usize> {
        match self.diff_mode {
            conflict_resolver::ConflictDiffMode::Split => {
                self.two_way_split_visible_ix_for_conflict(conflict_ix)
            }
            conflict_resolver::ConflictDiffMode::Inline => {
                let eager = self.eager();
                conflict_resolver::visible_index_for_two_way_conflict(
                    &eager.inline_row_conflict_map,
                    &eager.inline_visible_row_indices,
                    conflict_ix,
                )
            }
        }
    }

    /// Return (diff_row_count, inline_row_count) for trace recording.
    pub(super) fn two_way_row_counts(&self) -> (usize, usize) {
        match &self.mode_state {
            ConflictModeState::Streamed(s) => (s.split_row_index.total_rows(), 0),
            ConflictModeState::Eager(s) => (s.diff_rows.len(), s.inline_rows.len()),
        }
    }

    /// Pre-computed word highlights for a source row in the two-way split view.
    /// Returns `None` in giant mode (word highlights are computed on-the-fly
    /// via `compute_word_highlights_for_row` at render time instead).
    pub(super) fn two_way_split_word_highlight(
        &self,
        row_ix: usize,
    ) -> Option<&(Vec<std::ops::Range<usize>>, Vec<std::ops::Range<usize>>)> {
        match &self.mode_state {
            ConflictModeState::Streamed(_) => None,
            ConflictModeState::Eager(_) => self
                .diff_word_highlights_split
                .get(row_ix)
                .and_then(|o| o.as_ref()),
        }
    }

    /// Rebuild three-way visible state (conflict maps + visible map/projection)
    /// from current marker segments and line counts.
    pub(super) fn rebuild_three_way_visible_state(&mut self) {
        let include_line_maps = matches!(self.mode_state, ConflictModeState::Eager(_));
        let build_maps = if include_line_maps {
            conflict_resolver::build_three_way_conflict_maps
        } else {
            conflict_resolver::build_three_way_conflict_maps_without_line_maps
        };
        let maps = build_maps(
            &self.marker_segments,
            self.three_way_line_count(ThreeWayColumn::Base),
            self.three_way_line_count(ThreeWayColumn::Ours),
            self.three_way_line_count(ThreeWayColumn::Theirs),
        );
        self.apply_three_way_conflict_maps(maps);
        match &mut self.mode_state {
            ConflictModeState::Streamed(s) => {
                s.three_way_visible_projection =
                    conflict_resolver::build_three_way_visible_projection(
                        self.three_way_len,
                        &self.three_way_conflict_ranges[ThreeWayColumn::Ours],
                        &self.marker_segments,
                        self.hide_resolved,
                    );
            }
            ConflictModeState::Eager(s) => {
                s.three_way_visible_map = conflict_resolver::build_three_way_visible_map(
                    self.three_way_len,
                    &self.three_way_conflict_ranges[ThreeWayColumn::Ours],
                    &self.marker_segments,
                    self.hide_resolved,
                );
            }
        }
    }

    /// Rebuild two-way visible state from current marker segments.
    /// In streamed giant mode: rebuilds split row index and projection.
    /// In eager mode: rebuilds conflict maps and visible indices from existing diff rows.
    pub(super) fn rebuild_two_way_visible_state(&mut self) {
        if let ConflictModeState::Streamed(s) = &mut self.mode_state {
            s.split_row_index = conflict_resolver::ConflictSplitRowIndex::new(
                &self.marker_segments,
                conflict_resolver::BLOCK_LOCAL_DIFF_CONTEXT_LINES,
            );
        }
        self.rebuild_two_way_visible_projections();
    }

    /// Rebuild two-way visible projections and indices from already-set row data.
    /// Assumes `split_row_index` (streamed) or `diff_rows`/`inline_rows` (eager) are
    /// already populated. Use `rebuild_two_way_visible_state` to also rebuild the
    /// underlying row data.
    pub(super) fn rebuild_two_way_visible_projections(&mut self) {
        match &mut self.mode_state {
            ConflictModeState::Streamed(s) => {
                s.two_way_split_projection = conflict_resolver::TwoWaySplitProjection::new(
                    &s.split_row_index,
                    &self.marker_segments,
                    self.hide_resolved,
                );
            }
            ConflictModeState::Eager(s) => {
                let (split_map, inline_map) = conflict_resolver::map_two_way_rows_to_conflicts(
                    &self.marker_segments,
                    &s.diff_rows,
                    &s.inline_rows,
                );
                s.diff_row_conflict_map = split_map;
                s.inline_row_conflict_map = inline_map;
                s.diff_visible_row_indices = conflict_resolver::build_two_way_visible_indices(
                    &s.diff_row_conflict_map,
                    &self.marker_segments,
                    self.hide_resolved,
                );
                s.inline_visible_row_indices = conflict_resolver::build_two_way_visible_indices(
                    &s.inline_row_conflict_map,
                    &self.marker_segments,
                    self.hide_resolved,
                );
            }
        }
        self.debug_assert_rendering_mode_invariants();
    }

    /// Apply three-way conflict maps to state fields.
    pub(super) fn apply_three_way_conflict_maps(
        &mut self,
        maps: conflict_resolver::ThreeWayConflictMaps,
    ) {
        let [base_ranges, ours_ranges, theirs_ranges] = maps.conflict_ranges;
        self.three_way_conflict_ranges = ThreeWaySides {
            base: base_ranges,
            ours: ours_ranges,
            theirs: theirs_ranges,
        };
        if let ConflictModeState::Eager(s) = &mut self.mode_state {
            let [base_maps, ours_maps, theirs_maps] = maps.line_conflict_maps;
            s.three_way_line_conflict_map = ThreeWaySides {
                base: base_maps,
                ours: ours_maps,
                theirs: theirs_maps,
            };
        }
        self.conflict_has_base = maps.conflict_has_base;
    }
}

#[cfg(test)]
mod conflict_resolver_ui_state_tests {
    use super::{
        ConflictModeState, ConflictResolverUiState, Loadable, ThreeWayColumn, ThreeWaySides,
    };

    #[test]
    fn default_groups_three_way_side_fields() {
        let state = ConflictResolverUiState::default();

        assert!(state.three_way_text.base.is_empty());
        assert!(state.three_way_text.ours.is_empty());
        assert!(state.three_way_text.theirs.is_empty());
        assert!(!state.is_streamed_large_file());
        assert!(state.three_way_line_starts.base.is_empty());
        assert!(state.three_way_line_starts.ours.is_empty());
        assert!(state.three_way_line_starts.theirs.is_empty());

        assert!(state.three_way_line_conflict_map().base.is_empty());
        assert!(state.three_way_line_conflict_map().ours.is_empty());
        assert!(state.three_way_line_conflict_map().theirs.is_empty());

        assert!(state.three_way_word_highlights.base.is_empty());
        assert!(state.three_way_word_highlights.ours.is_empty());
        assert!(state.three_way_word_highlights.theirs.is_empty());

        assert!(matches!(
            state.markdown_preview.documents.base,
            Loadable::NotLoaded
        ));
        assert!(matches!(
            state.markdown_preview.documents.ours,
            Loadable::NotLoaded
        ));
        assert!(matches!(
            state.markdown_preview.documents.theirs,
            Loadable::NotLoaded
        ));
    }

    #[test]
    fn three_way_sides_keep_each_column_separate() {
        let mut sides = ThreeWaySides {
            base: vec![1],
            ours: vec![2],
            theirs: vec![3],
        };

        sides.base.push(10);
        sides.ours.push(20);
        sides.theirs.push(30);

        assert_eq!(sides.base, vec![1, 10]);
        assert_eq!(sides.ours, vec![2, 20]);
        assert_eq!(sides.theirs, vec![3, 30]);
    }

    #[test]
    fn three_way_sides_index_by_column() {
        let mut sides = ThreeWaySides {
            base: 10,
            ours: 20,
            theirs: 30,
        };

        assert_eq!(sides[ThreeWayColumn::Base], 10);
        assert_eq!(sides[ThreeWayColumn::Ours], 20);
        assert_eq!(sides[ThreeWayColumn::Theirs], 30);

        sides[ThreeWayColumn::Ours] = 42;
        assert_eq!(sides.ours, 42);
        assert_eq!(sides[ThreeWayColumn::Ours], 42);
    }

    #[test]
    fn source_line_text_for_choice_reads_two_way_inputs_from_indexed_text() {
        use super::conflict_resolver::{ConflictChoice, ConflictResolverViewMode};

        let mut state = ConflictResolverUiState::default();
        state.view_mode = ConflictResolverViewMode::TwoWayDiff;
        state.three_way_text.ours = "o0\no1\n".into();
        state.three_way_text.theirs = "t0\nt1\n".into();
        state.three_way_line_starts.ours = vec![0, 3].into();
        state.three_way_line_starts.theirs = vec![0, 3].into();

        assert_eq!(
            state.source_line_text_for_choice(ConflictChoice::Ours, 1),
            Some("o1")
        );
        assert_eq!(
            state.source_line_text_for_choice(ConflictChoice::Theirs, 0),
            Some("t0")
        );
        assert_eq!(
            state.source_line_text_for_choice(ConflictChoice::Base, 0),
            None
        );
        assert_eq!(
            state.source_line_text_for_choice(ConflictChoice::Both, 0),
            None
        );
    }

    #[test]
    fn source_line_text_for_choice_reads_base_only_in_three_way_mode() {
        use super::conflict_resolver::{ConflictChoice, ConflictResolverViewMode};

        let mut state = ConflictResolverUiState::default();
        state.view_mode = ConflictResolverViewMode::ThreeWay;
        state.three_way_text.base = "b0\nb1\n".into();
        state.three_way_text.ours = "o0\no1\n".into();
        state.three_way_text.theirs = "t0\nt1\n".into();
        state.three_way_line_starts.base = vec![0, 3].into();
        state.three_way_line_starts.ours = vec![0, 3].into();
        state.three_way_line_starts.theirs = vec![0, 3].into();

        assert_eq!(
            state.source_line_text_for_choice(ConflictChoice::Base, 1),
            Some("b1")
        );
        assert_eq!(
            state.source_line_text_for_choice(ConflictChoice::Ours, 0),
            Some("o0")
        );
        assert_eq!(
            state.source_line_text_for_choice(ConflictChoice::Theirs, 1),
            Some("t1")
        );
    }

    #[test]
    fn apply_three_way_conflict_maps_distributes_all_fields() {
        let mut state = ConflictResolverUiState::default();
        let maps = super::conflict_resolver::ThreeWayConflictMaps {
            conflict_ranges: [vec![0..3], vec![0..5], vec![0..4]],
            line_conflict_maps: [
                vec![Some(0), Some(0), Some(0)],
                vec![Some(0); 5],
                vec![Some(0); 4],
            ],
            conflict_has_base: vec![true],
        };
        state.apply_three_way_conflict_maps(maps.clone());

        assert_eq!(
            state.three_way_conflict_ranges.base,
            maps.conflict_ranges[0]
        );
        assert_eq!(
            state.three_way_conflict_ranges.ours,
            maps.conflict_ranges[1]
        );
        assert_eq!(
            state.three_way_conflict_ranges.theirs,
            maps.conflict_ranges[2]
        );
        // In eager mode, per-line conflict maps are populated.
        let eager = state.eager();
        assert_eq!(
            eager.three_way_line_conflict_map.base,
            maps.line_conflict_maps[0]
        );
        assert_eq!(
            eager.three_way_line_conflict_map.ours,
            maps.line_conflict_maps[1]
        );
        assert_eq!(
            eager.three_way_line_conflict_map.theirs,
            maps.line_conflict_maps[2]
        );
        assert_eq!(state.conflict_has_base, maps.conflict_has_base);
    }

    #[test]
    fn eager_mode_dispatch_uses_map_not_projection() {
        use super::conflict_resolver::ThreeWayVisibleItem;

        let mut state = ConflictResolverUiState::default();
        // Default is eager mode.
        state.eager_mut().three_way_visible_map = vec![
            ThreeWayVisibleItem::Line(0),
            ThreeWayVisibleItem::Line(1),
            ThreeWayVisibleItem::Line(2),
        ];

        // Dispatch uses the map.
        assert_eq!(state.three_way_visible_len(), 3);
        assert_eq!(
            state.three_way_visible_item(1),
            Some(ThreeWayVisibleItem::Line(1))
        );
    }

    #[test]
    fn rebuild_three_way_visible_state_eager_mode() {
        use super::conflict_resolver::{ConflictBlock, ConflictChoice, ConflictSegment};

        let mut state = ConflictResolverUiState::default();
        // Default is eager mode.
        state.marker_segments = vec![ConflictSegment::Block(ConflictBlock {
            base: None,
            ours: "a\nb\n".into(),
            theirs: "c\n".into(),
            choice: ConflictChoice::Ours,
            resolved: false,
        })];
        state.three_way_text.ours = "a\nb\n".into();
        state.three_way_text.theirs = "c\n".into();
        state.three_way_line_starts.ours = vec![0, 2].into();
        state.three_way_line_starts.theirs = vec![0].into();
        state.three_way_len = 2;

        state.rebuild_three_way_visible_state();

        // Eager: visible_map populated.
        assert!(!state.eager().three_way_visible_map.is_empty());
        assert_eq!(
            state.three_way_visible_len(),
            state.eager().three_way_visible_map.len()
        );
        // Conflict ranges populated.
        assert!(!state.three_way_conflict_ranges.ours.is_empty());
    }

    #[test]
    fn rebuild_three_way_visible_state_streamed_mode() {
        use super::StreamedConflictState;
        use super::conflict_resolver::{ConflictBlock, ConflictChoice, ConflictSegment};

        let mut state = ConflictResolverUiState::default();
        state.mode_state = ConflictModeState::Streamed(StreamedConflictState::default());
        state.marker_segments = vec![ConflictSegment::Block(ConflictBlock {
            base: None,
            ours: "a\nb\n".into(),
            theirs: "c\n".into(),
            choice: ConflictChoice::Ours,
            resolved: false,
        })];
        state.three_way_text.ours = "a\nb\n".into();
        state.three_way_text.theirs = "c\n".into();
        state.three_way_line_starts.ours = vec![0, 2].into();
        state.three_way_line_starts.theirs = vec![0].into();
        state.three_way_len = 2;

        state.rebuild_three_way_visible_state();

        // Streamed: projection populated.
        assert!(state.streamed().three_way_visible_projection.len() > 0);
        assert_eq!(
            state.three_way_visible_len(),
            state.streamed().three_way_visible_projection.len()
        );
        // Conflict ranges populated.
        assert!(!state.three_way_conflict_ranges.ours.is_empty());
    }

    #[test]
    fn streamed_conflict_index_for_side_line_uses_grouped_side_ranges() {
        use super::StreamedConflictState;

        let mut state = ConflictResolverUiState::default();
        state.mode_state = ConflictModeState::Streamed(StreamedConflictState::default());
        state.three_way_conflict_ranges = ThreeWaySides {
            base: vec![0..1, 4..6],
            ours: vec![2..5, 8..9],
            theirs: vec![1..3, 7..10],
        };

        assert_eq!(
            state.conflict_index_for_side_line(ThreeWayColumn::Base, 4),
            Some(1)
        );
        assert_eq!(
            state.conflict_index_for_side_line(ThreeWayColumn::Ours, 3),
            Some(0)
        );
        assert_eq!(
            state.conflict_index_for_side_line(ThreeWayColumn::Theirs, 8),
            Some(1)
        );
        assert_eq!(
            state.conflict_index_for_side_line(ThreeWayColumn::Base, 2),
            None
        );
    }

    #[test]
    fn streamed_mode_dispatch_uses_projection_not_map() {
        use super::StreamedConflictState;
        use super::conflict_resolver;

        let mut state = ConflictResolverUiState::default();
        state.mode_state = ConflictModeState::Streamed(StreamedConflictState::default());

        // Build a small projection with 5 lines.
        let segments = vec![conflict_resolver::ConflictSegment::Block(
            conflict_resolver::ConflictBlock {
                base: None,
                ours: "a\nb\nc\nd\ne\n".into(),
                theirs: "a\nb\nc\nd\ne\n".into(),
                choice: conflict_resolver::ConflictChoice::Ours,
                resolved: false,
            },
        )];
        let ranges = vec![0..5];
        state.streamed_mut().three_way_visible_projection =
            conflict_resolver::build_three_way_visible_projection(5, &ranges, &segments, false);

        // Dispatch uses the projection.
        assert_eq!(state.three_way_visible_len(), 5);
        assert_eq!(
            state.three_way_visible_item(2),
            Some(conflict_resolver::ThreeWayVisibleItem::Line(2))
        );
    }

    /// Build a minimal streamed state with one conflict block for dispatch tests.
    fn streamed_state_with_one_conflict() -> ConflictResolverUiState {
        use super::StreamedConflictState;
        use super::conflict_resolver::{
            ConflictBlock, ConflictChoice, ConflictSegment, ConflictSplitRowIndex,
            TwoWaySplitProjection,
        };

        let segments = vec![
            ConflictSegment::Text("ctx\n".into()),
            ConflictSegment::Block(ConflictBlock {
                base: None,
                ours: "a\nb\n".into(),
                theirs: "c\n".into(),
                choice: ConflictChoice::Ours,
                resolved: false,
            }),
        ];
        let index = ConflictSplitRowIndex::new(&segments, 3);
        let projection = TwoWaySplitProjection::new(&index, &segments, false);

        let mut state = ConflictResolverUiState::default();
        state.marker_segments = segments;
        state.mode_state = ConflictModeState::Streamed(StreamedConflictState {
            split_row_index: index,
            two_way_split_projection: projection,
            ..StreamedConflictState::default()
        });
        state
    }

    /// Build a minimal eager state with one conflict block for dispatch tests.
    fn eager_state_with_one_conflict() -> ConflictResolverUiState {
        use super::conflict_resolver::{ConflictBlock, ConflictChoice, ConflictSegment};
        use gitcomet_core::file_diff::{FileDiffRow, FileDiffRowKind};

        let segments = vec![
            ConflictSegment::Text("ctx\n".into()),
            ConflictSegment::Block(ConflictBlock {
                base: None,
                ours: "a\nb\n".into(),
                theirs: "c\n".into(),
                choice: ConflictChoice::Ours,
                resolved: false,
            }),
        ];

        let mut state = ConflictResolverUiState::default();
        state.marker_segments = segments;
        // Simulate 3 diff rows: context, ours-line-a, ours-line-b
        // Row 1 and 2 belong to conflict 0.
        let eager = state.eager_mut();
        eager.diff_rows = vec![
            FileDiffRow {
                kind: FileDiffRowKind::Context,
                old_line: Some(1),
                new_line: Some(1),
                old: Some("ctx".into()),
                new: Some("ctx".into()),
                eof_newline: None,
            },
            FileDiffRow {
                kind: FileDiffRowKind::Remove,
                old_line: Some(2),
                new_line: None,
                old: Some("a".into()),
                new: None,
                eof_newline: None,
            },
            FileDiffRow {
                kind: FileDiffRowKind::Remove,
                old_line: Some(3),
                new_line: None,
                old: Some("b".into()),
                new: None,
                eof_newline: None,
            },
        ];
        eager.diff_row_conflict_map = vec![None, Some(0), Some(0)];
        eager.diff_visible_row_indices = vec![0, 1, 2];
        eager.inline_rows = vec![];
        state
    }

    #[test]
    fn two_way_row_counts_dispatch() {
        let eager = eager_state_with_one_conflict();
        assert_eq!(eager.two_way_row_counts(), (3, 0));

        let streamed = streamed_state_with_one_conflict();
        let (diff_count, inline_count) = streamed.two_way_row_counts();
        assert!(diff_count > 0);
        assert_eq!(inline_count, 0);
    }

    #[test]
    fn two_way_split_conflict_ix_for_visible_dispatch() {
        let eager = eager_state_with_one_conflict();
        // visible_ix 0 is context (no conflict), 1 is conflict 0
        assert_eq!(eager.two_way_split_conflict_ix_for_visible(0), None);
        assert_eq!(eager.two_way_split_conflict_ix_for_visible(1), Some(0));

        let streamed = streamed_state_with_one_conflict();
        // Find a visible index that maps to conflict 0
        let vis_len = streamed.two_way_split_visible_len();
        let mut found_conflict = false;
        for ix in 0..vis_len {
            if streamed.two_way_split_conflict_ix_for_visible(ix) == Some(0) {
                found_conflict = true;
                break;
            }
        }
        assert!(
            found_conflict,
            "streamed mode should map some visible row to conflict 0"
        );
    }

    #[test]
    fn two_way_split_visible_row_dispatch() {
        let mut eager = eager_state_with_one_conflict();
        eager.eager_mut().diff_visible_row_indices = vec![1, 2];

        let (source_ix, row, conflict_ix) = eager
            .two_way_split_visible_row(0)
            .expect("eager visible row should map through visible indices");
        assert_eq!(source_ix, 1);
        assert_eq!(row.old.as_deref(), Some("a"));
        assert_eq!(row.new.as_deref(), None);
        assert_eq!(conflict_ix, Some(0));

        let streamed = streamed_state_with_one_conflict();
        let visible_ix = streamed
            .two_way_visible_ix_for_conflict(0)
            .expect("streamed visible row should exist for the unresolved conflict");
        let (source_ix, row, conflict_ix) = streamed
            .two_way_split_visible_row(visible_ix)
            .expect("streamed visible row should resolve through the projection");
        assert_eq!(conflict_ix, Some(0));
        assert!(
            row.old.is_some() || row.new.is_some(),
            "streamed visible row should expose real source text",
        );
        assert!(
            source_ix < streamed.two_way_row_counts().0,
            "streamed source row should stay within the split-row index",
        );
    }

    #[test]
    fn two_way_split_nav_entries_dispatch() {
        let eager = eager_state_with_one_conflict();
        let entries = eager.two_way_split_nav_entries();
        // One unresolved conflict → one nav entry
        assert_eq!(entries.len(), 1);

        let streamed = streamed_state_with_one_conflict();
        let entries = streamed.two_way_split_nav_entries();
        assert_eq!(entries.len(), 1);
    }

    /// Build a minimal eager state with inline data for unified two-way dispatch tests.
    fn eager_state_with_inline_data() -> ConflictResolverUiState {
        use super::conflict_resolver::{ConflictInlineRow, ConflictPickSide};
        use gitcomet_core::domain::DiffLineKind;

        let mut state = eager_state_with_one_conflict();
        // Add inline rows mirroring the split rows: context, ours-a, ours-b
        let eager = state.eager_mut();
        eager.inline_rows = vec![
            ConflictInlineRow {
                side: ConflictPickSide::Ours,
                kind: DiffLineKind::Context,
                old_line: Some(1),
                new_line: Some(1),
                content: "ctx".into(),
            },
            ConflictInlineRow {
                side: ConflictPickSide::Ours,
                kind: DiffLineKind::Remove,
                old_line: Some(2),
                new_line: None,
                content: "a".into(),
            },
            ConflictInlineRow {
                side: ConflictPickSide::Ours,
                kind: DiffLineKind::Remove,
                old_line: Some(3),
                new_line: None,
                content: "b".into(),
            },
        ];
        eager.inline_row_conflict_map = vec![None, Some(0), Some(0)];
        eager.inline_visible_row_indices = vec![0, 1, 2];
        state
    }

    #[test]
    fn two_way_nav_entries_dispatch() {
        use super::conflict_resolver::ConflictDiffMode;

        // Split mode (default) — delegates to two_way_split_nav_entries()
        let eager = eager_state_with_one_conflict();
        assert_eq!(eager.diff_mode, ConflictDiffMode::Split);
        let entries = eager.two_way_nav_entries();
        assert_eq!(entries.len(), 1);

        // Inline mode
        let mut eager_inline = eager_state_with_inline_data();
        eager_inline.diff_mode = ConflictDiffMode::Inline;
        let entries = eager_inline.two_way_nav_entries();
        assert_eq!(entries.len(), 1);

        // Streamed mode always uses Split
        let streamed = streamed_state_with_one_conflict();
        let entries = streamed.two_way_nav_entries();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn two_way_inline_visible_row_dispatch() {
        let mut eager = eager_state_with_inline_data();
        eager.eager_mut().inline_visible_row_indices = vec![1, 2];

        let (source_ix, row, conflict_ix) = eager
            .two_way_inline_visible_row(0)
            .expect("inline visible row should map through visible indices");
        assert_eq!(source_ix, 1);
        assert_eq!(row.content, "a");
        assert_eq!(conflict_ix, Some(0));

        let streamed = streamed_state_with_one_conflict();
        assert!(
            streamed.two_way_inline_visible_row(0).is_none(),
            "streamed mode should not expose inline rows",
        );
    }

    #[test]
    fn two_way_inline_row_by_source_dispatch() {
        let eager = eager_state_with_inline_data();
        let row = eager
            .two_way_inline_row_by_source(2)
            .expect("eager inline row should be accessible by source index");
        assert_eq!(row.content, "b");

        let streamed = streamed_state_with_one_conflict();
        assert!(
            streamed.two_way_inline_row_by_source(0).is_none(),
            "streamed mode should not expose inline rows by source index",
        );
    }

    #[test]
    fn two_way_conflict_ix_for_visible_dispatch() {
        use super::conflict_resolver::ConflictDiffMode;

        // Split mode
        let eager = eager_state_with_one_conflict();
        assert_eq!(eager.two_way_conflict_ix_for_visible(0), None); // context
        assert_eq!(eager.two_way_conflict_ix_for_visible(1), Some(0)); // conflict

        // Inline mode
        let mut eager_inline = eager_state_with_inline_data();
        eager_inline.diff_mode = ConflictDiffMode::Inline;
        assert_eq!(eager_inline.two_way_conflict_ix_for_visible(0), None);
        assert_eq!(eager_inline.two_way_conflict_ix_for_visible(1), Some(0));

        // Streamed mode
        let streamed = streamed_state_with_one_conflict();
        let vis_len = streamed.two_way_split_visible_len();
        let mut found = false;
        for ix in 0..vis_len {
            if streamed.two_way_conflict_ix_for_visible(ix) == Some(0) {
                found = true;
                break;
            }
        }
        assert!(found);
    }

    #[test]
    fn two_way_visible_ix_for_conflict_dispatch() {
        use super::conflict_resolver::ConflictDiffMode;

        // Split mode
        let eager = eager_state_with_one_conflict();
        let vis = eager.two_way_visible_ix_for_conflict(0);
        assert!(vis.is_some());
        assert_eq!(eager.two_way_visible_ix_for_conflict(99), None);

        // Inline mode
        let mut eager_inline = eager_state_with_inline_data();
        eager_inline.diff_mode = ConflictDiffMode::Inline;
        let vis = eager_inline.two_way_visible_ix_for_conflict(0);
        assert!(vis.is_some());
        assert_eq!(eager_inline.two_way_visible_ix_for_conflict(99), None);

        // Streamed mode
        let streamed = streamed_state_with_one_conflict();
        let vis = streamed.two_way_visible_ix_for_conflict(0);
        assert!(vis.is_some());
        assert_eq!(streamed.two_way_visible_ix_for_conflict(99), None);
    }

    #[test]
    fn mode_state_enum_prevents_mixed_state() {
        // The enum makes it structurally impossible to have eager fields in streamed mode
        // or vice versa. This test verifies the accessor behavior at the boundary.
        use super::StreamedConflictState;

        let eager_state = ConflictResolverUiState::default();
        assert!(!eager_state.is_streamed_large_file());
        assert!(eager_state.diff_rows().is_empty());
        assert!(eager_state.split_row_index().is_none());

        let mut streamed_state = ConflictResolverUiState::default();
        streamed_state.mode_state = ConflictModeState::Streamed(StreamedConflictState::default());
        assert!(streamed_state.is_streamed_large_file());
        assert!(streamed_state.diff_rows().is_empty()); // returns empty slice, not panic
        assert!(streamed_state.split_row_index().is_some());
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) enum ResolverPickTarget {
    /// Append a specific line from the 3-way resolver pane.
    ThreeWayLine {
        line_ix: usize,
        choice: conflict_resolver::ConflictChoice,
    },
    /// Append a specific line from the 2-way split resolver pane.
    TwoWaySplitLine {
        row_ix: usize,
        side: conflict_resolver::ConflictPickSide,
    },
    /// Append a specific line from the 2-way inline resolver pane.
    TwoWayInlineLine { row_ix: usize },
    /// Pick a full conflict chunk for the requested side.
    Chunk {
        conflict_ix: usize,
        choice: conflict_resolver::ConflictChoice,
        /// Optional resolved-output line that initiated this pick.
        /// When present, chunk pick scopes to the marker chunk at this line.
        output_line_ix: Option<usize>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum PopoverKind {
    RepoPicker,
    BranchPicker,
    CreateBranch,
    CheckoutRemoteBranchPrompt {
        repo_id: RepoId,
        remote: String,
        branch: String,
    },
    StashPrompt,
    StashDropConfirm {
        repo_id: RepoId,
        index: usize,
        message: String,
    },
    StashMenu {
        repo_id: RepoId,
        index: usize,
        message: String,
    },
    CloneRepo,
    Settings,
    SettingsThemeMenu,
    SettingsDateFormatMenu,
    SettingsTimezoneMenu,
    OpenSourceLicenses,
    ResetPrompt {
        repo_id: RepoId,
        target: String,
        mode: ResetMode,
    },
    CreateTagPrompt {
        repo_id: RepoId,
        target: String,
    },
    Repo {
        repo_id: RepoId,
        kind: RepoPopoverKind,
    },
    FileHistory {
        repo_id: RepoId,
        path: std::path::PathBuf,
    },
    PushSetUpstreamPrompt {
        repo_id: RepoId,
        remote: String,
    },
    ForcePushConfirm {
        repo_id: RepoId,
    },
    MergeAbortConfirm {
        repo_id: RepoId,
    },
    ConflictSaveStageConfirm {
        repo_id: RepoId,
        path: std::path::PathBuf,
        has_conflict_markers: bool,
        unresolved_blocks: usize,
    },
    ForceDeleteBranchConfirm {
        repo_id: RepoId,
        name: String,
    },
    ForceRemoveWorktreeConfirm {
        repo_id: RepoId,
        path: std::path::PathBuf,
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
    ConflictResolverInputRowMenu {
        line_label: SharedString,
        line_target: ResolverPickTarget,
        chunk_label: SharedString,
        chunk_target: ResolverPickTarget,
    },
    ConflictResolverChunkMenu {
        conflict_ix: usize,
        has_base: bool,
        is_three_way: bool,
        selected_choices: Vec<conflict_resolver::ConflictChoice>,
        output_line_ix: Option<usize>,
    },
    ConflictResolverOutputMenu {
        cursor_line: usize,
        selected_text: Option<String>,
        has_source_a: bool,
        has_source_b: bool,
        has_source_c: bool,
        is_three_way: bool,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RepoPopoverKind {
    Remote(RemotePopoverKind),
    Worktree(WorktreePopoverKind),
    Submodule(SubmodulePopoverKind),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RemotePopoverKind {
    AddPrompt,
    EditUrlPrompt { name: String, kind: RemoteUrlKind },
    RemoveConfirm { name: String },
    Menu { name: String },
    DeleteBranchConfirm { remote: String, branch: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum WorktreePopoverKind {
    SectionMenu,
    Menu { path: std::path::PathBuf },
    AddPrompt,
    OpenPicker,
    RemovePicker,
    RemoveConfirm { path: std::path::PathBuf },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SubmodulePopoverKind {
    SectionMenu,
    Menu { path: std::path::PathBuf },
    AddPrompt,
    OpenPicker,
    RemovePicker,
    RemoveConfirm { path: std::path::PathBuf },
}

impl PopoverKind {
    pub(super) fn remote(repo_id: RepoId, kind: RemotePopoverKind) -> Self {
        Self::Repo {
            repo_id,
            kind: RepoPopoverKind::Remote(kind),
        }
    }

    pub(super) fn worktree(repo_id: RepoId, kind: WorktreePopoverKind) -> Self {
        Self::Repo {
            repo_id,
            kind: RepoPopoverKind::Worktree(kind),
        }
    }

    pub(super) fn submodule(repo_id: RepoId, kind: SubmodulePopoverKind) -> Self {
        Self::Repo {
            repo_id,
            kind: RepoPopoverKind::Submodule(kind),
        }
    }
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RemoteRow {
    Header(String),
    Branch { remote: String, name: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DiffClickKind {
    Line,
    HunkHeader,
    FileHeader,
}

#[derive(Clone, Debug)]
pub(super) enum PatchSplitRow {
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GitCometViewMode {
    #[default]
    Normal,
    #[allow(dead_code)]
    FocusedMergetool,
}

#[derive(Clone, Debug, Default)]
pub struct GitCometViewConfig {
    pub initial_path: Option<std::path::PathBuf>,
    pub view_mode: GitCometViewMode,
    pub focused_mergetool: Option<FocusedMergetoolViewConfig>,
    pub focused_mergetool_exit_code: Option<Arc<AtomicI32>>,
    pub startup_crash_report: Option<StartupCrashReport>,
}

impl GitCometViewConfig {
    pub fn normal(
        initial_path: Option<std::path::PathBuf>,
        startup_crash_report: Option<StartupCrashReport>,
    ) -> Self {
        Self {
            initial_path,
            view_mode: GitCometViewMode::Normal,
            focused_mergetool: None,
            focused_mergetool_exit_code: None,
            startup_crash_report,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartupCrashReport {
    pub issue_url: String,
    pub summary: String,
    pub crash_log_path: std::path::PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FocusedMergetoolLabels {
    pub local: String,
    pub remote: String,
    pub base: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FocusedMergetoolViewConfig {
    pub repo_path: std::path::PathBuf,
    pub conflicted_file_path: std::path::PathBuf,
    pub labels: FocusedMergetoolLabels,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FocusedMergetoolBootstrap {
    pub(super) repo_path: std::path::PathBuf,
    pub(super) target_path: std::path::PathBuf,
}

impl FocusedMergetoolBootstrap {
    pub(super) fn from_view_config(config: FocusedMergetoolViewConfig) -> Self {
        let repo_path = normalize_bootstrap_repo_path(config.repo_path);
        let target_path = focused_mergetool_target_path(&repo_path, &config.conflicted_file_path);
        Self {
            repo_path,
            target_path,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum FocusedMergetoolBootstrapAction {
    OpenRepo(std::path::PathBuf),
    SetActiveRepo(RepoId),
    SelectDiff {
        repo_id: RepoId,
        target: DiffTarget,
    },
    LoadConflictFile {
        repo_id: RepoId,
        path: std::path::PathBuf,
    },
    Complete,
}

pub(super) fn normalize_bootstrap_repo_path(path: std::path::PathBuf) -> std::path::PathBuf {
    let path = if path.is_relative() {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(path)
    } else {
        path
    };
    canonicalize_path(path)
}

pub(super) fn focused_mergetool_target_path(
    repo_path: &std::path::Path,
    conflicted_file_path: &std::path::Path,
) -> std::path::PathBuf {
    if conflicted_file_path.is_relative() {
        return conflicted_file_path.to_path_buf();
    }

    if let Ok(relative) = conflicted_file_path.strip_prefix(repo_path) {
        return relative.to_path_buf();
    }

    let normalized_conflicted = canonicalize_path(conflicted_file_path.to_path_buf());
    if let Ok(relative) = normalized_conflicted.strip_prefix(repo_path) {
        return relative.to_path_buf();
    }

    conflicted_file_path.to_path_buf()
}

pub(super) fn canonicalize_path(path: std::path::PathBuf) -> std::path::PathBuf {
    strip_windows_verbatim_prefix(std::fs::canonicalize(&path).unwrap_or(path))
}

#[cfg(windows)]
pub(super) fn strip_windows_verbatim_prefix(path: std::path::PathBuf) -> std::path::PathBuf {
    use std::path::{Component, Prefix};

    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return path;
    };

    let mut out = match prefix.kind() {
        Prefix::VerbatimDisk(letter) => {
            std::path::PathBuf::from(format!("{}:", char::from(letter)))
        }
        Prefix::VerbatimUNC(server, share) => {
            let mut out = std::path::PathBuf::from(r"\\");
            out.push(server);
            out.push(share);
            out
        }
        Prefix::Verbatim(raw) => std::path::PathBuf::from(raw),
        _ => return path,
    };

    for component in components {
        out.push(component.as_os_str());
    }
    out
}

#[cfg(not(windows))]
pub(super) fn strip_windows_verbatim_prefix(path: std::path::PathBuf) -> std::path::PathBuf {
    path
}

pub(super) fn focused_mergetool_bootstrap_action(
    state: &AppState,
    bootstrap: &FocusedMergetoolBootstrap,
) -> Option<FocusedMergetoolBootstrapAction> {
    let Some(repo) = state
        .repos
        .iter()
        .find(|r| r.spec.workdir == bootstrap.repo_path)
    else {
        return Some(FocusedMergetoolBootstrapAction::OpenRepo(
            bootstrap.repo_path.clone(),
        ));
    };

    if state.active_repo != Some(repo.id) {
        return Some(FocusedMergetoolBootstrapAction::SetActiveRepo(repo.id));
    }

    if !matches!(repo.open, Loadable::Ready(())) {
        return None;
    }

    let target = DiffTarget::WorkingTree {
        area: DiffArea::Unstaged,
        path: bootstrap.target_path.clone(),
    };
    if repo.diff_state.diff_target.as_ref() != Some(&target) {
        return Some(FocusedMergetoolBootstrapAction::SelectDiff {
            repo_id: repo.id,
            target,
        });
    }

    let has_conflict_file_target =
        repo.conflict_state.conflict_file_path.as_ref() == Some(&bootstrap.target_path);
    if !has_conflict_file_target || matches!(repo.conflict_state.conflict_file, Loadable::NotLoaded)
    {
        return Some(FocusedMergetoolBootstrapAction::LoadConflictFile {
            repo_id: repo.id,
            path: bootstrap.target_path.clone(),
        });
    }

    Some(FocusedMergetoolBootstrapAction::Complete)
}

pub(super) fn renders_full_chrome(view_mode: GitCometViewMode) -> bool {
    matches!(view_mode, GitCometViewMode::Normal)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum ThemeMode {
    #[default]
    Automatic,
    Light,
    Dark,
}

impl ThemeMode {
    pub(super) const fn key(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub(super) fn from_key(raw: &str) -> Option<Self> {
        match raw {
            "automatic" => Some(Self::Automatic),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Automatic => "Automatic",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub(super) fn resolve_theme(self, appearance: gpui::WindowAppearance) -> AppTheme {
        match self {
            Self::Automatic => AppTheme::default_for_window_appearance(appearance),
            Self::Light => AppTheme::zed_one_light(),
            Self::Dark => AppTheme::zed_ayu_dark(),
        }
    }
}

pub struct GitCometView {
    pub(super) store: Arc<AppStore>,
    pub(super) state: Arc<AppState>,
    pub(super) _ui_model: Entity<AppUiModel>,
    pub(super) _poller: Poller,
    pub(super) _ui_model_subscription: gpui::Subscription,
    pub(super) _activation_subscription: gpui::Subscription,
    pub(super) _appearance_subscription: gpui::Subscription,
    pub(super) view_mode: GitCometViewMode,
    pub(super) theme_mode: ThemeMode,
    pub(super) theme: AppTheme,
    pub(super) title_bar: Entity<TitleBarView>,
    pub(super) sidebar_pane: Entity<SidebarPaneView>,
    pub(super) main_pane: Entity<MainPaneView>,
    pub(super) details_pane: Entity<DetailsPaneView>,
    pub(super) repo_tabs_bar: Entity<RepoTabsBarView>,
    pub(super) action_bar: Entity<ActionBarView>,
    pub(super) tooltip_host: Entity<TooltipHost>,
    pub(super) toast_host: Entity<ToastHost>,
    pub(super) popover_host: Entity<PopoverHost>,
    pub(super) focused_mergetool_bootstrap: Option<FocusedMergetoolBootstrap>,

    pub(super) last_window_size: Size<Pixels>,
    pub(super) ui_window_size_last_seen: Size<Pixels>,
    pub(super) ui_settings_persist_seq: u64,

    pub(super) date_time_format: DateTimeFormat,
    pub(super) timezone: Timezone,
    pub(super) show_timezone: bool,

    pub(super) open_repo_panel: bool,
    pub(super) open_repo_input: Entity<components::TextInput>,

    pub(super) hover_resize_edge: Option<ResizeEdge>,

    pub(super) sidebar_collapsed: bool,
    pub(super) details_collapsed: bool,
    pub(super) sidebar_width: Pixels,
    pub(super) details_width: Pixels,
    pub(super) sidebar_render_width: Pixels,
    pub(super) details_render_width: Pixels,
    pub(super) sidebar_width_anim_seq: u64,
    pub(super) details_width_anim_seq: u64,
    pub(super) sidebar_width_animating: bool,
    pub(super) details_width_animating: bool,
    pub(super) pane_resize: Option<PaneResizeState>,

    pub(super) last_mouse_pos: Point<Pixels>,
    pub(super) pending_pull_reconcile_prompt: Option<RepoId>,
    pub(super) pending_force_delete_branch_prompt: Option<(RepoId, String)>,
    pub(super) pending_force_remove_worktree_prompt: Option<(RepoId, std::path::PathBuf)>,
    pub(super) startup_crash_report: Option<StartupCrashReport>,

    pub(super) error_banner_input: Entity<components::TextInput>,
    pub(super) transient_error_banner: Option<SharedString>,
    pub(super) auth_prompt_username_input: Entity<components::TextInput>,
    pub(super) auth_prompt_secret_input: Entity<components::TextInput>,
    pub(super) auth_prompt_key: Option<String>,
    pub(super) active_context_menu_invoker: Option<SharedString>,
}

pub(super) struct DiffTextLayoutCacheEntry {
    pub(super) layout: ShapedLine,
    pub(super) last_used_epoch: u64,
}
