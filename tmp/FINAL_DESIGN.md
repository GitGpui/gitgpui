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
- ✅ Wired strategy dispatch into UI: removed `conflict_requires_resolver` gating, switched activation/search/preview hotpaths to `conflict_resolver_strategy()`, and defaulted non-full-text kinds to 2-way resolver mode — `crates/gitgpui-ui-gpui/src/view/panels/main/diff.rs`, `crates/gitgpui-ui-gpui/src/view/panels/main.rs`, `crates/gitgpui-ui-gpui/src/view/panes/main.rs`

### 3) Resolver UX Model
- ✅ Existing: A/B/C picks, next/prev conflict navigation, split/inline modes
- ✅ Solved/unsolved counters in domain model (ready for UI binding)
- ✅ Safety gate: detect unresolved markers before "Save & stage" — `text_contains_conflict_markers()` in `conflict_resolver.rs`, `ConflictSaveStageConfirm` popover with cancel/stage-anyway actions, warning indicator in header when markers remain
- ✅ Resolved/total counter display in conflict resolver toolbar — shows "Resolved X/Y" with green color when fully resolved — `crates/gitgpui-ui-gpui/src/view/panels/main.rs`
- ✅ Per-block resolved tracking (`ConflictBlock.resolved` field) — set on A/B/C picks, all-pick, and auto-resolve — `crates/gitgpui-ui-gpui/src/view/conflict_resolver.rs`, `crates/gitgpui-ui-gpui/src/view/panes/main.rs`
- ✅ Next/previous *unresolved* navigation in UI (wrap-around) — added unresolved index helpers + tests and wired toolbar/auto-advance to unresolved-only navigation — `crates/gitgpui-ui-gpui/src/view/conflict_resolver.rs`, `crates/gitgpui-ui-gpui/src/view/panes/main.rs`, `crates/gitgpui-ui-gpui/src/view/panels/main.rs`
- ⬜ Hide-resolved toggle
- 🔧 Bulk actions: "All → A/B/C" exists; auto-resolve safe conflicts now wired (see §4)

### 4) Auto-Resolution Engine (Safe-First)
- ✅ Pass 1 safe auto-resolve rules: identical sides, only-ours-changed, only-theirs-changed — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ `AutosolveRule` enum with traceability (rule ID + description) — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ `ConflictSession::auto_resolve_safe()` applies Pass 1 to all unresolved regions — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ `auto_resolve_segments()` applies Pass 1 safe rules directly to UI marker segments — `crates/gitgpui-ui-gpui/src/view/conflict_resolver.rs`
- ✅ "Auto-resolve safe" button in conflict resolver toolbar (shown when unresolved blocks remain) — `crates/gitgpui-ui-gpui/src/view/panels/main.rs`
- ✅ `conflict_resolver_auto_resolve()` method wires button to auto-resolve + text regeneration — `crates/gitgpui-ui-gpui/src/view/panes/main.rs`
- ✅ 10 unit tests for auto-resolve segments and resolved counting — `crates/gitgpui-ui-gpui/src/view/conflict_resolver.rs`
- ⬜ Pass 2: heuristic subchunk splitting (meld-inspired)
- ⬜ Pass 3: history/regex modes (opt-in)

### 5) Diff and Text Fidelity Upgrades
- ⬜ Model missing trailing newline states in `file_diff.rs`
- ⬜ Stronger pairing semantics for asymmetric modify/delete blocks
- ⬜ Stable row/region anchors for conflict-region mapping

### 6) Non-UTF8/Binary-Safe Data Path
- ✅ `ConflictPayload::from_bytes()` for lazy UTF-8 decode — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ `BinarySidePick` strategy auto-selected when any payload is binary — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ Upgraded `ConflictFileStages` to carry `base_bytes/ours_bytes/theirs_bytes` plus optional decoded text views — `crates/gitgpui-core/src/services.rs`, `crates/gitgpui-git-gix/src/repo/diff.rs`
- ✅ Updated state loading to preserve bytes-first conflict payloads (`base/ours/theirs/current`) with lazy UTF-8 decode for UI text fields — `crates/gitgpui-state/src/model.rs`, `crates/gitgpui-state/src/store/effects/repo_load.rs`
- ⬜ Binary/non-UTF8 resolver UI mode

### 7) Optional External Mergetool Bridge
- ⬜ Materialize BASE/LOCAL/REMOTE/MERGED temp files
- ⬜ Invoke configured tool command
- ⬜ Reload/validate merged output, stage on success

---

*Design reference: `tmp/conflict_resolution.md`*
