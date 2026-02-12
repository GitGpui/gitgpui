# Performance TODOs (CPU + rendering)

This file tracks concrete optimization work (and progress) for gitgpui’s CPU usage, frame-time stability, and “time to interactive”.

## Progress log

- **2026-02-12**: Reviewed `callgrind.out` + scanned render-path caches; added TODOs focused on layout/hit-testing churn, hash/map hot paths, and moving file diff/image diff cache building off the render thread.
- **2026-02-12**: Added TODOs for branch sidebar scaling and avoiding per-frame `Path::display().to_string()` formatting in always-visible chrome.
- **2026-02-12**: Added resize lag analysis (history truncation cache thrash + render-path load-more) in `resize_analyse.md`.

## Recently addressed

- [x] Removed redundant/stale `rebuild_diff_cache()` calls after `store.dispatch` (they rebuilt from the *old* snapshot and wasted CPU).
- [x] Stopped rebuilding the root view’s diff cache on every snapshot; the main pane owns diff rendering/state now.
- [x] Made “build patch for this hunk” read from `RepoState.diff` (avoids requiring an annotated/cloned diff in the root view).

## High-impact opportunities (current focus)

- [ ] **Diff**: Remove `annotate_unified` string duplication by sharing diff line text (e.g. `Arc<str>`), or by storing old/new line numbers alongside `DiffLine`.
  - Files: `crates/gitgpui-core/src/diff.rs`, `crates/gitgpui-core/src/domain.rs`, `crates/gitgpui-ui-gpui/src/view/panes/main/diff_cache.rs`
- [ ] **Diff**: `rebuild_diff_word_highlights` is O(diff) and can be expensive on large diffs; compute lazily for visible rows or move to a background task with coalescing.
  - File: `crates/gitgpui-ui-gpui/src/view/panes/main/diff_cache.rs`
- [ ] **History**: History cache builds graph + row VMs for every commit in the loaded page; for very large pages, compute only the `uniform_list` window/overscan or memoize per commit.
  - Files: `crates/gitgpui-ui-gpui/src/view/caches.rs`, `crates/gitgpui-ui-gpui/src/view/history_graph.rs`

## More high-impact opportunities

- [ ] **Diff (file view)**: `ensure_file_diff_cache()` runs in the diff render path and can do a lot of work on first render (side-by-side row building, per-row `format!`, and word-diff precompute); move rebuild off the UI thread and/or compute lazily for visible rows.
  - Files: `crates/gitgpui-ui-gpui/src/view/panels/main.rs`, `crates/gitgpui-ui-gpui/src/view/panes/main/diff_cache.rs`
- [ ] **Diff (image view)**: `gpui::Image::from_bytes` happens during render via `ensure_file_image_diff_cache()`; decode in a background task and cache decoded `Arc<Image>` (optionally downscale) to avoid UI stalls on large images.
  - Files: `crates/gitgpui-ui-gpui/src/view/panels/main.rs`, `crates/gitgpui-ui-gpui/src/view/panes/main/diff_cache.rs`
- [ ] **Diff (hit testing)**: Callgrind shows `gpui::bounds_tree::{insert,find_max_ordering}` as a top cost; reduce per-row hitboxes in diff rendering (e.g. accept I-beam cursor for whole row, aggregate hit-testing at the list level, or otherwise reduce `window.insert_hitbox` usage).
  - File: `crates/gitgpui-ui-gpui/src/view/rows/diff_canvas.rs`
- [ ] **Diff (hash/map hot paths)**: Callgrind shows hashing work (`ArcCow<str>::hash`) and heavy memcpy/malloc; use faster hashers/maps for hot caches and avoid `DefaultHasher` in tight loops (e.g. gutter text shaping cache, `diff_text_layout_cache`).
  - Files: `crates/gitgpui-ui-gpui/src/view/rows/diff_canvas.rs`, `crates/gitgpui-ui-gpui/src/view/panes/main.rs`
- [ ] **Diff (layout cache pruning)**: `prune_diff_text_layout_cache()` does an O(n log n) sort when over capacity; replace with an LRU-ish structure or incremental pruning to avoid spikes when scrolling huge diffs.
  - File: `crates/gitgpui-ui-gpui/src/view/panes/main.rs`
- [ ] **Diff (nav entries)**: `diff_nav_entries()`/`file_change_visible_indices()` scan all visible rows; cache results keyed by `(repo_id, diff_rev, diff_target, diff_view, is_file_view)` to avoid O(n) work when rendering nav UI / jumping.
  - File: `crates/gitgpui-ui-gpui/src/view/panes/main.rs`
- [ ] **UI (layout cost)**: Callgrind shows `taffy` flex layout dominating; audit large lists/panes to reduce element count and nested flexboxes (and prefer canvas-based rows where appropriate, like history graph rows).
  - Files: `crates/gitgpui-ui-gpui/src/view/panels/layout.rs`, `crates/gitgpui-ui-gpui/src/view/rows/history.rs`, `crates/gitgpui-ui-gpui/src/view/history_graph.rs`
- [ ] **UI (string formatting in hot paths)**: Callgrind shows `core::fmt::write` high; remove `format!`/`to_string()` from per-row render paths where possible (prefer tuple ids, cached `SharedString`, or precomputed labels).
  - File: `crates/gitgpui-ui-gpui/src/view/rows/status.rs`
- [ ] **Sidebar (branch list scaling)**: Branch sidebar builds a full `Vec<BranchSidebarRow>` in the render path when the fingerprint changes; for repos with huge branch counts, move rebuild to a background task and/or make row generation lazy.
  - Files: `crates/gitgpui-ui-gpui/src/view/panes/sidebar.rs`, `crates/gitgpui-ui-gpui/src/view/branch_sidebar.rs`
- [ ] **Chrome (workdir display formatting)**: Avoid `workdir.display().to_string()` in always-visible bars (repo tabs, action bar); use `cached_path_display` or store preformatted workdir labels in view state on repo switch.
  - File: `crates/gitgpui-ui-gpui/src/view/panels/bars.rs`
- [ ] **Diff search**: `diff_search_recompute_matches_for_current_view()` scans the entire visible diff and runs on the UI thread; move to a background task with coalescing/cancellation and consider a faster ASCII search implementation.
  - File: `crates/gitgpui-ui-gpui/src/view/panes/main/diff_search.rs`
- [ ] **History (resize jank)**: `history_canvas::shape_truncated_line_cached` keys cache by exact `max_width` and hashes full text; during window/column resize this thrashes and can clear frequently. Quantize width and/or pre-hash text; optionally degrade (clip w/o ellipsis) while resizing.
  - File: `crates/gitgpui-ui-gpui/src/view/rows/history_canvas.rs`
- [ ] **History (load-more on resize)**: `history_view` dispatches `LoadMoreHistory` from the render path; window resizing can change scroll `max_offset` and trigger extra loads. Move to scroll-event-driven dispatch or throttle while resizing.
  - File: `crates/gitgpui-ui-gpui/src/view/panels/main.rs`
