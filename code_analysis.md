# GitComet Code Analysis

Date: 2026-03-16

Scope: static source review of all crates — `gitcomet-core`, `gitcomet-app`, `gitcomet-git-gix`, `gitcomet-state`, and `gitcomet-ui-gpui`.

What I did not do in this pass:
- I did not run Criterion benches or add fresh measurements.
- I did not change production code.

Useful existing perf entry points:
- `crates/gitcomet-ui-gpui/benches/performance.rs`
- `crates/gitcomet-ui-gpui/src/bin/perf_budget_report.rs`
- `crates/gitcomet-ui-gpui/src/view/perf.rs`

---

## Executive Summary

The highest-value work is in the diff pipeline.

The current core diff engine already has a streamed plan representation, but several higher-level features fall back to fully materialized `Vec<FileDiffRow>` data and owned `String` copies. That means the project pays for both the fast path and the convenience path.

The other major hot path is repo monitoring and status refresh. Both still depend on extra `git` subprocess work in places where the rest of the stack is already using `gix`, which adds avoidable process and parsing overhead.

The main technical-debt theme is custom infrastructure with high maintenance surface:
- custom cache eviction in multiple places
- custom ignore matching via `git check-ignore`
- a large custom text model/input stack
- several very large source files that mix unrelated responsibilities

Additional themes from this deep-dive:
- **Rendering hot paths** in diff_text.rs and conflict_resolver.rs allocate strings, Vecs, and format! outputs on every frame
- **CLI argument parsing** in the app crate has duplicated logic, unnecessary PathBuf clones, and deeply nested match chains
- **Build configuration** has a duplicate resvg dependency and no dev/test profile tuning
- **Domain types** use owned Strings where Arc<str> would avoid cloning overhead

---

## Priority Backlog

### P0: Replacement alignment in `file_diff.rs` does repeated expensive similarity work

Evidence:
- `crates/gitcomet-core/src/file_diff.rs:593-602`
- `crates/gitcomet-core/src/file_diff.rs:1078-1151`

Why it matters:
- `push_aligned_replacement_runs_to_plan()` computes a DP matrix for delete/insert blocks.
- Every cell calls `replacement_pair_cost()`.
- `replacement_pair_cost()` calls `levenshtein_distance()`.
- `levenshtein_distance()` allocates `Vec<char>` for both strings on every call (lines 1127-1128).
- The prev/curr DP vectors (lines 1137-1138) are also freshly allocated per call.
- Near `REPLACEMENT_ALIGN_CELL_BUDGET` this becomes tens of thousands of repeated allocations and UTF-8 traversals in one diff region.

Action:
- Add a dedicated benchmark for replacement-heavy blocks, not just scroll/render benches.
- Pre-trim shared prefix/suffix before distance calculation.
- Cache per-line metadata needed by `replacement_pair_cost()` instead of recomputing it per cell.
- Replace the hand-rolled Levenshtein with an allocation-free or SIMD-backed implementation if benchmarking proves it wins. `triple_accel`, `rapidfuzz`, or a similar crate is worth evaluating here.
- Consider caching pair costs for repeated line pairs inside the same replacement block.
- Add a cheap initial heuristic (length difference, hash-based) before computing full distance to early-terminate obviously-bad pairs.

Validation:
- Extend the Criterion suite with a replacement-alignment benchmark.
- Watch both runtime and allocation count before/after.

### P0: Streamed diff providers still rematerialize owned rows and full inline text

Evidence:
- `crates/gitcomet-ui-gpui/src/view/panes/main/diff_cache.rs:250-318`
- `crates/gitcomet-ui-gpui/src/view/panes/main/diff_cache.rs:321-385`
- `crates/gitcomet-ui-gpui/src/view/panes/main/diff_cache.rs:497-509`
- `crates/gitcomet-ui-gpui/src/view/panes/main/diff_cache.rs:538-571`
- `crates/gitcomet-ui-gpui/src/view/panes/main/diff_cache.rs:649-682`
- `crates/gitcomet-ui-gpui/src/view/panes/main/diff_cache.rs:1752-1759`

Why it matters:
- `split_row()` and `inline_row()` convert borrowed line slices into owned `String`s on demand.
- Page caches store owned `FileDiffRow` / `AnnotatedDiffLine` values instead of lightweight refs.
- `build_inline_text()` walks the entire inline diff and rebuilds a full `SharedString`.
- `ensure_file_diff_inline_text_materialized()` triggers that full reconstruction even when a row provider already exists.

Action:
- Introduce compact row references backed by the plan plus source texts instead of storing owned strings in page caches.
- Reuse `SharedString` / `Arc<str>` where ownership is unavoidable.
- Change consumers that only need row iteration so they stop asking for full inline text.
- Add debug counters for:
  - page cache hit/miss
  - full inline-text materializations
  - rows materialized per frame/interaction

Validation:
- Existing benches: large diff scroll, paged rows, patch diff search.
- Add memory snapshots around opening and scrolling large diffs.

### P0: Multiple secondary features re-diff and rematerialize whole documents

Evidence:
- `crates/gitcomet-ui-gpui/src/view/markdown_preview.rs:248-268`
- `crates/gitcomet-ui-gpui/src/view/panes/main/helpers.rs:705-770`
- `crates/gitcomet-ui-gpui/src/view/conflict_resolver.rs:1975-1989`
- `crates/gitcomet-ui-gpui/src/view/conflict_resolver.rs:2945-3068`

Why it matters:
- Markdown diff preview calls `side_by_side_rows()` to compute change masks and row alignment.
- Conflict decision-region logic calls `side_by_side_rows_with_anchors()` even though it mainly needs anchors and line-emission counts.
- Conflict word highlighting calls `side_by_side_rows()` again before doing per-line word diffing.

Action:
- Extend `FileDiffPlan` with helpers/iterators for:
  - changed-line masks
  - region anchors
  - modify-pair iteration
  - prefix counts of "emits line on old/new side"
- Migrate markdown preview, conflict decision-region calculation, and word highlighting to those plan-level APIs.
- Keep `side_by_side_rows()` as a compatibility API for tests and small utilities only.

Validation:
- Existing markdown preview and conflict benches are the right guardrails.
- Add one benchmark that includes word-highlighting over a multi-block conflict.

---

### P1: Rendering hot paths allocate per-frame in diff_text.rs

Evidence:
- `crates/gitcomet-ui-gpui/src/view/panes/main/diff_text.rs:254-380` (diff_text_line_for_region)
- `crates/gitcomet-ui-gpui/src/view/panes/main/diff_text.rs:262` (expand_tabs)
- `crates/gitcomet-ui-gpui/src/view/panes/main/diff_text.rs:305,311,326,341,367` (SharedString clones)
- `crates/gitcomet-ui-gpui/src/view/panes/main/diff_text.rs:454` (format! in selection)
- `crates/gitcomet-ui-gpui/src/view/rows/diff_text.rs:398-416` (segment boundary Vec)
- `crates/gitcomet-ui-gpui/src/view/rows/diff_text.rs:900-902` (tab expansion when no tabs)

Why it matters:
- `diff_text_line_for_region()` clones SharedString at 5 return points per call, called O(visible_rows) per render.
- Tab expansion (line 262): when a line has no tabs, still does `s.to_string().into()` instead of `SharedString::from(s)` — unnecessary intermediate String.
- Line 454: `format!("{}\t{}", left, right)` builds a combined string for every row in split-view selection, even when the selection doesn't include that row.
- Segment boundary building (lines 398-416): allocates and sorts a Vec per line per render for syntax highlight boundaries.
- Tab expansion (line 900-902): when no tabs present, still allocates `text.to_string().into()` and clones the highlights Vec.

Action:
- Return `Cow<'_, SharedString>` or cache results from `diff_text_line_for_region()`.
- Fix `expand_tabs` to use `SharedString::from(s)` directly when no tabs present.
- Defer combined-string building in selection to only when actually needed.
- Pre-allocate and reuse the boundary Vec across line renders (thread-local or passed-in buffer).
- Return borrowed data from tab expansion when input is unchanged.

Validation:
- Profile with a 10k-line diff file and measure allocations per scroll frame.

### P1: Conflict resolver rendering allocates heavily per-frame

Evidence:
- `crates/gitcomet-ui-gpui/src/view/rows/conflict_resolver.rs:168-187` (Vec allocations)
- `crates/gitcomet-ui-gpui/src/view/rows/conflict_resolver.rs:197-277` (HashMap lookups in loop)
- `crates/gitcomet-ui-gpui/src/view/rows/conflict_resolver.rs:303,357,682-686,734-738` (format! for menu IDs)
- `crates/gitcomet-ui-gpui/src/view/rows/conflict_resolver.rs:548,556,568,607,636,666` (line.to_string() -> SharedString)

Why it matters:
- `conflict_choices` Vec and `real_line_indices` Vec are freshly allocated every render call.
- Context menu ID strings are built via `format!()` on every render even when no menu is open.
- `.map(|line| SharedString::from(line.to_string()))` appears 6+ times — intermediate String is unnecessary, use `SharedString::from(line)` directly.
- Multiple `.clone()` calls on closures/handlers per conflict chunk (lines 363, 367, 699-703, 751-755).

Action:
- Cache `conflict_choices` at the model layer; update only when segments change.
- Use lazy evaluation for context menu strings — only build when a menu is opened.
- Replace `SharedString::from(line.to_string())` with `SharedString::from(line)`.
- Consider using `Arc` wrapping for handler data to reduce clone cost.

Validation:
- Open a conflict with 20+ blocks and profile scroll performance.

### P1: Repo monitor ignore matching is process-heavy and semantically complex

Evidence:
- `crates/gitcomet-state/src/store/repo_monitor.rs:356-596`
- `crates/gitcomet-state/src/store/repo_monitor.rs:793-850`

Why it matters:
- Cache misses spawn `git check-ignore` work, sometimes in batches and sometimes with synthetic probe paths to emulate directory-only rules.
- This lives inside the file-event classification loop for the active repo.
- The code is careful, but it is also expensive and brittle: subprocess spawn, I/O, parsing, TTL cache, and rule-probe semantics all sit on the hot path.

Action:
- Add counters for ignore lookups and average/burst latency first.
- Replace subprocess-based matching with an in-process matcher.
- Prefer `gix` ignore support if it covers nested `.gitignore`, excludesfile, and tracked-vs-ignored behavior cleanly.
- If `gix` is awkward here, evaluate the `ignore` crate and keep the current test suite as the parity oracle.

Validation:
- Reuse the existing repo-monitor tests as regression coverage.
- Add a filesystem-event burst perf smoke test.

### P1: Status refresh still shells out to `git status` after running `gix` status

Evidence:
- `crates/gitcomet-git-gix/src/repo/status.rs:132-185`
- `crates/gitcomet-git-gix/src/repo/status.rs:356-398`

Why it matters:
- `status_impl()` already builds status with `gix`.
- It then always runs `git status --porcelain=v2 -z --ignore-submodules=none` to supplement gitlink/submodule entries.
- On frequently refreshed status paths this duplicates repository scanning and output parsing, even for repos that do not use submodules.

Action:
- Gate `supplement_gitlink_status_from_porcelain()` behind a cheap "repo likely has gitlinks/submodules" check.
- Cache that capability check per repo.
- Long-term, move gitlink status supplementation in-process if `gix` can provide enough data.
- If large submodule inventories matter, also replace linear `push_status_entry()` dedupe with set-backed insertion.

Validation:
- Compare repo-open and status-refresh timings on:
  - repo without submodules
  - repo with many submodules/gitlinks

### P1: Custom cache eviction is arbitrary, not usage-based

Evidence:
- `crates/gitcomet-ui-gpui/src/view/rows/mod.rs:36-57`
- `crates/gitcomet-ui-gpui/src/view/panes/main/diff_cache.rs:76-99`

Why it matters:
- These helpers evict `HashMap` keys by iterating `cache.keys().take(remove_count)`.
- That is arbitrary hash iteration order, not LRU or MRU.
- Hot entries can be evicted while cold entries survive, which creates unpredictable cache behavior and makes perf debugging harder.
- The eviction helper also allocates a `Vec<u64>` of keys to remove (could iterate and remove directly).

Action:
- Replace these helpers with a real cache policy.
- `lru`, `hashlink`, or `mini-moka` are better fits than more bespoke eviction code.
- Centralize cache wrappers so all caches expose the same counters and invalidation policy.

Validation:
- Measure hit ratio for history text shaping and diff page caches before/after.

### P1: Domain types use owned Strings where Arc<str> would save cloning

Evidence:
- `crates/gitcomet-core/src/domain.rs:16-29` (CommitId, Commit)

Why it matters:
- `CommitId(pub String)` and `Commit::summary`, `Commit::author` are frequently shared/passed by value.
- `DiffLine::text` already uses `Arc<str>` (line 226), but domain-level types don't follow the same pattern.
- Commit data is immutable after creation — perfect candidate for Arc<str>.

Action:
- Change `CommitId` to wrap `Arc<str>` instead of `String`.
- Use `Arc<str>` for `Commit::summary` and `Commit::author`.
- Audit other domain types passed by value to see if they should follow.

Validation:
- Check that existing tests pass and profile clone-heavy paths (history list, branch list).

---

### P2: text_input.rs has multiple per-edit and per-render allocation issues

Evidence:
- `crates/gitcomet-ui-gpui/src/kit/text_input.rs:1877-1878,2614-2615,2651-2652` (double .clone() on range/inserted)
- `crates/gitcomet-ui-gpui/src/kit/text_input.rs:2285,2296,2301,2316,2350` (debug_selector .to_string())
- `crates/gitcomet-ui-gpui/src/kit/text_input.rs:1232,1822,1830,2563` (selected text .to_string())

Why it matters:
- On every keystroke, `range.clone()` and `inserted.clone()` are called twice in a row to feed two consumers.
- `debug_selector(|| "text_input_context_cut".to_string())` creates new String allocations on every render, even in release builds.
- Selected text is freshly allocated via `.to_string()` on each access.

Action:
- Extract range/inserted pair once, pass by reference to both consumers.
- Use `&'static str` for debug selectors instead of runtime String allocation.
- Cache selected text or use `Cow<str>`.

Validation:
- Profile keystroke latency in a large text input.

### P2: text_model.rs piece operations clone unnecessarily

Evidence:
- `crates/gitcomet-ui-gpui/src/kit/text_model.rs:577-578` (ranges.iter().cloned())
- `crates/gitcomet-ui-gpui/src/kit/text_model.rs:612,622,624,677,687` (piece.clone() in split/merge)

Why it matters:
- `ranges.iter().cloned()` clones every Range when `.iter().enumerate()` suffices.
- Piece structs are cloned in split_pieces_at and merge_adjacent_pieces when moves would work.

Action:
- Use `ranges.iter().enumerate()` instead of `.cloned()`.
- Use move semantics for Piece in split/merge operations where possible.

### P2: The custom text model still pays full-document costs in important paths

Evidence:
- `crates/gitcomet-ui-gpui/src/kit/text_model.rs:75-121`
- `crates/gitcomet-ui-gpui/src/kit/text_model.rs:184-195`
- `crates/gitcomet-ui-gpui/src/kit/text_model.rs:321-348`

Why it matters:
- `LineIndex::apply_edit()` rebuilds the edited line-start array and then sorts/dedups it.
- `materialized()` reconstructs the whole document into a `SharedString`.
- The project already has dedicated benchmarks for text model load and snapshot clone cost, which is a sign this area is performance-sensitive.

Action:
- Low-risk improvement: rewrite `LineIndex::apply_edit()` so it emits monotonic output directly and removes the final `sort_unstable()` / `dedup()`.
- Add benchmarks for fragmented-buffer random edits and repeated `as_str()` / `as_shared_string()` access after edits.
- If editor ambitions keep expanding, evaluate `ropey` or `xi-rope` against the existing benchmarks before adding more bespoke structure around the current model.

Validation:
- Use the existing text-model and text-input benches as the decision point for whether a rope migration is justified.

### P2: Store mutation cost is hidden behind `Arc::make_mut`

Evidence:
- `crates/gitcomet-state/src/store/mod.rs:91-102`
- `crates/gitcomet-state/src/model.rs:136-143`
- `crates/gitcomet-state/src/model.rs:315-370`

Why it matters:
- Every dispatched message mutates `Arc<AppState>` through `Arc::make_mut()`.
- If the UI is holding a snapshot, the whole top-level state tree is cloned before the reducer mutates it.
- Many large payloads are behind `Arc`, so this may be acceptable today, but the cost is invisible without instrumentation.

Action:
- Add reducer timing and clone-cost counters before changing architecture.
- If it becomes visible in traces, split state into smaller shared nodes or move to a more selective propagation model.

Validation:
- Measure dispatch throughput during repo open, refresh storms, and conflict-view interactions.

### P2: Diff line classification uses linear prefix chain

Evidence:
- `crates/gitcomet-core/src/domain.rs:344-366` (classify_unified_line)

Why it matters:
- 10+ sequential `starts_with()` checks on every diff line.
- Could use first-byte dispatch for faster classification.

Action:
- Refactor to match on `raw.as_bytes().first()` then check the specific prefix:
```rust
match raw.as_bytes().first() {
    Some(b'@') if raw.starts_with("@@") => DiffLineKind::Hunk,
    Some(b'd') if raw.starts_with("diff ") => DiffLineKind::Header,
    Some(b'+') if !raw.starts_with("+++ ") => DiffLineKind::Add,
    Some(b'-') if !raw.starts_with("--- ") => DiffLineKind::Remove,
    _ => DiffLineKind::Context,
}
```

Validation:
- Benchmark against a large unified diff (10k+ lines).

### P2: Page caching has double allocation in load_page

Evidence:
- `crates/gitcomet-core/src/domain.rs:270-285` (PagedDiffLineProvider::load_page)

Why it matters:
- `self.lines[start..end].to_vec()` creates a temporary Vec, then converts to `Arc<[DiffLine]>` (two allocations where one suffices).

Action:
- Use `Arc::from(&self.lines[start..end])` or build directly from an iterator to avoid the intermediate Vec.

### P2: Histogram diff creates unnecessary slice copies

Evidence:
- `crates/gitcomet-core/src/file_diff.rs:1180-1181`

Why it matters:
- `old[old_start..old_end].to_vec()` and `new[new_start..new_end].to_vec()` allocate Vecs of `&str` references when slices could be passed directly.

Action:
- Pass `&old[old_start..old_end]` as a slice reference instead of `.to_vec()`.

---

## Code Complexity and Duplication (App Crate)

### Deeply nested match chains in CLI argument validation

Evidence:
- `crates/gitcomet-app/src/cli.rs:295-342` (classify_difftool_input — 3-level nested match, 9 arms)
- `crates/gitcomet-app/src/cli.rs:374-411` (validate_existing_merged_output_path — near-duplicate logic)
- `crates/gitcomet-app/src/cli.rs:474-554` (resolve_mergetool_with_env — 81 lines, 6 responsibilities)

Action:
- Extract `classify_path_or_symlink_target()` helper to deduplicate symlink metadata logic between the two functions.
- Split `resolve_mergetool_with_env()` into `validate_marker_size()`, `resolve_and_validate_merge_paths()`, `parse_conflict_style()`, `parse_diff_algorithm()`.

### Compat argument parsing is a 240-line if-chain

Evidence:
- `crates/gitcomet-app/src/cli/compat.rs:184-410` (parse_compat_external_mode_with_config)

Why it matters:
- 15+ sequential if-statements for CLI argument parsing.
- PathBuf clones from `positionals` vector instead of using moves (lines 343-344, 369-371, 378-380, 393-394).

Action:
- Refactor to match/state machine pattern.
- Use `Vec::into_iter()` or destructuring to move PathBufs instead of cloning.

### Duplicated hex encoding

Evidence:
- `crates/gitcomet-app/src/cli/compat.rs:89-97`
- `crates/gitcomet-app/src/crashlog.rs:256-264`

Action:
- Move `hex_encode()` to a shared utility or use the `hex` crate.

### Error formatting boilerplate

Evidence:
- `crates/gitcomet-app/src/difftool_mode.rs:276,298,318,326,330,365,391,394,396,402,417,419,445,460,481,500` (~20 occurrences)

Action:
- Create a macro: `io_err!($e, $op, $path)` to deduplicate.

### UTF-8 fallback conversion is suboptimal

Evidence:
- `crates/gitcomet-app/src/difftool_mode.rs:74-107` (bytes_to_text_preserving_utf8)

Why it matters:
- Called for every `git diff` invocation. Uses `write!(out, "\\x{byte:02x}")` macro overhead per invalid byte.

Action:
- Replace with direct char pushes: `out.push('\\'); out.push('x'); ...` or a lookup table.

---

## Code Complexity (Core Crate)

### Trailing newline merge logic

Evidence:
- `crates/gitcomet-core/src/merge.rs:509-563`

Why it matters:
- Complex 3-way merge decision with `#[allow(clippy::if_same_then_else)]` — recognized complexity.

Action:
- Extract to `merge_trailing_newline_decision()` with clear parameters and test cases.

### Redundant allocation in conflict merge

Evidence:
- `crates/gitcomet-core/src/merge.rs:297-335` (merge_hunks)

Why it matters:
- `reconstruct_side()` always allocates a new Vec. When ours == theirs, we allocate twice then discard one.

Action:
- Add a short-circuit: compute and compare hashes of both sides before allocating the second.

### Mutex handling pattern repeated

Evidence:
- `crates/gitcomet-core/src/auth.rs:44,50,56`

Action:
- Extract `fn lock_guard<T>(slot: &Mutex<T>) -> MutexGuard<T>` helper.

### Custom error boilerplate

Evidence:
- `crates/gitcomet-core/src/error.rs` (~80 lines of manual Display/Error impl)

Action:
- Consider `thiserror` to eliminate boilerplate. Low priority but reduces maintenance.

---

## Build and Dependency Issues

### Duplicate resvg versions

Evidence:
- `Cargo.lock` contains both `resvg v0.45.1` and `resvg v0.47.0`

Why it matters:
- Both versions compile, increasing build time and binary size.

Action:
- Identify which transitive dependency pulls `0.45.1` and update it to resolve to `0.47.0` only.

### Missing dev/test build profiles

Evidence:
- No `[profile.dev]` or `[profile.test]` in workspace Cargo.toml.

Action:
```toml
[profile.dev]
incremental = true
opt-level = 0

[profile.test]
opt-level = 1
split-debuginfo = "packed"
```

### Tree-sitter grammars are not feature-gated

Evidence:
- 12 language grammars (bash, css, go, html, javascript, json, python, rust, typescript, xml, yaml) always compiled.
- Each grammar adds ~500KB-2MB to the binary.

Action:
- Add feature flags for language subsets (e.g., `ts-all`, `ts-minimal`).
- Default feature includes common languages; full set opt-in.

### Large test/benchmark files slow incremental compilation

Evidence:
- `view/panels/tests.rs`: 9,899 lines
- `view/rows/benchmarks.rs`: 6,304 lines
- `view/conflict_resolver/tests.rs`: 4,707 lines
- `view/panels/popover/tests.rs`: 2,109 lines

Action:
- Move to separate `#[cfg(test)]` modules or a dedicated test crate.
- Expected 10-20% faster debug builds.

### Binary size: 41MB release build

Evidence:
- gitcomet-ui-gpui: 29MB, gitcomet-app: 41MB total

Action:
- Audit with `cargo bloat` after fixing resvg duplication and feature-gating tree-sitter.

---

## Simplification and Reuse Opportunities

### Reuse the streamed diff plan everywhere

This is the best reuse opportunity in the codebase.

Today, the codebase has both:
- a compact plan representation in `gitcomet_core::file_diff`
- several consumers that fall back to materialized `Vec<FileDiffRow>`

Consolidating on the plan-level API will:
- remove duplicate diff work
- reduce allocations
- make correctness fixes land in one place
- shrink the number of rendering/data adapters the team needs to maintain

### Replace ad hoc UI caches with a shared cache abstraction

The current cache story is fragmented:
- page caches in `diff_cache.rs`
- text-shape caches in `rows/mod.rs` and `history_canvas.rs`
- provider highlight caches in `text_input.rs`

A small shared cache module with:
- explicit policy (LRU via `lru` or `mini-moka`)
- counters
- uniform invalidation

would remove duplicated eviction code and make cache tuning much easier.

### Replace subprocess ignore matching with an in-process matcher

This is both a performance improvement and a simplification.

The current code works hard to preserve Git semantics. That effort is valuable, but it belongs inside a proper ignore engine rather than in bespoke subprocess orchestration and synthetic path probes.

### Deduplicate symlink classification in app crate

Two functions (`classify_difftool_input` and `validate_existing_merged_output_path`) have near-identical nested match logic for path/symlink classification. Extract shared helper.

---

## Large-File Technical Debt Map

These files are large enough that perf work inside them will remain risky until responsibilities are split:

| File | LOC | Risk |
|------|-----|------|
| `view/rows/diff_text/syntax.rs` | ~5.9k | Syntax parsing + projection + reuse |
| `kit/text_input.rs` | ~5.6k | Selection + wrap + highlight + paint + actions |
| `view/panes/main/diff_cache.rs` | ~4.4k | Streamed diff + providers + image cache |
| `view/conflict_resolver.rs` | ~4.3k | Bootstrap + render + word highlight + actions |
| `gitcomet-core/file_diff.rs` | ~2.0k | Plan + algorithms + anchors + materialize |
| `cli/compat.rs` | ~450 | Parsing + label assignment + mode detection |

Suggested decomposition:

- `text_input.rs`
  - `selection.rs`
  - `wrap.rs`
  - `highlight_provider.rs`
  - `paint_cache.rs`
  - `actions.rs`

- `diff_text/syntax.rs`
  - `prepared_document.rs`
  - `background_jobs.rs`
  - `line_projection.rs`
  - `reuse.rs`

- `diff_cache.rs`
  - `streamed_diff.rs`
  - `inline_provider.rs`
  - `syntax_cache.rs`
  - `image_cache.rs`
  - `pane_adapter.rs`

- `conflict_resolver.rs`
  - `bootstrap.rs`
  - `render_model.rs`
  - `word_highlight.rs`
  - `large_block_preview.rs`
  - `actions.rs`

- `file_diff.rs`
  - `plan.rs`
  - `algorithms/myers.rs`
  - `algorithms/patience.rs`
  - `anchors.rs`
  - `materialize.rs`
  - `tests.rs`

---

## Quick Wins (can be done independently, low risk)

These items require minimal context and can be picked up in any order:

| # | Item | File(s) | Est. effort |
|---|------|---------|-------------|
| 1 | Fix `expand_tabs` to skip allocation when no tabs | diff_text.rs:262 | 5 min |
| 2 | Replace `line.to_string()` -> SharedString with direct conversion | conflict_resolver.rs:548+ (6 places) | 10 min |
| 3 | Remove `.clone()` on Range<usize> (it's Copy) | conflict_resolver.rs:109 | 2 min |
| 4 | Use `&'static str` for debug_selector strings | text_input.rs:2285-2350 | 10 min |
| 5 | Fix double .clone() on range/inserted in edit path | text_input.rs:1877-1878 | 5 min |
| 6 | Pass slice refs instead of `.to_vec()` in histogram diff | file_diff.rs:1180-1181 | 5 min |
| 7 | Deduplicate `hex_encode()` | compat.rs + crashlog.rs | 15 min |
| 8 | Extract mutex lock helper | auth.rs:44,50,56 | 5 min |
| 9 | Use first-byte dispatch in classify_unified_line | domain.rs:344-366 | 15 min |
| 10 | Fix page cache double-allocation | domain.rs:270-285 | 10 min |

---

## Recommended Handover Plan

### Phase 0: Measure first

Add counters/benchmarks for:
- replacement-alignment cost in `file_diff.rs`
- full inline-text materialization count
- repo-monitor ignore lookup count and latency
- gitlink-status supplement frequency
- reducer clone time / dispatch latency
- rendering allocations per frame in diff_text and conflict_resolver

Tie new measurements into the existing Criterion and perf-budget infrastructure where it makes sense.

### Phase 1: Quick wins + low-risk improvements

Do these first (all independent, parallelizable):
- All items from the Quick Wins table above.
- Gate gitlink status supplementation so non-submodule repos do not pay for it.
- Replace arbitrary `HashMap` partial-eviction caches with a real LRU cache.
- Fix duplicate resvg dependency.
- Add dev/test build profiles.
- Stop using `side_by_side_rows*()` in helpers that only need anchors, masks, or modify-pair traversal.
- Remove `sort_unstable()` / `dedup()` from `LineIndex::apply_edit()` by constructing ordered output directly.

### Phase 2: Medium refactors

- Add plan-level iterators/helpers to `FileDiffPlan`.
- Migrate markdown preview and conflict highlighting to those plan-level APIs.
- Rework streamed diff page providers so they cache compact refs instead of owned strings.
- Refactor rendering hot paths in diff_text.rs to use buffers/caches instead of per-frame allocation.
- Cache conflict_choices and context menu IDs in conflict_resolver.
- Use Arc<str> for domain types (CommitId, Commit fields).
- Feature-gate tree-sitter language grammars.
- Extract large test/benchmark files to dedicated modules.

### Phase 3: Architectural bets

- Replace repo-monitor `git check-ignore` subprocess logic with an in-process matcher.
- Decide whether the custom text model remains worth the maintenance cost or whether a rope crate should replace it.
- Keep retiring `git` CLI fallbacks from hot paths where `gix` can cover the behavior.
- Split large files per the decomposition plan.
- Refactor compat argument parsing from if-chain to state machine.

## Recommended Order of Work

1. Quick wins (Phase 1 table) — get familiar with the codebase, build momentum.
2. `file_diff.rs` plus streamed diff provider cleanup.
3. Rendering hot path cleanup (diff_text + conflict_resolver).
4. Repo monitor ignore matching.
5. Cache-policy cleanup.
6. Domain type Arc<str> migration.
7. Build/dependency cleanup.
8. Text model and store architecture, but only after measurement.

## Questions to Answer Before Starting

- Which UI flows truly require full inline diff text instead of paged row access? I don't know, inline diff is used as toggle button functionality when diffing changes, but even that can use the streaming approach.
- How common are submodules/gitlinks in the repos you care about most? Submodules are rare
- Does repo monitoring need exact Git ignore semantics in all cases, or only enough fidelity to suppress noise? This is Git GUI so we should follow gitignore semantics
- Is the custom editor stack a strategic part of the product, or would a rope crate reduce more risk than it adds? Custom editor fits strategy, we may extend it later
- What's the target binary size? Is 41MB acceptable or should tree-sitter grammars be trimmed? 41MB is acceptable
