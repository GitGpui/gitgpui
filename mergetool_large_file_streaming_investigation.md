# Large Mergetool Streaming Investigation

This note complements `syntax_highlighting_design.md`.

## Update: 3-Way Whole-File Conflict Crash

Current status after the latest investigation/patch cycle:

- The 3-way crash was not only a provenance problem. The first bounded-preview fix was wired only into the `hide_resolved=true` path, while the real 3-way switch path still expanded the full unresolved block when `hide_resolved=false`.
- The resolver now emits bounded 3-way preview rows for very large conflict blocks regardless of the `hide_resolved` toggle.
- Very large 3-way side texts now skip full prepared tree-sitter document work and stay on bounded fallback syntax instead of scheduling background full-document parses.

This fixes the immediate whole-program crash/OOM path for the real repro shape, but it does not mean the focused mergetool is fully streamed yet. A remaining hotspot is the first full draw after switching to 3-way: the UI regression now completes instead of crashing, but it is still slow in debug on a synthetic whole-file conflict just above the large-block threshold. The next likely area to investigate is output-pane rendering and any remaining eager row/layout work during the first paint.

The syntax-highlighting design document is directionally correct, but the current focused mergetool crash on a very large HTML file is not only a tree-sitter problem. The mergetool bootstrap path still eagerly materializes large diff and merge structures for the entire file before first paint.

## Fixture And Method

Primary reproduction fixture:

- `/home/sampo/git/gitmess/gitmess/index.html`
- `47,865,848` bytes
- `502,734` lines
- max line length `764`
- conflict markers at lines `1`, `300000`, and `502732`
- effective shape: one conflict block spanning almost the entire file

This investigation is based on code inspection plus one existing automated test:

- `cargo test -p gitcomet-ui-gpui large_conflict_resolved_output_renders_plain_text_then_upgrades_after_background_syntax`

That test passed in this session, but it only covers the resolved-output background-syntax path on a `20,001`-line synthetic fixture. I did not launch the GUI against the `47.9 MB` HTML fixture in this session.

## Short Conclusion

The likely failure is a load-time memory and synchronous-work blow-up, not just "syntax highlighting is too slow."

The most important missing detail is that the real `index.html` is not "a huge file with a few small conflict islands." It is effectively one giant conflict block. That means the newer block-local diff path still degenerates into whole-file work for this reproduction unless the resolver also limits or streams work *inside* a large block.

The current focused mergetool path:

- duplicates the full conflict file several times in backend, state, session, and UI layers,
- computes full-file side-by-side diff rows for `ours` vs `theirs`,
- builds a second inline-row copy of that diff,
- allocates whole-file word-highlight vectors and line-to-conflict maps,
- and then still downgrades large merge-input panes to `HeuristicOnly` syntax above `4,000` rows.

For a `502,734`-line file, this architecture can easily produce multi-hundred-megabyte transient memory pressure and long UI-thread stalls before the first useful frame appears.

## What Already Works

Some of the architecture needed for large files already exists:

- `TextModel` is chunked and line-indexed (`crates/gitcomet-ui-gpui/src/kit/text_model.rs`).
- Resolved-output syntax can render plain text first and continue full-document syntax preparation in the background (`crates/gitcomet-ui-gpui/src/view/panes/main/helpers.rs`, `crates/gitcomet-ui-gpui/src/view/panes/main/core_impl.rs`).
- Resolved-output recompute has an incremental path for edits (`crates/gitcomet-ui-gpui/src/view/panes/main/core_impl.rs`).
- There is already a passing large resolved-output syntax test in `crates/gitcomet-ui-gpui/src/view/panels/tests.rs`.

Those are good patterns to extend. The main problem is that the merge-input side of the resolver still uses eager whole-file materialization.

## Where The Current Mergetool Path Scales Poorly

### 1. Conflict loading duplicates the same large file multiple times

`crates/gitcomet-state/src/store/effects/repo_load.rs` currently:

- asks the backend for `repo.conflict_session(&path)`,
- separately asks for `repo.conflict_file_stages(&path)`,
- separately reads the worktree file again for `current_bytes` / `current`.

Then `crates/gitcomet-state/src/model.rs` stores:

- `base_bytes`, `ours_bytes`, `theirs_bytes`, `current_bytes`,
- `base`, `ours`, `theirs`, `current`.

At the same time, state also carries a `ConflictSession` object. In the normal backend-supported path that session is already built from separately loaded payloads in `crates/gitcomet-git-gix/src/repo/diff.rs`; if that path is unavailable, `crates/gitcomet-state/src/store/reducer/effects.rs` falls back to building a session from `ConflictFile`. Either way, `ConflictFile` and `ConflictSession` coexist and both own large payload data.

Then `crates/gitcomet-ui-gpui/src/view/panes/main/actions_impl.rs` clones the side texts again into `SharedString` for the resolver UI state.

For a text conflict where all four versions are large, the same payload is copied several times before diff rendering even begins.

### 2. Focused mergetool bootstrap eagerly builds full-file diff state

`crates/gitcomet-ui-gpui/src/view/panes/main/actions_impl.rs` does all of this during resolver rebuild:

- parse conflict markers from the whole current file,
- generate the whole resolved output string,
- compute `gitcomet_core::file_diff::side_by_side_rows(ours_text, theirs_text)`,
- build `inline_rows` from that full diff,
- build line starts for all three sides,
- build three-way conflict maps,
- build two-way visible maps,
- compute three-way word highlights,
- compute two-way word highlights,
- push the entire resolved text into the `TextInput`,
- schedule resolved-output outline/provenance recompute.

This is the opposite of a streaming-first design. It materializes the entire merge model up front even though the UI is viewport-oriented.

### 3. `side_by_side_rows` is monolithic and clones line text per row

`crates/gitcomet-core/src/file_diff.rs`:

- splits both sides into `Vec<&str>` with `split_lines`,
- runs `myers_edits`,
- allocates one `FileDiffRow` per output row,
- clones line text into `String` for `old` and `new`,
- may then allocate another pass for replacement pairing,
- and `build_inline_rows` duplicates line text yet again.

This is a major scaling problem for a mergetool because:

- it re-diffs the whole file even though conflict markers already identify the conflict blocks,
- it stores context rows for the entire file even when most of the file is unchanged,
- and it pays full-file allocation cost before the viewport asks for any rows.

For large files, this is not just slow. It is structurally memory-heavy.

### 4. The merge-input syntax path is still hard-gated for large files

`crates/gitcomet-ui-gpui/src/view/rows/mod.rs` sets:

- `MAX_LINES_FOR_SYNTAX_HIGHLIGHTING = 4_000`

`crates/gitcomet-ui-gpui/src/view/rows/conflict_resolver.rs` then uses:

- `DiffSyntaxMode::Auto` only below that threshold,
- `DiffSyntaxMode::HeuristicOnly` above it.

So even if the crash is fixed, the current merge-input panes still will not provide full-document syntax highlighting for large files. They fall back to per-line heuristics.

This is separate from the resolved-output syntax path, which already has a background prepare model.

### 5. Resolved output is partially improved, but still not fully streamed

The resolved-output path is in better shape than the merge-input path, but it is not fully solved either.

Good:

- syntax preparation can time out and continue in the background,
- prepared syntax is range-driven,
- there is an incremental recompute path for some edits.

Remaining issue:

- `schedule_conflict_resolved_outline_recompute` eventually runs `recompute_conflict_resolved_outline_and_provenance*` inside `view.update(...)`, so provenance and gutter-marker recompute still happen on the UI thread,
- full recompute still scans the whole output and rebuilds whole-file metadata vectors.

That may be acceptable at `20k` lines and still become a visible stall at `500k` lines.

### 6. Edit actions still rebuild whole output strings

Several actions still regenerate the full resolved output or split/join the full output text:

- `conflict_resolver::generate_resolved_text(...)`
- `replace_output_lines_in_range(...)`
- `resolved_output_markers_for_text(...)`
- `split_output_lines_for_outline(...)`

These are not the first crash vector, but they will become the next scalability wall after initial load is fixed.

## Likely Crash / Freeze Chain

The most likely chain for the HTML fixture is:

1. Focused mergetool bootstrap loads conflict stages, current file, and conflict session with duplicated allocations.
2. The UI rebuild clones side texts again and builds full line-start vectors.
3. The resolver computes a full-file `ours` vs `theirs` diff and stores one row object per line, including cloned row strings.
4. Inline rows and word-highlight vectors add more whole-file allocations.
5. Resolved-output outline/provenance work still performs large whole-file scans on the UI thread.
6. If the process survives the memory pressure, the merge-input panes still only get heuristic syntax above `4,000` rows.

In other words: the current mergetool design does too much work on too much data too early.

## Important Architectural Observation

The mergetool does not actually need a whole-file side-by-side diff as its primary data model.

It already has better conflict structure available:

- stage 1 / 2 / 3 file contents,
- parsed marker segments from the merged file,
- per-conflict regions in `ConflictSession`.

That means the resolver can be driven by:

- shared side documents,
- conflict-block ranges,
- and lazy block-local diff projections.

Re-diffing the entire `ours` and `theirs` files into a giant `Vec<FileDiffRow>` is redundant for mergetool use cases.

One important correction from the first pass of this note: GitComet now has a block-local two-way diff path for sparse large files. That improvement is real, but it does not help when the merged file contains a single conflict block that wraps almost the whole document, because the block-local diff still receives whole-document-sized `ours` and `theirs` slices in that case.

## Programmatic Investigation Plan

### Phase 1: Add measurement before changing architecture

Add explicit timing and size logging for these stages:

- `repo.conflict_session(&path)`
- `repo.conflict_file_stages(&path)`
- worktree `current` read
- marker parsing
- `side_by_side_rows`
- `build_inline_rows`
- `build_three_way_conflict_maps`
- `compute_three_way_word_highlights`
- `compute_two_way_word_highlights`
- `input.set_text(...)`
- resolved-output outline/provenance recompute

Also log these counts:

- byte size per side
- line count per side
- total diff row count
- inline row count
- conflict block count
- resolved output line count

Add debug-only process memory snapshots around the same stages. On Linux this can read `/proc/self/status` or `/proc/self/statm`. The exact implementation can be platform-gated and debug-only.

Goal of Phase 1:

- produce one trace for the real `index.html` case,
- identify which step is the dominant peak for time and RSS,
- confirm whether the crash happens before, during, or after full diff row construction.

### Phase 2: Add the right regression fixtures

Current coverage proves only that large resolved output can plain-paint and later upgrade syntax. It does not cover focused mergetool bootstrap on a huge file.

Add:

- a synthetic focused-mergetool stress fixture around `500k` lines with low conflict density,
- a second fixture with higher conflict density,
- a whole-file conflict-block fixture where the marker starts at line `1` and the separator/end markers arrive near the end of the file,
- an ignored/manual stress test for very large conflict inputs,
- benchmark fixtures in `crates/gitcomet-ui-gpui/src/view/rows/benchmarks.rs` that separately measure:
  - conflict load duplication,
  - full-file two-way diff construction,
  - single-block preview / streaming projection,
  - block-local diff construction,
  - word-highlight generation,
  - resolved-output provenance rebuild.

The fixture should be HTML-like so syntax and line-count behavior match the real case. The whole-file-block variant matters more than density alone because it is the shape that defeats the current sparse-block optimization.

### Phase 3: Remove the biggest waste first

First high-value prototype:

- stop storing both `Vec<u8>` and `String` copies for every text side unless both are truly needed,
- stop loading `ConflictSession` and `ConflictFile` through independent full-copy paths,
- share side payloads with `Arc<str>` or `Arc<[u8]>` plus decoded views instead of cloning strings again per layer.

This will not solve the whole problem, but it is the lowest-risk way to reduce peak RSS quickly.

### Phase 4: Replace whole-file diff rows with a paged / block-local projection

For mergetool, the next architecture should be:

- keep base / ours / theirs as shared documents,
- keep conflict marker segments / conflict regions as the primary structure,
- compute two-way compare rows only for visible conflict blocks or visible windows,
- and, for giant individual blocks, stream or preview rows *within the block* instead of diffing the whole block up front,
- do not materialize one `FileDiffRow` for every file line up front,
- do not build `inline_rows` eagerly for the whole file,
- do not precompute whole-file word highlights.

The correct scaling target is:

- unchanged context comes from shared side texts plus line indices,
- changed regions get block-local diff projections,
- visible rows are synthesized on demand.

### Phase 5: Extend document-owned syntax sessions to merge-input sides

The resolved-output background syntax model should be generalized to the three merge-input documents:

- `base`
- `ours`
- `theirs`

Desired behavior:

- plain text first paint,
- background full-document parse when grammar exists,
- range-based highlight queries for visible rows only,
- syntax state shared across scroll, diff mode toggles, and pane resizes.

Do not just raise `MAX_LINES_FOR_SYNTAX_HIGHLIGHTING`. That would preserve the eager architecture and likely make crashes worse.

### Phase 6: Move resolved-output metadata closer to the text model

After load-time diff materialization is fixed, the next major improvement should be:

- keep resolved-output block ranges anchored against the text model,
- update provenance and marker metadata incrementally from edits,
- avoid whole-output re-generation for simple picks,
- move large recompute work off the UI thread when incremental fallback is not possible.

The resolved-output path already has partial infrastructure for this. It needs the same "range-first, viewport-first" discipline as the syntax work.

## Recommended Fix Order

If the goal is "works for any size file without freezing, crashing, or giving up on syntax," the best order is:

1. Add timing and RSS instrumentation to capture the real failure trace.
2. Eliminate duplicate conflict-file loading and duplicate text ownership across layers.
3. Add a large-file escape hatch that skips eager `side_by_side_rows`, `inline_rows`, and whole-file word highlights on mergetool bootstrap, including the single-giant-block case.
4. Replace that escape hatch with a real paged/block-local diff projection.
5. Extend background document syntax sessions to merge-input sides.
6. Finish moving resolved-output recompute to incremental and/or background execution.

## What Not To Do

These changes are unlikely to solve the real problem on their own:

- increasing syntax time budgets,
- raising the `4,000`-line syntax gate,
- moving only tree-sitter work to the background while leaving whole-file diff materialization intact,
- or increasing cache sizes.

The crash is likely caused by eager full-file diff/merge model construction and payload duplication. Syntax is only one part of that stack.

## Success Criteria

For the `47.9 MB / 502,734`-line HTML fixture, the mergetool should eventually satisfy all of these:

- first paint appears quickly with plain text and conflict structure,
- no whole-program crash,
- no long UI-thread freeze during initial load,
- merge-input panes can later show full syntax highlighting,
- diff / merge information is streamed or paged instead of fully materialized,
- resolved-output edits remain responsive,
- memory growth is dominated by shared source storage plus viewport-local caches, not by per-line full-file duplication.
