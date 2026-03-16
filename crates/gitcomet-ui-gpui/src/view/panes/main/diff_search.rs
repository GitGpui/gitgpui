use super::*;

impl MainPaneView {
    fn active_conflict_target(
        &self,
    ) -> Option<(
        std::path::PathBuf,
        Option<gitcomet_core::domain::FileConflictKind>,
    )> {
        let repo = self.active_repo()?;
        let DiffTarget::WorkingTree { path, area } = repo.diff_state.diff_target.as_ref()? else {
            return None;
        };
        if *area != DiffArea::Unstaged {
            return None;
        }
        let Loadable::Ready(status) = &repo.status else {
            return None;
        };
        let conflict = status
            .unstaged
            .iter()
            .find(|e| e.path == *path && e.kind == FileStatusKind::Conflicted)?;

        Some((path.clone(), conflict.conflict))
    }

    pub(in super::super::super) fn diff_search_recompute_matches(&mut self) {
        if !self.diff_search_active {
            self.diff_search_matches.clear();
            self.diff_search_match_ix = None;
            return;
        }

        if !self.is_file_preview_active() && self.active_conflict_target().is_none() {
            self.ensure_diff_visible_indices();
        }

        self.diff_search_recompute_matches_for_current_view();
    }

    pub(super) fn diff_search_recompute_matches_for_current_view(&mut self) {
        self.diff_search_matches.clear();
        self.diff_search_match_ix = None;

        let query = self.diff_search_query.as_ref().trim();
        if query.is_empty() {
            return;
        }

        if self.is_file_preview_active() {
            let Some(line_count) = self.worktree_preview_line_count() else {
                return;
            };
            for ix in 0..line_count {
                let Some(line) = self.worktree_preview_line_text(ix) else {
                    continue;
                };
                if contains_ascii_case_insensitive(line, query) {
                    self.diff_search_matches.push(ix);
                }
            }
        } else if let Some((_path, conflict_kind)) = self.active_conflict_target() {
            let is_conflict_resolver =
                Self::conflict_resolver_strategy(conflict_kind, false).is_some();

            match (is_conflict_resolver, self.diff_view) {
                (true, _) => {
                    let ctx = ConflictResolverSearchContext::from_conflict_resolver(
                        &self.conflict_resolver,
                    );
                    self.diff_search_matches = conflict_resolver_visible_match_indices(query, &ctx);
                }
                (false, DiffViewMode::Split) => {
                    for (ix, row) in self.conflict_resolver.diff_rows().iter().enumerate() {
                        if row
                            .old
                            .as_deref()
                            .is_some_and(|s| contains_ascii_case_insensitive(s, query))
                            || row
                                .new
                                .as_deref()
                                .is_some_and(|s| contains_ascii_case_insensitive(s, query))
                        {
                            self.diff_search_matches.push(ix);
                        }
                    }
                }
                (false, DiffViewMode::Inline) => {
                    for (ix, row) in self.conflict_resolver.inline_rows().iter().enumerate() {
                        if contains_ascii_case_insensitive(row.content.as_str(), query) {
                            self.diff_search_matches.push(ix);
                        }
                    }
                }
            }
        } else {
            let total = self.diff_visible_len();
            for visible_ix in 0..total {
                match self.diff_view {
                    DiffViewMode::Inline => {
                        let text =
                            self.diff_text_line_for_region(visible_ix, DiffTextRegion::Inline);
                        if contains_ascii_case_insensitive(text.as_ref(), query) {
                            self.diff_search_matches.push(visible_ix);
                        }
                    }
                    DiffViewMode::Split => {
                        let left =
                            self.diff_text_line_for_region(visible_ix, DiffTextRegion::SplitLeft);
                        let right =
                            self.diff_text_line_for_region(visible_ix, DiffTextRegion::SplitRight);
                        if contains_ascii_case_insensitive(left.as_ref(), query)
                            || contains_ascii_case_insensitive(right.as_ref(), query)
                        {
                            self.diff_search_matches.push(visible_ix);
                        }
                    }
                }
            }
        }

        if !self.diff_search_matches.is_empty() {
            self.diff_search_match_ix = Some(0);
            let first = self.diff_search_matches[0];
            self.diff_search_scroll_to_visible_ix(first);
        }
    }

    pub(in super::super::super) fn diff_search_prev_match(&mut self) {
        if !self.diff_search_active {
            return;
        }

        if self.diff_search_matches.is_empty() {
            self.diff_search_recompute_matches();
        }
        let len = self.diff_search_matches.len();
        if len == 0 {
            return;
        }

        let current = self
            .diff_search_match_ix
            .unwrap_or(0)
            .min(len.saturating_sub(1));
        let next_ix = if current == 0 { len - 1 } else { current - 1 };
        self.diff_search_match_ix = Some(next_ix);
        let target = self.diff_search_matches[next_ix];
        self.diff_search_scroll_to_visible_ix(target);
    }

    pub(in super::super::super) fn diff_search_next_match(&mut self) {
        if !self.diff_search_active {
            return;
        }

        if self.diff_search_matches.is_empty() {
            self.diff_search_recompute_matches();
        }
        let len = self.diff_search_matches.len();
        if len == 0 {
            return;
        }

        let current = self
            .diff_search_match_ix
            .unwrap_or(0)
            .min(len.saturating_sub(1));
        let next_ix = (current + 1) % len;
        self.diff_search_match_ix = Some(next_ix);
        let target = self.diff_search_matches[next_ix];
        self.diff_search_scroll_to_visible_ix(target);
    }

    fn diff_search_scroll_to_visible_ix(&mut self, visible_ix: usize) {
        if self.is_file_preview_active() {
            self.worktree_preview_scroll
                .scroll_to_item_strict(visible_ix, gpui::ScrollStrategy::Center);
            return;
        }

        if let Some((_path, conflict_kind)) = self.active_conflict_target() {
            if Self::conflict_resolver_strategy(conflict_kind, false).is_some() {
                self.conflict_resolver_diff_scroll
                    .scroll_to_item_strict(visible_ix, gpui::ScrollStrategy::Center);
            } else {
                self.diff_scroll
                    .scroll_to_item_strict(visible_ix, gpui::ScrollStrategy::Center);
            }
            return;
        }

        self.diff_scroll
            .scroll_to_item_strict(visible_ix, gpui::ScrollStrategy::Center);
        self.diff_selection_anchor = Some(visible_ix);
        self.diff_selection_range = Some((visible_ix, visible_ix));
    }
}

fn contains_ascii_case_insensitive(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }

    let haystack_bytes = haystack.as_bytes();
    let needle_bytes = needle.as_bytes();
    if needle_bytes.len() > haystack_bytes.len() {
        return false;
    }

    'outer: for start in 0..=(haystack_bytes.len() - needle_bytes.len()) {
        for (offset, needle_byte) in needle_bytes.iter().copied().enumerate() {
            let haystack_byte = haystack_bytes[start + offset];
            if !haystack_byte.eq_ignore_ascii_case(&needle_byte) {
                continue 'outer;
            }
        }
        return true;
    }

    false
}

#[derive(Clone, Copy)]
enum ConflictResolverSearchVisibleRows<'a> {
    Map(&'a [conflict_resolver::ThreeWayVisibleItem]),
    Projection(&'a conflict_resolver::ThreeWayVisibleProjection),
}

impl<'a> ConflictResolverSearchVisibleRows<'a> {
    fn from_conflict_resolver(
        conflict_resolver: &'a ConflictResolverUiState,
    ) -> ConflictResolverSearchVisibleRows<'a> {
        match &conflict_resolver.mode_state {
            ConflictModeState::Eager(s) => Self::Map(&s.three_way_visible_map),
            ConflictModeState::Streamed(s) => Self::Projection(&s.three_way_visible_projection),
        }
    }

    fn len(self) -> usize {
        match self {
            Self::Map(items) => items.len(),
            Self::Projection(projection) => projection.len(),
        }
    }

    fn get(self, visible_ix: usize) -> Option<conflict_resolver::ThreeWayVisibleItem> {
        match self {
            Self::Map(items) => items.get(visible_ix).copied(),
            Self::Projection(projection) => projection.get(visible_ix),
        }
    }
}

#[derive(Clone, Copy)]
enum ConflictResolverSearchTwoWayRows<'a> {
    Eager {
        diff_visible_row_indices: &'a [usize],
        inline_visible_row_indices: &'a [usize],
        diff_rows: &'a [gitcomet_core::file_diff::FileDiffRow],
        inline_rows: &'a [conflict_resolver::ConflictInlineRow],
    },
    Streamed {
        split_row_index: &'a conflict_resolver::ConflictSplitRowIndex,
        two_way_split_projection: &'a conflict_resolver::TwoWaySplitProjection,
    },
}

impl<'a> ConflictResolverSearchTwoWayRows<'a> {
    fn from_conflict_resolver(
        conflict_resolver: &'a ConflictResolverUiState,
    ) -> ConflictResolverSearchTwoWayRows<'a> {
        match &conflict_resolver.mode_state {
            ConflictModeState::Eager(s) => Self::Eager {
                diff_visible_row_indices: &s.diff_visible_row_indices,
                inline_visible_row_indices: &s.inline_visible_row_indices,
                diff_rows: &s.diff_rows,
                inline_rows: &s.inline_rows,
            },
            ConflictModeState::Streamed(s) => Self::Streamed {
                split_row_index: &s.split_row_index,
                two_way_split_projection: &s.two_way_split_projection,
            },
        }
    }
}

#[cfg(test)]
fn empty_conflict_resolver_search_two_way_rows() -> ConflictResolverSearchTwoWayRows<'static> {
    ConflictResolverSearchTwoWayRows::Eager {
        diff_visible_row_indices: &[],
        inline_visible_row_indices: &[],
        diff_rows: &[],
        inline_rows: &[],
    }
}

struct ConflictResolverSearchContext<'a> {
    view_mode: ConflictResolverViewMode,
    diff_mode: ConflictDiffMode,
    marker_segments: &'a [conflict_resolver::ConflictSegment],
    three_way_visible: ConflictResolverSearchVisibleRows<'a>,
    three_way_base_text: &'a str,
    three_way_base_line_starts: &'a [usize],
    three_way_ours_text: &'a str,
    three_way_ours_line_starts: &'a [usize],
    three_way_theirs_text: &'a str,
    three_way_theirs_line_starts: &'a [usize],
    two_way_rows: ConflictResolverSearchTwoWayRows<'a>,
}

impl<'a> ConflictResolverSearchContext<'a> {
    fn from_conflict_resolver(conflict_resolver: &'a ConflictResolverUiState) -> Self {
        Self {
            view_mode: conflict_resolver.view_mode,
            diff_mode: conflict_resolver.diff_mode,
            marker_segments: &conflict_resolver.marker_segments,
            three_way_visible: ConflictResolverSearchVisibleRows::from_conflict_resolver(
                conflict_resolver,
            ),
            three_way_base_text: &conflict_resolver.three_way_text.base,
            three_way_base_line_starts: &conflict_resolver.three_way_line_starts.base,
            three_way_ours_text: &conflict_resolver.three_way_text.ours,
            three_way_ours_line_starts: &conflict_resolver.three_way_line_starts.ours,
            three_way_theirs_text: &conflict_resolver.three_way_text.theirs,
            three_way_theirs_line_starts: &conflict_resolver.three_way_line_starts.theirs,
            two_way_rows: ConflictResolverSearchTwoWayRows::from_conflict_resolver(
                conflict_resolver,
            ),
        }
    }

    fn three_way_visible_len(&self) -> usize {
        self.three_way_visible.len()
    }

    fn three_way_visible_item(
        &self,
        visible_ix: usize,
    ) -> Option<conflict_resolver::ThreeWayVisibleItem> {
        self.three_way_visible.get(visible_ix)
    }
}

fn conflict_resolver_visible_match_indices(
    query: &str,
    ctx: &ConflictResolverSearchContext<'_>,
) -> Vec<usize> {
    let mut out = Vec::new();
    match ctx.view_mode {
        ConflictResolverViewMode::ThreeWay => match ctx.three_way_visible {
            // Streamed mode: iterate spans directly, avoiding per-item O(log n) projection lookup.
            ConflictResolverSearchVisibleRows::Projection(projection) => {
                search_three_way_via_spans(projection, ctx, query, &mut out);
            }
            // Eager mode: iterate per visible item with O(1) map lookup.
            ConflictResolverSearchVisibleRows::Map(_) => {
                for visible_ix in 0..ctx.three_way_visible_len() {
                    let Some(item) = ctx.three_way_visible_item(visible_ix) else {
                        continue;
                    };
                    if three_way_visible_item_matches_query(item, ctx, query) {
                        out.push(visible_ix);
                    }
                }
            }
        },
        ConflictResolverViewMode::TwoWayDiff => match ctx.diff_mode {
            ConflictDiffMode::Split => match ctx.two_way_rows {
                ConflictResolverSearchTwoWayRows::Streamed {
                    split_row_index,
                    two_way_split_projection,
                } => {
                    // Giant mode: search source texts directly without FileDiffRow allocation,
                    // then convert matching source rows to visible indices.
                    let matching_rows = split_row_index
                        .search_matching_rows(ctx.marker_segments, |text| {
                            contains_ascii_case_insensitive(text, query)
                        });
                    for source_row in matching_rows {
                        if let Some(vis) = two_way_split_projection.source_to_visible(source_row) {
                            out.push(vis);
                        }
                    }
                }
                ConflictResolverSearchTwoWayRows::Eager {
                    diff_visible_row_indices,
                    diff_rows,
                    ..
                } => {
                    for (visible_ix, &row_ix) in diff_visible_row_indices.iter().enumerate() {
                        let Some(row) = diff_rows.get(row_ix) else {
                            continue;
                        };
                        if row
                            .old
                            .as_deref()
                            .is_some_and(|s| contains_ascii_case_insensitive(s, query))
                            || row
                                .new
                                .as_deref()
                                .is_some_and(|s| contains_ascii_case_insensitive(s, query))
                        {
                            out.push(visible_ix);
                        }
                    }
                }
            },
            ConflictDiffMode::Inline => {
                if let ConflictResolverSearchTwoWayRows::Eager {
                    inline_visible_row_indices,
                    inline_rows,
                    ..
                } = ctx.two_way_rows
                {
                    for (visible_ix, &row_ix) in inline_visible_row_indices.iter().enumerate() {
                        let Some(row) = inline_rows.get(row_ix) else {
                            continue;
                        };
                        if contains_ascii_case_insensitive(row.content.as_str(), query) {
                            out.push(visible_ix);
                        }
                    }
                }
            }
        },
    }
    out
}

/// Search three-way source texts by iterating projection spans directly.
///
/// This avoids the per-visible-item O(log spans) projection lookup by walking
/// spans sequentially and extracting line text from the three source texts.
fn search_three_way_via_spans(
    projection: &conflict_resolver::ThreeWayVisibleProjection,
    ctx: &ConflictResolverSearchContext<'_>,
    query: &str,
    out: &mut Vec<usize>,
) {
    fn line_text<'a>(text: &'a str, line_starts: &[usize], line_ix: usize) -> &'a str {
        if text.is_empty() {
            return "";
        }
        let text_len = text.len();
        let start = line_starts.get(line_ix).copied().unwrap_or(text_len);
        if start >= text_len {
            return "";
        }
        let mut end = line_starts
            .get(line_ix.saturating_add(1))
            .copied()
            .unwrap_or(text_len)
            .min(text_len);
        if end > start && text.as_bytes().get(end.saturating_sub(1)) == Some(&b'\n') {
            end = end.saturating_sub(1);
        }
        text.get(start..end).unwrap_or("")
    }

    for span in projection.spans() {
        match *span {
            conflict_resolver::ThreeWayVisibleSpan::Lines {
                visible_start,
                source_line_start,
                len,
            } => {
                for i in 0..len {
                    let line_ix = source_line_start + i;
                    let base = line_text(
                        ctx.three_way_base_text,
                        ctx.three_way_base_line_starts,
                        line_ix,
                    );
                    let ours = line_text(
                        ctx.three_way_ours_text,
                        ctx.three_way_ours_line_starts,
                        line_ix,
                    );
                    let theirs = line_text(
                        ctx.three_way_theirs_text,
                        ctx.three_way_theirs_line_starts,
                        line_ix,
                    );
                    if contains_ascii_case_insensitive(base, query)
                        || contains_ascii_case_insensitive(ours, query)
                        || contains_ascii_case_insensitive(theirs, query)
                    {
                        out.push(visible_start + i);
                    }
                }
            }
            conflict_resolver::ThreeWayVisibleSpan::CollapsedResolvedBlock {
                visible_index,
                conflict_ix,
            } => {
                let choice_label = conflict_choice_for_index(ctx.marker_segments, conflict_ix)
                    .map(conflict_choice_label)
                    .unwrap_or("?");
                let summary = format!("Resolved: picked {choice_label}");
                if contains_ascii_case_insensitive(&summary, query) {
                    out.push(visible_index);
                }
            }
        }
    }
}

fn conflict_choice_for_index(
    segments: &[conflict_resolver::ConflictSegment],
    conflict_ix: usize,
) -> Option<conflict_resolver::ConflictChoice> {
    segments
        .iter()
        .filter_map(|seg| match seg {
            conflict_resolver::ConflictSegment::Block(block) => Some(block.choice),
            _ => None,
        })
        .nth(conflict_ix)
}

fn conflict_choice_label(choice: conflict_resolver::ConflictChoice) -> &'static str {
    match choice {
        conflict_resolver::ConflictChoice::Base => "Base (A)",
        conflict_resolver::ConflictChoice::Ours => "Local (B)",
        conflict_resolver::ConflictChoice::Theirs => "Remote (C)",
        conflict_resolver::ConflictChoice::Both => "Local+Remote (B+C)",
    }
}

fn three_way_visible_item_matches_query(
    item: conflict_resolver::ThreeWayVisibleItem,
    ctx: &ConflictResolverSearchContext<'_>,
    query: &str,
) -> bool {
    fn line_text<'a>(text: &'a str, line_starts: &[usize], line_ix: usize) -> &'a str {
        if text.is_empty() {
            return "";
        }
        let text_len = text.len();
        let start = line_starts.get(line_ix).copied().unwrap_or(text_len);
        if start >= text_len {
            return "";
        }
        let mut end = line_starts
            .get(line_ix.saturating_add(1))
            .copied()
            .unwrap_or(text_len)
            .min(text_len);
        if end > start && text.as_bytes().get(end.saturating_sub(1)) == Some(&b'\n') {
            end = end.saturating_sub(1);
        }
        text.get(start..end).unwrap_or("")
    }

    match item {
        conflict_resolver::ThreeWayVisibleItem::Line(ix) => {
            let base = line_text(ctx.three_way_base_text, ctx.three_way_base_line_starts, ix);
            let ours = line_text(ctx.three_way_ours_text, ctx.three_way_ours_line_starts, ix);
            let theirs = line_text(
                ctx.three_way_theirs_text,
                ctx.three_way_theirs_line_starts,
                ix,
            );

            contains_ascii_case_insensitive(base, query)
                || contains_ascii_case_insensitive(ours, query)
                || contains_ascii_case_insensitive(theirs, query)
        }
        conflict_resolver::ThreeWayVisibleItem::CollapsedBlock(conflict_ix) => {
            let choice_label = conflict_choice_for_index(ctx.marker_segments, conflict_ix)
                .map(conflict_choice_label)
                .unwrap_or("?");
            let summary = format!("Resolved: picked {choice_label}");
            contains_ascii_case_insensitive(&summary, query)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ConflictResolverSearchContext, ConflictResolverSearchTwoWayRows,
        ConflictResolverSearchVisibleRows, conflict_resolver_visible_match_indices,
        contains_ascii_case_insensitive, empty_conflict_resolver_search_two_way_rows,
    };
    use crate::view::conflict_resolver::{
        ConflictBlock, ConflictChoice, ConflictDiffMode, ConflictResolverViewMode, ConflictSegment,
        ConflictSplitRowIndex, ThreeWayVisibleItem, TwoWaySplitProjection,
        build_three_way_visible_projection,
    };
    use crate::view::{
        ConflictModeState, ConflictResolverUiState, EagerConflictState, StreamedConflictState,
        ThreeWaySides,
    };
    use gitcomet_core::domain::DiffLineKind;
    use gitcomet_core::file_diff::{FileDiffRow, FileDiffRowKind};
    use std::sync::Arc;

    /// Helper to build a three-way search context with default view mode (ThreeWay),
    /// diff mode (Split), and empty two-way rows. Text/line-starts are passed as tuples.
    fn three_way_search_context<'a>(
        marker_segments: &'a [ConflictSegment],
        visible: ConflictResolverSearchVisibleRows<'a>,
        base: (&'a str, &'a [usize]),
        ours: (&'a str, &'a [usize]),
        theirs: (&'a str, &'a [usize]),
    ) -> ConflictResolverSearchContext<'a> {
        ConflictResolverSearchContext {
            view_mode: ConflictResolverViewMode::ThreeWay,
            diff_mode: ConflictDiffMode::Split,
            marker_segments,
            three_way_visible: visible,
            three_way_base_text: base.0,
            three_way_base_line_starts: base.1,
            three_way_ours_text: ours.0,
            three_way_ours_line_starts: ours.1,
            three_way_theirs_text: theirs.0,
            three_way_theirs_line_starts: theirs.1,
            two_way_rows: empty_conflict_resolver_search_two_way_rows(),
        }
    }

    #[test]
    fn matches_empty_needle() {
        assert!(contains_ascii_case_insensitive("abc", ""));
    }

    #[test]
    fn matches_case_insensitively() {
        assert!(contains_ascii_case_insensitive("Hello", "he"));
        assert!(contains_ascii_case_insensitive("Hello", "HEL"));
        assert!(contains_ascii_case_insensitive("Hello", "lo"));
    }

    #[test]
    fn does_not_match_absent_substring() {
        assert!(!contains_ascii_case_insensitive("Hello", "world"));
    }

    #[test]
    fn conflict_search_three_way_mode_uses_three_way_visible_rows() {
        let marker_segments = vec![ConflictSegment::Block(ConflictBlock {
            base: Some("base".into()),
            ours: "ours".into(),
            theirs: "theirs".into(),
            choice: ConflictChoice::Theirs,
            resolved: true,
        })];
        let diff_rows = vec![FileDiffRow {
            kind: FileDiffRowKind::Modify,
            old_line: Some(1),
            new_line: Some(1),
            old: Some("split-only".into()),
            new: Some("split-only".into()),
            eof_newline: None,
        }];
        let inline_rows = vec![crate::view::conflict_resolver::ConflictInlineRow {
            side: crate::view::conflict_resolver::ConflictPickSide::Ours,
            kind: DiffLineKind::Add,
            old_line: Some(1),
            new_line: Some(1),
            content: "inline-only".into(),
        }];
        let three_way_visible_map = vec![ThreeWayVisibleItem::Line(0)];
        let three_way_base_text = "base text\n";
        let three_way_ours_text = "needle\n";
        let three_way_theirs_text = "remote text\n";
        let three_way_base_line_starts = vec![0];
        let three_way_ours_line_starts = vec![0];
        let three_way_theirs_line_starts = vec![0];

        let three_way_ctx = ConflictResolverSearchContext {
            view_mode: ConflictResolverViewMode::ThreeWay,
            diff_mode: ConflictDiffMode::Split,
            marker_segments: &marker_segments,
            three_way_visible: ConflictResolverSearchVisibleRows::Map(&three_way_visible_map),
            three_way_base_text,
            three_way_base_line_starts: &three_way_base_line_starts,
            three_way_ours_text,
            three_way_ours_line_starts: &three_way_ours_line_starts,
            three_way_theirs_text,
            three_way_theirs_line_starts: &three_way_theirs_line_starts,
            two_way_rows: ConflictResolverSearchTwoWayRows::Eager {
                diff_visible_row_indices: &[0],
                inline_visible_row_indices: &[0],
                diff_rows: &diff_rows,
                inline_rows: &inline_rows,
            },
        };

        assert_eq!(
            conflict_resolver_visible_match_indices("needle", &three_way_ctx),
            vec![0]
        );
        assert!(
            conflict_resolver_visible_match_indices("split-only", &three_way_ctx).is_empty(),
            "three-way search should ignore two-way rows",
        );

        let two_way_ctx = ConflictResolverSearchContext {
            view_mode: ConflictResolverViewMode::TwoWayDiff,
            diff_mode: ConflictDiffMode::Split,
            marker_segments: &marker_segments,
            three_way_visible: ConflictResolverSearchVisibleRows::Map(&three_way_visible_map),
            three_way_base_text,
            three_way_base_line_starts: &three_way_base_line_starts,
            three_way_ours_text,
            three_way_ours_line_starts: &three_way_ours_line_starts,
            three_way_theirs_text,
            three_way_theirs_line_starts: &three_way_theirs_line_starts,
            two_way_rows: ConflictResolverSearchTwoWayRows::Eager {
                diff_visible_row_indices: &[0],
                inline_visible_row_indices: &[0],
                diff_rows: &diff_rows,
                inline_rows: &inline_rows,
            },
        };
        assert_eq!(
            conflict_resolver_visible_match_indices("split-only", &two_way_ctx),
            vec![0]
        );
    }

    #[test]
    fn conflict_search_three_way_collapsed_rows_match_choice_summary() {
        let marker_segments = vec![ConflictSegment::Block(ConflictBlock {
            base: Some("base".into()),
            ours: "ours".into(),
            theirs: "theirs".into(),
            choice: ConflictChoice::Theirs,
            resolved: true,
        })];
        let three_way_visible_map = vec![ThreeWayVisibleItem::CollapsedBlock(0)];

        let ctx = three_way_search_context(
            &marker_segments,
            ConflictResolverSearchVisibleRows::Map(&three_way_visible_map),
            ("", &[]),
            ("", &[]),
            ("", &[]),
        );

        assert_eq!(
            conflict_resolver_visible_match_indices("resolved", &ctx),
            vec![0]
        );
        assert_eq!(
            conflict_resolver_visible_match_indices("remote", &ctx),
            vec![0]
        );
    }

    #[test]
    fn conflict_search_three_way_projection_uses_streamed_visible_rows() {
        let marker_segments = vec![ConflictSegment::Block(ConflictBlock {
            base: Some("base".into()),
            ours: "needle\n".into(),
            theirs: "remote\n".into(),
            choice: ConflictChoice::Ours,
            resolved: false,
        })];
        let conflict_ranges = vec![0..1];
        let three_way_visible_projection =
            build_three_way_visible_projection(1, &conflict_ranges, &marker_segments, false);

        let ctx = three_way_search_context(
            &marker_segments,
            ConflictResolverSearchVisibleRows::Projection(&three_way_visible_projection),
            ("base\n", &[0]),
            ("needle\n", &[0]),
            ("remote\n", &[0]),
        );

        assert_eq!(
            conflict_resolver_visible_match_indices("needle", &ctx),
            vec![0]
        );
    }

    #[test]
    fn three_way_span_search_matches_per_item_search() {
        // Build a multi-line conflict with text + block segments and verify
        // that span-based search (projection path) yields the same results
        // as per-item search (map path).
        let marker_segments = vec![
            ConflictSegment::Text("header\n".into()),
            ConflictSegment::Block(ConflictBlock {
                base: Some("base_needle\nbase_plain\n".into()),
                ours: "ours_plain\nours_needle\n".into(),
                theirs: "theirs_plain\ntheirs_plain\n".into(),
                choice: ConflictChoice::Ours,
                resolved: false,
            }),
            ConflictSegment::Text("footer\n".into()),
        ];

        // Three-way line count = max(text_lines) across segments = 1 + 2 + 1 = 4
        let three_way_len = 4;
        let conflict_ranges = vec![1..3]; // lines 1..3 are the conflict block

        let base_text = "header\nbase_needle\nbase_plain\nfooter\n";
        let ours_text = "header\nours_plain\nours_needle\nfooter\n";
        let theirs_text = "header\ntheirs_plain\ntheirs_plain\nfooter\n";
        let base_line_starts = vec![0, 7, 19, 30];
        let ours_line_starts = vec![0, 7, 18, 30];
        let theirs_line_starts = vec![0, 7, 21, 35];

        // Build both the map and projection for comparison.
        let three_way_visible_map: Vec<ThreeWayVisibleItem> =
            (0..three_way_len).map(ThreeWayVisibleItem::Line).collect();
        let projection = build_three_way_visible_projection(
            three_way_len,
            &conflict_ranges,
            &marker_segments,
            false,
        );

        let map_ctx = three_way_search_context(
            &marker_segments,
            ConflictResolverSearchVisibleRows::Map(&three_way_visible_map),
            (base_text, &base_line_starts),
            (ours_text, &ours_line_starts),
            (theirs_text, &theirs_line_starts),
        );
        let map_matches = conflict_resolver_visible_match_indices("needle", &map_ctx);

        let proj_ctx = three_way_search_context(
            &marker_segments,
            ConflictResolverSearchVisibleRows::Projection(&projection),
            (base_text, &base_line_starts),
            (ours_text, &ours_line_starts),
            (theirs_text, &theirs_line_starts),
        );
        let proj_matches = conflict_resolver_visible_match_indices("needle", &proj_ctx);

        assert_eq!(
            map_matches, proj_matches,
            "span-based search must produce same results as per-item search"
        );
        assert!(
            !proj_matches.is_empty(),
            "should find at least one needle match"
        );
    }

    #[test]
    fn two_way_source_text_search_matches_row_based_search() {
        // Build segments, create a ConflictSplitRowIndex + TwoWaySplitProjection,
        // and verify the source-text search path finds the same visible indices
        // as the old row-generation path.
        let marker_segments = vec![
            ConflictSegment::Text("context_line\n".into()),
            ConflictSegment::Block(ConflictBlock {
                base: None,
                ours: "alpha\nneedle_ours\ngamma\n".into(),
                theirs: "delta\nepsilon\nneedle_theirs\n".into(),
                choice: ConflictChoice::Ours,
                resolved: false,
            }),
        ];
        let index = ConflictSplitRowIndex::new(&marker_segments, 1);
        let proj = TwoWaySplitProjection::new(&index, &marker_segments, false);

        let query = "needle";

        // Source-text search path (new):
        let matching_rows = index.search_matching_rows(&marker_segments, |text| {
            contains_ascii_case_insensitive(text, query)
        });
        let mut source_text_matches: Vec<usize> = matching_rows
            .into_iter()
            .filter_map(|r| proj.source_to_visible(r))
            .collect();
        source_text_matches.sort();

        // Row-generation search path (old):
        let mut row_based_matches = Vec::new();
        for visible_ix in 0..proj.visible_len() {
            let Some((source_ix, _)) = proj.get(visible_ix) else {
                continue;
            };
            let Some(row) = index.row_at(&marker_segments, source_ix) else {
                continue;
            };
            if row
                .old
                .as_deref()
                .is_some_and(|s| contains_ascii_case_insensitive(s, query))
                || row
                    .new
                    .as_deref()
                    .is_some_and(|s| contains_ascii_case_insensitive(s, query))
            {
                row_based_matches.push(visible_ix);
            }
        }

        assert_eq!(
            source_text_matches, row_based_matches,
            "source-text search must match row-based search"
        );
        assert!(
            !source_text_matches.is_empty(),
            "should find needle matches"
        );
    }

    #[test]
    fn three_way_span_search_handles_collapsed_blocks() {
        // Verify that collapsed resolved blocks are searchable via span search.
        let marker_segments = vec![ConflictSegment::Block(ConflictBlock {
            base: Some("base\n".into()),
            ours: "ours\n".into(),
            theirs: "theirs\n".into(),
            choice: ConflictChoice::Theirs,
            resolved: true,
        })];
        let conflict_ranges = vec![0..1];
        let projection =
            build_three_way_visible_projection(1, &conflict_ranges, &marker_segments, true);

        let ctx = three_way_search_context(
            &marker_segments,
            ConflictResolverSearchVisibleRows::Projection(&projection),
            ("base\n", &[0]),
            ("ours\n", &[0]),
            ("theirs\n", &[0]),
        );

        // Collapsed block summary should match "Resolved" and "Remote".
        assert_eq!(
            conflict_resolver_visible_match_indices("resolved", &ctx),
            vec![0]
        );
        assert_eq!(
            conflict_resolver_visible_match_indices("remote", &ctx),
            vec![0]
        );
        // Should not match line content since it's collapsed.
        assert!(
            conflict_resolver_visible_match_indices("ours", &ctx).is_empty(),
            "collapsed block should not expose line content in search"
        );
    }

    #[test]
    fn search_context_from_conflict_resolver_uses_eager_mode_state() {
        let mut conflict_resolver = ConflictResolverUiState {
            view_mode: ConflictResolverViewMode::ThreeWay,
            diff_mode: ConflictDiffMode::Inline,
            three_way_text: ThreeWaySides {
                base: "base".into(),
                ours: "ours".into(),
                theirs: "theirs".into(),
            },
            mode_state: ConflictModeState::Eager(EagerConflictState {
                three_way_visible_map: vec![ThreeWayVisibleItem::Line(0)],
                diff_visible_row_indices: vec![2],
                inline_visible_row_indices: vec![1],
                diff_rows: vec![FileDiffRow {
                    kind: FileDiffRowKind::Context,
                    old_line: Some(1),
                    new_line: Some(1),
                    old: Some("old".into()),
                    new: Some("new".into()),
                    eof_newline: None,
                }],
                inline_rows: vec![crate::view::conflict_resolver::ConflictInlineRow {
                    side: crate::view::conflict_resolver::ConflictPickSide::Ours,
                    kind: DiffLineKind::Context,
                    old_line: Some(1),
                    new_line: Some(1),
                    content: "inline".into(),
                }],
                ..EagerConflictState::default()
            }),
            ..ConflictResolverUiState::default()
        };
        conflict_resolver.three_way_line_starts = ThreeWaySides {
            base: Arc::<[usize]>::from([0]),
            ours: Arc::<[usize]>::from([0]),
            theirs: Arc::<[usize]>::from([0]),
        };

        let ctx = ConflictResolverSearchContext::from_conflict_resolver(&conflict_resolver);

        assert!(matches!(
            ctx.three_way_visible,
            ConflictResolverSearchVisibleRows::Map(items) if items.len() == 1
        ));
        assert!(matches!(
            ctx.two_way_rows,
            ConflictResolverSearchTwoWayRows::Eager {
                diff_visible_row_indices,
                inline_visible_row_indices,
                diff_rows,
                inline_rows,
            } if diff_visible_row_indices == [2]
                && inline_visible_row_indices == [1]
                && diff_rows.len() == 1
                && inline_rows.len() == 1
        ));
    }

    #[test]
    fn search_context_from_conflict_resolver_uses_streamed_mode_state() {
        let mut conflict_resolver = ConflictResolverUiState {
            view_mode: ConflictResolverViewMode::TwoWayDiff,
            diff_mode: ConflictDiffMode::Split,
            mode_state: ConflictModeState::Streamed(StreamedConflictState::default()),
            ..ConflictResolverUiState::default()
        };
        conflict_resolver.marker_segments = vec![ConflictSegment::Text("context\n".into())];
        conflict_resolver.three_way_line_starts = ThreeWaySides {
            base: Arc::<[usize]>::from([]),
            ours: Arc::<[usize]>::from([0]),
            theirs: Arc::<[usize]>::from([0]),
        };
        conflict_resolver.three_way_text = ThreeWaySides {
            base: "".into(),
            ours: "context".into(),
            theirs: "context".into(),
        };

        let ctx = ConflictResolverSearchContext::from_conflict_resolver(&conflict_resolver);

        assert!(matches!(
            ctx.three_way_visible,
            ConflictResolverSearchVisibleRows::Projection(_)
        ));
        assert!(matches!(
            ctx.two_way_rows,
            ConflictResolverSearchTwoWayRows::Streamed { .. }
        ));
    }
}
