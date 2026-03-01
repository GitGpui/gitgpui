//! Text processing utilities for diff, merge, and text editing operations.
//!
//! Provides:
//! - Matching block extraction from sequence diffs
//! - Interval coalescing for overlapping ranges
//! - Newline-aware text manipulation

use crate::file_diff::{Edit, EditKind, myers_edits};
use std::fmt;

/// A contiguous matching block between two sequences.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MatchingBlock {
    /// Start position in sequence A.
    pub a_start: usize,
    /// Start position in sequence B.
    pub b_start: usize,
    /// Length of the matching block.
    pub length: usize,
}

/// Validation error for sync-point-constrained matching.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyncPointError {
    /// A sync point references a position outside either input sequence.
    OutOfBounds {
        index: usize,
        a_pos: usize,
        b_pos: usize,
        a_len: usize,
        b_len: usize,
    },
    /// Sync points must be strictly increasing in both sequences.
    NotStrictlyIncreasing {
        index: usize,
        prev_a: usize,
        prev_b: usize,
        a_pos: usize,
        b_pos: usize,
    },
}

impl fmt::Display for SyncPointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncPointError::OutOfBounds {
                index,
                a_pos,
                b_pos,
                a_len,
                b_len,
            } => write!(
                f,
                "sync point #{index} ({a_pos}, {b_pos}) is out of bounds for lengths ({a_len}, {b_len})"
            ),
            SyncPointError::NotStrictlyIncreasing {
                index,
                prev_a,
                prev_b,
                a_pos,
                b_pos,
            } => write!(
                f,
                "sync point #{index} ({a_pos}, {b_pos}) is not strictly increasing after ({prev_a}, {prev_b})"
            ),
        }
    }
}

impl std::error::Error for SyncPointError {}

/// Extract matching blocks between two strings at the character level.
///
/// Uses Myers diff to find an optimal alignment, then returns contiguous
/// runs of matching characters as blocks. Blocks are returned in order
/// and do not overlap.
pub fn matching_blocks_chars(a: &str, b: &str) -> Vec<MatchingBlock> {
    matching_blocks_chars_with_sync_points(a, b, &[])
        .expect("empty sync point list is always valid")
}

/// Extract matching blocks between two strings at the character level, while
/// forcing alignment to respect caller-provided sync points.
///
/// Each sync point `(ai, bi)` splits the input into independent diff chunks:
/// `a[prev_ai..ai]` is matched only against `b[prev_bi..bi]`. This mirrors
/// Meld's sync-point matcher behavior and allows deterministic alignment in
/// ambiguous regions.
pub fn matching_blocks_chars_with_sync_points(
    a: &str,
    b: &str,
    sync_points: &[(usize, usize)],
) -> Result<Vec<MatchingBlock>, SyncPointError> {
    let a_strs: Vec<String> = a.chars().map(|c| c.to_string()).collect();
    let b_strs: Vec<String> = b.chars().map(|c| c.to_string()).collect();
    let a_refs: Vec<&str> = a_strs.iter().map(String::as_str).collect();
    let b_refs: Vec<&str> = b_strs.iter().map(String::as_str).collect();

    matching_blocks_with_sync_points(&a_refs, &b_refs, sync_points)
}

/// Extract matching blocks between two line sequences.
///
/// Uses Myers diff on the line arrays and returns contiguous runs
/// of matching lines as blocks.
pub fn matching_blocks_lines<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<MatchingBlock> {
    matching_blocks_lines_with_sync_points(a, b, &[]).expect("empty sync point list is valid")
}

/// Extract matching blocks between two line sequences with sync-point
/// constraints.
pub fn matching_blocks_lines_with_sync_points<'a>(
    a: &[&'a str],
    b: &[&'a str],
    sync_points: &[(usize, usize)],
) -> Result<Vec<MatchingBlock>, SyncPointError> {
    matching_blocks_with_sync_points(a, b, sync_points)
}

fn matching_blocks_with_sync_points(
    a: &[&str],
    b: &[&str],
    sync_points: &[(usize, usize)],
) -> Result<Vec<MatchingBlock>, SyncPointError> {
    validate_sync_points(sync_points, a.len(), b.len())?;

    if sync_points.is_empty() {
        let edits = myers_edits(a, b);
        return Ok(edits_to_matching_blocks(&edits));
    }

    let mut blocks = Vec::new();
    let mut a_start = 0usize;
    let mut b_start = 0usize;

    for &(a_end, b_end) in sync_points {
        append_segment_blocks(a, b, a_start, a_end, b_start, b_end, &mut blocks);
        a_start = a_end;
        b_start = b_end;
    }

    append_segment_blocks(a, b, a_start, a.len(), b_start, b.len(), &mut blocks);

    Ok(blocks)
}

fn append_segment_blocks(
    a: &[&str],
    b: &[&str],
    a_start: usize,
    a_end: usize,
    b_start: usize,
    b_end: usize,
    out: &mut Vec<MatchingBlock>,
) {
    let segment_edits = myers_edits(&a[a_start..a_end], &b[b_start..b_end]);
    let segment_blocks = edits_to_matching_blocks(&segment_edits);
    for block in segment_blocks {
        out.push(MatchingBlock {
            a_start: a_start + block.a_start,
            b_start: b_start + block.b_start,
            length: block.length,
        });
    }
}

fn validate_sync_points(
    sync_points: &[(usize, usize)],
    a_len: usize,
    b_len: usize,
) -> Result<(), SyncPointError> {
    let mut prev: Option<(usize, usize)> = None;
    for (index, &(a_pos, b_pos)) in sync_points.iter().enumerate() {
        if a_pos > a_len || b_pos > b_len {
            return Err(SyncPointError::OutOfBounds {
                index,
                a_pos,
                b_pos,
                a_len,
                b_len,
            });
        }

        if let Some((prev_a, prev_b)) = prev {
            if a_pos <= prev_a || b_pos <= prev_b {
                return Err(SyncPointError::NotStrictlyIncreasing {
                    index,
                    prev_a,
                    prev_b,
                    a_pos,
                    b_pos,
                });
            }
        }
        prev = Some((a_pos, b_pos));
    }
    Ok(())
}

fn edits_to_matching_blocks(edits: &[Edit<'_>]) -> Vec<MatchingBlock> {
    let mut blocks = Vec::new();
    let mut a_pos = 0usize;
    let mut b_pos = 0usize;
    let mut match_start: Option<(usize, usize)> = None;
    let mut match_len = 0usize;

    for edit in edits {
        match edit.kind {
            EditKind::Equal => {
                if match_start.is_none() {
                    match_start = Some((a_pos, b_pos));
                    match_len = 0;
                }
                match_len += 1;
                a_pos += 1;
                b_pos += 1;
            }
            EditKind::Delete => {
                if let Some((sa, sb)) = match_start.take() {
                    blocks.push(MatchingBlock {
                        a_start: sa,
                        b_start: sb,
                        length: match_len,
                    });
                }
                a_pos += 1;
            }
            EditKind::Insert => {
                if let Some((sa, sb)) = match_start.take() {
                    blocks.push(MatchingBlock {
                        a_start: sa,
                        b_start: sb,
                        length: match_len,
                    });
                }
                b_pos += 1;
            }
        }
    }

    if let Some((sa, sb)) = match_start {
        blocks.push(MatchingBlock {
            a_start: sa,
            b_start: sb,
            length: match_len,
        });
    }

    blocks
}

/// Merge overlapping or adjacent intervals into non-overlapping intervals.
///
/// Each interval is `(start, end)` inclusive on both ends. Intervals that
/// touch (one ends where another starts) are merged. The result is sorted
/// by start position with no overlaps.
pub fn merge_intervals(intervals: &[(usize, usize)]) -> Vec<(usize, usize)> {
    if intervals.is_empty() {
        return Vec::new();
    }

    let mut sorted: Vec<(usize, usize)> = intervals.to_vec();
    sorted.sort_unstable();

    let mut result = vec![sorted[0]];

    for &(start, end) in &sorted[1..] {
        let last = result.last_mut().unwrap();
        if start <= last.1 {
            last.1 = last.1.max(end);
        } else {
            result.push((start, end));
        }
    }

    result
}

/// Delete the last line of text, respecting line endings.
///
/// If the text ends with a line ending (`\n`, `\r\n`, or `\r`), removes that
/// trailing line ending (effectively deleting the empty last line).
/// Otherwise, finds the last line ending and removes everything from there
/// to the end of the string (the last line and its preceding separator).
///
/// Returns an empty string if the text is empty or has no line endings
/// (single line).
pub fn delete_last_line(text: &str) -> &str {
    let bytes = text.as_bytes();
    let len = bytes.len();

    if len == 0 {
        return "";
    }

    // If text ends with a line ending, strip just that ending.
    if len >= 2 && bytes[len - 2] == b'\r' && bytes[len - 1] == b'\n' {
        return &text[..len - 2];
    }
    if bytes[len - 1] == b'\n' || bytes[len - 1] == b'\r' {
        return &text[..len - 1];
    }

    // Text doesn't end with a line ending.
    // Find the last line ending and remove from there to end.
    if len < 2 {
        return "";
    }

    let mut pos = len - 2;
    loop {
        match bytes[pos] {
            b'\n' => {
                if pos > 0 && bytes[pos - 1] == b'\r' {
                    return &text[..pos - 1];
                }
                return &text[..pos];
            }
            b'\r' => {
                return &text[..pos];
            }
            _ => {}
        }
        if pos == 0 {
            break;
        }
        pos -= 1;
    }

    // No line ending found — single line.
    ""
}
