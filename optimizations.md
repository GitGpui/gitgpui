# Optimization opportunities (gitgpui ⇄ Zed-inspired)

This document is a prioritized list of optimization opportunities aimed at making gitgpui feel “instant” (low input latency, no UI stalls) even on huge repositories, large diffs, and long histories.

The theme throughout: **avoid work on the UI thread**, **virtualize big collections**, **coalesce updates**, and **never clone or re-render more than needed**.

## 1) Biggest wins first (responsiveness killers)

### 1.1 Remove UI-side polling loop + stop cloning the entire app state

- **What’s happening now**
  - The UI waits for store events and, on change, pulls a **full snapshot** via `store.snapshot()` and applies it. `crates/gitgpui-ui-gpui/src/view/mod.rs`
  - `AppState`/`RepoState` contains potentially huge `Diff`, `RepoStatus`, `LogPage`, `command_log`, etc, and is cloned per snapshot. `crates/gitgpui-state/src/model.rs:14`
- **Why it hurts**
  - `store.snapshot()` clones large structures frequently, which can jam the allocator and create long pauses exactly when a user is scrolling/typing.
- **What to do**
  - Replace polling with an event-driven receiver that can `await` (Zed uses async channels and does not require a 10ms loop for this kind of thing).
  - Stop cloning giant state:
    - Convert heavy fields to `Arc<T>` (e.g. `RepoStatus`, `LogPage`, `Diff`, `CommitDetails`) so snapshots become cheap structural clones.
    - Or switch to “delta” updates: store emits a `StoreEvent::RepoUpdated { repo_id, patch }` (or per-field updated messages) so the UI doesn’t need a full snapshot.
    - Or move the store into a GPUI `Entity` so updates happen through `cx.update()`/`cx.notify()` rather than cloning.
- **Zed inspiration**
  - Zed’s UI lists are virtualized and avoid full rebuilds; it also has an in-app profiler to see stalls. `zed/crates/miniprofiler_ui/src/miniprofiler_ui.rs:1`

- **Status**
  - ✅ Implemented: removed UI-side 10ms polling; UI now awaits store events and drains/coalesces bursts. `crates/gitgpui-ui-gpui/src/view/mod.rs`
  - ✅ Implemented: coalesced store notifications using a bounded async channel (at most one pending `StateChanged`). `crates/gitgpui-state/src/store/mod.rs`
  - ✅ Implemented: structurally shared the heaviest repo fields in snapshots via `Arc` (status/log/file_history/blame/commit_details/diff). `crates/gitgpui-state/src/model.rs`
  - ⏳ Remaining: consider sharing other potentially-large fields (`command_log`, `diagnostics`, large `Vec`s like `branches`) and/or switch to delta events to avoid cloning most of `AppState`.

### 1.2 Coalesce refresh fan-out to avoid “refresh storms”

- **What’s happening now**
  - A refresh tends to schedule many effects at once (branches/remotes/status/log/stashes/reflog/etc.). `crates/gitgpui-state/src/store/reducer.rs:1489` (see `refresh_effects`)
  - Each effect spawns a background task. `crates/gitgpui-state/src/store/effects.rs:15`
- **Why it hurts**
  - On large repos, parallel loads can saturate CPU/disk and cause noticeable UI sluggishness (even if “background”, it still competes for resources).
  - Repeated refresh requests (window focus changes, filesystem changes, user actions) can queue redundant work.
- **What to do**
  - Add **backpressure**: per-repo “refresh in flight” tokens, a single refresh queue, and coalescing (keep only the latest request).
  - Prioritize: status + head + divergence first; history next; branches/tags/remotes last; stashes/reflog lazily or on-demand.
  - Cancel stale tasks (or at least ignore their results) when a newer refresh supersedes them.

- **Status**
  - ✅ Implemented: per-repo coalescing/backpressure for refresh fan-out via `RepoLoadsInFlight` (prevents duplicate loads when focus/external events fire repeatedly). `crates/gitgpui-state/src/model.rs`
  - ✅ Implemented: focus refresh (`SetActiveRepo` for already-active repo) now runs a primary refresh only (head/divergence/rebase/status/log), not the full fan-out. `crates/gitgpui-state/src/store/reducer.rs`
  - ✅ Added tests ensuring refresh coalescing replays exactly once per kind. `crates/gitgpui-state/src/store/tests.rs`

## 2) Rendering & layout (big lists, huge diffs)

### 2.1 Ensure all potentially large lists are virtualized and stable

- **What to confirm**
  - Status lists already use `uniform_list` (good). `crates/gitgpui-ui-gpui/src/view/panels/layout.rs:651`
  - Verify **history**, **branches**, **remote branches**, **commit files**, **blame**, and **diff** lists are also virtualized and not building large `Vec<AnyElement>` per frame.
- **Why it matters**
  - Building element trees for 10k rows guarantees frame drops.
- **Zed inspiration**
  - `uniform_list` is explicitly built for “large lists”. `zed/crates/gpui/src/elements/uniform_list.rs:1`
  - Zed also uses a table abstraction with virtualized lists. `zed/crates/ui/src/components/data_table.rs:720`

### 2.2 Avoid per-frame expensive derived computations; compute on change and cache

- **Hotspots to look for**
  - Large `HashSet` rebuilds during `apply_state_snapshot` (e.g. selection reconciliation). `crates/gitgpui-ui-gpui/src/view/mod.rs:3304`
  - Recomputing diff-derived structures (visible indices, scrollbar markers, split caches) when inputs haven’t changed.
- **What to do**
  - Introduce “fingerprints” (hashes of input identity + versions) and recompute only when they change.
  - Keep caches bounded and LRU/epoch-based (some exist already; ensure they’re used consistently).

- **Status**
  - ✅ Implemented: `StatusMultiSelection` reconciliation no longer builds `HashSet`s on every snapshot; it now runs only when `RepoStatus` changes (using `Arc::ptr_eq`) and uses allocation-free pruning. `crates/gitgpui-ui-gpui/src/view/mod.rs`

### 2.3 Guardrails for worst-case diffs (avoid pathological CPU time)

- **Where things can explode**
  - Word diff algorithms can be expensive on long lines; there’s even a perf stress test for `word_diff_ranges`. `crates/gitgpui-ui-gpui/src/view/mod.rs:5547`
- **What to do**
  - Add hard caps and fallback behaviors:
    - Limit word-diff to a max line length / token count; beyond that, show plain add/remove highlighting.
    - Defer word-diff until a line is visible (never compute word-diff for offscreen rows).
    - Cache word-diff results keyed by `(old_line_hash, new_line_hash, settings)` and reuse across frames.

- **Status**
  - ✅ Implemented: capped word-diff computation for very large lines (fallback to no word highlights) to prevent UI stalls on pathological inputs. `crates/gitgpui-ui-gpui/src/view/mod.rs`

### 2.4 Reduce string churn in render paths

- **What to do**
  - Prefer `SharedString`/`Arc<str>` for repeated labels (paths, branch names, commit ids).
  - Cache expensive `PathBuf -> display string` conversions (some already exist). `crates/gitgpui-ui-gpui/src/view/mod.rs:568` (path display cache)
  - Avoid `.to_string()` inside tight render loops; precompute in view-model updates.

## 3) State & data flow (make updates cheap)

### 3.1 Replace “clone-everything snapshots” with persistent/structural sharing

- **Why**
  - `RepoState` contains large `Vec`s and `String`s (`RepoStatus`, `Diff`, logs, blame). `crates/gitgpui-state/src/model.rs:47`
- **Options**
  - Wrap heavy fields in `Arc<…>` and update by swapping arcs (cheap clones; fast equality checks by pointer).
  - Split state into per-repo entities in the UI layer so only affected subtrees re-render.
  - Emit deltas instead of snapshots (store event carries “what changed”).

### 3.2 Reduce update frequency / coalesce UI notifications

- **What to do**
  - If multiple store messages arrive quickly, do one UI update per frame (or per ~16ms budget), not per message.
  - Ensure `cx.notify()` is called once for a batch, not repeatedly in loops.

## 4) Git backend + data scaling

### 4.1 Use incremental/paginated data everywhere

- **History**
  - Already paginated in the model (`LogCursor`/`LogPage`)—ensure UI never tries to render “all commits” at once.
- **Status**
  - For huge repos, `status()` should allow:
    - A fast summary mode (counts) first.
    - A capped list + “Load more” or filtering.
    - Optional pathspec scoping (only status for visible paths / selected folder).

### 4.2 Avoid spawning `git` processes in hot paths; prefer gix where possible

- **Why**
  - Process spawning and parsing text outputs is slower and can become jittery under load.
- **What to do**
  - Expand the `gix` backend coverage so common operations (status/log/branches/diff) do not call `git` CLI.
  - Where `git` CLI is still used, prefer stable machine formats and streaming parsing (`-z` formats) to reduce allocations.

### 4.3 Cache and reuse expensive results with explicit invalidation

- Candidates:
  - Commit details for recently viewed commits.
  - File history and blame for recently viewed paths.
  - Diff parse/annotation structures for the active selection.
- Invalidation signals:
  - External git state changes (HEAD/index/refs), or explicit operations (commit, checkout, rebase, etc).

## 5) Watching & refresh (event driven, but safe)

### 5.1 Use a single global watcher (multiplexed) instead of per-repo watchers

- **Current state**
  - gitgpui watches only the active repo, which is good for limiting load, but it still spins up per-repo watcher threads.
- **Opportunity**
  - Adopt Zed’s pattern: one global watcher instance with registrations and callbacks, plus platform-specific recursion strategies and filtering. `zed/crates/fs/src/fs_watcher.rs:1`
  - This reduces OS watcher overhead and centralizes tricky platform behavior.

### 5.2 Filter noisy events and avoid refresh loops

- Zed filters `EventKind::Access(_)` events on Linux due to real-world notify issues. `zed/crates/fs/src/fs_watcher.rs:229`
- Recommended improvements:
  - Filter access events where relevant.
  - Debounce + cap refresh frequency (already done in gitgpui’s debouncer; ensure it remains bounded).
  - Avoid triggering a refresh from the refresh itself (e.g., reading `.git/index` shouldn’t cause new refreshes).

## 6) Instrumentation (so performance stays good)

### 6.1 Add an in-app performance overlay

- Zed ships a profiler window that surfaces per-task timings. `zed/crates/miniprofiler_ui/src/miniprofiler_ui.rs:1`
- Add a lightweight version to gitgpui to make it obvious when:
  - A render pass is taking too long
  - A store snapshot clone spikes
  - Diff parsing dominates CPU

### 6.2 Add regression tests and benchmarks for “huge data”

- Add benchmark-style tests (ignored by default) for:
  - Rendering ~10k status entries with virtualization
  - Applying snapshots with large `RepoStatus`/`Diff`
  - Parsing large diffs and word-diff bounds
- Track key metrics over time (CI optional):
  - “time to first interactive” after opening a large repo
  - frame-time p95 while scrolling diff/history

## 7) Concrete next steps (low risk → high reward)

1. Replace `Poller`’s 10ms loop with a truly event-driven receiver (no periodic wakeups). `crates/gitgpui-ui-gpui/src/view/mod.rs:5118`
2. Stop cloning giant `AppState` snapshots; introduce structural sharing (`Arc` fields) or deltas. `crates/gitgpui-state/src/model.rs:47`
3. Add per-repo refresh coalescing + cancellation/backpressure so external changes don’t trigger refresh storms.
4. Cap pathological work (word diff, giant diffs) and compute lazily only for visible rows.
5. Add a small profiler overlay (Zed-style) to keep performance work measurable and prevent regressions.
