use gpui::{App, HighlightStyle, Hsla, Pixels, ShapedLine, SharedString, TextRun, TextStyle, Window, px};
use lru::LruCache;
use rustc_hash::FxHasher;
use smallvec::SmallVec;
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{BuildHasherDefault, Hash, Hasher};
use std::num::NonZeroUsize;
use std::ops::Range;
use std::sync::Arc;

pub(crate) const TRUNCATION_ELLIPSIS: &str = "…";
const TRUNCATED_LAYOUT_CACHE_MAX_ENTRIES: usize = 8_192;

type FxLruCache<K, V> = LruCache<K, V, BuildHasherDefault<FxHasher>>;

thread_local! {
    static TRUNCATED_LAYOUT_CACHE: RefCell<FxLruCache<TruncatedLayoutCacheKey, Arc<TruncatedLineLayout>>> =
        RefCell::new(FxLruCache::with_hasher(
            NonZeroUsize::new(TRUNCATED_LAYOUT_CACHE_MAX_ENTRIES)
                .expect("truncated layout cache capacity must be > 0"),
            BuildHasherDefault::default(),
        ));
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum TextTruncationProfile {
    End,
    Middle,
    Path,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TruncationProjection {
    source_len: usize,
    display_len: usize,
    segments: SmallVec<[ProjectionSegment; 4]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProjectionSegment {
    Source {
        source_range: Range<usize>,
        display_range: Range<usize>,
    },
    Ellipsis {
        hidden_range: Range<usize>,
        display_range: Range<usize>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct TruncatedLineLayout {
    pub(crate) display_text: SharedString,
    pub(crate) shaped_line: ShapedLine,
    pub(crate) projection: Arc<TruncationProjection>,
    pub(crate) truncated: bool,
    pub(crate) line_height: Pixels,
    pub(crate) has_background_highlights: bool,
}

#[derive(Clone, Debug)]
struct CandidateLayout {
    display_text: SharedString,
    display_highlights: Vec<(Range<usize>, HighlightStyle)>,
    projection: Arc<TruncationProjection>,
    truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Affinity {
    Start,
    End,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TruncatedLayoutCacheKey {
    text_hash: u64,
    max_width_key: i32,
    font_size_bits: u32,
    font_family: SharedString,
    font_weight_bits: u32,
    font_style_hash: u64,
    color_hash: u64,
    profile: TextTruncationProfile,
    highlights_hash: u64,
    focus_hash: u64,
}

#[derive(Clone, Copy, Debug)]
enum SegmentGrowth {
    Left,
    Right,
}

enum SegmentSpec {
    Source(Range<usize>),
    Ellipsis(Range<usize>),
}

impl TruncationProjection {
    pub(crate) fn source_len(&self) -> usize {
        self.source_len
    }

    pub(crate) fn len(&self) -> usize {
        self.display_len
    }

    pub(crate) fn is_truncated(&self) -> bool {
        self.segments
            .iter()
            .any(|segment| matches!(segment, ProjectionSegment::Ellipsis { .. }))
    }

    pub(crate) fn source_to_display_offset(&self, offset: usize) -> usize {
        self.source_to_display_offset_with_affinity(offset, Affinity::Start)
    }

    pub(crate) fn source_to_display_offset_at_end(&self, offset: usize) -> usize {
        self.source_to_display_offset_with_affinity(offset, Affinity::End)
    }

    pub(crate) fn source_to_display_offset_with_affinity(
        &self,
        offset: usize,
        affinity: Affinity,
    ) -> usize {
        let offset = offset.min(self.source_len);
        for segment in &self.segments {
            match segment {
                ProjectionSegment::Source {
                    source_range,
                    display_range,
                } if offset >= source_range.start && offset <= source_range.end => {
                    return display_range.start + offset.saturating_sub(source_range.start);
                }
                ProjectionSegment::Ellipsis {
                    hidden_range,
                    display_range,
                } if offset >= hidden_range.start && offset <= hidden_range.end => {
                    return match affinity {
                        Affinity::Start => display_range.start,
                        Affinity::End => display_range.end,
                    };
                }
                _ => {}
            }
        }

        self.display_len
    }

    pub(crate) fn display_to_source_offset(
        &self,
        display_offset: usize,
        affinity: Affinity,
    ) -> usize {
        let display_offset = display_offset.min(self.display_len);
        for segment in &self.segments {
            match segment {
                ProjectionSegment::Source {
                    source_range,
                    display_range,
                } if display_offset >= display_range.start && display_offset <= display_range.end => {
                    return source_range.start + display_offset.saturating_sub(display_range.start);
                }
                ProjectionSegment::Ellipsis {
                    hidden_range,
                    display_range,
                } if display_offset >= display_range.start && display_offset <= display_range.end => {
                    return match affinity {
                        Affinity::Start => hidden_range.start,
                        Affinity::End => hidden_range.end,
                    };
                }
                _ => {}
            }
        }

        self.source_len
    }

    pub(crate) fn display_to_source_start_offset(&self, display_offset: usize) -> usize {
        self.display_to_source_offset(display_offset, Affinity::Start)
    }

    pub(crate) fn display_to_source_end_offset(&self, display_offset: usize) -> usize {
        self.display_to_source_offset(display_offset, Affinity::End)
    }

    pub(crate) fn selection_display_ranges(
        &self,
        selection: Range<usize>,
    ) -> SmallVec<[Range<usize>; 4]> {
        let mut ranges = SmallVec::new();
        if selection.is_empty() {
            return ranges;
        }

        for segment in &self.segments {
            match segment {
                ProjectionSegment::Source {
                    source_range,
                    display_range,
                } => {
                    let start = selection.start.max(source_range.start);
                    let end = selection.end.min(source_range.end);
                    if start >= end {
                        continue;
                    }
                    ranges.push(
                        display_range.start + start.saturating_sub(source_range.start)
                            ..display_range.start + end.saturating_sub(source_range.start),
                    );
                }
                ProjectionSegment::Ellipsis {
                    hidden_range,
                    display_range,
                } => {
                    if selection.start < hidden_range.end && selection.end > hidden_range.start {
                        ranges.push(display_range.clone());
                    }
                }
            }
        }
        ranges
    }

    pub(crate) fn ellipsis_segment_at_display_offset(
        &self,
        display_offset: usize,
    ) -> Option<(Range<usize>, Range<usize>)> {
        self.segments.iter().find_map(|segment| match segment {
            ProjectionSegment::Ellipsis {
                hidden_range,
                display_range,
            } if display_offset >= display_range.start && display_offset <= display_range.end => {
                Some((hidden_range.clone(), display_range.clone()))
            }
            _ => None,
        })
    }

    pub(crate) fn ellipsis_segment_for_source_offset(
        &self,
        source_offset: usize,
    ) -> Option<(Range<usize>, Range<usize>)> {
        self.segments.iter().find_map(|segment| match segment {
            ProjectionSegment::Ellipsis {
                hidden_range,
                display_range,
            } if source_offset >= hidden_range.start && source_offset <= hidden_range.end => {
                Some((hidden_range.clone(), display_range.clone()))
            }
            _ => None,
        })
    }
}

pub(crate) fn shape_truncated_line_cached(
    window: &mut Window,
    cx: &mut App,
    base_style: &TextStyle,
    text: &SharedString,
    max_width: Option<Pixels>,
    profile: TextTruncationProfile,
    highlights: &[(Range<usize>, HighlightStyle)],
    focus_range: Option<Range<usize>>,
) -> Arc<TruncatedLineLayout> {
    let font_size = base_style.font_size.to_pixels(window.rem_size());
    let line_height = base_style.line_height_in_pixels(window.rem_size());
    let key = TruncatedLayoutCacheKey {
        text_hash: hash_value(text.as_ref()),
        max_width_key: max_width.map(width_cache_key).unwrap_or(i32::MIN),
        font_size_bits: f32::from(font_size).to_bits(),
        font_family: base_style.font_family.clone(),
        font_weight_bits: base_style.font_weight.0.to_bits(),
        font_style_hash: hash_value(&base_style.font_style),
        color_hash: hash_color(base_style.color),
        profile,
        highlights_hash: hash_highlights(highlights),
        focus_hash: hash_focus_range(focus_range.as_ref()),
    };

    if let Some(entry) = TRUNCATED_LAYOUT_CACHE.with(|cache| cache.borrow_mut().get(&key).cloned())
    {
        return entry;
    }

    let layout = Arc::new(shape_truncated_line_uncached(
        window,
        cx,
        base_style,
        text,
        max_width,
        profile,
        highlights,
        focus_range,
        font_size,
        line_height,
    ));

    TRUNCATED_LAYOUT_CACHE.with(|cache| {
        cache.borrow_mut().put(key, Arc::clone(&layout));
    });

    layout
}

fn shape_truncated_line_uncached(
    window: &mut Window,
    cx: &mut App,
    base_style: &TextStyle,
    text: &SharedString,
    max_width: Option<Pixels>,
    profile: TextTruncationProfile,
    highlights: &[(Range<usize>, HighlightStyle)],
    focus_range: Option<Range<usize>>,
    font_size: Pixels,
    line_height: Pixels,
) -> TruncatedLineLayout {
    let candidate = build_truncated_candidate(
        window,
        cx,
        base_style,
        text,
        max_width,
        profile,
        highlights,
        focus_range,
        font_size,
    );
    let has_background_highlights = candidate
        .display_highlights
        .iter()
        .any(|(_, highlight)| highlight.background_color.is_some());
    let runs =
        compute_highlight_runs(&candidate.display_text, base_style, &candidate.display_highlights);
    let shaped_line = window
        .text_system()
        .shape_line(candidate.display_text.clone(), font_size, &runs, None);

    TruncatedLineLayout {
        display_text: candidate.display_text,
        shaped_line,
        projection: candidate.projection,
        truncated: candidate.truncated,
        line_height,
        has_background_highlights,
    }
}

fn build_truncated_candidate(
    window: &mut Window,
    cx: &mut App,
    base_style: &TextStyle,
    text: &SharedString,
    max_width: Option<Pixels>,
    profile: TextTruncationProfile,
    highlights: &[(Range<usize>, HighlightStyle)],
    focus_range: Option<Range<usize>>,
    font_size: Pixels,
) -> CandidateLayout {
    let Some(max_width) = max_width else {
        return full_candidate(text, highlights);
    };
    if max_width <= px(0.0) {
        return ellipsis_only_candidate(text.len());
    }

    let full = full_candidate(text, highlights);
    if candidate_width(window, cx, base_style, font_size, &full) <= max_width {
        return full;
    }

    if let Some(focus) = normalized_focus_range(text.as_ref(), focus_range) {
        let focused = truncate_around_focus(
            window,
            cx,
            base_style,
            text,
            highlights,
            font_size,
            max_width,
            focus,
        );
        if candidate_width(window, cx, base_style, font_size, &focused) <= max_width {
            return focused;
        }
    }

    match profile {
        TextTruncationProfile::End => truncate_from_end(
            window,
            cx,
            base_style,
            text,
            highlights,
            font_size,
            max_width,
        ),
        TextTruncationProfile::Middle => truncate_from_middle(
            window,
            cx,
            base_style,
            text,
            highlights,
            font_size,
            max_width,
        ),
        TextTruncationProfile::Path => truncate_path_like(
            window,
            cx,
            base_style,
            text,
            highlights,
            font_size,
            max_width,
        ),
    }
}

fn full_candidate(
    text: &SharedString,
    highlights: &[(Range<usize>, HighlightStyle)],
) -> CandidateLayout {
    let projection = Arc::new(TruncationProjection {
        source_len: text.len(),
        display_len: text.len(),
        segments: SmallVec::from_vec(vec![ProjectionSegment::Source {
            source_range: 0..text.len(),
            display_range: 0..text.len(),
        }]),
    });
    CandidateLayout {
        display_text: text.clone(),
        display_highlights: highlights.to_vec(),
        projection,
        truncated: false,
    }
}

fn ellipsis_only_candidate(source_len: usize) -> CandidateLayout {
    let display_text: SharedString = TRUNCATION_ELLIPSIS.into();
    let projection = Arc::new(TruncationProjection {
        source_len,
        display_len: display_text.len(),
        segments: SmallVec::from_vec(vec![ProjectionSegment::Ellipsis {
            hidden_range: 0..source_len,
            display_range: 0..display_text.len(),
        }]),
    });
    CandidateLayout {
        display_text,
        display_highlights: Vec::new(),
        projection,
        truncated: true,
    }
}

fn truncate_from_end(
    window: &mut Window,
    cx: &mut App,
    base_style: &TextStyle,
    text: &SharedString,
    highlights: &[(Range<usize>, HighlightStyle)],
    font_size: Pixels,
    max_width: Pixels,
) -> CandidateLayout {
    let boundaries = char_boundaries(text.as_ref());
    if boundaries.len() <= 2 {
        return ellipsis_only_candidate(text.len());
    }

    let mut best = ellipsis_only_candidate(text.len());
    for &prefix_end in boundaries.iter().skip(1) {
        let candidate = candidate_from_segments(
            text,
            &[SegmentSpec::Source(0..prefix_end), SegmentSpec::Ellipsis(prefix_end..text.len())],
            highlights,
        );
        if candidate_width(window, cx, base_style, font_size, &candidate) <= max_width {
            best = candidate;
        } else {
            break;
        }
    }
    best
}

fn truncate_from_start(
    window: &mut Window,
    cx: &mut App,
    base_style: &TextStyle,
    text: &SharedString,
    highlights: &[(Range<usize>, HighlightStyle)],
    font_size: Pixels,
    max_width: Pixels,
) -> CandidateLayout {
    let boundaries = char_boundaries(text.as_ref());
    if boundaries.len() <= 2 {
        return ellipsis_only_candidate(text.len());
    }

    let mut best = ellipsis_only_candidate(text.len());
    for &suffix_start in boundaries.iter().rev().skip(1) {
        let candidate = candidate_from_segments(
            text,
            &[SegmentSpec::Ellipsis(0..suffix_start), SegmentSpec::Source(suffix_start..text.len())],
            highlights,
        );
        if candidate_width(window, cx, base_style, font_size, &candidate) <= max_width {
            best = candidate;
        } else {
            break;
        }
    }
    best
}

fn truncate_from_middle(
    window: &mut Window,
    cx: &mut App,
    base_style: &TextStyle,
    text: &SharedString,
    highlights: &[(Range<usize>, HighlightStyle)],
    font_size: Pixels,
    max_width: Pixels,
) -> CandidateLayout {
    let boundaries = char_boundaries(text.as_ref());
    if boundaries.len() <= 2 {
        return ellipsis_only_candidate(text.len());
    }

    let prefix_end = boundaries[1];
    let suffix_start = boundaries[boundaries.len().saturating_sub(2)];
    if prefix_end >= suffix_start {
        return truncate_from_end(window, cx, base_style, text, highlights, font_size, max_width);
    }

    let mut best = candidate_from_segments(
        text,
        &[SegmentSpec::Source(0..prefix_end), SegmentSpec::Ellipsis(prefix_end..suffix_start), SegmentSpec::Source(suffix_start..text.len())],
        highlights,
    );
    if candidate_width(window, cx, base_style, font_size, &best) > max_width {
        return truncate_from_end(window, cx, base_style, text, highlights, font_size, max_width);
    }

    let mut left_ix = 1usize;
    let mut right_ix = boundaries.len().saturating_sub(2);
    let mut next = SegmentGrowth::Right;

    loop {
        let mut advanced = false;
        for _ in 0..2 {
            let try_left = matches!(next, SegmentGrowth::Left);
            next = match next {
                SegmentGrowth::Left => SegmentGrowth::Right,
                SegmentGrowth::Right => SegmentGrowth::Left,
            };
            let (candidate, grew) = if try_left {
                try_expand_middle_candidate(
                    text,
                    highlights,
                    &boundaries,
                    &mut left_ix,
                    &mut right_ix,
                    true,
                )
            } else {
                try_expand_middle_candidate(
                    text,
                    highlights,
                    &boundaries,
                    &mut left_ix,
                    &mut right_ix,
                    false,
                )
            };

            let Some(candidate) = candidate else {
                continue;
            };
            if candidate_width(window, cx, base_style, font_size, &candidate) <= max_width {
                best = candidate;
                advanced = true;
                break;
            }
            if grew {
                if try_left {
                    left_ix = left_ix.saturating_sub(1);
                } else {
                    right_ix = right_ix.saturating_add(1).min(boundaries.len().saturating_sub(2));
                }
            }
        }
        if !advanced {
            break;
        }
    }

    best
}

fn try_expand_middle_candidate(
    text: &SharedString,
    highlights: &[(Range<usize>, HighlightStyle)],
    boundaries: &[usize],
    left_ix: &mut usize,
    right_ix: &mut usize,
    grow_left: bool,
) -> (Option<CandidateLayout>, bool) {
    let original_left = *left_ix;
    let original_right = *right_ix;

    if grow_left {
        if *left_ix + 1 >= *right_ix {
            return (None, false);
        }
        *left_ix = (*left_ix + 1).min(boundaries.len().saturating_sub(2));
    } else {
        if *right_ix == 0 || *right_ix <= *left_ix + 1 {
            return (None, false);
        }
        *right_ix = right_ix.saturating_sub(1);
    }

    let prefix_end = boundaries[*left_ix];
    let suffix_start = boundaries[*right_ix];
    if prefix_end >= suffix_start {
        *left_ix = original_left;
        *right_ix = original_right;
        return (None, false);
    }

    (
        Some(candidate_from_segments(
            text,
            &[
                SegmentSpec::Source(0..prefix_end),
                SegmentSpec::Ellipsis(prefix_end..suffix_start),
                SegmentSpec::Source(suffix_start..text.len()),
            ],
            highlights,
        )),
        true,
    )
}

fn truncate_around_focus(
    window: &mut Window,
    cx: &mut App,
    base_style: &TextStyle,
    text: &SharedString,
    highlights: &[(Range<usize>, HighlightStyle)],
    font_size: Pixels,
    max_width: Pixels,
    focus: Range<usize>,
) -> CandidateLayout {
    let boundaries = char_boundaries(text.as_ref());
    let mut start = focus.start;
    let mut end = focus.end;
    let mut best = candidate_with_focus_window(text, highlights, start, end);

    if candidate_width(window, cx, base_style, font_size, &best) > max_width {
        return truncate_from_middle(window, cx, base_style, text, highlights, font_size, max_width);
    }

    let mut next = SegmentGrowth::Left;
    loop {
        let mut advanced = false;
        for _ in 0..2 {
            let grow_left = matches!(next, SegmentGrowth::Left);
            next = match next {
                SegmentGrowth::Left => SegmentGrowth::Right,
                SegmentGrowth::Right => SegmentGrowth::Left,
            };

            let maybe = if grow_left {
                previous_boundary(&boundaries, start).map(|next_start| (next_start, end))
            } else {
                next_boundary(&boundaries, end).map(|next_end| (start, next_end))
            };
            let Some((candidate_start, candidate_end)) = maybe else {
                continue;
            };
            let candidate =
                candidate_with_focus_window(text, highlights, candidate_start, candidate_end);
            if candidate_width(window, cx, base_style, font_size, &candidate) <= max_width {
                start = candidate_start;
                end = candidate_end;
                best = candidate;
                advanced = true;
                break;
            }
        }
        if !advanced {
            break;
        }
    }

    best
}

fn candidate_with_focus_window(
    text: &SharedString,
    highlights: &[(Range<usize>, HighlightStyle)],
    start: usize,
    end: usize,
) -> CandidateLayout {
    let mut segments = Vec::with_capacity(3);
    if start > 0 {
        segments.push(SegmentSpec::Ellipsis(0..start));
    }
    segments.push(SegmentSpec::Source(start..end));
    if end < text.len() {
        segments.push(SegmentSpec::Ellipsis(end..text.len()));
    }
    candidate_from_segments(text, &segments, highlights)
}

fn truncate_path_like(
    window: &mut Window,
    cx: &mut App,
    base_style: &TextStyle,
    text: &SharedString,
    highlights: &[(Range<usize>, HighlightStyle)],
    font_size: Pixels,
    max_width: Pixels,
) -> CandidateLayout {
    let Some(path) = path_boundaries(text.as_ref()) else {
        return truncate_from_middle(window, cx, base_style, text, highlights, font_size, max_width);
    };

    let mut prefix_end = path.min_prefix_end;
    let mut suffix_start = path.min_suffix_start;
    if prefix_end >= suffix_start {
        return truncate_from_start(window, cx, base_style, text, highlights, font_size, max_width);
    }

    let mut best = candidate_from_segments(
        text,
        &[
            SegmentSpec::Source(0..prefix_end),
            SegmentSpec::Ellipsis(prefix_end..suffix_start),
            SegmentSpec::Source(suffix_start..text.len()),
        ],
        highlights,
    );
    if candidate_width(window, cx, base_style, font_size, &best) > max_width {
        return truncate_from_start(window, cx, base_style, text, highlights, font_size, max_width);
    }

    let mut left_ix = path
        .prefix_cuts
        .iter()
        .position(|&cut| cut == prefix_end)
        .unwrap_or(0);
    let mut right_ix = path
        .suffix_starts
        .iter()
        .position(|&cut| cut == suffix_start)
        .unwrap_or(0);
    let mut next = SegmentGrowth::Right;

    loop {
        let mut advanced = false;
        for _ in 0..2 {
            let grow_left = matches!(next, SegmentGrowth::Left);
            next = match next {
                SegmentGrowth::Left => SegmentGrowth::Right,
                SegmentGrowth::Right => SegmentGrowth::Left,
            };

            let maybe = if grow_left {
                path.prefix_cuts
                    .get(left_ix + 1)
                    .copied()
                    .map(|next_prefix_end| (next_prefix_end, suffix_start, true))
            } else if right_ix + 1 < path.suffix_starts.len() {
                Some((prefix_end, path.suffix_starts[right_ix + 1], false))
            } else {
                None
            };

            let Some((candidate_prefix_end, candidate_suffix_start, mutated_left)) = maybe else {
                continue;
            };
            if candidate_prefix_end >= candidate_suffix_start {
                continue;
            }

            let candidate = candidate_from_segments(
                text,
                &[
                    SegmentSpec::Source(0..candidate_prefix_end),
                    SegmentSpec::Ellipsis(candidate_prefix_end..candidate_suffix_start),
                    SegmentSpec::Source(candidate_suffix_start..text.len()),
                ],
                highlights,
            );
            if candidate_width(window, cx, base_style, font_size, &candidate) <= max_width {
                prefix_end = candidate_prefix_end;
                suffix_start = candidate_suffix_start;
                if mutated_left {
                    left_ix += 1;
                } else {
                    right_ix += 1;
                }
                best = candidate;
                advanced = true;
                break;
            }
        }
        if !advanced {
            break;
        }
    }

    best
}

fn candidate_from_segments(
    text: &SharedString,
    segments: &[SegmentSpec],
    highlights: &[(Range<usize>, HighlightStyle)],
) -> CandidateLayout {
    let mut display = String::new();
    let mut projection_segments = SmallVec::<[ProjectionSegment; 4]>::new();
    let mut truncated = false;

    for segment in segments {
        let display_start = display.len();
        match segment {
            SegmentSpec::Source(source_range) => {
                display.push_str(&text[source_range.clone()]);
                projection_segments.push(ProjectionSegment::Source {
                    source_range: source_range.clone(),
                    display_range: display_start..display.len(),
                });
            }
            SegmentSpec::Ellipsis(hidden_range) => {
                truncated = true;
                display.push_str(TRUNCATION_ELLIPSIS);
                projection_segments.push(ProjectionSegment::Ellipsis {
                    hidden_range: hidden_range.clone(),
                    display_range: display_start..display.len(),
                });
            }
        }
    }

    let projection = Arc::new(TruncationProjection {
        source_len: text.len(),
        display_len: display.len(),
        segments: projection_segments,
    });
    let display_text: SharedString = display.into();
    let display_highlights = remap_highlights(&projection, highlights);

    CandidateLayout {
        display_text,
        display_highlights,
        projection,
        truncated,
    }
}

fn remap_highlights(
    projection: &TruncationProjection,
    highlights: &[(Range<usize>, HighlightStyle)],
) -> Vec<(Range<usize>, HighlightStyle)> {
    if highlights.is_empty() {
        return Vec::new();
    }

    let mut remapped = Vec::new();
    for segment in &projection.segments {
        let ProjectionSegment::Source {
            source_range,
            display_range,
        } = segment
        else {
            continue;
        };
        for (highlight_range, highlight_style) in highlights {
            let start = highlight_range.start.max(source_range.start);
            let end = highlight_range.end.min(source_range.end);
            if start >= end {
                continue;
            }
            remapped.push((
                display_range.start + start.saturating_sub(source_range.start)
                    ..display_range.start + end.saturating_sub(source_range.start),
                *highlight_style,
            ));
        }
    }
    remapped.sort_by(|(a, _), (b, _)| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));
    remapped
}

fn candidate_width(
    window: &mut Window,
    _cx: &mut App,
    base_style: &TextStyle,
    font_size: Pixels,
    candidate: &CandidateLayout,
) -> Pixels {
    let runs =
        compute_highlight_runs(&candidate.display_text, base_style, &candidate.display_highlights);
    window
        .text_system()
        .shape_line(candidate.display_text.clone(), font_size, &runs, None)
        .width
}

fn compute_highlight_runs(
    text: &str,
    default_style: &TextStyle,
    highlights: &[(Range<usize>, HighlightStyle)],
) -> Vec<TextRun> {
    if highlights.is_empty() {
        return vec![default_style.to_run(text.len())];
    }

    let mut runs = Vec::with_capacity(highlights.len() * 2 + 1);
    let mut ix = 0usize;
    for (range, highlight) in highlights {
        if ix < range.start {
            runs.push(default_style.clone().to_run(range.start - ix));
        }
        runs.push(
            default_style
                .clone()
                .highlight(*highlight)
                .to_run(range.len()),
        );
        ix = range.end;
    }
    if ix < text.len() {
        runs.push(default_style.clone().to_run(text.len() - ix));
    }
    runs
}

fn normalized_focus_range(text: &str, focus_range: Option<Range<usize>>) -> Option<Range<usize>> {
    let focus_range = focus_range?;
    if focus_range.is_empty() || focus_range.start >= text.len() {
        return None;
    }

    let mut start = focus_range.start.min(text.len());
    let mut end = focus_range.end.min(text.len());
    while start > 0 && !text.is_char_boundary(start) {
        start = start.saturating_sub(1);
    }
    while end < text.len() && !text.is_char_boundary(end) {
        end += 1;
    }
    (start < end).then_some(start..end)
}

fn char_boundaries(text: &str) -> Vec<usize> {
    let mut boundaries = Vec::with_capacity(text.chars().count() + 1);
    boundaries.push(0);
    boundaries.extend(text.char_indices().skip(1).map(|(ix, _)| ix));
    boundaries.push(text.len());
    boundaries
}

fn previous_boundary(boundaries: &[usize], current: usize) -> Option<usize> {
    boundaries
        .iter()
        .rev()
        .copied()
        .find(|&boundary| boundary < current)
}

fn next_boundary(boundaries: &[usize], current: usize) -> Option<usize> {
    boundaries.iter().copied().find(|&boundary| boundary > current)
}

struct PathBoundaries {
    prefix_cuts: Vec<usize>,
    suffix_starts: Vec<usize>,
    min_prefix_end: usize,
    min_suffix_start: usize,
}

fn path_boundaries(text: &str) -> Option<PathBoundaries> {
    let mut separator_ends = Vec::new();
    let mut separator_starts = Vec::new();
    for (ix, ch) in text.char_indices() {
        if ch == '/' || ch == '\\' {
            separator_starts.push(ix);
            separator_ends.push(ix + ch.len_utf8());
        }
    }
    if separator_ends.is_empty() {
        return None;
    }

    let root_end = match separator_ends.first().copied() {
        Some(first) if text.starts_with('/') || text.starts_with('\\') => Some(first),
        Some(first) if first == 3
            && text.len() >= 2
            && text.as_bytes().get(1) == Some(&b':')
            && text[..first].chars().last().is_some_and(|ch| ch == '/' || ch == '\\') =>
        {
            Some(first)
        }
        _ => None,
    };

    let min_prefix_end = root_end
        .and_then(|root| separator_ends.iter().copied().find(|&cut| cut > root))
        .or_else(|| separator_ends.first().copied())?;
    let min_suffix_start = separator_ends.last().copied()?;

    let mut prefix_cuts = separator_ends.clone();
    prefix_cuts.retain(|&cut| cut < text.len());
    prefix_cuts.sort_unstable();
    prefix_cuts.dedup();

    let mut suffix_starts: Vec<usize> = separator_starts
        .into_iter()
        .map(|ix| ix + 1)
        .filter(|&start| start < text.len())
        .collect();
    suffix_starts.push(min_suffix_start);
    suffix_starts.sort_unstable_by(|a, b| b.cmp(a));
    suffix_starts.dedup();

    Some(PathBoundaries {
        prefix_cuts,
        suffix_starts,
        min_prefix_end,
        min_suffix_start,
    })
}

fn width_cache_key(width: Pixels) -> i32 {
    let mut key = f32::from(width.round()) as i32;
    if key == i32::MIN {
        key = i32::MIN + 1;
    }
    key
}

fn hash_value(value: &(impl Hash + ?Sized)) -> u64 {
    let mut hasher = FxHasher::default();
    value.hash(&mut hasher);
    hasher.finish()
}

fn hash_color(color: Hsla) -> u64 {
    let mut hasher = DefaultHasher::new();
    color.hash(&mut hasher);
    hasher.finish()
}

fn hash_highlights(highlights: &[(Range<usize>, HighlightStyle)]) -> u64 {
    hash_value(&highlights)
}

fn hash_focus_range(focus_range: Option<&Range<usize>>) -> u64 {
    match focus_range {
        Some(range) => hash_value(range),
        None => 0,
    }
}

#[cfg(test)]
pub(crate) fn clear_truncated_layout_cache_for_test() {
    TRUNCATED_LAYOUT_CACHE.with(|cache| cache.borrow_mut().clear());
}

#[cfg(test)]
pub(crate) fn truncated_layout_cache_len_for_test() -> usize {
    TRUNCATED_LAYOUT_CACHE.with(|cache| cache.borrow().len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_display_ranges_include_ellipsis_when_hidden_range_selected() {
        let projection = TruncationProjection {
            source_len: 12,
            display_len: 7,
            segments: SmallVec::from_vec(vec![
                ProjectionSegment::Source {
                    source_range: 0..3,
                    display_range: 0..3,
                },
                ProjectionSegment::Ellipsis {
                    hidden_range: 3..9,
                    display_range: 3..6,
                },
                ProjectionSegment::Source {
                    source_range: 9..12,
                    display_range: 6..9,
                },
            ]),
        };

        let ranges = projection.selection_display_ranges(2..10);
        assert_eq!(ranges.as_slice(), &[2..3, 3..6, 6..7]);
    }

    #[test]
    fn display_to_source_offset_maps_ellipsis_to_hidden_edges() {
        let projection = TruncationProjection {
            source_len: 10,
            display_len: 7,
            segments: SmallVec::from_vec(vec![
                ProjectionSegment::Source {
                    source_range: 0..2,
                    display_range: 0..2,
                },
                ProjectionSegment::Ellipsis {
                    hidden_range: 2..8,
                    display_range: 2..5,
                },
                ProjectionSegment::Source {
                    source_range: 8..10,
                    display_range: 5..7,
                },
            ]),
        };

        assert_eq!(
            projection.display_to_source_offset(2, Affinity::Start),
            2
        );
        assert_eq!(projection.display_to_source_offset(2, Affinity::End), 8);
    }
}
