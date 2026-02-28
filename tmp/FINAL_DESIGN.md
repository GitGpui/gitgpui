## STATUS: COMPLETE

## Implementation Progress

### 1) Unified Conflict Session Model
- ✅ `ConflictPayload` enum (Text, Binary, Absent) with `from_bytes` conversion — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ `ConflictRegion` struct with base/ours/theirs + resolution state — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ `ConflictRegionResolution` enum (Unresolved, PickBase/Ours/Theirs/Both, ManualEdit, AutoResolved) — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ `ConflictSession` struct with path, kind, strategy, payloads, regions, counters, navigation — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ Solved/unsolved counters (`solved_count`, `unsolved_count`, `is_fully_resolved`) — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ Next/previous unresolved navigation with wrap-around — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ 36 unit tests covering all types and behaviors — `crates/gitgpui-core/src/conflict_session.rs`

### 2) Conflict Strategy by Kind
- ✅ `ConflictResolverStrategy` enum (FullTextResolver, TwoWayKeepDelete, DecisionOnly, BinarySidePick) — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ `ConflictResolverStrategy::for_conflict()` maps every `FileConflictKind` + binary flag to strategy — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ Wired strategy dispatch into UI: removed `conflict_requires_resolver` gating, switched activation/search/preview hotpaths to `conflict_resolver_strategy()`, defaulted non-full-text kinds to 2-way resolver mode, and threaded `is_binary` flag through for binary detection — `crates/gitgpui-ui-gpui/src/view/panels/main/diff.rs`, `crates/gitgpui-ui-gpui/src/view/panels/main.rs`, `crates/gitgpui-ui-gpui/src/view/panes/main.rs`
- ✅ Added integration fixture matrix covering all seven `FileConflictKind` values (`DD/AU/UD/UA/DU/AA/UU`) plus sparse stage-shape validation via `conflict_file_stages()` — `crates/gitgpui-git-gix/tests/status_integration.rs`

### 3) Resolver UX Model
- ✅ Existing: A/B/C picks, next/prev conflict navigation, split/inline modes
- ✅ Solved/unsolved counters in domain model (ready for UI binding)
- ✅ Safety gate: detect unresolved markers before "Save & stage" — `text_contains_conflict_markers()` in `conflict_resolver.rs`, `ConflictSaveStageConfirm` popover with cancel/stage-anyway actions, warning indicator in header when markers remain
- ✅ Resolved/total counter display in conflict resolver toolbar — shows "Resolved X/Y" with green color when fully resolved — `crates/gitgpui-ui-gpui/src/view/panels/main.rs`
- ✅ Per-block resolved tracking (`ConflictBlock.resolved` field) — set on A/B/C picks, all-pick, and auto-resolve — `crates/gitgpui-ui-gpui/src/view/conflict_resolver.rs`, `crates/gitgpui-ui-gpui/src/view/panes/main.rs`
- ✅ Next/previous *unresolved* navigation in UI (wrap-around) — added unresolved index helpers + tests and wired toolbar/auto-advance to unresolved-only navigation — `crates/gitgpui-ui-gpui/src/view/conflict_resolver.rs`, `crates/gitgpui-ui-gpui/src/view/panes/main.rs`, `crates/gitgpui-ui-gpui/src/view/panels/main.rs`
- ✅ Hide-resolved toggle — `ThreeWayVisibleItem` enum and `build_three_way_visible_map()` for collapsing resolved conflicts in three-way view, toggle button in toolbar ("Hide resolved" / "Show resolved"), collapsed rows render with green summary banner — `crates/gitgpui-ui-gpui/src/view/conflict_resolver.rs`, `crates/gitgpui-ui-gpui/src/view/mod.rs`, `crates/gitgpui-ui-gpui/src/view/panes/main.rs`, `crates/gitgpui-ui-gpui/src/view/panels/main.rs`, `crates/gitgpui-ui-gpui/src/view/rows/conflict_resolver.rs` — 4 unit tests
- ✅ Bulk actions: "All → A/B/C" exists; auto-resolve safe conflicts wired with Pass 1 + Pass 2 subchunk splitting (see §4)

### 4) Auto-Resolution Engine (Safe-First)
- ✅ Pass 1 safe auto-resolve rules: identical sides, only-ours-changed, only-theirs-changed — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ `AutosolveRule` enum with traceability (rule ID + description) — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ `ConflictSession::auto_resolve_safe()` applies Pass 1 to all unresolved regions — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ `auto_resolve_segments()` applies Pass 1 safe rules directly to UI marker segments — `crates/gitgpui-ui-gpui/src/view/conflict_resolver.rs`
- ✅ "Auto-resolve safe" button in conflict resolver toolbar (shown when unresolved blocks remain) — `crates/gitgpui-ui-gpui/src/view/panels/main.rs`
- ✅ `conflict_resolver_auto_resolve()` method wires button to auto-resolve + text regeneration — `crates/gitgpui-ui-gpui/src/view/panes/main.rs`
- ✅ 10 unit tests for auto-resolve segments and resolved counting — `crates/gitgpui-ui-gpui/src/view/conflict_resolver.rs`
- ✅ Pass 2: heuristic subchunk splitting (meld-inspired) — `split_conflict_into_subchunks()` performs 3-way line-level merge using two strategies: per-line comparison (same line count) and diff-based hunk merge with per-line decomposition of overlapping regions (different line counts); `Subchunk` enum (Resolved/Conflict) in core, `auto_resolve_segments_pass2()` splits UI blocks into finer segments, wired into "Auto-resolve safe" button (Pass 1 → Pass 2 → Pass 1 re-run on sub-blocks); 15 unit tests for core splitting + 4 UI-layer tests — `crates/gitgpui-core/src/conflict_session.rs`, `crates/gitgpui-ui-gpui/src/view/conflict_resolver.rs`, `crates/gitgpui-ui-gpui/src/view/panes/main.rs`
- ✅ Pass 3: regex-assisted opt-in mode implemented with `RegexAutosolveOptions`, `regex_assisted_auto_resolve_pick()`, and `ConflictSession::auto_resolve_regex()` in core plus explicit "Auto-resolve regex" toolbar action and UI wiring — `crates/gitgpui-core/src/conflict_session.rs`, `crates/gitgpui-ui-gpui/src/view/conflict_resolver.rs`, `crates/gitgpui-ui-gpui/src/view/panes/main.rs`, `crates/gitgpui-ui-gpui/src/view/panels/main.rs`
- ✅ Pass 3: history-aware auto-resolve mode (kdiff3-inspired) — `HistoryAutosolveOptions` with configurable section/entry regex patterns and presets (keepachangelog, bullet-list), `history_merge_region()` detects changelog sections and merges entries by deduplication with optional sorting and max-entry truncation, `AutosolveRule::HistoryMerged` variant, `ConflictSession::auto_resolve_history()` method, `auto_resolve_segments_history()` UI-layer function, "Auto-resolve history" toolbar button; 11 core tests + 3 UI-layer tests — `crates/gitgpui-core/src/conflict_session.rs`, `crates/gitgpui-ui-gpui/src/view/conflict_resolver.rs`, `crates/gitgpui-ui-gpui/src/view/panes/main.rs`, `crates/gitgpui-ui-gpui/src/view/panels/main.rs`

### 5) Diff and Text Fidelity Upgrades
- ✅ Modeled missing trailing newline states in `file_diff.rs` via `FileDiffEofNewline` row metadata and EOF delta annotation (including newline-only diffs promoted to `Modify`) with dedicated tests — `crates/gitgpui-core/src/file_diff.rs`
- ✅ Stronger pairing semantics for asymmetric modify/delete blocks — replaced positional delete/add pairing with bounded cost-based alignment (Levenshtein + boundary similarity heuristic) so asymmetric replacement runs align best-matching lines while leaving clearly dissimilar lines as add/remove; includes regression tests for prefix insertions and contextual asymmetric replacements — `crates/gitgpui-core/src/file_diff.rs`
- ✅ Stable row/region anchors for conflict-region mapping — added `FileDiffRowAnchor`, `FileDiffRegionAnchor`, `FileDiffAnchors`, and `FileDiffRowsWithAnchors` plus deterministic `compute_row_region_anchors()` / `side_by_side_rows_with_anchors()` APIs with unit tests for region grouping, ordinals, missing-line-number rows, and determinism — `crates/gitgpui-core/src/file_diff.rs`

### 6) Non-UTF8/Binary-Safe Data Path
- ✅ `ConflictPayload::from_bytes()` for lazy UTF-8 decode — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ `BinarySidePick` strategy auto-selected when any payload is binary — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ Upgraded `ConflictFileStages` to carry `base_bytes/ours_bytes/theirs_bytes` plus optional decoded text views — `crates/gitgpui-core/src/services.rs`, `crates/gitgpui-git-gix/src/repo/diff.rs`
- ✅ Updated state loading to preserve bytes-first conflict payloads (`base/ours/theirs/current`) with lazy UTF-8 decode for UI text fields — `crates/gitgpui-state/src/model.rs`, `crates/gitgpui-state/src/store/effects/repo_load.rs`
- ✅ Binary/non-UTF8 resolver UI mode — `conflict_resolver_strategy()` now accepts `is_binary` flag (detects non-UTF8 bytes from loaded conflict file), `sync_conflict_resolver()` short-circuits text processing for binary files, dedicated `render_binary_conflict_resolver()` panel shows file sizes and "Use Ours"/"Use Theirs" buttons dispatching `Msg::CheckoutConflictSide`, binary conflicts skip text-specific header controls — `crates/gitgpui-ui-gpui/src/view/panels/main/diff.rs`, `crates/gitgpui-ui-gpui/src/view/panels/main/binary_conflict.rs`, `crates/gitgpui-ui-gpui/src/view/panels/main.rs`, `crates/gitgpui-ui-gpui/src/view/panes/main.rs`, `crates/gitgpui-ui-gpui/src/view/mod.rs` — 2 new test assertions for BinarySidePick strategy

### 7) Optional External Mergetool Bridge
- ✅ `MergetoolResult` struct and `launch_mergetool()` method on `GitRepository` trait — `crates/gitgpui-core/src/services.rs`
- ✅ Full mergetool implementation: reads `merge.tool` and `mergetool.<tool>.cmd` from git config, materializes BASE/LOCAL/REMOTE temp files from conflict stages (`:1:/:2:/:3:`), invokes tool with variable substitution ($BASE/$LOCAL/$REMOTE/$MERGED), respects `trustExitCode` config, reads back merged output and stages on success — `crates/gitgpui-git-gix/src/repo/mergetool.rs`
- ✅ `Msg::LaunchMergetool`, `Effect::LaunchMergetool`, `RepoCommandKind::LaunchMergetool` message/effect/command plumbing with full reducer, effect scheduler, and command log integration — `crates/gitgpui-state/src/msg/message.rs`, `crates/gitgpui-state/src/msg/effect.rs`, `crates/gitgpui-state/src/msg/repo_command_kind.rs`, `crates/gitgpui-state/src/store/reducer.rs`, `crates/gitgpui-state/src/store/reducer/actions_emit_effects.rs`, `crates/gitgpui-state/src/store/effects.rs`, `crates/gitgpui-state/src/store/effects/repo_commands.rs`, `crates/gitgpui-state/src/store/reducer/util.rs`
- ✅ "Mergetool" button in text conflict resolver toolbar (next to Save/Save & stage) and "External Mergetool" button in binary conflict resolver panel — `crates/gitgpui-ui-gpui/src/view/panels/main.rs`, `crates/gitgpui-ui-gpui/src/view/panels/main/binary_conflict.rs`
- ✅ 3 unit tests for git config reading and stage byte extraction — `crates/gitgpui-git-gix/src/repo/mergetool.rs`

### 8) Quality Pass (Iteration 9)
- ✅ Fixed mergetool `trustExitCode=false` bug: was checking only `.exists()` which always passes; now compares file mtime and length before/after tool invocation — `crates/gitgpui-git-gix/src/repo/mergetool.rs`
- ✅ Fixed WS toggle button label clippy warning (identical `if`/`else` branches) — `crates/gitgpui-ui-gpui/src/view/panels/main.rs`
- ✅ Removed stale `#[allow(dead_code)]` on `ConflictChoice::Base` (actively used) — `crates/gitgpui-ui-gpui/src/view/conflict_resolver.rs`
- ✅ Removed dead `if all_resolved` no-op block and unused variable in `auto_resolve_segments_pass2` — `crates/gitgpui-ui-gpui/src/view/conflict_resolver.rs`
- ✅ Fixed all conflict-resolution clippy warnings: collapsible `if` chains (4 sites), `derivable_impls` for `HistoryAutosolveOptions`, `needless_range_loop` (2 sites), `type_complexity` (added `WordHighlights` and `TwoWayWordHighlights` type aliases), `map_or` → `is_some_and` — across `conflict_session.rs`, `conflict_resolver.rs`, `panes/main.rs`, `panels/main.rs`, `rows/conflict_resolver.rs`, `view/mod.rs`
- ✅ Deduplicated `decode_utf8_optional` helper: extracted to `gitgpui_core::services::decode_utf8_optional()`, removed copies from `gitgpui-git-gix/src/repo/diff.rs` and `gitgpui-state/src/store/effects/repo_load.rs`

### 9) Rollout and Compatibility Flags
- ✅ Added persisted UI settings flags for advanced autosolve modes (`conflict_enable_regex_autosolve`, `conflict_enable_history_autosolve`) with round-trip tests — `crates/gitgpui-state/src/session.rs`
- ✅ Threaded advanced autosolve flags from session load into `MainPaneView` startup defaults (`false` unless explicitly enabled) — `crates/gitgpui-ui-gpui/src/view/mod.rs`, `crates/gitgpui-ui-gpui/src/view/panes/main.rs`
- ✅ Gated Pass 3 actions and toolbar buttons behind opt-in flags; safe autosolve remains always available — `crates/gitgpui-ui-gpui/src/view/panes/main.rs`, `crates/gitgpui-ui-gpui/src/view/panels/main.rs`
- ✅ Added Settings popover toggles to enable/disable regex/history autosolve and persist changes — `crates/gitgpui-ui-gpui/src/view/panels/popover/settings.rs`, `crates/gitgpui-ui-gpui/src/view/panels/popover.rs`, `crates/gitgpui-ui-gpui/src/view/tooltip.rs`

---

*Design reference: `tmp/conflict_resolution.md`*
