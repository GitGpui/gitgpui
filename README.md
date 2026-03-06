## GitComet

Fast, resource-efficient, fully open source Git GUI written in Rust, targeting GitKraken/SourceTree/GitHub Desktop-class workflows using `gpui` for the UI.

### Goals

- Pure Rust Git backend (recommended: `gix`/gitoxide backend).
- Fast UI for very large repositories (virtualized lists, incremental loading, caching).
- Modular architecture with clear boundaries, to support benchmarking and testing.
- Drop-in replacement for `git difftool` and `git mergetool` with CLI compatibility for Meld and KDiff3.

### Workspace layout

- `crates/gitcomet-core`: domain types, merge algorithm, conflict session, text utils.
- `crates/gitcomet-git`: Git abstraction + no-op backend.
- `crates/gitcomet-git-gix`: `gix`/gitoxide backend implementation.
- `crates/gitcomet-state`: MVU state store, reducers, effects, conflict session management.
- `crates/gitcomet-ui`: UI model/state (toolkit-independent).
- `crates/gitcomet-ui-gpui`: gpui views/components (focused diff/merge windows, conflict resolver, word diff).
- `crates/gitcomet-app`: binary entrypoint, CLI (clap), difftool/mergetool/setup modes.

### Getting started

Offline-friendly default build (does not build the UI or the Git backend):

```bash
cargo build
```

To build the actual app you'll enable features (requires network for dependencies):

```bash
cargo build -p gitcomet-app --features ui,gix
```

To also compile the gpui-based UI crate:

```bash
cargo build -p gitcomet-app --features ui-gpui,gix
```

Run (opens the repo passed as the first arg, or falls back to the current directory):

```bash
cargo run -p gitcomet-app --features ui-gpui,gix -- /path/to/repo
```

### Using as a Git difftool / mergetool

GitComet can be used as a standalone diff and merge tool invoked by `git difftool` and `git mergetool`. It supports both headless (algorithm-only) and GUI (interactive GPUI window) modes.

#### Automatic setup

The built-in `setup` command configures Git globally:

```bash
gitcomet-app setup
```

This registers both headless and GUI tool variants with `guiDefault=auto`, so Git automatically picks the GUI tool when a display is available and falls back to headless otherwise.

#### Manual setup

```bash
GITGPUI_BIN="/absolute/path/to/gitcomet-app"

# Headless tool — algorithm-only merge/diff for CI, scripts, and no-display environments
git config --global merge.tool gitcomet
git config --global mergetool.gitcomet.cmd \
  "'$GITGPUI_BIN' mergetool --base \"\$BASE\" --local \"\$LOCAL\" --remote \"\$REMOTE\" --merged \"\$MERGED\""
git config --global mergetool.gitcomet.trustExitCode true
git config --global mergetool.prompt false

git config --global diff.tool gitcomet
git config --global difftool.gitcomet.cmd \
  "'$GITGPUI_BIN' difftool --local \"\$LOCAL\" --remote \"\$REMOTE\" --path \"\$MERGED\""
git config --global difftool.gitcomet.trustExitCode true
git config --global difftool.prompt false

# GUI tool — opens focused GPUI windows for interactive diff/merge
git config --global merge.guitool gitcomet-gui
git config --global mergetool.gitcomet-gui.cmd \
  "'$GITGPUI_BIN' mergetool --gui --base \"\$BASE\" --local \"\$LOCAL\" --remote \"\$REMOTE\" --merged \"\$MERGED\""
git config --global mergetool.gitcomet-gui.trustExitCode true

git config --global diff.guitool gitcomet-gui
git config --global difftool.gitcomet-gui.cmd \
  "'$GITGPUI_BIN' difftool --gui --local \"\$LOCAL\" --remote \"\$REMOTE\" --path \"\$MERGED\""
git config --global difftool.gitcomet-gui.trustExitCode true

# Auto-select GUI tool when DISPLAY is available, headless otherwise
git config --global mergetool.guiDefault auto
git config --global difftool.guiDefault auto
```

#### CLI modes

**Difftool:**

```bash
gitcomet-app difftool --local <path> --remote <path> [--path <display_name>] [--label-left <label>] [--label-right <label>]
```

Also reads `LOCAL`/`REMOTE` from environment as a fallback when invoked by Git.

**Mergetool:**

```bash
gitcomet-app mergetool --local <path> --remote <path> --merged <path> [--base <path>] [--label-local <label>] [--label-remote <label>] [--label-base <label>]
```

Also reads `LOCAL`/`REMOTE`/`MERGED`/`BASE` from environment. Base is optional for add/add conflicts.

#### Compatibility

KDiff3 and Meld invocation forms are supported (`--L1/--L2/--L3`, `-o/--output/--out`, `--base`, positional arguments), so GitComet can be a drop-in replacement.

#### Exit codes

| Code | Meaning |
|------|---------|
| `0`  | User completed the action and the result was saved |
| `1`  | User canceled or closed with unresolved result |
| `>=2`| Input, I/O, or internal error |

### Testing

Full headless test suite (CI mode):

```bash
cargo test --workspace --no-default-features --features gix
```

Clippy (CI mode):

```bash
cargo clippy --workspace --no-default-features --features gix -- -D warnings
```

The test suite covers:

- Core merge algorithm (ported from Git t6403/t6427)
- KDiff3-style fixture harness with permutation corpus
- Meld algorithm parity tests
- Git mergetool and difftool E2E integration
- Standalone tool mode (CLI arg/env parsing, compatibility forms, exit codes)
- State management (reducers, effects, conflict sessions)
- UI components (focused merge/diff windows, word diff, conflict resolver)

### Profiling (Callgrind)

To profile the app with Valgrind Callgrind (interactive on/off instrumentation):

```bash
bash scripts/profile-callgrind.sh --open -- /path/to/repo
```

### Crash logs

If the app crashes due to a Rust panic, GitComet writes a crash log to:

- Linux: `$XDG_STATE_HOME/gitcomet/crashes/` (fallback: `~/.local/state/gitcomet/crashes/`)
- macOS: `~/Library/Logs/gitcomet/crashes/`
- Windows: `%LOCALAPPDATA%\gitcomet\crashes\` (fallback: `%APPDATA%\gitcomet\crashes\`)

### Roadmap (high level)

- Open repositories; show status + commit history timeline.
- Branch/remote tracking; pull/push; fetch with progress.
- Stash create/apply/drop; discard changes; stage/unstage.
- Visualize branch/merge topology from refs (commit graph lanes).
- Benchmarks for log/graph/status/diff on large repos.
