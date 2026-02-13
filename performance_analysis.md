# Performance analysis: why Zed feels “instant” vs why gitgpui can feel “laggy”

This document is a codebase-grounded explanation of why the **Zed** editor feels extremely responsive (low perceived input latency, few frame drops), while **gitgpui** currently can feel “a little laggy everywhere”.

Both projects use **gpui** and Rust, so the perceived difference is mostly about **how work is scheduled**, **how often the UI is invalidated**, **how much work happens on the UI thread**, and **how much is allocated per frame**.

---

## TL;DR (most likely causes of “lag everywhere” in gitgpui)

Ordered by “likely to affect everything”:

1. **Over-wide invalidation:** the whole window (root + panes) tends to re-render on *every* store update.
   - Root view observes `AppUiModel` and calls `cx.notify()` unconditionally: `crates/gitgpui-ui-gpui/src/view/mod.rs`.
   - Sidebar / main / details panes also observe the same model and call `cx.notify()` per update: `crates/gitgpui-ui-gpui/src/view/panes/{sidebar,main,details}.rs`.
2. **Layout + element-tree costs dominate:** Callgrind shows substantial time in `taffy` flexbox layout + `memcpy`/`malloc`/`free`.
   - See `callgrind.out` and notes in `resize_analyse.md`.
3. **Render-path “hidden work”:** there are still hotspots that can land in render/row-processor paths:
   - Per-line Tree-sitter parsing for diff syntax highlighting: `crates/gitgpui-ui-gpui/src/view/rows/diff_text/syntax.rs`.
   - Text shaping/truncation cache churn during resize: `resize_analyse.md` (history canvas).
4. **Background task contention:** the state store can fan out many background Git loads in parallel, which can compete with the UI thread for CPU/cache/disk.
   - Store + worker pool: `crates/gitgpui-state/src/store/{mod.rs,executor.rs,effects.rs}`.
5. **Version/production gap:** Zed ships as a highly optimized release build and often tracks newer gpui changes; gitgpui is frequently run from `cargo run` (debug) while iterating.

---

## What “feels fast” actually means

Users perceive “fast UI” mostly as:

- **Low input-to-photon latency**: typing/scrolling/hover feels immediate (p95 frame time stays under ~16ms at 60Hz).
- **No jank under load**: background IO/CPU doesn’t steal enough time to cause missed frames.
- **Stable frame pacing**: fewer spikes are more important than a lower average.

Zed’s architecture and component patterns are designed to protect that budget aggressively.

---

## Runtime & scheduling: Zed vs gitgpui

### Zed (high-level)

From code in `zed/`:

- Zed invests heavily in **instrumentation** so regressions are visible:
  - In-app profiler UI: `zed/crates/miniprofiler_ui/src/miniprofiler_ui.rs`.
- Zed supports **alternative allocators** to reduce allocator contention/fragmentation under UI workloads:
  - Optional global allocator: `#[cfg(feature = "mimalloc")]` in `zed/crates/zed/src/main.rs`.
- Zed uses a **global filesystem watcher** with filtering and batching to avoid event storms:
  - Global watcher + filtering out `EventKind::Access(_)`: `zed/crates/fs/src/fs_watcher.rs`.

Taken together: Zed treats “performance as a feature”: it measures, filters noise, and minimizes unpredictable stalls.

### gitgpui (current)

gitgpui has a clean, testable MVU-ish store, but the default wiring tends to produce broader UI churn:

- **Store thread**: `AppStore::new` runs a dedicated thread with:
  - `std::sync::mpsc` message queue (unbounded),
  - a fixed worker pool (up to 8 threads) for effects, and
  - a coalesced “state changed” event channel (bounded 1).
  - See: `crates/gitgpui-state/src/store/mod.rs`, `crates/gitgpui-state/src/store/executor.rs`.
- **UI poller**:
  - Waits for `StoreEvent::StateChanged`,
  - clones a full `AppState` snapshot on a background executor, then
  - updates `AppUiModel` with a new `Arc<AppState>`.
  - See: `crates/gitgpui-ui-gpui/src/view/poller.rs`, `crates/gitgpui-ui-gpui/src/view/app_model.rs`.
- **Multiple subscribers**:
  - Root view + each pane observe the same model and notify on every update.
  - See: `crates/gitgpui-ui-gpui/src/view/mod.rs`, `crates/gitgpui-ui-gpui/src/view/panes/{sidebar,main,details}.rs`.

This is *correct*, but it increases the probability that “any background change” translates into “the whole window did a bunch of work”.

---

## Component patterns & invalidation scope (the biggest difference)

gpui is immediate-mode-ish: `render()` builds an element tree, and `cx.notify()` schedules another render.

That means performance is dominated by:

1. **How often you render**, and
2. **How much work each render implies** (layout, hit-testing, text shaping, allocations).

### Zed’s common pattern

Zed tends to:

- Split the UI into many **feature-local entities** (each with its own state and render surface).
  - Example: the Git panel is its own view with its own `uniform_list` and local state: `zed/crates/git_ui/src/git_panel.rs`.
- Keep “expensive derivations” out of render paths and behind caches/version counters.
- Notify only the parts that changed (or at least make it easy to do so).

### gitgpui’s current pattern

gitgpui already has pane entities (`SidebarPaneView`, `MainPaneView`, `DetailsPaneView`), but today:

- All panes observe the same `AppUiModel` and notify per update.
- The root view *also* observes the same model and notifies per update.

So a single Git load finishing can trigger:

- store reduces state → event emitted
- poller snapshots → model updated
- root rerender + sidebar rerender + main rerender + details rerender

Even if only one pane’s visible data actually changed, you still pay:

- layout costs (taffy),
- hit-testing/bounds-tree work,
- allocations in render composition,
- and possible derived-cache updates.

This is the most “global” explanation for “a little lag everywhere”.

---

## Evidence from profiling already in this repo

### Callgrind: layout + allocations are significant

`callgrind.out` (generated from `./target/release-with-debug/gitgpui-app`) shows large exclusive costs in:

- `taffy::compute::flexbox::*` (layout)
- `memcpy`/`malloc`/`free` (allocation + copying)
- gpui bounds-tree operations (hit-testing / ordering)

This aligns with a UI that re-renders broadly and builds a lot of element/layout work.

### Resize-specific jank: cache thrash

`resize_analyse.md` identifies a high-confidence culprit during resizing:

- history text truncation/shaping cache keys by exact width → cache miss storms during pixel-by-pixel drags.

Even if that’s “only resize”, it’s an example of a broader theme:
**the cache key granularity matters**, and “work per pixel” is visible as jank.

### Diff syntax highlighting can be expensive

gitgpui highlights diff lines by sometimes running Tree-sitter per-line parsing:

- `crates/gitgpui-ui-gpui/src/view/rows/diff_text/syntax.rs`

Even with guardrails (line-length caps), doing parsing in a per-visible-line path can add latency while scrolling diffs.

---

## Why Zed feels faster (practical reasons)

These are the concrete, repeatable reasons Zed tends to feel “instant”:

1. **Strict UI-thread budget discipline**: expensive work is pushed off-thread and staged.
2. **Fine-grained invalidation**: most changes don’t force a full-window relayout.
3. **Aggressive virtualization**: large collections are always virtualized (lists/tables).
4. **Caching tuned to real interactions**: caches hit during drags/scrolls; keys are chosen to avoid thrash.
5. **Observability**: Zed ships a profiler UI and uses tracing to find/kill latency spikes.

gitgpui already has pieces of this (virtualized lists, caches, background work) but still tends to “pay globally” on updates.

---

## What to improve in gitgpui (actionable, gpui-specific)

### 1) Reduce invalidation scope (highest ROI)

Goal: when only one pane’s data changes, only that pane rerenders.

Practical approaches:

- **Stop unconditionally notifying the root on every `AppUiModel` update.**
  - Root should notify only when root-owned state changes (toasts, tooltip overlay, window chrome state).
- **Gate each pane’s notify on a cheap fingerprint** of the data it renders:
  - Pointer equality for large `Arc<T>` fields (`Arc::ptr_eq`),
  - per-repo “revision counters” in state (already used in some caches),
  - or a small `u64` fingerprint stored alongside the pane state.

This one change tends to make “everything” feel faster because it reduces baseline frame work.

### 2) Make render paths boring

Audit render paths and `uniform_list` processors for:

- `format!` / string building
- hashing large strings
- building large `Vec<AnyElement>` outside the visible range
- Tree-sitter parsing / diff splitting / expensive scans

Move these to:

- “on state change” caches (as already done in some places), or
- background tasks that populate caches.

### 3) Treat layout as a budgeted resource

Callgrind indicates layout is a meaningful cost; reducing layout work pays off broadly:

- Flatten deeply nested flex containers where possible.
- Prefer canvas-style rows for dense, repeated content (history already does this).
- Ensure stable element IDs in lists so gpui can reuse where possible.

### 4) Avoid background contention storms

Even if work is “background”, it competes for CPU/cache/disk.

- Keep refresh fan-out coalesced and prioritized (see `optimizations.md`).
- Consider lowering worker thread counts while interactive (or using priorities).

### 5) Close the “shipping gap”

When comparing to Zed, compare like-for-like:

- Always evaluate UX in `--release` or `--profile release-with-debug`.
- Consider enabling `mimalloc` (Zed supports it; gitgpui currently doesn’t).
- Consider tracking newer gpui changes if you’re on an older release.

---

## Suggested next measurements (to make this non-speculative)

1. **Frame-time tracking**
   - Add a simple “frame time p95” overlay (Zed-style) or log render durations.
2. **Notification counters**
   - Count how often root/panes call `cx.notify()` per second during idle and during a repo refresh.
3. **Layout cost sampling**
   - Track how often layout runs and how long it takes during scroll/hover.
4. **Diff scroll profiling**
   - Specifically measure time spent in diff row processing + syntax tokenization.

The goal is to turn “feels laggy” into “p95 frame time spikes when X happens”, then fix X.

---

## Related notes in this repo

- Broader optimization backlog: `optimizations.md`
- Resize jank deep-dive: `resize_analyse.md`
- Performance guidelines: `docs/PERFORMANCE.md`

---

## Entity-splitting plan (tracked)

Goal: make invalidation **feature-local** so a single hover, store update, or background load does **not** force a full-window re-render/relayout.

### Inventory: extraction candidates

These are the highest-value “Entity boundaries” in this repo (some already exist, but still leak state or rely on root notifications):

- **Window shell (root)**
  - Window resize-edge hit testing, pane-resize drag state, UI-settings persistence, window frame.
- **Global overlays**
  - Tooltip system (delay + anchor tracking)
  - Toast system
  - Popover host + scrim + context menus
- **Top chrome**
  - Title bar (app menu, window controls)
  - Repo tabs bar (hover close state, repo switching)
  - Action bar (repo/branch pickers, pull/push, etc)
  - Open-repo panel + clone prompt triggers
  - Error banner
- **Panes (already separate entities)**
  - Sidebar pane (branch tree / remotes)
  - Main pane (history vs diff vs preview)
  - Details pane (status lists, commit box, commit details)
- **Within Main/Details panes**
  - Diff panel, History panel, Conflict resolver panel, File preview panel
  - Commit details view, Status lists view

### Implementation steps (do one at a time)

- [x] **Step 1: Extract `TooltipHost` entity**
  - Move tooltip state/timers/anchored rendering out of `GitGpuiView` so hover tooltips don’t re-render the whole window.
  - Rewire `set_tooltip_text_if_changed` / `clear_tooltip_if_matches` call sites to update `TooltipHost` and avoid `cx.notify()` on the caller view.
- [x] **Step 2: Extract `RepoTabsBarView` entity**
  - Move `hovered_repo_tab` and tab hover/close logic out of root so hovering tabs doesn’t re-render the full window.
- [x] **Step 3: Extract `ActionBarView` entity**
  - Localize action-bar rendering (notify gating comes later in Step 8).
- [x] **Step 4: Extract `TitleBarView` entity**
  - Localize window-control rendering (and app-menu trigger) so window activation changes don’t force a full re-render.
- [x] **Step 5: Extract `ToastHost` entity**
  - Move toast state + timers out of root (toasts should never imply a full relayout).
- [x] **Step 6: Popover host decoupling**
  - Render popovers from a dedicated `PopoverHost` entity (root just composes it).
- [x] **Step 7: Remove root’s unconditional `AppUiModel` subscription notify**
  - Root is a mostly-static compositor; it only notifies on root-owned visual changes (e.g. error banner).
- [x] **Step 8: Add per-entity notify gating**
  - Each always-visible entity only notifies when its own render-relevant fingerprint changes.
