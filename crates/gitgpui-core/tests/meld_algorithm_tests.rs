//! Meld-derived algorithm tests (Phase 5A/5B/5C).
//!
//! Ports test concepts from Meld's `test_matchers.py`, `test_misc.py`,
//! and `test_chunk_actions.py` as verification tests for gitgpui's diff
//! engine, interval utilities, and newline-aware text operations.

use gitgpui_core::text_utils::{
    MatchingBlock, SyncPointError, delete_last_line, matching_blocks_chars,
    matching_blocks_chars_with_sync_points, matching_blocks_lines,
    matching_blocks_lines_with_sync_points, merge_intervals,
};

// ---------------------------------------------------------------------------
// Invariant helpers
// ---------------------------------------------------------------------------

/// Verify that matching blocks are valid, ordered, and non-overlapping.
fn verify_matching_blocks_chars(a: &str, b: &str, blocks: &[MatchingBlock]) {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    for (i, block) in blocks.iter().enumerate() {
        assert!(
            block.a_start + block.length <= a_chars.len(),
            "block {i} exceeds bounds of sequence A"
        );
        assert!(
            block.b_start + block.length <= b_chars.len(),
            "block {i} exceeds bounds of sequence B"
        );

        // Content at indicated positions must match.
        for j in 0..block.length {
            assert_eq!(
                a_chars[block.a_start + j],
                b_chars[block.b_start + j],
                "block {i} content mismatch at offset {j}"
            );
        }
    }

    // Blocks must be in strictly increasing order and non-overlapping.
    for w in blocks.windows(2) {
        assert!(
            w[0].a_start + w[0].length <= w[1].a_start,
            "blocks overlap in sequence A: {:?} and {:?}",
            w[0],
            w[1]
        );
        assert!(
            w[0].b_start + w[0].length <= w[1].b_start,
            "blocks overlap in sequence B: {:?} and {:?}",
            w[0],
            w[1]
        );
    }
}

fn total_matched(blocks: &[MatchingBlock]) -> usize {
    blocks.iter().map(|b| b.length).sum()
}

// ===========================================================================
// Phase 5A — Myers matching blocks
// ===========================================================================

/// Meld `myers_basic` test concept.
/// Input: a = "abcbdefgabcdefg", b = "gfabcdefcd"
/// Meld expects blocks: [(0,2,3), (4,5,3), (10,8,2)] (total matched = 8).
///
/// Our Myers implementation may produce a different (but equally valid)
/// shortest-edit-script alignment. We verify invariants and that the total
/// matched length equals the LCS length.
#[test]
fn myers_matching_blocks_basic() {
    let a = "abcbdefgabcdefg";
    let b = "gfabcdefcd";

    let blocks = matching_blocks_chars(a, b);
    verify_matching_blocks_chars(a, b, &blocks);

    // LCS length for these inputs is 8.
    assert_eq!(
        total_matched(&blocks),
        8,
        "total matched length should equal LCS length; blocks: {blocks:?}"
    );
}

/// Meld `myers_postprocess` test concept.
/// Input: a = "abcfabgcd", b = "afabcgabgcabcd"
/// Meld expects blocks: [(0,2,3), (4,6,3), (7,12,2)] (total matched = 8).
#[test]
fn myers_matching_blocks_postprocess() {
    let a = "abcfabgcd";
    let b = "afabcgabgcabcd";

    let blocks = matching_blocks_chars(a, b);
    verify_matching_blocks_chars(a, b, &blocks);

    // LCS length for these inputs is 8.
    assert_eq!(
        total_matched(&blocks),
        8,
        "total matched length should equal LCS length; blocks: {blocks:?}"
    );
}

/// Meld `myers_inline_trigram` test concept.
/// Input: a = "red, blue, yellow, white", b = "black green, hue, white"
/// Meld expects only [(17,16,7)] due to trigram filtering.
///
/// Our standard Myers diff finds ALL common characters (no trigram filter),
/// producing more matching blocks across the strings. We verify invariants,
/// total matched length exceeds the trigram-filtered 7, and that the
/// "hite" suffix is matched (the tail of ", white").
#[test]
fn myers_matching_blocks_inline() {
    let a = "red, blue, yellow, white";
    let b = "black green, hue, white";

    let blocks = matching_blocks_chars(a, b);
    verify_matching_blocks_chars(a, b, &blocks);

    // Our unfiltered Myers finds more matches than Meld's
    // trigram-filtered result of 7 characters.
    assert!(
        total_matched(&blocks) >= 7,
        "should match at least 7 characters; blocks: {blocks:?}"
    );

    // Verify that the tail of both strings ("hite") is matched.
    // Our Myers aligns "w" separately due to earlier comma/space matches
    // consuming those characters, but "hite" at the end should form a
    // contiguous block.
    let matches_tail = blocks.iter().any(|bl| {
        let a_end = bl.a_start + bl.length;
        let b_end = bl.b_start + bl.length;
        a_end == a.len() && b_end == b.len() && bl.length >= 4
    });
    assert!(
        matches_tail,
        "should match the tail 'hite' as a block ending both strings; blocks: {blocks:?}"
    );
}

/// Meld `sync_point_none` test concept — same inputs as `basic` with no
/// sync points. Since our algorithm never uses sync points, the result
/// should be identical to the basic test.
#[test]
fn myers_matching_blocks_no_sync_points_same_as_basic() {
    let a = "abcbdefgabcdefg";
    let b = "gfabcdefcd";

    let blocks_basic = matching_blocks_chars(a, b);
    let blocks_no_sync =
        matching_blocks_chars_with_sync_points(a, b, &[]).expect("empty sync points");

    assert_eq!(blocks_basic, blocks_no_sync);
}

/// Meld `sync_point_one` concept: one sync point forces a different
/// (but deterministic) alignment than plain Myers.
#[test]
fn myers_matching_blocks_one_sync_point() {
    let a = "012a3456c789";
    let b = "0a3412b5678";

    let blocks = matching_blocks_chars_with_sync_points(a, b, &[(3, 6)]).expect("valid sync point");
    assert_eq!(
        blocks,
        vec![
            MatchingBlock {
                a_start: 0,
                b_start: 0,
                length: 1
            },
            MatchingBlock {
                a_start: 1,
                b_start: 4,
                length: 2
            },
            MatchingBlock {
                a_start: 6,
                b_start: 7,
                length: 2
            },
            MatchingBlock {
                a_start: 9,
                b_start: 9,
                length: 2
            },
        ]
    );
}

/// Meld `sync_point_two` concept: two sync points force chunk-local
/// matching and produce a stable block layout.
#[test]
fn myers_matching_blocks_two_sync_points() {
    let a = "012a3456c789";
    let b = "02a341b5678";

    let blocks =
        matching_blocks_chars_with_sync_points(a, b, &[(3, 2), (8, 6)]).expect("valid sync points");
    assert_eq!(
        blocks,
        vec![
            MatchingBlock {
                a_start: 0,
                b_start: 0,
                length: 1
            },
            MatchingBlock {
                a_start: 2,
                b_start: 1,
                length: 1
            },
            MatchingBlock {
                a_start: 3,
                b_start: 2,
                length: 3
            },
            MatchingBlock {
                a_start: 9,
                b_start: 9,
                length: 2
            },
        ]
    );
}

#[test]
fn myers_matching_blocks_sync_point_validation() {
    let err = matching_blocks_chars_with_sync_points("abc", "abc", &[(4, 1)])
        .expect_err("sync point should be out of bounds");
    assert!(matches!(
        err,
        SyncPointError::OutOfBounds {
            index: 0,
            a_pos: 4,
            b_pos: 1,
            a_len: 3,
            b_len: 3
        }
    ));

    let err = matching_blocks_chars_with_sync_points("abcdef", "abcdef", &[(2, 2), (2, 3)])
        .expect_err("sync points should be strictly increasing");
    assert!(matches!(
        err,
        SyncPointError::NotStrictlyIncreasing {
            index: 1,
            prev_a: 2,
            prev_b: 2,
            a_pos: 2,
            b_pos: 3
        }
    ));
}

/// Line-level matching blocks for simple sequences.
#[test]
fn matching_blocks_lines_basic() {
    let a = &["aaa", "bbb", "ccc", "ddd"][..];
    let b = &["aaa", "xxx", "ccc", "ddd"][..];

    let blocks = matching_blocks_lines(a, b);
    assert_eq!(
        blocks,
        vec![
            MatchingBlock {
                a_start: 0,
                b_start: 0,
                length: 1
            },
            MatchingBlock {
                a_start: 2,
                b_start: 2,
                length: 2
            },
        ]
    );
}

/// Line-level matching blocks with completely disjoint sequences.
#[test]
fn matching_blocks_lines_no_common() {
    let a = &["aaa", "bbb"][..];
    let b = &["xxx", "yyy"][..];

    let blocks = matching_blocks_lines(a, b);
    assert!(blocks.is_empty());
}

/// Line-level matching blocks with identical sequences.
#[test]
fn matching_blocks_lines_identical() {
    let a = &["aaa", "bbb", "ccc"][..];
    let blocks = matching_blocks_lines(a, a);
    assert_eq!(
        blocks,
        vec![MatchingBlock {
            a_start: 0,
            b_start: 0,
            length: 3
        }]
    );
}

/// Line-level matching blocks with empty inputs.
#[test]
fn matching_blocks_lines_empty() {
    let empty: &[&str] = &[];
    let a = &["aaa"][..];

    assert!(matching_blocks_lines(empty, empty).is_empty());
    assert!(matching_blocks_lines(empty, a).is_empty());
    assert!(matching_blocks_lines(a, empty).is_empty());
}

#[test]
fn matching_blocks_lines_sync_points_respected() {
    let a = &["a0", "a1", "a2", "a3"][..];
    let b = &["a0", "b1", "a2", "a3"][..];

    let blocks = matching_blocks_lines_with_sync_points(a, b, &[(1, 1)]).expect("valid sync");
    assert_eq!(
        blocks,
        vec![
            MatchingBlock {
                a_start: 0,
                b_start: 0,
                length: 1
            },
            MatchingBlock {
                a_start: 2,
                b_start: 2,
                length: 2
            }
        ]
    );
}

// ===========================================================================
// Phase 5B — Interval merging
// ===========================================================================

/// Meld `intervals_dominated`: one interval dominates all others.
#[test]
fn intervals_dominated() {
    let input = [(1, 5), (5, 9), (10, 11), (0, 20)];
    assert_eq!(merge_intervals(&input), vec![(0, 20)]);
}

/// Meld `intervals_disjoint`: no intervals overlap or touch.
#[test]
fn intervals_disjoint() {
    let input = [(1, 5), (7, 9), (11, 13)];
    assert_eq!(merge_intervals(&input), vec![(1, 5), (7, 9), (11, 13)]);
}

/// Meld `intervals_two_groups`: two pairs of touching intervals.
#[test]
fn intervals_two_groups() {
    let input = [(1, 5), (5, 9), (10, 12), (11, 20)];
    assert_eq!(merge_intervals(&input), vec![(1, 9), (10, 20)]);
}

/// Meld `intervals_unsorted`: same as two_groups but unsorted input.
#[test]
fn intervals_unsorted() {
    let input = [(11, 20), (5, 9), (10, 12), (1, 5)];
    assert_eq!(merge_intervals(&input), vec![(1, 9), (10, 20)]);
}

/// Meld `intervals_duplicate`: duplicated intervals are deduplicated.
#[test]
fn intervals_duplicate() {
    let input = [(1, 5), (7, 8), (1, 5)];
    assert_eq!(merge_intervals(&input), vec![(1, 5), (7, 8)]);
}

/// Meld `intervals_chain`: overlapping chain merges into one.
#[test]
fn intervals_chain() {
    let input = [(1, 5), (4, 10), (9, 15)];
    assert_eq!(merge_intervals(&input), vec![(1, 15)]);
}

/// Edge case: empty input.
#[test]
fn intervals_empty() {
    let input: &[(usize, usize)] = &[];
    assert!(merge_intervals(input).is_empty());
}

/// Edge case: single interval.
#[test]
fn intervals_single() {
    let input = [(3, 7)];
    assert_eq!(merge_intervals(&input), vec![(3, 7)]);
}

// ===========================================================================
// Phase 5C — Newline-aware operations
// ===========================================================================

/// Meld `delete_last_line_crlf`: CRLF-separated, no trailing newline.
#[test]
fn delete_last_line_crlf() {
    assert_eq!(delete_last_line("ree\r\neee"), "ree");
}

/// Meld `delete_last_line_crlf_trail`: CRLF-separated, trailing CRLF.
#[test]
fn delete_last_line_crlf_trail() {
    assert_eq!(delete_last_line("ree\r\neee\r\n"), "ree\r\neee");
}

/// Meld `delete_last_line_lf`: LF-separated, no trailing newline.
#[test]
fn delete_last_line_lf() {
    assert_eq!(delete_last_line("ree\neee"), "ree");
}

/// Meld `delete_last_line_lf_trail`: LF-separated, trailing LF.
#[test]
fn delete_last_line_lf_trail() {
    assert_eq!(delete_last_line("ree\neee\n"), "ree\neee");
}

/// Meld `delete_last_line_cr`: CR-separated, no trailing newline.
#[test]
fn delete_last_line_cr() {
    assert_eq!(delete_last_line("ree\reee"), "ree");
}

/// Meld `delete_last_line_cr_trail`: CR-separated, trailing CR.
#[test]
fn delete_last_line_cr_trail() {
    assert_eq!(delete_last_line("ree\reee\r"), "ree\reee");
}

/// Meld `delete_last_line_mixed`: mixed line endings.
#[test]
fn delete_last_line_mixed() {
    assert_eq!(delete_last_line("ree\r\neee\nqqq"), "ree\r\neee");
}

/// Edge case: empty input.
#[test]
fn delete_last_line_empty() {
    assert_eq!(delete_last_line(""), "");
}

/// Edge case: single character (no newline).
#[test]
fn delete_last_line_single_char() {
    assert_eq!(delete_last_line("x"), "");
}

/// Edge case: single newline.
#[test]
fn delete_last_line_single_newline() {
    assert_eq!(delete_last_line("\n"), "");
}

/// Edge case: single CRLF.
#[test]
fn delete_last_line_single_crlf() {
    assert_eq!(delete_last_line("\r\n"), "");
}

/// Edge case: only content on first line, newline at end.
#[test]
fn delete_last_line_single_line_with_newline() {
    assert_eq!(delete_last_line("hello\n"), "hello");
}
