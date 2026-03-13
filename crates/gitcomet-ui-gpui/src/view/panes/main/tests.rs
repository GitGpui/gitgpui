use super::{
    ClearDiffSelectionAction, ResolvedOutputConflictMarker, apply_conflict_choice_provenance_hints,
    apply_three_way_empty_base_provenance_hints, build_line_starts,
    build_resolved_output_conflict_markers, clear_diff_selection_action,
    conflict_marker_nav_entries_from_markers, conflict_resolver_output_context_line,
    dirty_byte_range_to_line_range, first_output_marker_line_for_conflict,
    focused_mergetool_save_exit_code, output_line_range_for_conflict_block_in_text,
    pane_content_width_for_layout, parse_conflict_canvas_rows_env,
    remap_line_keyed_cache_for_delta, replace_output_lines_in_range,
    resolved_outline_delta_between_texts, resolved_output_conflict_block_ranges_in_text,
    resolved_output_marker_for_line, resolved_output_markers_for_text,
    split_target_conflict_block_into_subchunks,
};
use crate::view::GitCometViewMode;
use crate::view::conflict_resolver::{
    self, ConflictBlock, ConflictChoice, ConflictResolverViewMode, ConflictSegment,
    ResolvedLineSource, SourceLines,
};
use rustc_hash::FxHashMap as HashMap;

#[test]
fn clear_diff_selection_action_is_clear_for_normal_mode() {
    assert_eq!(
        clear_diff_selection_action(GitCometViewMode::Normal),
        ClearDiffSelectionAction::ClearSelection
    );
}

#[test]
fn clear_diff_selection_action_exits_focused_mergetool_mode() {
    assert_eq!(
        clear_diff_selection_action(GitCometViewMode::FocusedMergetool),
        ClearDiffSelectionAction::ExitFocusedMergetool
    );
}

#[test]
fn focused_mergetool_save_exit_code_is_success_when_all_resolved() {
    assert_eq!(focused_mergetool_save_exit_code(0, 0), 0);
    assert_eq!(focused_mergetool_save_exit_code(3, 3), 0);
}

#[test]
fn focused_mergetool_save_exit_code_is_canceled_when_unresolved_remain() {
    assert_eq!(focused_mergetool_save_exit_code(3, 2), 1);
}

#[test]
fn parse_conflict_canvas_rows_env_accepts_truthy_values() {
    assert!(parse_conflict_canvas_rows_env("1"));
    assert!(parse_conflict_canvas_rows_env("true"));
    assert!(parse_conflict_canvas_rows_env("on"));
    assert!(parse_conflict_canvas_rows_env("yes"));
    assert!(parse_conflict_canvas_rows_env("maybe"));
}

#[test]
fn parse_conflict_canvas_rows_env_rejects_falsey_values() {
    assert!(!parse_conflict_canvas_rows_env("0"));
    assert!(!parse_conflict_canvas_rows_env("false"));
    assert!(!parse_conflict_canvas_rows_env("off"));
    assert!(!parse_conflict_canvas_rows_env("no"));
}

#[test]
fn replace_output_lines_in_range_replaces_only_target_chunk() {
    let output = "top\nkeep\nalso-keep\nbottom";
    let replacement = vec!["picked".to_string()];
    let next = replace_output_lines_in_range(output, 1..3, &replacement);
    assert_eq!(next, "top\npicked\nbottom");
}

#[test]
fn replace_output_lines_in_range_preserves_trailing_newline() {
    let output = "a\nb\n";
    let replacement = vec!["x".to_string(), "y".to_string()];
    let next = replace_output_lines_in_range(output, 1..2, &replacement);
    assert_eq!(next, "a\nx\ny\n");
}

#[test]
fn resolved_outline_delta_between_texts_clamps_to_utf8_boundaries() {
    let old_text = "prefix ä\nsuffix";
    let new_text = "prefix ö\nsuffix";
    let delta = resolved_outline_delta_between_texts(old_text, new_text).expect("delta");
    assert_eq!(old_text.get(delta.old_range.clone()), Some("ä"));
    assert_eq!(new_text.get(delta.new_range.clone()), Some("ö"));
}

#[test]
fn dirty_byte_range_to_line_range_includes_line_join_delete() {
    let text = "a\nb\nc";
    let line_starts = build_line_starts(text);
    // Delete the newline between "a" and "b".
    let dirty = dirty_byte_range_to_line_range(&line_starts, text.len(), 1..2);
    assert_eq!(dirty, 0..2);
}

#[test]
fn remap_line_keyed_cache_for_delta_shifts_suffix_entries() {
    let mut cache: HashMap<usize, usize> = HashMap::default();
    cache.insert(0, 10);
    cache.insert(4, 40);
    cache.insert(7, 70);

    remap_line_keyed_cache_for_delta(&mut cache, 2..5, 2..3);
    assert_eq!(cache.get(&0), Some(&10));
    assert_eq!(cache.get(&4), None);
    assert_eq!(cache.get(&5), Some(&70));
}

#[test]
fn resolved_output_conflict_block_ranges_match_point_lookup() {
    let segments = vec![
        ConflictSegment::Text("top\n".to_string()),
        ConflictSegment::Block(ConflictBlock {
            base: None,
            ours: "a\n".to_string(),
            theirs: "x\n".to_string(),
            choice: ConflictChoice::Ours,
            resolved: true,
        }),
        ConflictSegment::Text("mid\n".to_string()),
        ConflictSegment::Block(ConflictBlock {
            base: None,
            ours: "b\nc\n".to_string(),
            theirs: "y\n".to_string(),
            choice: ConflictChoice::Ours,
            resolved: true,
        }),
    ];
    let output = conflict_resolver::generate_resolved_text(&segments);
    let ranges =
        resolved_output_conflict_block_ranges_in_text(&segments, &output).expect("block ranges");
    assert_eq!(ranges.len(), 2);
    assert_eq!(
        output_line_range_for_conflict_block_in_text(&segments, &output, 0),
        ranges.first().cloned()
    );
    assert_eq!(
        output_line_range_for_conflict_block_in_text(&segments, &output, 1),
        ranges.get(1).cloned()
    );
}

#[test]
fn output_line_range_for_conflict_block_in_text_maps_middle_blocks_exactly() {
    let segments = vec![
        ConflictSegment::Text("top\n".to_string()),
        ConflictSegment::Block(ConflictBlock {
            base: None,
            ours: "a\n".to_string(),
            theirs: "x\ny\n".to_string(),
            choice: ConflictChoice::Ours,
            resolved: true,
        }),
        ConflictSegment::Text("mid\n".to_string()),
        ConflictSegment::Block(ConflictBlock {
            base: None,
            ours: "b\nc\n".to_string(),
            theirs: "z\n".to_string(),
            choice: ConflictChoice::Ours,
            resolved: true,
        }),
        ConflictSegment::Text("tail\n".to_string()),
    ];

    let output = conflict_resolver::generate_resolved_text(&segments);
    assert_eq!(
        output_line_range_for_conflict_block_in_text(&segments, &output, 0),
        Some(1..2)
    );
    assert_eq!(
        output_line_range_for_conflict_block_in_text(&segments, &output, 1),
        Some(3..5)
    );
}

#[test]
fn output_line_range_for_conflict_block_in_text_maps_eof_block_without_newline() {
    let segments = vec![
        ConflictSegment::Text("top\n".to_string()),
        ConflictSegment::Block(ConflictBlock {
            base: None,
            ours: "tail".to_string(),
            theirs: "other".to_string(),
            choice: ConflictChoice::Ours,
            resolved: true,
        }),
    ];

    let output = conflict_resolver::generate_resolved_text(&segments);
    assert_eq!(
        output_line_range_for_conflict_block_in_text(&segments, &output, 0),
        Some(1..2)
    );
}

#[test]
fn output_line_range_for_conflict_block_in_text_returns_none_when_output_drifts() {
    let segments = vec![
        ConflictSegment::Text("top\n".to_string()),
        ConflictSegment::Block(ConflictBlock {
            base: None,
            ours: "a\n".to_string(),
            theirs: "x\n".to_string(),
            choice: ConflictChoice::Ours,
            resolved: true,
        }),
        ConflictSegment::Text("mid\n".to_string()),
        ConflictSegment::Block(ConflictBlock {
            base: None,
            ours: "b\n".to_string(),
            theirs: "y\n".to_string(),
            choice: ConflictChoice::Ours,
            resolved: true,
        }),
    ];

    let drifted_output = "top\ndrift\nmid\nb\n";
    assert_eq!(
        output_line_range_for_conflict_block_in_text(&segments, drifted_output, 1),
        None
    );
}

#[test]
fn build_resolved_output_conflict_markers_maps_chunk_boundaries() {
    let segments = vec![
        ConflictSegment::Text("top\n".to_string()),
        ConflictSegment::Block(ConflictBlock {
            base: None,
            ours: "a\n".to_string(),
            theirs: "x\ny\n".to_string(),
            choice: ConflictChoice::Ours,
            resolved: true,
        }),
        ConflictSegment::Text("mid\n".to_string()),
        ConflictSegment::Block(ConflictBlock {
            base: None,
            ours: "b\nc\n".to_string(),
            theirs: "z\n".to_string(),
            choice: ConflictChoice::Ours,
            resolved: true,
        }),
        ConflictSegment::Text("tail\n".to_string()),
    ];

    let output = conflict_resolver::generate_resolved_text(&segments);
    let line_count = conflict_resolver::split_output_lines_for_outline(&output).len();
    let markers = build_resolved_output_conflict_markers(&segments, &output, line_count);

    assert_eq!(
        markers[1],
        Some(ResolvedOutputConflictMarker {
            conflict_ix: 0,
            range_start: 1,
            range_end: 2,
            is_start: true,
            is_end: true,
            unresolved: false,
        })
    );
    assert_eq!(
        markers[3],
        Some(ResolvedOutputConflictMarker {
            conflict_ix: 1,
            range_start: 3,
            range_end: 5,
            is_start: true,
            is_end: false,
            unresolved: false,
        })
    );
    assert_eq!(
        markers[4],
        Some(ResolvedOutputConflictMarker {
            conflict_ix: 1,
            range_start: 3,
            range_end: 5,
            is_start: false,
            is_end: true,
            unresolved: false,
        })
    );
}

#[test]
fn build_resolved_output_conflict_markers_anchors_zero_length_ranges() {
    let segments = vec![
        ConflictSegment::Text("top\n".to_string()),
        ConflictSegment::Block(ConflictBlock {
            base: Some(String::new()),
            ours: String::new(),
            theirs: "x\n".to_string(),
            choice: ConflictChoice::Ours,
            resolved: true,
        }),
        ConflictSegment::Text("tail\n".to_string()),
    ];

    let output = conflict_resolver::generate_resolved_text(&segments);
    let line_count = conflict_resolver::split_output_lines_for_outline(&output).len();
    let markers = build_resolved_output_conflict_markers(&segments, &output, line_count);

    assert_eq!(
        markers[1],
        Some(ResolvedOutputConflictMarker {
            conflict_ix: 0,
            range_start: 1,
            range_end: 1,
            is_start: true,
            is_end: true,
            unresolved: false,
        })
    );
}

#[test]
fn build_resolved_output_conflict_markers_marks_unresolved_blocks() {
    let segments = vec![
        ConflictSegment::Text("top\n".to_string()),
        ConflictSegment::Block(ConflictBlock {
            base: None,
            ours: "a\n".to_string(),
            theirs: "x\n".to_string(),
            choice: ConflictChoice::Ours,
            resolved: false,
        }),
        ConflictSegment::Text("tail\n".to_string()),
    ];

    let output = conflict_resolver::generate_resolved_text(&segments);
    let line_count = conflict_resolver::split_output_lines_for_outline(&output).len();
    let markers = build_resolved_output_conflict_markers(&segments, &output, line_count);

    assert_eq!(
        markers[1],
        Some(ResolvedOutputConflictMarker {
            conflict_ix: 0,
            range_start: 1,
            range_end: 2,
            is_start: true,
            is_end: true,
            unresolved: true,
        })
    );
}

#[test]
fn conflict_marker_nav_entries_include_only_marker_starts() {
    let markers = vec![
        None,
        Some(ResolvedOutputConflictMarker {
            conflict_ix: 0,
            range_start: 1,
            range_end: 3,
            is_start: true,
            is_end: false,
            unresolved: true,
        }),
        Some(ResolvedOutputConflictMarker {
            conflict_ix: 0,
            range_start: 1,
            range_end: 3,
            is_start: false,
            is_end: true,
            unresolved: true,
        }),
        Some(ResolvedOutputConflictMarker {
            conflict_ix: 1,
            range_start: 3,
            range_end: 4,
            is_start: true,
            is_end: true,
            unresolved: false,
        }),
    ];
    assert_eq!(
        conflict_marker_nav_entries_from_markers(&markers),
        vec![1, 3]
    );
}

#[test]
fn conflict_marker_nav_entries_dedup_conflicts_with_multiple_start_ranges() {
    let markers = vec![
        None,
        Some(ResolvedOutputConflictMarker {
            conflict_ix: 0,
            range_start: 1,
            range_end: 3,
            is_start: true,
            is_end: false,
            unresolved: true,
        }),
        Some(ResolvedOutputConflictMarker {
            conflict_ix: 0,
            range_start: 1,
            range_end: 3,
            is_start: false,
            is_end: true,
            unresolved: true,
        }),
        None,
        Some(ResolvedOutputConflictMarker {
            conflict_ix: 0,
            range_start: 4,
            range_end: 5,
            is_start: true,
            is_end: true,
            unresolved: true,
        }),
        Some(ResolvedOutputConflictMarker {
            conflict_ix: 1,
            range_start: 5,
            range_end: 6,
            is_start: true,
            is_end: true,
            unresolved: false,
        }),
    ];
    assert_eq!(
        conflict_marker_nav_entries_from_markers(&markers),
        vec![1, 5]
    );
}

#[test]
fn first_output_marker_line_for_conflict_returns_first_start() {
    let markers = vec![
        None,
        Some(ResolvedOutputConflictMarker {
            conflict_ix: 0,
            range_start: 1,
            range_end: 3,
            is_start: true,
            is_end: false,
            unresolved: true,
        }),
        Some(ResolvedOutputConflictMarker {
            conflict_ix: 0,
            range_start: 1,
            range_end: 3,
            is_start: false,
            is_end: true,
            unresolved: true,
        }),
        Some(ResolvedOutputConflictMarker {
            conflict_ix: 0,
            range_start: 3,
            range_end: 4,
            is_start: true,
            is_end: true,
            unresolved: true,
        }),
        Some(ResolvedOutputConflictMarker {
            conflict_ix: 1,
            range_start: 4,
            range_end: 5,
            is_start: true,
            is_end: true,
            unresolved: false,
        }),
    ];

    assert_eq!(first_output_marker_line_for_conflict(&markers, 0), Some(1));
    assert_eq!(first_output_marker_line_for_conflict(&markers, 1), Some(4));
    assert_eq!(first_output_marker_line_for_conflict(&markers, 2), None);
}

#[test]
fn conflict_resolver_output_context_line_prefers_clicked_offset() {
    let content = "top\nmiddle\nbottom\n";
    let cursor_offset = 0usize;
    let clicked_offset = "top\nmiddle\n".len();
    assert_eq!(
        conflict_resolver_output_context_line(content, cursor_offset, Some(clicked_offset)),
        2
    );
    assert_eq!(
        conflict_resolver_output_context_line(content, "top\n".len(), None),
        1
    );
}

#[test]
fn clicked_unresolved_line_maps_to_chunk_marker() {
    let segments = vec![
        ConflictSegment::Text("top\n".to_string()),
        ConflictSegment::Block(ConflictBlock {
            base: None,
            ours: "ours-1\nours-2\n".to_string(),
            theirs: "theirs-1\ntheirs-2\n".to_string(),
            choice: ConflictChoice::Ours,
            resolved: false,
        }),
        ConflictSegment::Text("tail\n".to_string()),
    ];
    let output = conflict_resolver::generate_resolved_text(&segments);
    let cursor_offset = 0usize;
    let clicked_offset = "top\nours-1\n".len();
    let clicked_line =
        conflict_resolver_output_context_line(&output, cursor_offset, Some(clicked_offset));
    let marker = resolved_output_marker_for_line(&segments, &output, clicked_line).expect("marker");
    assert!(marker.unresolved);
    assert_eq!(marker.conflict_ix, 0);
}

#[test]
fn build_resolved_output_conflict_markers_splits_unresolved_subchunks() {
    let segments = vec![
        ConflictSegment::Text("pre\n".to_string()),
        ConflictSegment::Block(ConflictBlock {
            base: Some("a\ncommon\nb\n".to_string()),
            ours: "ao\ncommon\nbo\n".to_string(),
            theirs: "at\ncommon\nbt\n".to_string(),
            choice: ConflictChoice::Base,
            resolved: false,
        }),
        ConflictSegment::Text("post\n".to_string()),
    ];

    let output = conflict_resolver::generate_resolved_text(&segments);
    let line_count = conflict_resolver::split_output_lines_for_outline(&output).len();
    let markers = build_resolved_output_conflict_markers(&segments, &output, line_count);

    let starts = markers
        .iter()
        .flatten()
        .filter(|m| m.conflict_ix == 0 && m.is_start)
        .count();
    assert_eq!(starts, 2, "expected two unresolved subchunk starts");
    assert!(
        markers.get(2).is_some_and(|m| m.is_none()),
        "resolved middle line should not be marked as conflict"
    );
}

#[test]
fn build_resolved_output_conflict_markers_splits_method_edit_and_trailing_insertion() {
    let segments = vec![ConflictSegment::Block(ConflictBlock {
            base: Some(
                "pub fn opposite(self) -> Color {\n    match self {\n        Color::White => Color::Black,\n        Color::Black => Color::White,\n    }\n}\n"
                    .to_string(),
            ),
            ours: "pub fn opposite(self) -> Color {\n    match self {\n        Color::White => Color::Black,\n        Color::Black => Color::White,\n    }\n}\n"
                .to_string(),
            theirs: "pub fn opposite(self) -> Self {\n    match self {\n        Self::White => Self::Black,\n        Self::Black => Self::White,\n    }\n}\n\npub fn name(self) -> &'static str {\n    match self {\n        Self::White => \"White\",\n        Self::Black => \"Black\",\n    }\n}\n"
                .to_string(),
            choice: ConflictChoice::Ours,
            resolved: false,
        })];

    let output = conflict_resolver::generate_resolved_text(&segments);
    let line_count = conflict_resolver::split_output_lines_for_outline(&output).len();
    let markers = build_resolved_output_conflict_markers(&segments, &output, line_count);

    let starts = markers
        .iter()
        .flatten()
        .filter(|m| m.conflict_ix == 0 && m.is_start)
        .count();
    assert_eq!(starts, 2, "expected two decision marker starts");
}

#[test]
fn build_resolved_output_conflict_markers_matches_combined_conflict_marker_case() {
    let conflict_text = "impl Color {\n<<<<<<< HEAD\n    pub fn opposite(self) -> Color {\n        match self {\n            Color::White => Color::Black,\n            Color::Black => Color::White,\n=======\n    pub fn opposite(self) -> Self {\n        match self {\n            Self::White => Self::Black,\n            Self::Black => Self::White,\n        }\n    }\n\n    pub fn name(self) -> &'static str {\n        match self {\n            Self::White => \"White\",\n            Self::Black => \"Black\",\n>>>>>>> origin/version2\n        }\n    }\n}\n";
    let base_text = "impl Color {\n    pub fn opposite(self) -> Color {\n        match self {\n            Color::White => Color::Black,\n            Color::Black => Color::White,\n        }\n    }\n}\n";
    let mut segments = conflict_resolver::parse_conflict_markers(conflict_text);
    conflict_resolver::populate_block_bases_from_ancestor(&mut segments, base_text);

    let output = conflict_resolver::generate_resolved_text(&segments);
    let line_count = conflict_resolver::split_output_lines_for_outline(&output).len();
    let markers = build_resolved_output_conflict_markers(&segments, &output, line_count);
    let starts = markers
        .iter()
        .flatten()
        .filter(|m| m.conflict_ix == 0 && m.is_start)
        .count();
    assert_eq!(starts, 2, "expected two marker starts for impl Color case");
}

#[test]
fn split_target_conflict_block_into_subchunks_isolates_close_markers() {
    let conflict_text = "impl Color {\n<<<<<<< HEAD\n    pub fn opposite(self) -> Color {\n        match self {\n            Color::White => Color::Black,\n            Color::Black => Color::White,\n=======\n    pub fn opposite(self) -> Self {\n        match self {\n            Self::White => Self::Black,\n            Self::Black => Self::White,\n        }\n    }\n\n    pub fn name(self) -> &'static str {\n        match self {\n            Self::White => \"White\",\n            Self::Black => \"Black\",\n>>>>>>> origin/version2\n        }\n    }\n}\n";
    let base_text = "impl Color {\n    pub fn opposite(self) -> Color {\n        match self {\n            Color::White => Color::Black,\n            Color::Black => Color::White,\n        }\n    }\n}\n";
    let mut segments = conflict_resolver::parse_conflict_markers(conflict_text);
    conflict_resolver::populate_block_bases_from_ancestor(&mut segments, base_text);
    let mut region_indices = conflict_resolver::sequential_conflict_region_indices(&segments);
    let output_before = conflict_resolver::generate_resolved_text(&segments);

    let before_markers = resolved_output_markers_for_text(&segments, &output_before);
    let before_starts = before_markers
        .iter()
        .flatten()
        .filter(|m| m.conflict_ix == 0 && m.is_start)
        .count();
    assert_eq!(
        before_starts, 2,
        "fixture should begin with two close markers"
    );

    assert!(
        split_target_conflict_block_into_subchunks(&mut segments, &mut region_indices, 0),
        "expected target block to split"
    );

    assert_eq!(conflict_resolver::conflict_count(&segments), 2);
    assert_eq!(region_indices, vec![0, 0]);
    let output_after = conflict_resolver::generate_resolved_text(&segments);
    assert_eq!(
        output_after, output_before,
        "split should preserve output text"
    );

    let after_markers = resolved_output_markers_for_text(&segments, &output_after);
    let mut starts_by_conflict: std::collections::BTreeMap<usize, usize> =
        std::collections::BTreeMap::new();
    for marker in after_markers.iter().flatten().filter(|m| m.is_start) {
        *starts_by_conflict.entry(marker.conflict_ix).or_default() += 1;
    }
    assert_eq!(starts_by_conflict.get(&0).copied(), Some(1));
    assert_eq!(starts_by_conflict.get(&1).copied(), Some(1));
}

#[test]
fn conflict_region_index_is_unique_detects_split_subchunk_duplicates() {
    assert!(super::conflict_region_index_is_unique(&[0], 0));
    assert!(super::conflict_region_index_is_unique(&[0, 1], 0));
    assert!(!super::conflict_region_index_is_unique(&[0, 0], 0));
}

#[test]
fn append_choice_after_conflict_block_appends_selected_order_for_single_marker() {
    let mut segments = vec![
        ConflictSegment::Text("pre\n".to_string()),
        ConflictSegment::Block(ConflictBlock {
            base: None,
            ours: "ours\n".to_string(),
            theirs: "theirs\n".to_string(),
            choice: ConflictChoice::Ours,
            resolved: true,
        }),
        ConflictSegment::Text("post\n".to_string()),
    ];
    let mut region_indices = vec![0];

    let inserted_ix = super::append_choice_after_conflict_block(
        &mut segments,
        &mut region_indices,
        0,
        ConflictChoice::Theirs,
    );

    assert_eq!(inserted_ix, Some(1));
    assert_eq!(conflict_resolver::conflict_count(&segments), 2);
    assert_eq!(region_indices, vec![0, 0]);
    let output = conflict_resolver::generate_resolved_text(&segments);
    assert_eq!(output, "pre\nours\ntheirs\npost\n");
}

#[test]
fn append_choice_after_conflict_block_from_same_marker_keeps_single_choice_per_side() {
    let mut segments = vec![
        ConflictSegment::Text("pre\n".to_string()),
        ConflictSegment::Block(ConflictBlock {
            base: Some("base\n".to_string()),
            ours: "ours\n".to_string(),
            theirs: "theirs\n".to_string(),
            choice: ConflictChoice::Base,
            resolved: true,
        }),
        ConflictSegment::Text("post\n".to_string()),
    ];
    let mut region_indices = vec![0];

    assert_eq!(
        super::append_choice_after_conflict_block(
            &mut segments,
            &mut region_indices,
            0,
            ConflictChoice::Ours,
        ),
        Some(1)
    );
    assert_eq!(
        super::append_choice_after_conflict_block(
            &mut segments,
            &mut region_indices,
            0,
            ConflictChoice::Theirs,
        ),
        Some(2)
    );
    // Picking C again from the same marker should not append duplicate chunks.
    assert_eq!(
        super::append_choice_after_conflict_block(
            &mut segments,
            &mut region_indices,
            0,
            ConflictChoice::Theirs,
        ),
        None
    );

    assert_eq!(
        super::conflict_group_selected_choices_for_ix(&segments, &region_indices, 0),
        vec![
            ConflictChoice::Base,
            ConflictChoice::Ours,
            ConflictChoice::Theirs
        ]
    );
    assert_eq!(conflict_resolver::conflict_count(&segments), 3);
    assert_eq!(
        conflict_resolver::generate_resolved_text(&segments),
        "pre\nbase\nours\ntheirs\npost\n"
    );
}

#[test]
fn non_contiguous_matching_blocks_do_not_share_choice_group() {
    let mut segments = vec![
        ConflictSegment::Text("pre\n".to_string()),
        ConflictSegment::Block(ConflictBlock {
            base: Some("base\n".to_string()),
            ours: "ours\n".to_string(),
            theirs: "theirs\n".to_string(),
            choice: ConflictChoice::Theirs,
            resolved: true,
        }),
        ConflictSegment::Text("middle\n".to_string()),
        ConflictSegment::Block(ConflictBlock {
            base: Some("base\n".to_string()),
            ours: "ours\n".to_string(),
            theirs: "theirs\n".to_string(),
            choice: ConflictChoice::Ours,
            resolved: false,
        }),
        ConflictSegment::Text("post\n".to_string()),
    ];
    // Simulate subchunk-derived duplicate region ids while preserving a text boundary.
    let mut region_indices = vec![0, 0];

    assert_eq!(
        super::conflict_group_selected_choices_for_ix(&segments, &region_indices, 1),
        Vec::<ConflictChoice>::new()
    );

    assert!(
        super::reset_conflict_block_selection(&mut segments, &mut region_indices, 0),
        "resetting first block should not remove it due later non-contiguous match"
    );
    assert_eq!(conflict_resolver::conflict_count(&segments), 2);
}

#[test]
fn adjacent_markers_with_same_text_but_different_regions_do_not_interfere() {
    let mut segments = vec![
        ConflictSegment::Block(ConflictBlock {
            base: Some("base\n".to_string()),
            ours: "ours\n".to_string(),
            theirs: "theirs\n".to_string(),
            choice: ConflictChoice::Theirs,
            resolved: true,
        }),
        ConflictSegment::Block(ConflictBlock {
            base: Some("base\n".to_string()),
            ours: "ours\n".to_string(),
            theirs: "theirs\n".to_string(),
            choice: ConflictChoice::Ours,
            resolved: false,
        }),
    ];
    let mut region_indices = vec![10, 11];

    assert_eq!(
        super::conflict_group_selected_choices_for_ix(&segments, &region_indices, 1),
        Vec::<ConflictChoice>::new()
    );
    assert_eq!(
        super::conflict_group_indices_for_choice(
            &segments,
            &region_indices,
            1,
            ConflictChoice::Theirs
        ),
        Vec::<usize>::new()
    );

    assert_eq!(
        super::append_choice_after_conflict_block(
            &mut segments,
            &mut region_indices,
            1,
            ConflictChoice::Theirs,
        ),
        None
    );
    assert_eq!(conflict_resolver::conflict_count(&segments), 2);
}

#[test]
fn pick_sequence_is_reversible_to_original_unpicked_state() {
    let mut segments = vec![
        ConflictSegment::Text("pre\n".to_string()),
        ConflictSegment::Block(ConflictBlock {
            base: Some("base\n".to_string()),
            ours: "ours\n".to_string(),
            theirs: "theirs\n".to_string(),
            choice: ConflictChoice::Ours,
            resolved: false,
        }),
        ConflictSegment::Text("post\n".to_string()),
    ];
    let original = segments.clone();
    let mut region_indices = vec![0];

    // Pick A.
    let target = segments.iter_mut().find_map(|seg| match seg {
        ConflictSegment::Block(block) => Some(block),
        _ => None,
    });
    if let Some(block) = target {
        block.choice = ConflictChoice::Base;
        block.resolved = true;
    } else {
        panic!("expected conflict block");
    }
    // Pick B then C in order.
    assert_eq!(
        super::append_choice_after_conflict_block(
            &mut segments,
            &mut region_indices,
            0,
            ConflictChoice::Ours,
        ),
        Some(1)
    );
    assert_eq!(
        super::append_choice_after_conflict_block(
            &mut segments,
            &mut region_indices,
            1,
            ConflictChoice::Theirs,
        ),
        Some(2)
    );
    assert_eq!(
        conflict_resolver::generate_resolved_text(&segments),
        "pre\nbase\nours\ntheirs\npost\n"
    );

    // Deselect A, then B, then C.
    assert!(super::reset_conflict_block_selection(
        &mut segments,
        &mut region_indices,
        0
    ));
    assert!(super::reset_conflict_block_selection(
        &mut segments,
        &mut region_indices,
        0
    ));
    assert!(super::reset_conflict_block_selection(
        &mut segments,
        &mut region_indices,
        0
    ));

    assert_eq!(segments, original);
    assert_eq!(region_indices, vec![0]);
    assert_eq!(
        conflict_resolver::generate_resolved_text(&segments),
        conflict_resolver::generate_resolved_text(&original)
    );
}

#[test]
fn pick_and_deselect_multiple_orders_always_restore_original_state() {
    fn initial_segments() -> Vec<ConflictSegment> {
        vec![
            ConflictSegment::Text("pre\n".to_string()),
            ConflictSegment::Block(ConflictBlock {
                base: Some("base\n".to_string()),
                ours: "ours\n".to_string(),
                theirs: "theirs\n".to_string(),
                choice: ConflictChoice::Ours,
                resolved: false,
            }),
            ConflictSegment::Text("post\n".to_string()),
        ]
    }

    fn find_conflict_ix_by_choice(
        segments: &[ConflictSegment],
        choice: ConflictChoice,
    ) -> Option<usize> {
        segments
            .iter()
            .filter_map(|seg| match seg {
                ConflictSegment::Block(block) => Some(block),
                _ => None,
            })
            .enumerate()
            .find_map(|(ix, block)| (block.resolved && block.choice == choice).then_some(ix))
    }

    fn apply_pick_sequence(
        segments: &mut Vec<ConflictSegment>,
        region_indices: &mut Vec<usize>,
        picks: &[ConflictChoice],
    ) {
        let mut current_ix = 0usize;
        for (ix, choice) in picks.iter().copied().enumerate() {
            if ix == 0 {
                let target = segments.iter_mut().find_map(|seg| match seg {
                    ConflictSegment::Block(block) => Some(block),
                    _ => None,
                });
                if let Some(block) = target {
                    block.choice = choice;
                    block.resolved = true;
                } else {
                    panic!("expected conflict block");
                }
                continue;
            }
            let inserted_ix = super::append_choice_after_conflict_block(
                segments,
                region_indices,
                current_ix,
                choice,
            );
            assert_eq!(inserted_ix, Some(current_ix.saturating_add(1)));
            current_ix = inserted_ix.unwrap_or(current_ix);
        }
    }

    let original = initial_segments();
    let cases: Vec<(Vec<ConflictChoice>, Vec<ConflictChoice>)> = vec![
        // Full three-pick flows in different select/deselect orders.
        (
            vec![
                ConflictChoice::Base,
                ConflictChoice::Ours,
                ConflictChoice::Theirs,
            ],
            vec![
                ConflictChoice::Base,
                ConflictChoice::Ours,
                ConflictChoice::Theirs,
            ],
        ),
        (
            vec![
                ConflictChoice::Base,
                ConflictChoice::Ours,
                ConflictChoice::Theirs,
            ],
            vec![
                ConflictChoice::Theirs,
                ConflictChoice::Ours,
                ConflictChoice::Base,
            ],
        ),
        (
            vec![
                ConflictChoice::Theirs,
                ConflictChoice::Base,
                ConflictChoice::Ours,
            ],
            vec![
                ConflictChoice::Base,
                ConflictChoice::Theirs,
                ConflictChoice::Ours,
            ],
        ),
        (
            vec![
                ConflictChoice::Ours,
                ConflictChoice::Theirs,
                ConflictChoice::Base,
            ],
            vec![
                ConflictChoice::Base,
                ConflictChoice::Ours,
                ConflictChoice::Theirs,
            ],
        ),
        // Repeated two-pick cycle case.
        (
            vec![ConflictChoice::Ours, ConflictChoice::Theirs],
            vec![ConflictChoice::Theirs, ConflictChoice::Ours],
        ),
    ];

    for (picks, deselects) in cases {
        // Run each case twice to cover repeated select/deselect cycles.
        for _ in 0..2 {
            let mut segments = original.clone();
            let mut region_indices = vec![0];

            apply_pick_sequence(&mut segments, &mut region_indices, &picks);

            for deselect_choice in deselects.iter().copied() {
                let Some(conflict_ix) = find_conflict_ix_by_choice(&segments, deselect_choice)
                else {
                    panic!(
                        "expected to find selected conflict for {:?}",
                        deselect_choice
                    );
                };
                assert!(
                    super::reset_conflict_block_selection(
                        &mut segments,
                        &mut region_indices,
                        conflict_ix
                    ),
                    "expected deselect to succeed for {:?}",
                    deselect_choice
                );
            }

            assert_eq!(segments, original);
            assert_eq!(region_indices, vec![0]);
            assert_eq!(
                conflict_resolver::generate_resolved_text(&segments),
                conflict_resolver::generate_resolved_text(&original)
            );
        }
    }
}

#[test]
fn conflict_choice_hints_override_identical_text_to_selected_source() {
    fn shared(s: &str) -> gpui::SharedString {
        s.to_string().into()
    }

    let segments = vec![ConflictSegment::Block(ConflictBlock {
        base: Some("same\n".to_string()),
        ours: "same\n".to_string(),
        theirs: "same\n".to_string(),
        choice: ConflictChoice::Ours,
        resolved: true,
    })];
    let output = conflict_resolver::generate_resolved_text(&segments);
    let output_lines = conflict_resolver::split_output_lines_for_outline(&output);
    let sources = SourceLines {
        a: &[shared("same")],
        b: &[shared("same")],
        c: &[shared("same")],
    };

    let mut meta = conflict_resolver::compute_resolved_line_provenance(&output_lines, &sources);
    // Raw text matching alone picks A because A has higher matching priority.
    assert_eq!(meta[0].source, ResolvedLineSource::A);

    apply_conflict_choice_provenance_hints(
        &mut meta,
        &segments,
        &output,
        ConflictResolverViewMode::ThreeWay,
    );

    assert_eq!(meta[0].source, ResolvedLineSource::B);
    assert_eq!(meta[0].input_line, Some(1));
}

#[test]
fn empty_base_conflict_hint_overrides_false_a_badge() {
    fn shared(s: &str) -> gpui::SharedString {
        s.to_string().into()
    }

    let segments = vec![
        ConflictSegment::Text("dup\n".to_string()),
        ConflictSegment::Block(ConflictBlock {
            base: Some(String::new()),
            ours: "dup\n".to_string(),
            theirs: "other\n".to_string(),
            choice: ConflictChoice::Ours,
            resolved: true,
        }),
    ];
    let output = conflict_resolver::generate_resolved_text(&segments);
    let output_lines = conflict_resolver::split_output_lines_for_outline(&output);

    let a = vec![shared("dup")];
    let b = vec![shared("dup"), shared("dup")];
    let c = vec![shared("dup"), shared("other")];
    let sources = SourceLines {
        a: &a,
        b: &b,
        c: &c,
    };

    let mut meta = conflict_resolver::compute_resolved_line_provenance(&output_lines, &sources);
    // Raw content matching can pick A because "dup" exists in A.
    assert_eq!(meta[1].source, ResolvedLineSource::A);

    apply_three_way_empty_base_provenance_hints(&mut meta, &segments, &output);

    assert_eq!(meta[1].source, ResolvedLineSource::B);
    assert_eq!(meta[1].input_line, Some(2));
    assert_eq!(
        conflict_resolver::build_resolved_output_line_sources_index(
            &meta,
            &output_lines,
            ConflictResolverViewMode::ThreeWay
        )
        .contains(&conflict_resolver::SourceLineKey::new(
            ConflictResolverViewMode::ThreeWay,
            ResolvedLineSource::B,
            2,
            "dup"
        )),
        true
    );
}

#[test]
fn pane_content_width_for_layout_omits_hidden_handles_when_panels_collapsed() {
    let total_w = gpui::px(1000.0);
    let expanded =
        pane_content_width_for_layout(total_w, gpui::px(280.0), gpui::px(420.0), false, false);
    let both_collapsed =
        pane_content_width_for_layout(total_w, gpui::px(34.0), gpui::px(34.0), true, true);

    assert_eq!(expanded, gpui::px(284.0));
    assert_eq!(both_collapsed, gpui::px(932.0));
}

#[test]
fn pane_content_width_for_layout_clamps_at_zero_for_tight_space() {
    let total_w = gpui::px(200.0);
    let width =
        pane_content_width_for_layout(total_w, gpui::px(140.0), gpui::px(80.0), false, false);

    assert_eq!(width, gpui::px(0.0));
}
