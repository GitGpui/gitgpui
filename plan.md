# GitComet Performance Action Plan

## Summary

This plan turns the static performance findings into implementation work that can be picked up without prior context. The main risk areas are status-list rendering, selected-diff work scheduling, synchronous session I/O, followed-file history pagination, git subprocess overhead, image diff memory use, and app-state snapshot churn.

Recommended order:

1. Fix status row indexing and render-path submodule/filesystem work.
2. Coalesce stale selected-diff loads.
3. Move remaining synchronous session I/O off reducer/UI paths.
4. Cache or stream `git log --follow` pagination.
5. Reduce subprocess buffering/thread overhead and eager image loading.
6. Validate state clone-on-write behavior with the existing diagnostics.

The codebase already has useful performance infrastructure:

- `scripts/run-full-perf-suite.sh` runs the local performance suite.
- `crates/gitcomet-ui-gpui/benches/performance.rs` contains Criterion UI benchmarks.
- `crates/gitcomet-state/src/benchmarks.rs` exposes store benchmark helpers behind the `benchmarks` feature.
- `crates/gitcomet-state/src/store/mod.rs` records clone-on-write diagnostics when `Arc<AppState>` is shared during mutation.

## Progress

- Completed initial pass: Section 1 now materializes filtered status-section indexes inside `StatusSectionEntries`, making `len()` and `get()` direct lookups for filtered sections. A cross-render projection cache can still be added later if profiling shows construction cost is material.
- Completed: Section 2 no longer performs filesystem metadata checks or per-row linear submodule scans in status row rendering; rows use one submodule status lookup built per render.
- Verified: `cargo test -p gitcomet-ui-gpui status_section_entries_index_filtered_sections_directly` and `cargo check -p gitcomet-ui-gpui`.
- Completed initial pass: Section 3 now tracks `diff_target_rev` and selected-diff scheduled tasks preflight that revision before starting backend work and before publishing results.
- Verified: `cargo test -p gitcomet-state set_diff_target_bumps_target_rev_only_on_change`, `cargo test -p gitcomet-state diff_selection`, `cargo test -p gitcomet-state rev_counter`, `cargo check -p gitcomet-state`, and `cargo check -p gitcomet-ui-gpui`.
- Completed initial pass: Section 4 now routes recent-repo and repo-history-mode persistence through session executor effects instead of synchronous reducer writes. Session effect scheduling resolves the session file path before handing work to the background executor, preserving test overrides and avoiding worker-thread path ambiguity.
- Verified: `cargo test -p gitcomet-state session_update_effects_persist_on_session_executor`, `cargo test -p gitcomet-state repo_management`, `cargo test -p gitcomet-state external_and_history`, and `cargo check -p gitcomet-ui-gpui`.
- Completed initial pass: Section 5 now keeps first-page `git log --follow` requests bounded, then caches the full follow history for cursor pages by path and HEAD. Page 2 performs the unavoidable full scan, and page 3+ reuse the cached vector while the key remains current.
- Verified: `cargo test -p gitcomet-git-gix cursor_file_history_pages_reuse_cached_follow_history`, `cargo test -p gitcomet-git-gix log::tests`, and `cargo check -p gitcomet-git-gix`.
- Completed initial pass: Section 6 converts the large `git log --follow` stdout path from full string capture to incremental parsing through `run_git_parsed_stdout`. This still uses the existing parser-thread helper, but it removes the extra full-output `Vec<u8>` and `String` copies for followed-file history.
- Verified: `cargo test -p gitcomet-git-gix parse_git_log_pretty_records_from_reader_matches_string_parser`, `cargo test -p gitcomet-git-gix log::tests`, and `cargo check -p gitcomet-git-gix`.
- Completed initial pass: Section 7 adds a 64 MiB per-side image diff payload limit. Worktree image sides are checked with metadata before `read`, and object-database image sides are checked through gix object headers before blob materialization. A deeper lazy file/source representation would require changing the shared `FileDiffImage` domain type and UI cache contract.
- Verified: `cargo test -p gitcomet-git-gix read_worktree_image_file_bytes`, `cargo test -p gitcomet-git-gix --test status_integration diff_file_image`, and `cargo check -p gitcomet-git-gix`.
- Completed validation pass: Section 8 already has clone-on-write diagnostics and focused coverage for held-snapshot COW behavior. No app-state shape refactor was made because the existing plan requires benchmark or trace evidence before changing the shared state contract.
- Verified: `cargo test -p gitcomet-state reducer_diagnostics_track_dispatches_and_clone_on_write` and `cargo check -p gitcomet-state`.
- Final focused sweep passed: `cargo check -p gitcomet-git-gix`, `cargo check -p gitcomet-state`, `cargo check -p gitcomet-ui-gpui`, `cargo test -p gitcomet-ui-gpui status::tests`, `cargo test -p gitcomet-state diff_selection`, `cargo test -p gitcomet-state repo_management`, `cargo test -p gitcomet-state external_and_history`, `cargo test -p gitcomet-git-gix log::tests`, and `git diff --check`.
- Blocked outside this change: `cargo check -p gitcomet-ui-gpui --features benchmarks` currently fails in existing benchmark-only code paths unrelated to this status-rendering pass.
- Pending: broad performance benchmark run when local benchmark prerequisites are fixed.

## 1. Make Status Section Row Access O(1)

### Problem

Status rendering repeatedly scans filtered status entries.

Relevant code:

- `crates/gitcomet-ui-gpui/src/view/mod_helpers.rs`
  - `StatusSectionEntries::len()` calls `self.iter().count()`.
  - `StatusSectionEntries::get(index)` calls `self.iter().nth(index)`.
  - `StatusSectionIter::next()` filters entries with `find(...)`.
- `crates/gitcomet-ui-gpui/src/view/rows/status.rs`
  - `render_status_rows_for_section` calls `entries.len()` for the visible signature.
  - The same function calls `entries.get(ix)` for every visible row.

For filtered sections like `Untracked` and `Unstaged`, each visible-row lookup starts a new filtered iterator from the beginning. Rendering 40 visible rows near index 5,000 can scan about 200,000 entries plus an extra full scan for `len()`.

### Action Items

- Add a status-section projection/cache keyed by repo id, section, and the status revision/source that changes when status entries change.
- Store:
  - filtered row count
  - `Vec<usize>` mapping section row index to the original `FileStatus` index
- Change status row rendering to use direct indexed lookup instead of `iter().nth(index)`.
- Keep section behavior identical for combined unstaged, untracked, unstaged, and staged sections.
- Invalidate the cache whenever the underlying worktree or staged status entries change.

### Acceptance Criteria

- Deep scrolling in a large filtered status section is proportional to visible row count, not total status count times visible row count.
- Existing status row ordering and filtering behavior stays unchanged.
- Add or update a benchmark/test that renders a large filtered status section at a deep visible range.

## 2. Remove Filesystem and Linear Submodule Work From Status Row Rendering

### Problem

`render_status_rows_for_section` does per-row work that should be cached:

- It calls `repo.spec.workdir.join(&entry.path).is_dir()` during row rendering.
- It scans submodules with `submodules.iter().find(...)` for every visible submodule candidate.

Relevant code:

- `crates/gitcomet-ui-gpui/src/view/rows/status.rs`

This puts filesystem latency and submodule count directly in the row paint path.

### Action Items

- Build a per-repo submodule lookup when submodule data is ready:
  - `HashMap<PathBuf, SubmoduleStatus>`, or an equivalent path-keyed structure
  - keyed/invalidation-bound to the current repo and submodule data revision
- Replace `submodules.iter().find(...)` in row rendering with a direct lookup.
- Derive `is_submodule` from submodule/status data rather than calling `is_dir()` during rendering.
- If a directory check is still required for correctness, perform and cache it outside the render path, invalidated by status/submodule refresh.

### Acceptance Criteria

- Status row rendering performs no filesystem metadata calls.
- Submodule status lookup is effectively O(1) per visible row.
- Submodule display remains correct for staged, unstaged, and untracked status entries.

## 3. Coalesce and Cancel Stale Selected-Diff Work

### Problem

Selecting a diff can schedule several backend tasks:

- patch diff
- file text
- preview text file
- submodule summary
- image diff

Relevant code:

- `crates/gitcomet-state/src/store/reducer/diff_selection.rs`
  - `select_diff` updates `diff_target` and emits `Effect::LoadSelectedDiff`.
  - result reducers drop stale results if the selected target no longer matches.
- `crates/gitcomet-state/src/store/effects.rs`
  - `Effect::LoadSelectedDiff` resolves the current target and schedules work.
- `crates/gitcomet-state/src/store/effects/repo_load.rs`
  - schedules individual diff-related backend calls.
- `crates/gitcomet-state/src/store/executor.rs`
  - uses a fixed-size FIFO executor backed by an unbounded `mpsc::channel`.

The stale-result checks protect correctness, but stale expensive tasks still run and occupy worker threads. Fast keyboard navigation through files can enqueue git diff/blob/image work that is no longer useful.

### Action Items

- Add a selected-diff generation token per repo.
- Increment the token whenever `diff_target` changes.
- Capture the token when scheduling selected-diff work.
- Before starting expensive backend work, verify that the captured token still matches the repo's current selected-diff token.
- For long-running work, check the token again before sending the loaded result.
- Coalesce selected-diff work so only the latest target per repo is allowed to start expensive operations.
- Keep existing stale-target checks in reducers as the final correctness guard.

### Acceptance Criteria

- Rapid selection movement across many files does not run full backend loads for every intermediate file.
- The latest selected diff still loads patch/text/preview/image/submodule data according to the existing load plan.
- Stale work exits before expensive git/blob/image operations whenever possible.
- Existing stale-result reducer behavior remains intact.

## 4. Move Synchronous Session I/O Off Reducer and UI Paths

### Problem

Some session persistence is already asynchronous through `Effect::PersistSession`, but several reducer/UI paths still read or write session JSON synchronously.

Relevant code:

- `crates/gitcomet-state/src/store/effects.rs`
  - `Effect::PersistSession` uses `session_persist_executor`.
- `crates/gitcomet-state/src/store/reducer/repo_management.rs`
  - `open_repo` calls `session::persist_recent_repo`.
  - `open_repo` calls `session::load_repo_session_preferences`.
  - `open_repo` may call `session::persist_repo_history_mode`.
  - `restore_session` calls `session::persist_repo_history_modes_batch`.
- `crates/gitcomet-state/src/store/reducer/external_and_history.rs`
  - `set_history_scope` calls `session::persist_repo_history_mode`.
- `crates/gitcomet-ui-gpui/src/view/state_apply.rs`
  - macOS state application calls `session::load()` to refresh recent repos.
- `crates/gitcomet-state/src/session.rs`
  - `load_file` reads/parses JSON.
  - `persist_to_path` serializes/writes JSON through a temp file.

Reducers should stay fast and deterministic, and UI state application should not perform disk I/O.

### Action Items

- Load session preferences once during startup/session restore and carry the needed preferences in app state or initialization context.
- Route recent-repo and history-mode writes through the existing session persistence executor.
- Add narrow async effects for small session updates if full-state persistence is too broad.
- Remove direct `session::load()` from UI state application.
- Drive recent repo menu refresh from app state or an async session-refresh message.
- Preserve current diagnostics for session read/write failures.

### Acceptance Criteria

- Reducers no longer perform direct filesystem session reads/writes.
- UI state application no longer calls `session::load()`.
- Repo open, session restore, recent repos, and history-scope persistence behave the same across restart.
- Failed session persistence still surfaces diagnostics.

## 5. Improve `git log --follow` File History Pagination

### Problem

Followed-file history pagination rescans too much history.

Relevant code:

- `crates/gitcomet-git-gix/src/repo/log.rs`
  - `log_follow_commits` shells out to `git log --follow` and captures full output.
  - `log_file_page_impl` bounds only the first page.
  - cursor pages scan the full follow history because `git log --follow` does not combine reliably with `--skip` across renames.

For files with long rename history, each "load more" repeats the full command, captures all output, parses all commits, and then paginates in memory.

### Action Items

- Add a follow-history cache keyed by:
  - repo/workdir identity
  - path
  - current head or relevant history source revision
  - history scope/mode if it affects the result
- Reuse cached follow results when loading additional pages for the same file.
- Invalidate the cache when the relevant head/history source changes.
- If memory is a concern, cache enough cursor metadata to resume pagination rather than storing unbounded commit vectors.
- Prefer streaming parse for `git log --follow` output when practical, stopping once the requested cursor plus page has been found.
- Do not replace this with unreliable `--skip` behavior across renames.

### Acceptance Criteria

- Loading page 2+ for the same followed file does not rerun a full-history scan when head/path/scope are unchanged.
- Rename-follow behavior remains correct.
- Cache memory is bounded or clearly invalidated.

## 6. Reduce Git Subprocess Thread and Buffering Overhead

### Problem

The git command helpers often spawn extra OS threads and fully buffer command output.

Relevant code:

- `crates/gitcomet-git-gix/src/util.rs`
  - `run_command_with_timeout` spawns stdout and stderr reader threads.
  - `run_git_parsed_stdout` spawns a parser thread and stderr reader thread.
  - `run_git_capture` and `run_git_capture_bytes` capture all stdout before parsing.

This is acceptable for rare commands, but it adds overhead during bursts of short commands and can be expensive for large outputs.

### Action Items

- Audit hot callers of `run_git_capture` and `run_git_capture_bytes`.
- Convert large-output paths to streaming parse APIs where feasible.
- For frequent short commands, evaluate simpler `wait_with_output` handling where pipe deadlock risk is controlled.
- Avoid adding new full-output capture paths for diff/log/status-like data.
- Preserve timeout, stderr diagnostics, and auth-prompt behavior.

### Acceptance Criteria

- Large-output git commands stream or stop early where possible.
- Frequent short commands avoid unnecessary helper-thread overhead where safe.
- Existing timeout and auth behavior remains correct.

## 7. Make Image Diff Loading Lazy or Size-Limited

### Problem

Image diffs eagerly load old and new sides into memory.

Relevant code:

- `crates/gitcomet-git-gix/src/repo/diff.rs`
  - `diff_file_image_impl` loads old/new blobs or worktree files into `Vec<u8>`.
  - `read_worktree_file_bytes_optional` reads the full worktree file.

Large binary assets can consume significant memory and worker time even if the UI later downscales them or does not need both sides immediately.

### Action Items

- Add a maximum eager image byte threshold.
- For large images, return metadata plus a lazy source reference instead of full bytes:
  - worktree file path
  - cached blob file path
  - blob id plus repo reference, if the image cache can resolve it safely
- Reuse the preview blob cache approach where appropriate.
- Let the UI image cache own decoding and downscaling decisions.
- Surface a clear "too large to preview" state if an image exceeds supported limits.

### Acceptance Criteria

- Selecting a large image diff does not copy both sides into app state by default.
- Normal small image diffs still render without visible regression.
- Memory use remains bounded for large binary files.

## 8. Validate App-State Clone-On-Write and Snapshot Churn

### Problem

The store keeps `Arc<AppState>` behind an `RwLock`, and UI views hold snapshots. Mutating state while snapshots are still alive triggers `Arc::make_mut`.

Relevant code:

- `crates/gitcomet-state/src/store/mod.rs`
  - `make_mut_state_with_diagnostics` records clone-on-write diagnostics.
- `crates/gitcomet-ui-gpui/src/view/poller.rs`
  - UI state snapshots are fetched after state-change events.
- `crates/gitcomet-ui-gpui/src/view/state_apply.rs`
  - the view stores the latest snapshot.

Large payloads are often behind `Arc`, so this may not copy every byte, but repo vectors and state structures can still clone during high-frequency updates.

### Action Items

- Review clone-on-write diagnostics during performance benchmarks and real interaction traces.
- Minimize UI snapshot lifetimes where practical.
- Avoid emitting state-changed notifications for no-op or redundant updates.
- Keep large stable payloads behind `Arc` or equivalent shared structures.
- Do not refactor app-state shape until diagnostics show meaningful clone cost.

### Acceptance Criteria

- Clone-on-write diagnostics are checked before and after other performance changes.
- No-op state updates do not create unnecessary UI snapshots.
- Any state-shape refactor is justified by measured clone cost.

## Verification Plan

Run focused checks after each change, then run the broader performance suite before considering the work complete.

Recommended focused checks:

- Status rendering:
  - benchmark deep visible ranges in large filtered status sections
  - verify status selection, multi-selection, and section filtering tests
- Selected diff coalescing:
  - test rapid selection changes
  - assert stale generations do not publish results
  - verify final selected diff still loads all required data
- Session I/O:
  - test repo open, restore session, recent repos, history scope persistence, and persistence failure diagnostics
- File history:
  - test followed file pagination across renames
  - test cache invalidation when head changes
- Image diffs:
  - test small image preview
  - test large image threshold/lazy path
- Subprocess changes:
  - test timeout handling, stderr reporting, auth prompt paths, and large-output parsing

Recommended broad checks:

- `cargo test --workspace`
- `cargo check --workspace`
- `cargo bench -p gitcomet-ui-gpui --features benchmarks --bench performance -- --noplot`
- `scripts/run-full-perf-suite.sh` when a full local performance run is appropriate

## Implementation Notes

- Keep changes incremental. Each numbered section can be implemented independently.
- Preserve existing lazy/paged diff rendering behavior.
- Prefer extending existing benchmark hooks over creating unrelated performance tooling.
- Do not remove stale-result checks when adding cancellation/coalescing; cancellation is an optimization, reducer checks are still correctness guards.
- Avoid broad app-state refactors until measured diagnostics show they are needed.
