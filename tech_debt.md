# GitComet Technical Debt & Code Review

Comprehensive review of the GitComet codebase covering security vulnerabilities, bugs,
performance bottlenecks, cross-platform issues, and technical debt across all 7 crates.

Reviewed crates: `gitcomet-app`, `gitcomet-core`, `gitcomet-git`, `gitcomet-git-gix`,
`gitcomet-state`, `gitcomet-ui`, `gitcomet-ui-gpui`, plus CI/CD workflows, build scripts,
and dependencies.

---

## Table of Contents

- [Critical & High Severity](#critical--high-severity)
- [Security Vulnerabilities](#security-vulnerabilities)
- [Bugs & Defects](#bugs--defects)
- [Performance Issues](#performance-issues)
- [Cross-Platform Issues](#cross-platform-issues)
- [Technical Debt](#technical-debt)
- [CI/CD & Build Configuration](#cicd--build-configuration)
- [Dependency Concerns](#dependency-concerns)

---

## Critical & High Severity

These are the issues most likely to impact users, cause crashes, or create security/reliability risk.

- [x] ### 1. Worker thread panics can permanently degrade async execution [Reliability / High]
**File:** `gitcomet-state/src/store/executor.rs:24-33`

`TaskExecutor` runs `task()` directly in worker threads without `std::panic::catch_unwind`.
A panic kills that worker thread permanently. With capped worker count (1-8 threads), a few
panicking tasks can starve all background operations. The existing test
`send_failures.rs:29-61` already codifies this failure mode.

**Recommendation:** Wrap each task execution in `catch_unwind` and keep workers alive. Add a
supervisor/respawn mechanism and surface a diagnostic when worker panic count is non-zero.

- [x] ### 2. Myers diff O((n+m)^2) memory usage [Performance / High]
**File:** `gitcomet-core/src/file_diff.rs:812-879`

The Myers diff implementation stores the full trace for backtracking via
`trace.push(v.clone())`. Each iteration clones the entire `v` vector (size `2*(n+m)+1`).
For two 5,000-line files, this allocates roughly `10000 * 20001 * 8 bytes` ~= 1.5 GB.
The overflow guard at line 815 protects against `isize::MAX` but not practical memory
exhaustion.

**Recommendation:** Add a configurable line-count threshold (e.g., 5,000 lines per side)
that falls back to a simpler algorithm, or use a divide-and-conquer (linear space) Myers
variant.

- [x] ### 3. Tree-sitter re-parses every line independently [Performance / High]
**File:** `gitcomet-ui-gpui/src/view/rows/diff_text/syntax.rs:147-233`

`syntax_tokens_for_line_treesitter()` calls `parser.parse()` with a fresh tree for every
line of diff text. Tree-sitter is designed to parse entire files; its startup cost per parse
is significant. For a file with 1000 visible lines, this means 1000 separate parser
invocations. Cross-line constructs (multi-line strings, block comments) will also not be
correctly highlighted since each line is parsed in isolation.

**Recommendation:** Parse the full file content once and map tokens to lines, caching the
parse tree per file.

- [x] ### 4. `snapshot()` clones entire `AppState` on every call [Performance / High]
**File:** `gitcomet-state/src/store/mod.rs:129-134`

`snapshot()` acquires a read lock and clones the entire `AppState` including all
`Vec<RepoState>`, every `Loadable`, `String` fields, `Vec<CommandLogEntry>`, etc. If the
UI calls `snapshot()` at 60fps, this is a significant allocation hotspot.

**Recommendation:** Return `Arc<AppState>` instead, or use a generation-based approach that
avoids cloning unchanged data.

- [x] ### 5. Subprocess spawn per gitignore check [Performance / High]
**File:** `gitcomet-state/src/store/repo_monitor.rs:299-341`

`query_git_check_ignore()` spawns `git check-ignore --quiet` for each previously-unseen
file path. For a large initial batch of filesystem events (e.g., on first watcher
registration), this can spawn hundreds of git processes. The result is cached but the cache
starts empty. Each directory event can trigger up to 3 subprocess invocations.

**Recommendation:** Batch paths into a single `git check-ignore` call with stdin, or use a
library-based gitignore matcher (like `globset`, which is already a dependency).

- [x] ### 6. Repository config can trigger arbitrary shell command execution [Security / High]
**File:** `gitcomet-git-gix/src/repo/mergetool.rs:200-250`

The mergetool command is read from git config (`mergetool.<tool>.cmd`) including
repository-local config, then executed through `sh -c` (Unix) or `cmd /C` (Windows).
Opening an untrusted repository and invoking mergetool can execute attacker-controlled
commands. This aligns with native git behavior, but GitComet currently has no explicit
trust boundary prompt.

**Recommendation:** Add trust-mode controls: global-only config by default, repo-local
commands behind explicit user consent. Display the exact command before first execution
per repo/tool.

- [x] ### 7. Unsanitized user strings passed as git CLI arguments [Security / High]
**Files:** Multiple locations in `gitcomet-git-gix/src/repo/` (porcelain.rs, remotes.rs,
tags.rs, history.rs)

User-controlled strings (branch names, remote names, tag names, commit IDs) are passed
directly as arguments to `Command::new("git")`. While `std::process::Command::arg()`
prevents shell injection, a value starting with `-` (e.g., `-d` as a branch name) would
be interpreted as a git flag. `CommitId` is `pub struct CommitId(pub String)` with no
validation.

**Recommendation:** Validate all user-provided ref names before passing to git commands.
At minimum, reject values starting with `-`. Use `--` to separate options from arguments
where applicable. Validate `CommitId` contains only hex characters.

- [x] ### 8. No dependency vulnerability scanning [Security / High]
**Files:** CI workflows, `Cargo.toml`

No `cargo-audit`, `cargo-deny`, Dependabot, or Renovate is configured. With 9131 lines
in `Cargo.lock`, any CVE in the dependency tree would go undetected.

**Recommendation:** Add `cargo-audit` to CI and configure Dependabot or Renovate for
automated dependency updates.

- [x] ### 9. `unwrap()` on window creation -- will panic [Bug / High]
**Files:** `gitcomet-ui-gpui/src/app.rs:182`, `gitcomet-ui-gpui/src/focused_diff.rs:306`

`cx.open_window(...).unwrap()` will panic if the window system fails to create a window
(e.g., headless environments, Wayland compositor crashes, GPU driver failures). The crash
handler will catch it but the error message will be cryptic.

**Recommendation:** Use `.expect("failed to open window")` with a descriptive message, or
handle the error and show a graceful diagnostic.

- [x] ### 10. Directory difftool dereferences symlinks outside repository boundaries [Security / High]
**File:** `gitcomet-app/src/difftool_mode.rs:124-241`

`copy_tree_dereferencing_symlinks` follows symlinks recursively without checking that
resolved targets stay within the repository root or any allowed boundary. While cycle
detection exists (line 140-144 via `active_dirs` HashSet), a malicious symlink pointing to
`/etc/shadow` or to a very large directory tree would be followed and copied to the temp
staging area. This enables both information disclosure and disk-space DoS.

**Recommendation:** Restrict dereferencing to paths under explicitly allowed roots. Add
file/byte/entry limits and fail-safe cutoffs for recursive staging.

---

## Security Vulnerabilities

- [x] ### SEC-1. Git subprocesses have no timeout and can hang worker capacity [Medium]
**Files:** `gitcomet-git-gix/src/util.rs:12-15`, `gitcomet-state/src/store/effects/clone.rs:19-27`

Most git operations use blocking `.output()` / `.wait()` with no timeout. Credential
prompts, network stalls, or hooks can pin worker threads indefinitely. With capped worker
count, a few hung commands can starve all background work.

**Recommendation:** Add configurable timeouts + cancellation path. For non-interactive
workflows, set `GIT_TERMINAL_PROMPT=0` and controlled `stdin`.

- [x] ### SEC-2. Environment-controlled file write path [Medium]
**File:** `gitcomet-app/src/cli/compat.rs:24-39`

`maybe_record_compat_argv` reads a file path from `GITCOMET_COMPAT_ARGV_LOG` and writes
command-line arguments to it without validation. An attacker controlling the environment
could set this to an arbitrary path and write near-arbitrary content.

**Recommendation:** Restrict writes to a specific directory, or document as debug-only.

- [x] ### SEC-3. Clone URL not validated [Medium]
**File:** `gitcomet-state/src/store/effects/clone.rs:19-27`

The `url` parameter is passed directly to `git clone`. Git interprets certain URL schemes
that can execute arbitrary code (e.g., `ext::`). No validation checks for plausible
repository URLs.

**Recommendation:** Validate clone URLs against an allowlist of schemes (https, ssh, git,
file).

- [x] ### SEC-4. Predictable temp file naming in patch operations [Medium]
**File:** `gitcomet-git-gix/src/repo/patch.rs:51-58, 90-97`

Temp files use `gitcomet-index-patch-{pid}-{nanos}.patch` naming via `fs::write` instead
of secure temp file creation. On multi-user systems, an attacker could predict the filename
and create a symlink (TOCTOU race / symlink attack).

**Recommendation:** Use `tempfile::NamedTempFile` (already a dependency) instead of manual
temp file creation.

- [x] ### SEC-5. URL/path passed to shell open without protocol validation [Medium]
**File:** `gitcomet-ui-gpui/src/view/platform_open.rs:5-9, 63-121`

`open_url()` does not validate the URL scheme. On Windows, `cmd /C start "" <arg>` has
tricky parsing/escaping semantics and can execute commands with specially-crafted input.
`open_path()` accepts paths from context menus without sanitization.

**Recommendation:** Validate URL schemes against an allowlist (http, https, mailto).
Consider replacing `cmd start` with direct Windows `ShellExecuteW` API or a safer
alternative.

- [x] ### SEC-6. Unbounded regex compilation from user input [Medium]
**Files:** `gitcomet-core/src/conflict_session/history.rs:88-89`,
`gitcomet-core/src/conflict_session/autosolve.rs:196-197`

User-supplied regex patterns are compiled without complexity limits. The Rust `regex` crate
has built-in safeguards against catastrophic backtracking, but no size limits are
configured.

**Recommendation:** Use `RegexBuilder` with `.size_limit()` and `.dfa_size_limit()`.

- [x] ### SEC-7. Session persistence not safe for multi-process concurrency [Medium]
**File:** `gitcomet-state/src/session.rs:504-528`

Uses fixed temp file name (`*.json.tmp`) next to target. Concurrent instances can clobber
each other's temp writes. The Windows fallback uses non-atomic `copy` then `remove_file`.

**Recommendation:** Use unique temp files (via `tempfile` crate) in the same directory,
then atomic replace. Consider advisory lock if multi-instance support is required.

- [x] ### SEC-8. `SaveWorktreeFile` path traversal [Low]
**File:** `gitcomet-state/src/store/effects/repo_commands.rs:55-60`

`save_worktree_file` joins a user-provided `path` with the workdir. If `path` is
`../../etc/something` or absolute, `join()` would write outside the working directory.

**Recommendation:** Validate that the resolved path stays within the working directory.

- [x] ### SEC-9. TOCTOU races in file type classification [Low]
**Files:** `gitcomet-app/src/cli.rs:296-344, 376-421`

`classify_difftool_input` checks `symlink_metadata` then `metadata` separately. Filesystem
state could change between calls. Low-risk for a local desktop tool.

- [x] ### SEC-10. Temp-file image cache uses predictable path [Low]
**File:** `gitcomet-ui-gpui/src/view/panes/main/diff_cache.rs:22-38`

Image diff cache uses non-cryptographic `DefaultHasher` at predictable temp path
`$TMPDIR/gitcomet/image_diff_cache/{hash}.{ext}`. On multi-user systems, this could allow
cache poisoning.

---

## Bugs & Defects

- [x] ### BUG-1. Browser mode ignores parsed `path` argument [Medium]
**File:** `gitcomet-app/src/main.rs:101-104`

The parsed and validated `path` is discarded with `let _ = path;`. The underlying `run()`
reads `std::env::args_os().nth(1)` internally. If argument ordering ever changes, this
will silently break.

- [x] ### BUG-2. `log_file_page_impl` ignores cursor parameter -- pagination broken [Medium]
**File:** `gitcomet-git-gix/src/repo/log.rs:271-290`

The `_cursor` parameter is explicitly ignored (underscore-prefixed). For paginated file
history, this means the caller cannot page past the first batch of results. The returned
`LogPage` always has `next_cursor: None`.

- [x] ### BUG-3. `commit_details_impl` makes 4 separate `git show` calls [Medium]
**File:** `gitcomet-git-gix/src/repo/log.rs:292-357`

Four separate `git show` commands are spawned sequentially for a single commit detail
request (message, committed_at, parents, files). This is unnecessarily slow and creates
a race condition if repository state changes between calls.

**Recommendation:** Combine into a single `git show` with a custom format string.

- [x] ### BUG-4. `FetchAll` always passes `prune: false` regardless of setting [Medium]
**File:** `gitcomet-state/src/store/reducer/actions_emit_effects.rs:171-175`

The `fetch_all()` function hardcodes `prune: false`. The
`fetch_prune_deleted_remote_tracking_branches` setting is persisted in the session file
but never wired into the actual fetch operation.

- [x] ### BUG-5. `set_status` unconditionally bumps revision counter [Medium]
**File:** `gitcomet-state/src/model.rs:405-423`

`set_status()` always increments `status_rev` even when the new value is identical,
causing unnecessary UI re-renders. Same issue with `set_log`, `set_log_loading_more`,
`set_log_scope`. Inconsistent with other setters (like `set_head_branch`, `set_branches`)
which skip the rev bump when unchanged.

- [x] ### BUG-6. `Diff::from_unified` misclassifies lines starting with `+++`/`---` without trailing space [Low]
**File:** `gitcomet-core/src/domain.rs:208-228`

The Header arm checks `"--- "` and `"+++ "` (with trailing space). The Add arm at line 222
guards with `!raw.starts_with("+++")` (3 chars, no space). This means a content line like
`"+++text"` (adding `"++text"`) is NOT matched by the Header arm (no trailing space) but
IS excluded by the Add arm guard, falling through to `Context`. The guard should be
`!raw.starts_with("+++ ")` to match the header check.

- [x] ### BUG-7. `spawn_with_repo` silently drops tasks for unknown `repo_id` [Low]
**File:** `gitcomet-state/src/store/effects/util.rs:11-21`

If `repos.get(&repo_id)` returns `None`, the task closure is silently dropped. The
`loads_in_flight` flag will remain set forever, preventing future loads of that type from
being scheduled.

- [x] ### BUG-8. Missing `begin_local_action` for several operations [Low]
**File:** `gitcomet-state/src/store/reducer.rs`

`Msg::MergeRef`, `Msg::Reset`, `Msg::Rebase`, `Msg::CreateTag`, `Msg::DeleteTag`,
`Msg::AddRemote`, `Msg::RemoveRemote`, `Msg::SaveWorktreeFile`, and several others do not
call `begin_local_action()`. This means `local_actions_in_flight` will not accurately
reflect all in-progress operations for a "busy" indicator.

- [x] ### BUG-9. `merge_commit_message_impl` off-by-one edge case [Low]
**File:** `gitcomet-git-gix/src/repo/history.rs:136-142`

`unwrap_or(start)` should be `unwrap_or(start + 1)` when `rposition` finds no non-empty
line after filtering comments, otherwise the start line itself is excluded.

- [x] ### BUG-10. Duplicate functions that should be consolidated [Low]
**Files:** `gitcomet-app/src/mergetool_mode.rs:123-165` (merged_filename vs
merged_display_name), `gitcomet-app/src/difftool_mode.rs:117-122` (display_input_kind vs
DifftoolInputKind::display_name)

Identical implementations in each pair that should be consolidated into single functions.

---

## Performance Issues

- [x] ### PERF-1. `chars_to_tokens` creates one heap-allocated `String` per character [Medium]
**File:** `gitcomet-core/src/text_utils.rs:259-261`

```rust
fn chars_to_tokens(input: &str) -> Vec<String> {
    input.chars().map(|c| c.to_string()).collect()
}
```

For a 10,000-character string, this creates 10,000 heap allocations. Used by
`matching_blocks_chars` and `matching_blocks_chars_inline`.

**Recommendation:** Use `Vec<char>` or `Vec<&str>` via `char_indices`.

- [x] ### PERF-2. Excessive string cloning in merge hunk processing [Medium]
**File:** `gitcomet-core/src/merge.rs:238-250`

Every inserted line is cloned to a `String`, then `reconstruct_side` clones again.
`coalesce_zealous_conflicts` clones separator lines twice.

**Recommendation:** Use `Cow<str>` or references to avoid redundant allocations.

- [x] ### PERF-3. `word_diff_ranges` Myers trace clones per edit step [Medium]
**File:** `gitcomet-ui-gpui/src/view/word_diff.rs:141`

Same Myers trace issue as the core crate but bounded to 128 tokens per side, limiting
impact to ~1MB per call. Still significant when called for every modified line pair in a
diff.

- [x] ### PERF-4. Thread-local caches grow then clear entirely [Medium]
**Files:** `gitcomet-ui-gpui/src/view/rows/diff_canvas.rs:876-880`,
`history_canvas.rs:15-17`

`GUTTER_TEXT_LAYOUT_CACHE` (16K entries) and `HISTORY_TEXT_LAYOUT_CACHE` (8K entries) grow
until cap, then `cache.clear()` drops everything at once, causing a burst of re-shaping
work on the next frame.

**Recommendation:** Use LRU eviction or partial eviction instead of full clear.

- [x] ### PERF-5. Linear scan in hot rendering path [Medium]
**File:** `gitcomet-ui-gpui/src/view/rows/diff_text.rs:109-110`

`word_ranges.iter().any(...)` and `query_ranges.iter().any(...)` perform linear scans for
each segment boundary. With many ranges, this is O(segments * ranges).

**Recommendation:** Use a cursor/index approach since ranges are sorted.

- [x] ### PERF-6. SVG rasterization up to 4096x4096 on UI thread [Medium]
**File:** `gitcomet-ui-gpui/src/view/diff_utils.rs:94-134`

SVG rasterization can create ~67MB pixmaps synchronously. The related
`ensure_file_diff_cache` correctly uses `smol::unblock` for off-thread work, but
`ensure_file_image_diff_cache` calls `rasterize_svg_preview_image` synchronously.

**Recommendation:** Move SVG rasterization to `smol::unblock`.

- [x] ### PERF-7. Synchronous file I/O in reducer critical path [Medium]
**File:** `gitcomet-state/src/session.rs:131-152`

`persist_from_state()` performs file I/O (read existing session, write temp file, rename)
while holding the state write lock, blocking all message processing. Called on every
`open_repo`, `close_repo`, `set_active_repo`, `reorder_repo_tabs`. During
`restore_session` with N repos, this results in N+1 synchronous file writes.

**Recommendation:** Delegate to an async effect or debounce/batch persistence.

- [x] ### PERF-8. Repeated linear scans for repo lookup by ID [Medium]
**File:** Multiple files in `gitcomet-state`

`state.repos.iter_mut().find(|r| r.id == repo_id)` is O(n) in the number of open repos.
Some functions perform this scan multiple times within the same call (e.g.,
`commit_finished`, `repo_command_finished`).

- [x] ### PERF-9. `conflict_session_impl` triggers full repo status scan [Medium]
**File:** `gitcomet-git-gix/src/repo/diff.rs:270-315`

Full repo status is computed to determine a single file's conflict status. For large
repos, this is O(n) in tracked files.

- [x] ### PERF-10. Sequential subprocess calls for multi-remote/tag operations [Medium]
**Files:** `gitcomet-git-gix/src/repo/tags.rs:38-85, 150-249`, `remotes.rs:409-529`

`list_remote_tags` spawns one `git ls-remote` per remote sequentially.
`prune_local_tags` spawns one `git tag -d` per tag. `prune_merged_branches` spawns one
`git show-ref` per branch.

**Recommendation:** Batch into single commands or parallelize.

- [x] ### PERF-11. Full directory tree copy for directory diffs [Medium]
**File:** `gitcomet-app/src/difftool_mode.rs:100-114`

Copies entire directory tree into temp directory to dereference symlinks. For large trees,
this is slow and disk-intensive with no size limits.

- [x] ### PERF-12. `LINE_NUMBER_STRINGS` cache grows unbounded [Medium]
**File:** `gitcomet-ui-gpui/src/view/rows/mod.rs:7-28`

Thread-local `Vec<SharedString>` grows to accommodate any line number and never shrinks.
Peak memory from the largest file ever viewed persists for the process lifetime.

- [x] ### PERF-13. `incoming_mask` computation is O(lanes^2) per commit [Medium]
**File:** `gitcomet-ui-gpui/src/view/history_graph.rs:183`

`incoming_ids.contains(&lane.id)` is O(n) on a `Vec`, called once per lane.

**Recommendation:** Use `HashSet` for `incoming_ids`.

- [x] ### PERF-14. Image diff cache grows unbounded in temp storage [Low]
**File:** `gitcomet-ui-gpui/src/view/panes/main/diff_cache.rs:22-37`

Every unique image bytes hash is persisted to disk with no TTL, size cap, or periodic
cleanup. Long-running usage can create silent disk bloat.

**Recommendation:** Add max-size + age-based cleanup on startup and opportunistically
during writes.

- [~] ### PERF-15. `index_matching_kmers` creates trigram tuples by cloning [Low]
> **Skipped:** PERF-15 is stale here; `index_matching_kmers` no longer creates cloned `String` trigram tuples.
**File:** `gitcomet-core/src/text_utils.rs:347-376`

Creates `(String, String, String)` tuples for every trigram window, cloning three strings
per trigram. For a 10,000-token sequence, ~30,000 string clones.

---

## Cross-Platform Issues

- [x] ### XPLAT-1. `install-linux.sh --debug` is broken [High]
**File:** `scripts/install-linux.sh:37`

`cargo build -p gitcomet-app --${mode}` produces `--debug` when mode="debug", which is
**not a valid cargo flag**. Should use no flag (debug is default) or `--profile dev`.

- [x] ### XPLAT-2. No Rust toolchain pinning [High]
**File:** Repository root (missing `rust-toolchain.toml`)

The workspace uses `edition = "2024"` which requires Rust 1.85+. Without a pinned
toolchain, contributors may use incompatible Rust versions. Edition 2024 is very new
(stabilized Feb 2025).

- [x] ### XPLAT-3. `setup` rejects non-UTF-8 executable paths [Medium]
**File:** `gitcomet-app/src/setup_mode.rs:764-770`

`bin_path.to_str()` returns `None` for non-UTF-8 paths on Unix, causing setup to fail with
an error even though the tool itself runs fine. The POSIX shell quoting handles the path as
a string, but this conversion gate rejects valid Unix paths.

**Recommendation:** Preserve `OsStr` where possible, or use `to_string_lossy()` for shell
quoting.

- [x] ### XPLAT-4. Inconsistent `cfg!()` vs `#[cfg()]` in crash_dir [Medium]
**File:** `gitcomet-app/src/crashlog.rs:103-147`

Uses runtime `cfg!()` checks that include dead code from non-matching platforms.
Inconsistent with the compile-time `#[cfg()]` pattern used elsewhere in the same file.

- [x] ### XPLAT-5. Linux desktop `Exec=` path not escaped per desktop entry spec [Low]
**File:** `gitcomet-ui-gpui/src/view/linux_desktop_integration.rs:89-92`

`Exec=` line uses `exe.display()` which doesn't escape special characters (spaces, `%`,
etc.) per the FreeDesktop desktop entry spec. Paths containing spaces or special chars will
break launcher behavior.

**Recommendation:** Escape per desktop entry spec or install a wrapper script with a safe
name.

- [x] ### XPLAT-6. Linux desktop integration only auto-installs on GNOME [Low]
**File:** `gitcomet-ui-gpui/src/view/linux_desktop_integration.rs:16-23`

Checks for GNOME in `XDG_CURRENT_DESKTOP` and only installs `.desktop` entry there. The
`.desktop` file standard is FreeDesktop-wide and works on KDE, XFCE, Sway, etc.

- [x] ### XPLAT-7. `embed-resource` compiled unconditionally [Low]
**File:** `gitcomet-app/Cargo.toml:29`

Windows-only build dependency is compiled on all platforms. Should be gated with
`[target.'cfg(target_os = "windows")'.build-dependencies]`.

- [x] ### XPLAT-8. Linux `open_file_location` doesn't select the file [Low]
**File:** `gitcomet-ui-gpui/src/view/platform_open.rs:42-46`

Opens parent directory rather than selecting the file, unlike macOS (`open -R`) and Windows
(`explorer /select,`). Some file managers support `org.freedesktop.FileManager1` D-Bus.

- [x] ### XPLAT-9. `parse_name_status_line` path separator inconsistency on Windows [Low]
**File:** `gitcomet-git-gix/src/util.rs:299`

Git outputs forward slashes; `PathBuf::from("a/b")` on Windows keeps them, causing
potential mismatches with `Path::join()` which uses `\`.

- [x] ### XPLAT-10. Repository URL mismatch [Low]
**Files:** `Cargo.toml:27` vs `gitcomet-app/src/crashlog.rs:9`

Workspace `repository` is `github.com/GitComet/gitcomet` while crash issue URL points to
`github.com/Auto-Explore/GitComet`. One is stale.

---

## Technical Debt

- [x] ### TD-1. Massive `PopoverKind` enum with 50+ variants [High]
**File:** `gitcomet-ui-gpui/src/view/mod_helpers.rs:474-693`

Makes pattern matching exhaustive, increases compile times. Several variants carry the same
`repo_id` + data pattern. Consider grouping related popovers into sub-enums.

- [x] ### TD-2. `ConflictResolverUiState` has 40+ fields [High]
**File:** `gitcomet-ui-gpui/src/view/mod_helpers.rs:336-396`

God object for conflict resolver state. Related fields (e.g., `three_way_base_lines`,
`three_way_ours_lines`, `three_way_theirs_lines`) should be grouped into sub-structs.

- [x] ### TD-3. `RepoState` struct has 50+ fields [Medium]
**File:** `gitcomet-state/src/model.rs:128-251`

God object covering every aspect of repository state. Increases clone cost and makes state
transitions error-prone. Consider sub-structs: `DiffState`, `ConflictState`,
`HistoryState`, etc.

- [x] ### TD-4. Heavy CLI subprocess reliance in the "gix" backend [Medium]
**File:** `gitcomet-git-gix` crate

Despite being named the gitoxide backend, the majority of operations (commit, push, pull,
fetch, checkout, stash, rebase, merge, blame, worktree, submodule, patch, tag) shell out
to `git` CLI. Only status, list operations, and blob reads use the `gix` library.

- [x] ### TD-5. Massive code duplication in push/pull in-flight tracking [Medium]
**File:** `gitcomet-state/src/store/reducer/actions_emit_effects.rs:160-310`

11 functions (`fetch_all`, `prune_merged_branches`, `pull`, `push`, `force_push`,
`push_set_upstream`, `delete_remote_branch`, `push_tag`, `delete_remote_tag`, etc.) follow
the exact same pattern: check repo exists, find repo state, increment counter, bump rev,
return effect. Should be refactored into a helper function.

- [x] ### TD-6. Diff reload logic duplicated in 4+ places [Medium]
**File:** Multiple locations in `gitcomet-state`

The `diff_reload_effects` utility exists in `util.rs` but is only used in some places;
`diff_selection.rs`, `actions_emit_effects.rs`, and `repo_management.rs` duplicate the
logic manually.

- [x] ### TD-7. Code duplication across canvas rendering functions [Medium]
**File:** `gitcomet-ui-gpui/src/view/rows/diff_canvas.rs`

`inline_diff_line_row_canvas`, `split_diff_line_row_canvas`,
`patch_split_column_row_canvas`, and `worktree_preview_row_canvas` share nearly identical
mouse event handling logic (mouse-down/mouse-up/context-menu pattern copy-pasted with
minor variations).

- [x] ### TD-8. `pull_impl` / `pull_with_output_impl` near-duplicates [Medium]
**File:** `gitcomet-git-gix/src/repo/remotes.rs:116-218`

The `_impl` / `_with_output_impl` pattern is copy-pasted across pull, push, push_force,
and many other operations. Only difference is calling `run_git_simple` vs
`run_git_with_output`.

**Recommendation:** Extract a shared helper that returns output and optionally discards it.

- [x] ### TD-9. `build_unified_patch_for_hunk_selection` near-duplicate [Medium]
**File:** `gitcomet-ui-gpui/src/view/diff_utils.rs:431-595`

Two 80-line functions differ only in how unselected add/remove lines are handled.

- [x] ### TD-10. Compat argument parser is 366 lines of repetitive code [Medium]
**File:** `gitcomet-app/src/cli/compat.rs:42-408`

12 nearly identical blocks for `-L1`/`-L2`/`-L3` flag variations could be replaced with a
small parser combinator or helper function.

- [x] ### TD-11. Code duplication between `merge.rs` and `subchunk.rs` [Medium]
**Files:** `gitcomet-core/src/merge.rs:220-254`,
`gitcomet-core/src/conflict_session/subchunk.rs:90-126`

`edits_to_hunks` and `edits_to_line_hunks` are structurally identical. `reconstruct_side`
and `side_content` also duplicate logic. Could be extracted into a shared generic function.

- [x] ### TD-12. `Error` Display uses Debug formatting [Medium]
**File:** `gitcomet-core/src/error.rs:18-22`

`write!(f, "{:?}", self.kind)` produces `Backend("message")` instead of human-readable
`message`. The `ErrorKind` enum should have its own `Display` impl.

- [x] ### TD-13. Result handling pattern repeated 5 times in main.rs [Medium]
**File:** `gitcomet-app/src/main.rs:44-264`

Same print-stdout/stderr-flush-exit pattern for each mode. A helper function like
`run_and_exit(result)` could eliminate this duplication.

- [x] ### TD-14. `Msg` enum has 80+ variants [Low]
**File:** `gitcomet-state/src/msg/message.rs`

Manual `Debug` implementation in `message_debug.rs` is 773 lines of maintenance burden.
Consider splitting internal result messages from external command messages.

- [x] ### TD-15. `canonicalize_path` duplicated in two locations [Low]
**Files:** `gitcomet-state/src/store/reducer/util.rs:247-265`,
`gitcomet-state/src/store/repo_monitor.rs:108-126`

Identical `canonicalize_path` and `strip_windows_verbatim_prefix` implementations.

- [x] ### TD-16. `with_alpha` helper duplicated in 3 places [Low]
**Files:** `gitcomet-ui-gpui/src/theme.rs:141`, `focused_diff.rs:244`, `view/color.rs`

- [x] ### TD-17. Six identical empty `DragGhost` render structs [Low]
**File:** `gitcomet-ui-gpui/src/view/mod_helpers.rs:134-219`

All have identical `Render` implementations returning `div()`. Should be a single type.

- [x] ### TD-18. Dead code: `_workdir` field, `GitOpMode` variants, `use_legacy_constructor` [Low]
**Files:** `gitcomet-git-gix/src/repo/mod.rs:31` (`_workdir` never read),
`gitcomet-git-gix/src/repo/git_ops.rs:9-15` (`GixOnly` and `CliOnly` never used),
`gitcomet-ui-gpui/src/app.rs:41, 147, 166-168` (`use_legacy_constructor` always false)

- [x] ### TD-19. `Submodule::status` uses raw `char` instead of enum [Low]
**File:** `gitcomet-core/src/domain.rs:100`

Loses type safety and self-documentation.

- [x] ### TD-20. Gitignore cache never has a TTL or max size [Low]
**File:** `gitcomet-state/src/store/repo_monitor.rs:266-297`

The `GitignoreRules` cache grows unboundedly as new file paths are observed. Only
invalidated when `.gitignore` itself changes, which resets it entirely.

- [x] ### TD-21. `#[allow(dead_code)]` annotations suggest unfinished features [Low]
**File:** `gitcomet-ui-gpui/src/view/mod_helpers.rs:450, 513, 518, 640`

Several `PopoverKind` variants and `ResolverPickTarget` variants have `#[allow(dead_code)]`.

---

## CI/CD & Build Configuration

- [x] ### CI-1. Main CI has no Rust caching [High]
**File:** `.github/workflows/rust.yml`

No `Swatinem/rust-cache` or `actions/cache` step. Every run compiles everything from
scratch (5-15+ min of compilation). The cross-platform workflow correctly uses caching.

- [~] ### CI-2. No Rust toolchain action in main CI [High]
> **Skipped:** tech debt item CI-2 is already implemented in `/home/sampo/git/GitComet2/.github/workflows/rust.yml` (Rust toolchain action is present in all Rust jobs).
**File:** `.github/workflows/rust.yml`

Relies on pre-installed Rust on GitHub runners. Runner image updates can silently break
builds. The cross-platform workflow correctly uses `dtolnay/rust-toolchain@stable`.

- [x] ### CI-3. No `cargo fmt --check` in CI [Medium]
**Files:** Both CI workflows

No formatting enforcement and no `.rustfmt.toml`.

- [x] ### CI-4. No concurrency control in main CI [Medium]
**File:** `.github/workflows/rust.yml`

Rapid pushes spawn parallel workflows that all build from scratch. The cross-platform
workflow correctly has `concurrency: cancel-in-progress: true`.

- [x] ### CI-5. No `timeout-minutes` on any job [Medium]
**Files:** Both CI workflows

A hung benchmark or test runs until the 6-hour GitHub Actions default.

- [x] ### CI-6. Clippy coverage gap [Medium]
**File:** `.github/workflows/rust.yml`

Does not lint `gitcomet-git`, `gitcomet-ui`, or `gitcomet-ui-gpui` (the full GPUI UI crate
is understandably hard to lint on headless CI, but the gap should be acknowledged).

- [x] ### CI-7. No `[profile.release]` optimizations [Medium]
**File:** `Cargo.toml`

Release builds use Cargo defaults: `lto = false`, `codegen-units = 256`, `strip = "none"`.
Adding `lto = "thin"`, `codegen-units = 1`, `strip = "symbols"` would yield smaller/faster
binaries.

- [x] ### CI-8. `build.sh` and `valgrind_cdp` are not proper scripts [Low]
**File:** `scripts/build.sh`, `scripts/valgrind_cdp`

No shebang line, not executable as standalone scripts. `valgrind_cdp` reads more like
personal notes.

- [x] ### CI-9. `ubuntu-latest` overlaps with `ubuntu-24.04` in CI matrix [Low]
**File:** `.github/workflows/cross-platform-tests.yml`

`ubuntu-latest` currently maps to `ubuntu-24.04`, so two jobs run on the same OS.

- [x] ### CI-10. Benchmarks on shared runners [Low]
**File:** `.github/workflows/rust.yml`

Performance benchmarks on `ubuntu-latest` shared runners are inherently noisy. The job is
labeled "alert-only" which mitigates this.

- [x] ### CI-11. Test warning noise from unused imports [Low]
**Files:** `gitcomet-app/tests/difftool_git_integration.rs:4`,
`gitcomet-app/tests/standalone_tool_mode_integration.rs:5`,
`gitcomet-app/tests/mergetool_git_integration.rs:5`,
`gitcomet-git-gix/tests/{log_integration.rs:5, status_integration.rs:11,
submodules_integration.rs:6, upstream_integration.rs:6}`

Test runs produce warning noise from unused imports.

---

## Dependency Concerns

- [x] ### DEP-1. Duplicate `resvg` versions compiled [Medium]
**File:** `Cargo.lock`

Two versions compiled: v0.45.1 (transitive via gpui) and v0.47.0 (direct). Two complete
SVG rendering stacks are linked into the binary.

- [x] ### DEP-2. `gix` / `gix-diff` version coupling risk [Low]
**File:** `Cargo.toml`

`gix = "0.80"` and `gix-diff = "0.60"` specified independently. The gix ecosystem has
tightly coupled versions; bumping one without the other risks skew. Consider whether
`gix-diff` can be re-exported from `gix` via feature flags.

- [x] ### DEP-3. Inconsistent tree-sitter grammar version pinning [Low]
**File:** `Cargo.toml`

Mix of pinned (`html = "0.23.2"`) and floating (`go = "0.25"`) versions. Some will
auto-upgrade on `cargo update`, others won't.

- [x] ### DEP-4. `tag.gpgsign` silently overridden [Low]
**File:** `gitcomet-git-gix/src/repo/tags.rs:92-107`

`create_tag_with_output_impl` disables GPG signing via `-c tag.gpgsign=false`, which may
violate user expectations if they have signing enabled globally.

- [x] ### DEP-5. Inconsistent `HashSet` vs `FxHashSet` usage [Low]
**File:** Multiple files in `gitcomet-git-gix`

`rustc-hash` is a dependency but `std::collections::HashSet` is used in some files while
`FxHashSet` in others. Should be consistent.

---

## Summary

| Category             | High | Medium | Low |
|----------------------|------|--------|-----|
| Security             | 4    | 7      | 3   |
| Bugs & Defects       | 1    | 5      | 4   |
| Performance          | 4    | 11     | 2   |
| Cross-Platform       | 2    | 2      | 6   |
| Technical Debt       | 2    | 11     | 8   |
| CI/CD & Build        | 2    | 5      | 4   |
| Dependencies         | 0    | 1      | 4   |
| **Total**            | **15** | **42** | **31** |

### Top 5 Recommendations (highest impact)

1. **Wrap worker tasks in `catch_unwind`** -- prevents permanent worker thread loss from any
   panicking task, improving reliability of all background operations.

2. **Add `cargo-audit` to CI and pin Rust toolchain** -- catches known CVEs and prevents
   build breakage from runner image updates. Quick wins for the CI pipeline.

3. **Add configurable size threshold to Myers diff** -- prevents OOM on large files. Fall
   back to delete-all/insert-all or a linear-space variant above a threshold.

4. **Parse files once with tree-sitter** -- replaces 1000 per-line parser invocations with
   one per-file parse. Fixes multi-line construct highlighting and significantly improves
   rendering performance.

5. **Batch gitignore checks** -- replace per-path subprocess spawning with a single batched
   `git check-ignore` call or a library-based matcher, eliminating the biggest subprocess
   overhead.
