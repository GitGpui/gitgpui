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
- 🔧 Existing: A/B/C picks, next/prev conflict navigation, split/inline modes
- ✅ Solved/unsolved counters in domain model (ready for UI binding)
- ✅ Safety gate: detect unresolved markers before "Save & stage" — `text_contains_conflict_markers()` in `conflict_resolver.rs`, `ConflictSaveStageConfirm` popover with cancel/stage-anyway actions, warning indicator in header when markers remain
- 🔧 Marker-based conflict counter (Conflict N/M) already in panel UI; full solved/unsolved counters need `ConflictSession` integration
- ⬜ Next/previous *unresolved* navigation in UI (wrap-around)
- ⬜ Hide-resolved toggle
- ⬜ Bulk actions: apply pick to all unresolved, autosolve safe conflicts

### 4) Auto-Resolution Engine (Safe-First)
- ✅ Pass 1 safe auto-resolve rules: identical sides, only-ours-changed, only-theirs-changed — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ `AutosolveRule` enum with traceability (rule ID + description) — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ `ConflictSession::auto_resolve_safe()` applies Pass 1 to all unresolved regions — `crates/gitgpui-core/src/conflict_session.rs`
- ⬜ Pass 2: heuristic subchunk splitting (meld-inspired)
- ⬜ Pass 3: history/regex modes (opt-in)
- ⬜ Wire autosolve into UI and state layer

### 5) Diff and Text Fidelity Upgrades
- ⬜ Model missing trailing newline states in `file_diff.rs`
- ⬜ Stronger pairing semantics for asymmetric modify/delete blocks
- ⬜ Stable row/region anchors for conflict-region mapping

### 6) Non-UTF8/Binary-Safe Data Path
- ✅ `ConflictPayload::from_bytes()` for lazy UTF-8 decode — `crates/gitgpui-core/src/conflict_session.rs`
- ✅ `BinarySidePick` strategy auto-selected when any payload is binary — `crates/gitgpui-core/src/conflict_session.rs`
- ⬜ Upgrade `ConflictFileStages` to carry bytes (not just `Option<String>`)
- ⬜ Update state loading to use bytes-first path
- ⬜ Binary/non-UTF8 resolver UI mode

### 7) Optional External Mergetool Bridge
- ⬜ Materialize BASE/LOCAL/REMOTE/MERGED temp files
- ⬜ Invoke configured tool command
- ⬜ Reload/validate merged output, stage on success

---

*Design reference: `tmp/conflict_resolution.md`*
