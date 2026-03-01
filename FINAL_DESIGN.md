## Implementation Progress

### External Diff/Merge Usage Design (`external_usage.md`)

- ✅ CLI subcommands and argument model (`gitgpui-app difftool`, `gitgpui-app mergetool`) implemented in `crates/gitgpui-app/src/cli.rs`.
- ✅ Arg/env resolution + validation implemented for `LOCAL`, `REMOTE`, `MERGED`, `BASE`, labels, missing-input and missing-path errors.
- ✅ Exit code constants aligned to design (`0`, `1`, `>=2`) defined in app CLI module.
- 🔧 Focused difftool/mergetool UI execution path is still not implemented; `main.rs` currently exits with an explicit not-yet-implemented error for those modes.
- ✅ External mergetool backend launch exists (`launch_mergetool`) with stage materialization (`BASE/LOCAL/REMOTE`), trust-exit behavior, unresolved-marker rejection, and staging semantics.
- ✅ Mergetool GUI selection and path override support implemented:
  - `merge.guitool` + `mergetool.guiDefault` precedence logic
  - `mergetool.<tool>.path` executable override (when `.cmd` is not set)
  - unit + integration test coverage added
- 🔧 Git behavior parity matrix coverage is partial. Implemented/covered: spaced paths, no-base handling for stage extraction, trust-exit semantics, deleted output handling. Remaining explicit coverage: symlink, submodule conflict invocation paths, CRLF preservation assertions, dir-diff mode, cancel/close exit semantics.
- 🔧 Git-like scenario porting is partial. Existing and new tests cover a subset of t7610-style behavior (`trustExitCode`, custom cmd with braced env, gui preference); `--tool-help`, full gui-default parity flow, order-file, delete/delete interaction prompts, and submodule-specific flows remain.
- ⬜ Dedicated difftool mode integration tests are not implemented yet.
- ⬜ End-to-end tests that invoke `git difftool`/`git mergetool` with global-like config and `gitgpui-app` as the tool are not implemented yet.
- ⬜ KDiff3-style fixture harness and generated corpus integration are not implemented yet.

### Reference Test Portability Plan (`docs/REFERENCE_TEST_PORTABILITY.md`)

- 🔧 Phase 1A (git `t6403` algorithm-focused cases): partially covered by existing `conflict_session` tests and conflict parsing tests, but not yet ported as the explicit named `t6403` matrix.
- ⬜ Phase 1B (git `t6427` `zdiff3` 4-case portability set): not implemented yet.
- ⬜ Phase 1C (conflict marker label formatting cases): not implemented yet.
- ⬜ Phase 2 (KDiff3 fixture harness with `*_base/*_contrib/*_expected_result` discovery + invariants): not implemented yet.
- ⬜ Phase 3A (permutation corpus generation): not implemented yet.
- ⬜ Phase 3C (real-world merge extraction harness): not implemented yet.
- 🔧 Phase 4A (critical `t7610-mergetool` E2E): partially implemented in `gitgpui-git-gix` tests:
  - ✅ trust-exit behavior and content-change semantics
  - ✅ custom command invocation and braced env variables
  - ✅ gui tool preference path via `merge.guitool` + `mergetool.guiDefault=true`
  - ✅ tool path override via `mergetool.<tool>.path`
  - ⬜ remaining cases (tool-help, nonexistent tool messaging parity, writeToTemp/orderFile/delete-delete prompt flow/submodule matrix/no-base file E2E via `git mergetool` command) still pending
- ⬜ Phase 4B (critical `t7800-difftool` E2E): not implemented yet.
- ⬜ Phase 5A/5B/5C (Meld-derived matcher/interval/newline test ports): not implemented yet.

### Iteration 1 Component Delivered

- Implemented foundational mergetool selection and executable resolution parity improvements in `crates/gitgpui-git-gix/src/repo/mergetool.rs`:
  - Added `mergetool.guiDefault` parsing (`true`/`false`/`auto`) with deterministic tool selection.
  - Added `merge.guitool` preference when GUI-default resolution requires it.
  - Added `mergetool.<tool>.path` support for non-`cmd` tool invocation.
  - Added targeted unit tests and integration tests in `status_integration.rs`.
