# Architecture split opportunities (maintenance + render performance)

This is a codebase-focused review of where gitgpui can be split into smaller Rust modules (and/or gpui views/components) to improve maintainability, boundaries, and render performance. It also lists concrete places where render-path computations/allocation can be moved out of `render()`/row processors.

## Progress

- [x] Branch sidebar rows cache + state rev counters (`crates/gitgpui-state/src/model.rs`, `crates/gitgpui-state/src/store/reducer.rs`, `crates/gitgpui-ui-gpui/src/view/mod.rs`, `crates/gitgpui-ui-gpui/src/view/panels/layout.rs`, `crates/gitgpui-ui-gpui/src/view/rows/sidebar.rs`)
- [x] History derived cache: worktree counts + stash-id set (`crates/gitgpui-ui-gpui/src/view/mod.rs`, `crates/gitgpui-ui-gpui/src/view/panels/main.rs`, `crates/gitgpui-ui-gpui/src/view/rows/history.rs`)
- [x] History row view-model cache (preformatted dates/refs) built off render path (`crates/gitgpui-ui-gpui/src/view/mod.rs`, `crates/gitgpui-ui-gpui/src/view/rows/history.rs`, `crates/gitgpui-ui-gpui/src/view/rows/history_canvas.rs`)
- [x] Status selection membership (avoid per-processor `HashSet` alloc) (`crates/gitgpui-ui-gpui/src/view/rows/status.rs`)
- [x] Split `view/mod.rs` into focused modules (no behavior change) (`crates/gitgpui-ui-gpui/src/view/mod.rs`, `crates/gitgpui-ui-gpui/src/view/caches.rs`, `crates/gitgpui-ui-gpui/src/view/state_apply.rs`)
- [x] Split popovers/context menus into per-kind modules (`crates/gitgpui-ui-gpui/src/view/panels/popover/*.rs`, `crates/gitgpui-ui-gpui/src/view/panels/popover/context_menu/*.rs`)
- [x] Extract pane shells into separate gpui view entities (`crates/gitgpui-ui-gpui/src/view/panes/*`)
- [x] Introduce an `AppUiModel` entity (observer-based updates) (`crates/gitgpui-ui-gpui/src/view/app_model.rs`, `crates/gitgpui-ui-gpui/src/view/mod.rs`)
- [x] Move sidebar pane render logic/state out of the root view (remove `WeakEntity<GitGpuiView>` delegation in `crates/gitgpui-ui-gpui/src/view/panes/sidebar.rs`, `crates/gitgpui-ui-gpui/src/view/rows/sidebar.rs`, `crates/gitgpui-ui-gpui/src/view/mod.rs`)
- [ ] Move main pane render logic/state out of the root view (remove `WeakEntity<GitGpuiView>` delegation in `crates/gitgpui-ui-gpui/src/view/panes/main.rs`)
- [ ] Move details pane render logic/state out of the root view (remove `WeakEntity<GitGpuiView>` delegation in `crates/gitgpui-ui-gpui/src/view/panes/details.rs`)

## High-level theme

- **Make “view code” small**: keep `Render` impls and view composition readable by delegating to small modules/components.
- **Keep derived data out of render paths**: compute expensive/alloc-heavy derived structures on state change (or in background tasks), not while rendering rows.
- **Avoid cascading renders** in gpui by splitting the UI into multiple view `Entity`s and notifying only the subtrees that actually changed (gpui does not have a built-in `should_component_update`, so you emulate it with structure + selective `cx.notify()`).

## Biggest “module split” targets (by size / complexity)

### `crates/gitgpui-ui-gpui` (largest payoff)

- `crates/gitgpui-ui-gpui/src/view/mod.rs` (~5440 LOC)
  - Currently mixes: root `GitGpuiView` state, cache structures, diff/file-diff caching, tooltip/toast systems, popover state + `PopoverKind` enum, and non-trivial algorithms (word-diff, patch splitting helpers, etc).
  - **Opportunity**: turn this into a small “root view” + a set of focused submodules (see “Module layout”).

- `crates/gitgpui-ui-gpui/src/view/panels/popover.rs` (~1640 LOC) + `crates/gitgpui-ui-gpui/src/view/panels/popover/context_menu.rs` (~470 LOC)
  - Most per-kind views/actions are extracted under `crates/gitgpui-ui-gpui/src/view/panels/popover/` and `crates/gitgpui-ui-gpui/src/view/panels/popover/context_menu/`, but the hosts still centralize wiring/dispatch.
  - **Opportunity**: finish shrinking the hosts by moving “shared helpers” into a dedicated `popover_host` module and keeping the `match` as a thin router.

- `crates/gitgpui-ui-gpui/src/view/panels/main.rs` (~1700 LOC)
  - Contains history, diff, and file-preview main views plus substantial UI-building logic.
  - **Opportunity**: split panels into `history.rs`, `diff.rs`, maybe `file_preview.rs`, and keep `main.rs` only as glue.

- `crates/gitgpui-ui-gpui/src/view/rows/*` hot-path row renderers:
  - `crates/gitgpui-ui-gpui/src/view/rows/diff.rs` (~1250 LOC)
  - `crates/gitgpui-ui-gpui/src/view/rows/diff_text/syntax.rs` (~1390 LOC)
  - `crates/gitgpui-ui-gpui/src/view/rows/history.rs` (~500+ LOC within a bigger file)
  - `crates/gitgpui-ui-gpui/src/view/rows/status.rs` (~500+ LOC within a bigger file)
  - **Opportunity**: split per mode (inline vs split diff, file diff vs patch diff), and isolate syntax tokenization/highlighting utilities.

- `crates/gitgpui-ui-gpui/src/kit/text_input.rs` (~1250 LOC)
  - A full input widget: model, editing ops, selection, layout/shaping, and render/paint all in one file.
  - **Opportunity**: split into a `text_input/` module tree and make `Render for TextInput` mostly composition.

### `crates/gitgpui-git-gix` (architecture clarity)

- `crates/gitgpui-git-gix/src/lib.rs` (~2300 LOC) is a “one file backend”.
  - **Opportunity**: split by GitRepository responsibilities (log/status/diff/branches/remotes/stash/reflog/worktrees/submodules, plus shared utils/error mapping).

### `crates/gitgpui-state` (testability + reducibility)

- `crates/gitgpui-state/src/store/reducer.rs` (~1850 LOC) + `crates/gitgpui-state/src/msg.rs` (~1320 LOC)
  - The reducer is large enough that “find the handler” becomes work.
  - **Opportunity**: split reducer into submodules per message domain and keep a thin dispatch `match` at the top.

## Module layout (current → target)

### 1) Split the root view into “root”, “panels/panes”, and “derived caches”

Status: `app_model.rs`, `caches.rs`, `state_apply.rs`, `panes/*`, and `panels/*` already exist, but `view/mod.rs` is still the main root view module.

Current (roughly):

```
crates/gitgpui-ui-gpui/src/view/
  mod.rs                 // GitGpuiView + shared state (still large)
  app_model.rs           // AppUiModel entity
  state_apply.rs         // apply_state_snapshot + reconciliation
  caches.rs              // derived caches (history/diff/sidebar)
  panes/
    mod.rs
    sidebar.rs
    main.rs              // chooses history/diff/file-preview view
    details.rs
  panels/                // layout/main/bars/popover hosts
  rows/                  // hot-path uniform_list row renderers
```

Target (next refactors):

```
crates/gitgpui-ui-gpui/src/view/
  mod.rs                 // reexports + small shared types
  root.rs                // GitGpuiView struct + impl Render
  app_model.rs
  state_apply.rs
  overlays/              // tooltip/toasts/popover layer helpers
    mod.rs
    tooltip.rs
    toasts.rs
  panes/                 // real render/state lives here (not delegated)
    mod.rs
    sidebar.rs
    main.rs
    details.rs
  caches/                // split caches.rs into focused modules
    mod.rs
    history.rs
    diff.rs
    branch_sidebar.rs
  interactions/          // input/resize/scroll behaviors
    mod.rs
    pane_resize.rs
    history_column_resize.rs
```

This keeps “what is rendered” separate from “how we derive data” and “how we react to input”.

### 2) Turn popovers/context menus into a dedicated subsystem

Status: most per-kind popovers and context menus already live under `crates/gitgpui-ui-gpui/src/view/panels/popover/` and `crates/gitgpui-ui-gpui/src/view/panels/popover/context_menu/`, but core types still leak into other modules.

Next steps:

- Move `PopoverKind` out of `crates/gitgpui-ui-gpui/src/view/mod.rs` into `crates/gitgpui-ui-gpui/src/view/panels/popover/` (it is a UI concept).
- Move `ContextMenuAction/Item/Model` out of `crates/gitgpui-ui-gpui/src/view/panels/mod.rs` into the context-menu subsystem.

Suggested structure:

```
crates/gitgpui-ui-gpui/src/view/panels/popover/
  mod.rs                 // module exports + shared helpers
  kind.rs                // PopoverKind
  host.rs                // positioning/anchor, close behavior
  repo_picker.rs
  branch_picker.rs
  settings.rs
  …
  context_menu/
    mod.rs
    model.rs             // ContextMenuAction/Item/Model
    pull.rs
    push.rs
    commit.rs
    status_file.rs
    branch.rs
    tag.rs
```

Each file should expose a single `fn view(&mut GitGpuiView, ...) -> impl IntoElement` (or a small `RenderOnce` component), so the popover host only needs a short `match` that delegates.

### 3) Componentize repeated UI patterns using `RenderOnce`

Zed leans heavily on `RenderOnce` components to keep view code modular and readable (see `zed/crates/picker/src/popover_menu.rs` and `zed/crates/ui/src/components/*`).

gitgpui already has `zed_port/*` components; the next step is to standardize:

- Prefer small “data-only” structs implementing `RenderOnce` for reusable UI snippets (headers, empty states, icon+label rows, etc).
- Keep “stateful things” as separate `Entity<View>` types (history panel, diff panel, sidebar), not just methods on `GitGpuiView`.

## “Should component update” in gpui (how to avoid cascading renders)

gpui’s `Render` trait has only `render(&mut self, ...)` — there is no built-in `should_component_update`. The practical equivalent is:

1) **Split the UI into multiple view entities** so updates don’t force a root re-render.
2) **Call `cx.notify()` only when the view’s own relevant state changed** (use pointer equality / fingerprints to cheaply detect changes).
3) **Move derived computations to observers/handlers**, not render.

### A concrete approach for gitgpui

- `Entity<AppUiModel>` exists and is owned by the window (`crates/gitgpui-ui-gpui/src/view/app_model.rs`).
  - Poller updates the model; the root view observes it and applies snapshots (`crates/gitgpui-ui-gpui/src/view/state_apply.rs`).
  - **Next step**: let pane entities observe the model directly so they can notify independently and avoid a root-level “everything changed” notify.

- Pane shells exist as their own entities (`crates/gitgpui-ui-gpui/src/view/panes/*`), but they currently delegate rendering back into `GitGpuiView`.
  - **Next step**: migrate the “real” render/state for each pane into its pane entity (so updates can be localized and `GitGpuiView` shrinks further).

This is the closest analog to React-style “should update”: state change arrives, each subview decides whether to notify based on what changed.

## Render-path hotspots to move out of render (CPU/alloc wins)

These are concrete places where work is done inside render paths (either `render()` or `uniform_list` row processors) that can be moved into caches updated on state change.

### 1) Branch sidebar rows were recomputed per visible-range render

- `crates/gitgpui-ui-gpui/src/view/branch_sidebar.rs` builds `Vec<BranchSidebarRow>` by:
  - constructing slash trees,
  - sorting/deduping remote branches,
  - allocating many `String`s/`SharedString`s.
- Previously, that derived `Vec` was recomputed in both:
  - `crates/gitgpui-ui-gpui/src/view/panels/layout.rs` (for `row_count`),
  - `crates/gitgpui-ui-gpui/src/view/rows/sidebar.rs` (for row rendering).

**Status**

- Cached off render path via `GitGpuiView::branch_sidebar_rows_cached()` (`crates/gitgpui-ui-gpui/src/view/caches.rs`), fingerprinted by per-repo rev counters from `crates/gitgpui-state/src/model.rs`.

### 2) History row processor repeatedly scanned status + allocated sets

Previously, `crates/gitgpui-ui-gpui/src/view/rows/history.rs`:

- computes “working tree counts” by iterating every `FileStatus` (can be thousands) on each processor call,
- builds `stash_ids: HashSet<&str>` each time,
- formats timestamps per-row (`format_datetime_utc`) as rows are built.

**Status**

- Cached off render path via `ensure_history_worktree_summary_cache()` + `ensure_history_stash_ids_cache()` and `HistoryCache` commit row VMs (`crates/gitgpui-ui-gpui/src/view/caches.rs`).

### 3) Status row processor built a `HashSet` on every call

`crates/gitgpui-ui-gpui/src/view/rows/status.rs` builds:

```rust
let selected_set: HashSet<&PathBuf> = this.status_selected_paths_for_area(...).iter().collect();
```

This was done per processor call.

**Status**

- Removed the per-row `HashSet` allocation; selection membership uses a linear scan over selected paths (`crates/gitgpui-ui-gpui/src/view/rows/status.rs`).

### 4) Per-row `format!`/`to_string()` for element IDs and labels

There are several render-hot paths that allocate strings for element IDs (and some labels) via `format!`, e.g. in status rows and sidebar rows.

**Fix direction**

- Prefer tuple IDs (already used elsewhere via `.id(("diff_missing", ix))`), e.g.:
  - `.id(("status_row", repo_id.0, area, ix))` instead of `format!("status_row_...")`.
- Prefer `SharedString`/cached strings for repeatedly shown labels (paths, dates, branch names).

### 5) Side-effects and widget updates inside render

Examples:

- `crates/gitgpui-ui-gpui/src/view/mod.rs` updates `error_banner_input` inside `GitGpuiView::render` when an error exists.
- `crates/gitgpui-ui-gpui/src/view/panels/main.rs` triggers `Msg::LoadMoreHistory` inside `history_view` when scroll is near bottom.

**Fix direction**

- Update widgets (text input content/theme/read-only) when state changes (`apply_state_snapshot`), not every render.
- Prefer scroll-event-driven pagination triggers (or “edge reached” signals) so “load more” isn’t a render-time side effect.

## Suggested refactor sequence (low churn → deeper improvements)

1) Done: **Add caches for known hotspots** (branch sidebar rows, history worktree counts + stash ids, status selection membership).
2) Done: **Split `view/mod.rs` into modules** without changing behavior (pure move/refactor; keep public surface stable).
3) Done: **Split popovers/context menus** into per-kind modules; keep one host that positions/closes.
4) Done: **Extract pane shells into separate view entities** (sidebar/main/details).
5) Next: **Move render/state into pane entities** and have each pane observe `AppUiModel` directly, so notifications are localized.

---

If you want, the next step can be migrating one pane at a time (start with `SidebarPaneView`) to own its state/rendering and observe `AppUiModel` directly, shrinking `GitGpuiView` and localizing `cx.notify()`.
