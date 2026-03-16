# Reviewable Large Conflict Streaming Design

This note is stricter than `mergetool_large_file_streaming_investigation.md`.

For very large conflict blocks, the user must be able to review all source
content. A row like `Large conflict block preview omitted ...` can exist only as
a transient loading placeholder, not as the final steady-state UI.

## Current Situation

- 2-way large blocks are permanently truncated in
  `crates/gitcomet-ui-gpui/src/view/conflict_resolver.rs` by
  `push_large_conflict_block_preview_rows()`.
- 3-way large blocks are permanently truncated in
  `crates/gitcomet-ui-gpui/src/view/conflict_resolver.rs` by
  `build_three_way_visible_map()` via
  `ThreeWayVisibleItem::LargeBlockPreviewGap`.
- The actual row rendering is already viewport-based:
  - `crates/gitcomet-ui-gpui/src/view/panels/main/diff_view.rs` uses
    `uniform_list(...)` for both 3-way and 2-way resolver lists.
  - `crates/gitcomet-ui-gpui/src/view/rows/conflict_resolver.rs` renders only
    the requested `Range<usize>`.
- 3-way text access is already random-access by line:
  - `three_way_text` stores the full side texts.
  - `three_way_line_starts` stores per-side line offsets.
  - `three_way_line_text()` resolves one visible line at a time.
- Prepared syntax already has chunked behavior for full-document documents, so
  syntax does not require eager whole-file row materialization.

## Key Finding

The omission behavior is not a renderer limitation. It is a projection/data
model choice.

For 3-way mode, GitComet already owns all data needed for full review:

- full base / ours / theirs texts,
- full line-start tables,
- full conflict ranges,
- full per-line conflict maps.

That means the product can expose every line in a large conflict without first
computing or storing a whole-file 2-way diff.

The current 3-way omission row mainly hides content from the user. It does not
eliminate the largest existing allocations, because `build_three_way_conflict_maps()`
already builds full line-space maps for each side.

## What "Reviewable" Must Mean

For giant conflict blocks, the final UI state must satisfy all of these:

- The user can scroll through every input line.
- Search can match text inside the middle of a previously omitted block.
- Navigation can land inside any part of the block.
- Context menus and pick actions still know which conflict block and line are
  active.
- Any placeholder row is temporary. Once a chunk is ready, the real lines must
  become visible.

## Recommended Fix Order

## 1. Make 3-way mode fully reviewable first

This is the lowest-risk step because the 3-way renderer already works from
source texts plus line indices.

Recommended change:

- Stop using `LargeBlockPreviewGap` as the steady-state representation for
  unresolved large blocks.
- Replace `Vec<ThreeWayVisibleItem>` with a span-based projection:
  - `Lines { visible_start, line_start, len }`
  - `CollapsedResolvedBlock { visible_index, conflict_ix }`
- When `hide_resolved == false`, unresolved blocks should contribute their full
  line ranges.
- When `hide_resolved == true`, only resolved blocks should collapse.

Why this works:

- `render_conflict_resolver_three_way_rows()` already renders only the visible
  range.
- `three_way_line_text()` already gives O(1) access to any requested line.
- Large-block 3-way word highlights are already skipped, so removing the gap
  does not re-enable the old whole-block highlight blow-up.

Important implementation note:

- Do not replace the preview gap with a full `Vec<ThreeWayVisibleItem>` for
  every line if it can be avoided.
- A span projection is better because it keeps memory proportional to conflict
  structure, not file length.
- Visible-index to source-line lookup can be done with a small binary search
  over spans.

## 2. Default giant blocks to a full-review mode, not a preview mode

If a block crosses the current large-block threshold, the product should prefer
"full review with bounded chrome" over "small preview with hidden content."

That means:

- keep syntax minimal or background-only,
- keep word diff off for giant blocks,
- keep inline diff off if needed,
- but keep the source lines themselves scrollable and searchable.

This is a better tradeoff than hiding the block, because the user can still do
the important work: inspect the real file contents.

## 3. Replace large-block 2-way preview rows with streamed split compare

The current 2-way large-block path is the real reason the exact
`Large conflict block preview omitted ...` message appears. That path should not
end in a permanent omission row.

The minimum viable replacement is not a full streamed Myers diff. It is a
streamed split compare mode for giant blocks:

- one row per line index within the block,
- left cell reads from `ours`,
- right cell reads from `theirs`,
- row count is `max(ours_line_count, theirs_line_count)`,
- rows are resolved on demand from line-start tables,
- only the requested chunk is styled,
- adjacent chunks can be prefetched.

This keeps every line reviewable while avoiding eager whole-block diff row
construction.

Tradeoff:

- exact diff alignment is weaker than a full Myers result,
- but reviewability is preserved,
- and the implementation is much simpler and safer than trying to page the
  existing `Vec<FileDiffRow>` model directly.

For giant blocks, this is an acceptable intermediate state.

## 4. Later, add a high-fidelity paged diff projection if needed

If exact 2-way alignment is still important for giant blocks, add it as a
second-phase improvement instead of making it a prerequisite for reviewability.

Recommended direction:

- use coarse anchors across the whole block first,
- then materialize exact diff rows only for the requested anchor interval.

Useful building blocks already exist:

- `crates/gitcomet-core/src/file_diff.rs` has
  `side_by_side_rows_with_anchors()`,
  `compute_row_region_anchors()`,
  and histogram / patience diff logic.

The right long-term model is:

- keep source texts shared,
- keep stable anchor metadata cheap,
- build detailed rendered rows only for visible chunks,
- discard or cache chunks independently.

## Data Model Sketch

Suggested 3-way projection:

```rust
struct ThreeWayVisibleProjection {
    spans: Vec<ThreeWayVisibleSpan>,
    visible_len: usize,
    conflict_visible_starts: Vec<usize>,
}

enum ThreeWayVisibleSpan {
    Lines {
        visible_start: usize,
        line_start: usize,
        len: usize,
    },
    CollapsedResolvedBlock {
        visible_index: usize,
        conflict_ix: usize,
    },
}
```

Suggested giant-block streamed compare cache:

```rust
struct LargeBlockChunkKey {
    conflict_ix: usize,
    mode: LargeBlockViewMode,
    chunk_ix: usize,
}

struct LargeBlockChunk {
    start_row: usize,
    rows: Vec<StreamedCompareRow>,
}

struct StreamedCompareRow {
    local_line_ix: usize,
    ours_line: Option<SharedString>,
    theirs_line: Option<SharedString>,
}
```

Suggested chunk policy:

- chunk size `256` or `512` rows,
- synchronous render uses cached chunk if ready,
- otherwise show a temporary "loading chunk" row range,
- queue the visible chunk plus one adjacent chunk on each side,
- invalidate only overlapping rows when the chunk completes.

## Search And Navigation

Search and navigation should stop depending on omission-summary rows for giant
blocks.

Recommended behavior:

- 3-way search reads directly from `three_way_line_text()`.
- streamed 2-way giant-block search reads directly from the source texts or from
  chunk rows when already available.
- navigation to a conflict should target the first real visible line of that
  block, not a synthetic omission row.

## What Not To Do

These are the wrong fixes for this requirement:

- raising `LARGE_CONFLICT_BLOCK_PREVIEW_LINES`,
- replacing the omission text with a different omission text,
- increasing diff or syntax time budgets while keeping the eager row model,
- or treating the current preview rows as the final large-file UX.

All of those keep the same product bug: the user still cannot review the real
content.

## Practical Recommendation

If the goal is to improve the product in small, safe steps, the best order is:

1. Replace 3-way omission with a span-based full-line projection.
2. Prefer that 3-way full-review path for giant blocks.
3. Replace giant-block 2-way preview rows with streamed split compare rows.
4. Only after that, decide whether a paged exact diff is still necessary.

That sequence gives users full reviewability quickly, without waiting for a
full lazy-diff architecture rewrite.
