# Mergetool Stream-Everything Execution Plan

This plan turns the large-conflict investigation into an implementation order.

Target outcome:

- no permanent hidden-content rows,
- no eager 500k-line diff materialization,
- no eager 500k-line tree-sitter parse,
- viewport stays fully virtualized,
- diffs are computed only for the visible window and nearby prefetched windows.

## Hard Invariants

These are non-negotiable for giant conflict files:

1. Bootstrap may do one linear pass for byte/line indexing and conflict marker
   parsing, but must not run whole-block Myers diff.
2. Bootstrap must not allocate one rendered row object per file line.
3. Bootstrap must not start full-document tree-sitter on a 500k-line conflict.
4. The final UI must allow scrolling through every input line.
5. `uniform_list(...)` virtualization remains the core rendering model.
6. Search, navigation, and conflict picking must work without first materializing
   the entire block.
7. Resolved output must not require `generate_resolved_text(...)` +
   `input.set_text(...)` for giant-file bootstrap.

Allowed up-front work:

- reading source bytes,
- building line-start tables,
- parsing conflict markers,
- building per-conflict range metadata,
- building sparse anchor metadata if it is linear and compact.

Forbidden up-front work for giant mode:

- `side_by_side_rows(whole_block_ours, whole_block_theirs)`,
- `build_inline_rows(...)` for a whole giant block,
- whole-block word diff vectors,
- full `Vec<Option<usize>>` per-line conflict maps,
- full output text generation,
- full-document syntax parsing.

## Existing Pieces To Reuse

These are already present and should be reused instead of redesigned:

- chunked source storage and line indexing:
  [text_model.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/kit/text_model.rs)
- viewport virtualization via `uniform_list(...)`:
  [diff_view.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/panels/main/diff_view.rs)
- prepared syntax chunk polling:
  [core_impl.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/panes/main/core_impl.rs)
- paged row provider pattern for patch diffs:
  [diff_cache.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/panes/main/diff_cache.rs)
- current conflict parsing and segment model:
  [conflict_resolver.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/conflict_resolver.rs)

## New Runtime Modes

Add an explicit mode split:

```rust
enum ConflictRenderingMode {
    EagerSmallFile,
    StreamedLargeFile,
}
```

Selection rule:

- use `EagerSmallFile` for normal conflicts,
- use `StreamedLargeFile` when any block or the combined file crosses the large
  threshold.

Do not hide this behind scattered threshold checks. The mode should be chosen
once during resolver bootstrap and stored in UI state.

## Phase 0: Instrumentation And Guardrails

Goal: make accidental regressions obvious before larger refactors.

Concrete changes:

- extend mergetool trace points in
  [actions_impl.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/panes/main/actions_impl.rs)
  to record:
  - rendering mode,
  - whether whole-block diff ran,
  - whether full output text was generated,
  - whether full syntax parse was requested.
- add debug assertions in giant mode that reject:
  - `LargeBlockPreviewGap` creation,
  - eager `side_by_side_rows(...)` on whole giant blocks,
  - eager full output generation during bootstrap.

Acceptance:

- trace logs clearly distinguish `EagerSmallFile` vs `StreamedLargeFile`,
- a regression that reintroduces eager whole-block diff fails in debug/tests.

## Phase 1: Replace Per-Line Conflict Maps With Range Metadata

Goal: remove the remaining eager line-space structures that scale with full file
line count.

Current eager structures to phase out for giant mode:

- `three_way_visible_map: Vec<ThreeWayVisibleItem>`
- `three_way_line_conflict_map: ThreeWaySides<Vec<Option<usize>>>`
- `diff_visible_row_indices: Vec<usize>`
- `inline_visible_row_indices: Vec<usize>`

Replace them with compact range/span structures:

```rust
struct ConflictRangeIndex {
    base_ranges: Vec<Range<usize>>,
    ours_ranges: Vec<Range<usize>>,
    theirs_ranges: Vec<Range<usize>>,
    block_visible_starts: Vec<usize>,
}

struct ThreeWayVisibleProjection {
    spans: Vec<ThreeWayVisibleSpan>,
    visible_len: usize,
}

enum ThreeWayVisibleSpan {
    Lines {
        visible_start: usize,
        source_line_start: usize,
        len: usize,
    },
    CollapsedResolvedBlock {
        visible_index: usize,
        conflict_ix: usize,
    },
}
```

Concrete file changes:

- add range/projection types to
  [conflict_resolver.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/conflict_resolver.rs)
- replace UI state fields in
  [mod_helpers.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/mod_helpers.rs)
- update visible-index helpers in
  [actions_impl.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/panes/main/actions_impl.rs)

Implementation rule:

- membership lookup must use binary search on ranges/spans, not per-line vectors.

Acceptance:

- no `Vec<Option<usize>>` of full line count exists in giant mode,
- visible length can still be reported to `uniform_list(...)`,
- navigation to a conflict remains O(log n) or O(number of conflicts), not
  O(file lines).

## Phase 2: Stream 3-Way Inputs First

Goal: make the user able to review every line in large conflicts without any
permanent omission row.

Concrete behavior:

- remove `ThreeWayVisibleItem::LargeBlockPreviewGap` from giant mode,
- unresolved giant blocks expose all lines through the projection,
- resolved blocks may still collapse when `hide_resolved == true`.

Renderer changes:

- `render_conflict_resolver_three_way_rows(...)` in
  [rows/conflict_resolver.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/rows/conflict_resolver.rs)
  must resolve visible rows via span lookup instead of indexing a giant `Vec`.
- row text still comes from:
  - `three_way_text`
  - `three_way_line_starts`
  - `three_way_line_text()`

Styling rules in giant mode:

- no full-document syntax parse,
- no three-way word diff for giant blocks,
- optional per-line heuristic syntax only for currently visible rows.

Important constraint:

- giant 3-way mode is allowed to be visually simpler,
- it is not allowed to hide content.

Acceptance:

- a 500k-line whole-file conflict in 3-way mode has visible length equal to the
  full line count except resolved collapses,
- the omission text no longer appears,
- scrolling to the middle of the file renders real lines without preloading the
  whole block.

## Phase 3: Introduce A Streamed 2-Way Provider

Goal: stop using eager `diff_rows` / `inline_rows` for giant blocks.

Add a dedicated provider modeled after `PagedPatchDiffRows`:

```rust
struct PagedConflictCompareRows { ... }
struct PagedConflictInlineRows { ... } // optional second step
```

Use the existing provider shape from
  [domain.rs](/home/sampo/git/GitComet3/crates/gitcomet-core/src/domain.rs):

```rust
trait DiffRowProvider {
    fn len_hint(&self) -> usize;
    fn row(&self, ix: usize) -> Option<Self::RowRef>;
    fn slice(&self, start: usize, end: usize) -> Self::SliceIter<'_>;
}
```

### Phase 3A: Split compare without global diff

First implementation for giant blocks:

- provider row count = `max(ours_lines, theirs_lines)` within the active block,
- row text is pulled lazily from line starts,
- rows are emitted in split view only,
- inline mode is disabled or downgraded for giant blocks.

This preserves reviewability immediately.

### Phase 3B: Local diff refinement per page

After basic split compare works, refine visible pages:

- chunk size `256` rows,
- when page `N` is requested, compute only page `N` plus adjacent pages,
- use a local diff window with anchor margins,
- cache pages by `(conflict_ix, page_ix, mode, source_hash)`.

Do not compute diff for lines outside the requested page interval.

Concrete file changes:

- new provider types in
  [diff_cache.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/panes/main/diff_cache.rs)
  or a new conflict-specific paging module
- UI state changes in
  [mod_helpers.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/mod_helpers.rs)
- bootstrap path in
  [actions_impl.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/panes/main/actions_impl.rs)
- diff rendering path in
  [rows/conflict_resolver.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/rows/conflict_resolver.rs)

Acceptance:

- giant mode no longer builds whole-block `diff_rows` or `inline_rows`,
- first visible split rows appear from provider pages only,
- page requests do not materialize earlier unseen pages.

## Phase 4: Sparse Anchor Index For Stable Local Diff

Goal: improve alignment without falling back to whole-block Myers.

Add a compact anchor pass per giant block:

- line hash scan over the block,
- keep only sparse unique-line anchors or chunk signatures,
- store anchor pairs compactly,
- use anchors to choose local diff windows for visible pages.

Allowed complexity:

- one linear scan across the block to build anchors,
- compact memory proportional to anchor count,
- no rendered-row allocation.

Forbidden:

- full-file `FileDiffRow` generation as part of anchor building.

Concrete output:

```rust
struct ConflictAnchorIndex {
    ours_to_theirs: Vec<(u32, u32)>,
}
```

Use anchors to:

- snap visible diff pages to stable boundaries,
- reduce visual misalignment for insert/delete heavy pages,
- support jump-to-next-change later.

Acceptance:

- provider page diff quality improves for insert/delete regions,
- anchor build remains linear and compact,
- no whole-block row vector is created.

## Phase 5: Search And Navigation Over Providers

Goal: remove the last feature dependencies on eager row vectors.

### 3-way search

- search directly over source texts using line-start windows,
- materialize only matching lines/pages.

### 2-way giant-block search

- search source texts, not prebuilt diff rows,
- convert source matches to provider page targets,
- request target page if missing.

### Conflict navigation

- use conflict range metadata + visible projection,
- never depend on omission-summary rows.

Concrete file changes:

- [diff_search.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/panes/main/diff_search.rs)
- [actions_impl.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/panes/main/actions_impl.rs)
- [conflict_resolver.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/conflict_resolver.rs)

Acceptance:

- searching text in the middle of a 500k-line block returns a real line,
- jumping to the result requests only the destination page,
- next/previous conflict navigation still works in O(number of conflicts).

## Phase 6: Stream Resolved Output Too

Goal: remove full output generation from giant-file bootstrap.

Current eager path to replace:

- `generate_resolved_text(...)`
- `input.set_text(resolved, cx)`

Replace with:

```rust
struct ResolvedOutputProjection {
    segments: Vec<ResolvedSegmentRef>,
    visible_len: usize,
}
```

Behavior in giant mode:

- output pane is virtualized and read-only by default,
- visible rows are synthesized from current picks + text segments,
- full concatenated output is assembled only on:
  - explicit edit transition,
  - save/write,
  - external export if needed.

Editing strategy:

- first milestone: read-only streamed output in giant mode,
- second milestone: chunk-edit mode that materializes only the edited region
  into a temporary `TextModel`,
- final assembly occurs at write time.

Concrete file changes:

- output bootstrap in
  [actions_impl.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/panes/main/actions_impl.rs)
- resolved-output state in
  [mod_helpers.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/mod_helpers.rs)
- output rendering and provenance helpers in
  [helpers.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/panes/main/helpers.rs)

Acceptance:

- giant-file bootstrap does not call `input.set_text(...)` with a 500k-line
  string,
- resolved output can still be reviewed,
- final write still emits correct full text.

## Phase 7: Syntax Policy For Giant Mode

Goal: keep syntax from reintroducing full-document work.

Policy:

- giant mode must not create full prepared syntax documents for merge inputs,
- giant mode may use:
  - plain text,
  - heuristic line styling,
  - optional tiny local parse windows for current page only.

If local parse windows are added later:

- parse only current page plus a small margin,
- throw away old windows aggressively,
- never allow this path to become an implicit full-document parse.

Concrete changes:

- gate syntax creation in
  [actions_impl.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/panes/main/actions_impl.rs)
- keep chunk polling infra for normal-sized files only
  or adapt it to page-local syntax windows later.

Acceptance:

- no full prepared syntax document is present for a giant conflict side,
- visible rows still render immediately,
- syntax policy cannot silently upgrade into a whole-file parse.

## Phase 8: Tests And Benchmarks

Update tests to assert streaming behavior, not bounded omission behavior.

Tests to add or rewrite:

- rewrite current omission-based expectations in
  [panels/tests.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/panels/tests.rs)
  so giant blocks are fully reviewable
- add tests:
  - 3-way whole-file conflict exposes full visible length
  - scrolling to a deep visible index does not require eager whole-block rows
  - giant 2-way provider materializes only requested pages
  - search for a line in the middle loads only destination pages
  - giant bootstrap skips full syntax creation
  - giant bootstrap skips full output materialization

Benchmarks to add:

- provider first-page latency,
- provider page cache hit latency,
- deep-scroll page miss latency,
- anchor-build latency,
- giant bootstrap RSS vs old bounded-preview path.

Good places:

- [panels/tests.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/panels/tests.rs)
- [rows/benchmarks.rs](/home/sampo/git/GitComet3/crates/gitcomet-ui-gpui/src/view/rows/benchmarks.rs)

## Recommended Delivery Order

This is the order to implement:

1. Add `ConflictRenderingMode` and instrumentation.
2. Replace giant-mode per-line maps with range/spans.
3. Stream 3-way full review and remove omission rows.
4. Add giant split compare provider with page caching.
5. Add sparse anchor index for local diff refinement.
6. Move search/navigation to providers and projections.
7. Stream resolved output instead of bootstrapping full text.
8. Lock the behavior down with rewritten tests and benchmarks.

## Definition Of Done

The plan is complete when all of these are true for a 500k-line whole-file
conflict:

- the UI shows every input line,
- the literal omission message is gone,
- no whole-block `side_by_side_rows(...)` runs during bootstrap,
- no full-document syntax parse starts,
- no full resolved-output text is generated during bootstrap,
- deep scroll requests only a small number of nearby pages,
- search and conflict navigation still work,
- memory scales primarily with source storage, line indices, and page caches,
  not with full rendered-row duplication.
