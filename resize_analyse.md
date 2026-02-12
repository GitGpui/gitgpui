# Resize lag analysis (window + columns)

This note documents likely causes of lag/jank when:

- resizing the app window, and
- resizing “columns” (history table columns + sidebar/details pane widths).

The goal is to identify **extra/unnecessary work** happening during resize interactions and propose targeted fixes.

## Quick summary (most likely culprits)

1. **History text truncation reshapes on every pixel change** and its cache is effectively disabled during resize:
   - `history_canvas::shape_truncated_line_cached` keys its cache by `max_width` (exact pixels) and hashes the full text each call.
   - During window/column drag, `max_width` changes continuously, causing cache misses, rapid cache growth, and periodic `cache.clear()`.
2. **History view can dispatch `LoadMoreHistory` from the render path** based on scroll geometry.
   - When the window height changes, `max_offset` can flip to `0` which makes the “near bottom” logic treat the view as needing more items.
   - This can trigger background git work while resizing, which competes for CPU and increases jank.
3. **Layout itself is expensive during resize** (expected), but callgrind shows layout dominates.
   - `taffy::compute::flexbox::*` is a top hotspot in `callgrind.out` (layout recalculation becomes the baseline cost of every resize frame).

## Where resize events come from

### 1) History table column resizing

History column widths are updated on drag move:

- `MainPaneView::history_column_headers` (`crates/gitgpui-ui-gpui/src/view/panels/popover.rs`)
  - `on_mouse_down` starts a `HistoryColResizeState`
  - `on_drag_move` updates `history_col_*` and calls `cx.notify()` on every drag event

This is “normal” (the UI needs to update), but it amplifies any per-frame render/layout cost.

### 2) Sidebar/details pane resizing

Pane widths are updated similarly:

- `GitGpuiView::pane_resize_handle` (`crates/gitgpui-ui-gpui/src/view/mod.rs`)
  - `on_drag_move` updates `sidebar_width` / `details_width` and calls `cx.notify()` per drag event

### 3) Window resizing

Window size changes are observed during render:

- `GitGpuiView::render` updates `last_window_size` and schedules settings persistence (`crates/gitgpui-ui-gpui/src/view/mod.rs`, `crates/gitgpui-ui-gpui/src/view/tooltip.rs`)
- `MainPaneView::render` also updates `last_window_size` and chooses history vs diff (`crates/gitgpui-ui-gpui/src/view/panes/main.rs`)

## Suspected extra/unnecessary work during resize

### A) History text truncation cache thrash (high confidence)

History rows paint text via `history_canvas::history_commit_row_canvas` (`crates/gitgpui-ui-gpui/src/view/rows/history_canvas.rs`).

Key details:

- Text is truncated and shaped with `line_wrapper(...).truncate_line(...)` inside `shape_truncated_line_cached`.
- The cache key includes:
  - `text.as_ref().hash(&mut hasher)` (O(text length) hashing per call)
  - `max_width.hash(&mut hasher)` (exact pixel width)

Why this hurts specifically during resize:

- When window width or history column widths change, the computed `max_width` changes constantly.
- That makes the cache nearly useless: every pixel width becomes a new cache key.
- The cache is cleared when it exceeds `HISTORY_TEXT_LAYOUT_CACHE_MAX_ENTRIES` (8192). During drag, it can fill quickly and then repeatedly clear, forcing repeated expensive shaping work.

This is the best match for the “feels laggy while dragging columns/window” symptom.

#### What to change (options)

1. **Quantize width in the cache key**
   - Bucket `max_width` to e.g. 4–8 px steps for caching:
     - `bucket = (max_width / 8px).round() * 8px`
   - This dramatically increases hit rate during drag while keeping visuals stable enough.
2. **Pre-hash text**
   - Store a stable `u64` hash in `HistoryCommitRowVm` (for `branches_text`, `summary`, `when`, `short_sha`) and use that in the cache key instead of hashing the full string each time.
3. **Degrade during active resize**
   - While `history_col_resize.is_some()` (or `pane_resize.is_some()`), skip `truncate_line` and:
     - shape the full text once and rely on clipping (`ContentMask`) without ellipsis, or
     - only re-truncate at a lower rate (coalesce to 30 Hz / next animation frame).
   - After resize ends, do the “nice” truncate+ellipsis pass once.

### B) Render-path `LoadMoreHistory` dispatch during window resize (medium confidence)

In `MainPaneView::history_view` (`crates/gitgpui-ui-gpui/src/view/panels/main.rs`), the render path can dispatch:

- `self.store.dispatch(Msg::LoadMoreHistory { repo_id });`

This is gated by a “near bottom” check. However:

- If `scroll_handle.max_offset().height` becomes `0` (no overflow), the code treats that as “should load by scroll = true”.
- During window resize (especially height increases), the list may temporarily become non-scrollable even if it was scrollable before.

Impact:

- Can trigger extra history loading while resizing, which can kick off git log work and cause CPU contention and extra UI updates.

Mitigations:

- Trigger “load more” from **scroll events** rather than from render.
- Or add a throttle: don’t dispatch `LoadMoreHistory` in response to pure window-size changes (track a “resizing” epoch, or require an actual scroll delta).

### C) Unconditional `cx.notify()` even when clamped widths don’t change (low confidence, easy win)

Both history column resizing and pane resizing call `cx.notify()` on every drag move, even if:

- the computed width hits a min/max clamp and stops changing.

This causes redundant renders/layout passes at the tail ends of drags.

Fix:

- Only `cx.notify()` if the width actually changed (`if next != current { ... }`).

### D) Baseline layout cost is high (expected, but sets the ceiling)

`callgrind.out` shows `taffy::compute::flexbox::compute_preliminary` as a top instruction cost.

Resize interactions force layout recalculation, so this becomes the “floor” of resize performance. If resizing still feels laggy after addressing A/B, the next step is to reduce:

- element tree depth in hot views,
- nested flex containers, and/or
- expensive per-row layout (prefer canvas-based rows where possible; history already does this).

## Suggested measurements to confirm

- Add counters around `shape_truncated_line_cached`:
  - calls/frame, cache hit rate, and number of clears.
- Temporarily log when `history_view` dispatches `LoadMoreHistory` and correlate with window resize.
- Capture a short `callgrind`/`perf` trace while continuously resizing history columns and look for:
  - `truncate_line`, `shape_line`, hashing, and cache clear activity.

