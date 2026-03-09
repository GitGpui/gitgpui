use crate::file_diff::{
    DiffHunk, Edit, edits_to_hunks_with, myers_edits, reconstruct_side_with, split_lines,
};
use crate::text_utils::{LineEndingDetectionMode, detect_line_ending_from_texts};

// ---------------------------------------------------------------------------
// Pass 2: heuristic subchunk splitting (meld-inspired)
// ---------------------------------------------------------------------------

/// A subchunk produced by splitting a conflict block into line-level pieces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Subchunk {
    /// Lines that could be auto-resolved (identical across sides, or only one
    /// side changed from base).
    Resolved(String),
    /// Lines where both sides changed differently — still needs resolution.
    Conflict {
        base: String,
        ours: String,
        theirs: String,
    },
}

/// A contiguous range of base lines that were changed (deleted/replaced/inserted)
/// on one side of a 2-way diff.
type LineHunk = DiffHunk<String>;

/// Maximum number of lines per side before we skip subchunk splitting.
const SUBCHUNK_MAX_LINES: usize = 500;

/// Split a conflict region into line-level subchunks using 3-way diff/merge.
///
/// Returns `Some(subchunks)` if the block can be meaningfully decomposed into
/// a mix of resolved and conflicting pieces. Returns `None` if:
/// - Pass 1 would handle this (identical sides, only one side changed)
/// - Input is too large
/// - Splitting doesn't improve over the original block (all conflict, no context)
pub fn split_conflict_into_subchunks(
    base: &str,
    ours: &str,
    theirs: &str,
) -> Option<Vec<Subchunk>> {
    // Pass 1 would handle these — don't split.
    if ours == theirs || ours == base || theirs == base {
        return None;
    }

    let base_lines = split_lines(base);
    let ours_lines = split_lines(ours);
    let theirs_lines = split_lines(theirs);

    if base_lines.len() > SUBCHUNK_MAX_LINES
        || ours_lines.len() > SUBCHUNK_MAX_LINES
        || theirs_lines.len() > SUBCHUNK_MAX_LINES
    {
        return None;
    }

    // Detect dominant line ending from the input texts so that
    // reconstructed subchunk text preserves CRLF when appropriate.
    let line_ending = detect_line_ending_from_texts(
        [base, ours, theirs],
        LineEndingDetectionMode::DominantCrlfVsLf,
    );

    let subchunks =
        if base_lines.len() == ours_lines.len() && base_lines.len() == theirs_lines.len() {
            // Same number of lines: simple per-line 3-way comparison.
            per_line_merge(&base_lines, &ours_lines, &theirs_lines, line_ending)
        } else {
            // Different line counts: use diff-based hunk merge.
            let edits_ours = myers_edits(&base_lines, &ours_lines);
            let edits_theirs = myers_edits(&base_lines, &theirs_lines);
            let hunks_ours = edits_to_line_hunks(&edits_ours);
            let hunks_theirs = edits_to_line_hunks(&edits_theirs);
            merge_line_hunks(&base_lines, &hunks_ours, &hunks_theirs, line_ending)
        };

    // Only worth returning if at least some content is resolved.
    let has_resolved = subchunks.iter().any(|c| matches!(c, Subchunk::Resolved(_)));
    if has_resolved { Some(subchunks) } else { None }
}

/// Convert a Myers edit script into line-level hunks relative to the base.
fn edits_to_line_hunks(edits: &[Edit<'_>]) -> Vec<LineHunk> {
    edits_to_hunks_with(edits, |line| line.to_string())
}

/// Per-line 3-way merge for three sequences of equal length.
///
/// Walks line-by-line, classifying each line:
/// - all three equal → context (resolved)
/// - only ours changed → resolved (pick ours)
/// - only theirs changed → resolved (pick theirs)
/// - both changed same way → resolved (pick either)
/// - both changed differently → conflict
///
/// Groups consecutive lines with the same classification into subchunks.
fn per_line_merge(
    base_lines: &[&str],
    ours_lines: &[&str],
    theirs_lines: &[&str],
    line_ending: &str,
) -> Vec<Subchunk> {
    debug_assert_eq!(base_lines.len(), ours_lines.len());
    debug_assert_eq!(base_lines.len(), theirs_lines.len());

    let len = base_lines.len();
    let mut subchunks = Vec::new();
    let mut i = 0;

    while i < len {
        let same_bo = base_lines[i] == ours_lines[i];
        let same_bt = base_lines[i] == theirs_lines[i];
        let same_ot = ours_lines[i] == theirs_lines[i];

        if same_bo && same_bt {
            // All three equal → context.
            let start = i;
            while i < len && base_lines[i] == ours_lines[i] && base_lines[i] == theirs_lines[i] {
                i += 1;
            }
            subchunks.push(Subchunk::Resolved(lines_to_text(
                &base_lines[start..i],
                line_ending,
            )));
        } else if !same_bo && same_bt {
            // Only ours changed from base.
            let start = i;
            while i < len && base_lines[i] != ours_lines[i] && base_lines[i] == theirs_lines[i] {
                i += 1;
            }
            subchunks.push(Subchunk::Resolved(lines_to_text(
                &ours_lines[start..i],
                line_ending,
            )));
        } else if same_bo && !same_bt {
            // Only theirs changed from base.
            let start = i;
            while i < len && base_lines[i] == ours_lines[i] && base_lines[i] != theirs_lines[i] {
                i += 1;
            }
            subchunks.push(Subchunk::Resolved(lines_to_text(
                &theirs_lines[start..i],
                line_ending,
            )));
        } else if same_ot {
            // Both changed, same way.
            let start = i;
            while i < len && base_lines[i] != ours_lines[i] && ours_lines[i] == theirs_lines[i] {
                i += 1;
            }
            subchunks.push(Subchunk::Resolved(lines_to_text(
                &ours_lines[start..i],
                line_ending,
            )));
        } else {
            // Both changed differently → conflict.
            let start = i;
            while i < len
                && base_lines[i] != ours_lines[i]
                && base_lines[i] != theirs_lines[i]
                && ours_lines[i] != theirs_lines[i]
            {
                i += 1;
            }
            subchunks.push(Subchunk::Conflict {
                base: lines_to_text(&base_lines[start..i], line_ending),
                ours: lines_to_text(&ours_lines[start..i], line_ending),
                theirs: lines_to_text(&theirs_lines[start..i], line_ending),
            });
        }
    }

    subchunks
}

/// Merge two sets of line hunks (from base→ours and base→theirs diffs)
/// into a list of subchunks.
///
/// Non-overlapping single-side changes become `Resolved`. Overlapping changes
/// from both sides become `Conflict` (unless the replacement is identical or
/// the region can be further decomposed via per-line comparison).
/// Unchanged base regions become `Resolved` context.
fn merge_line_hunks(
    base_lines: &[&str],
    ours_hunks: &[LineHunk],
    theirs_hunks: &[LineHunk],
    line_ending: &str,
) -> Vec<Subchunk> {
    let mut result = Vec::new();
    let mut base_pos = 0usize;
    let mut oi = 0usize;
    let mut ti = 0usize;

    loop {
        let oh_start = ours_hunks
            .get(oi)
            .map(|h| h.base_start)
            .unwrap_or(usize::MAX);
        let th_start = theirs_hunks
            .get(ti)
            .map(|h| h.base_start)
            .unwrap_or(usize::MAX);

        if oh_start == usize::MAX && th_start == usize::MAX {
            // No more hunks — emit remaining base as context.
            if base_pos < base_lines.len() {
                result.push(Subchunk::Resolved(lines_to_text(
                    &base_lines[base_pos..],
                    line_ending,
                )));
            }
            break;
        }

        let change_start = oh_start.min(th_start);

        // Emit context (unchanged base lines) before the next change.
        if change_start > base_pos && base_pos < base_lines.len() {
            let ctx_end = change_start.min(base_lines.len());
            result.push(Subchunk::Resolved(lines_to_text(
                &base_lines[base_pos..ctx_end],
                line_ending,
            )));
            base_pos = ctx_end;
        }

        // Expand the change region to include all overlapping hunks from both sides.
        // First consume hunks that start exactly at change_start (the trigger),
        // then expand with strictly overlapping hunks (base_start < region_end).
        // This prevents adjacent but non-overlapping hunks from being merged.
        let mut region_end = base_pos;
        let oi_start = oi;
        let ti_start = ti;

        // Consume initial hunks at change_start.
        while let Some(oh) = ours_hunks.get(oi) {
            if oh.base_start == change_start {
                region_end = region_end.max(oh.base_end);
                oi += 1;
            } else {
                break;
            }
        }
        while let Some(th) = theirs_hunks.get(ti) {
            if th.base_start == change_start {
                region_end = region_end.max(th.base_end);
                ti += 1;
            } else {
                break;
            }
        }

        // Expand with hunks that strictly overlap (start before region_end).
        loop {
            let mut extended = false;

            while let Some(oh) = ours_hunks.get(oi) {
                if oh.base_start < region_end {
                    region_end = region_end.max(oh.base_end);
                    oi += 1;
                    extended = true;
                } else {
                    break;
                }
            }

            while let Some(th) = theirs_hunks.get(ti) {
                if th.base_start < region_end {
                    region_end = region_end.max(th.base_end);
                    ti += 1;
                    extended = true;
                } else {
                    break;
                }
            }

            if !extended {
                break;
            }
        }

        let oi_end = oi;
        let ti_end = ti;
        let ours_involved = oi_end > oi_start;
        let theirs_involved = ti_end > ti_start;
        let region_base_end = region_end.min(base_lines.len());

        if ours_involved && theirs_involved {
            let base_text = lines_to_text(&base_lines[base_pos..region_base_end], line_ending);
            let ours_text = side_content(
                base_lines,
                base_pos,
                region_end,
                &ours_hunks[oi_start..oi_end],
                line_ending,
            );
            let theirs_text = side_content(
                base_lines,
                base_pos,
                region_end,
                &theirs_hunks[ti_start..ti_end],
                line_ending,
            );

            if ours_text == theirs_text {
                result.push(Subchunk::Resolved(ours_text));
            } else {
                // Try per-line decomposition of the overlapping region.
                let sub_base = split_lines(&base_text);
                let sub_ours = split_lines(&ours_text);
                let sub_theirs = split_lines(&theirs_text);

                if sub_base.len() == sub_ours.len() && sub_base.len() == sub_theirs.len() {
                    result.extend(per_line_merge(
                        &sub_base,
                        &sub_ours,
                        &sub_theirs,
                        line_ending,
                    ));
                } else {
                    result.push(Subchunk::Conflict {
                        base: base_text,
                        ours: ours_text,
                        theirs: theirs_text,
                    });
                }
            }
        } else if ours_involved {
            let ours_text = side_content(
                base_lines,
                base_pos,
                region_end,
                &ours_hunks[oi_start..oi_end],
                line_ending,
            );
            result.push(Subchunk::Resolved(ours_text));
        } else if theirs_involved {
            let theirs_text = side_content(
                base_lines,
                base_pos,
                region_end,
                &theirs_hunks[ti_start..ti_end],
                line_ending,
            );
            result.push(Subchunk::Resolved(theirs_text));
        }

        base_pos = region_end;
    }

    result
}

/// Reconstruct one side's content for a base line range, applying the given hunks.
///
/// Between hunks, base lines are kept unchanged. Hunk ranges provide
/// replacement content.
fn side_content(
    base_lines: &[&str],
    range_start: usize,
    range_end: usize,
    hunks: &[LineHunk],
    line_ending: &str,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    reconstruct_side_with(
        base_lines,
        range_start,
        range_end,
        hunks,
        &mut lines,
        |h| h.base_start,
        |h| h.base_end,
        |line| line.to_string(),
        |h, out| out.extend(h.new_lines.iter().cloned()),
    );
    let line_refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    lines_to_text(&line_refs, line_ending)
}

/// Join a slice of line strings into text with the given line ending.
/// Each line gets a trailing line ending (matching conflict block content convention).
fn lines_to_text(lines: &[&str], line_ending: &str) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let le_len = line_ending.len();
    let total: usize = lines.iter().map(|l| l.len() + le_len).sum();
    let mut s = String::with_capacity(total);
    for line in lines {
        s.push_str(line);
        s.push_str(line_ending);
    }
    s
}
