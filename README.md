## GitGpui

Fast, resource-efficient, fully open source Git GUI written in Rust, targeting GitKraken/SourceTree/GitHub Desktop-class workflows using `gpui` for the UI.

### Goals

- Pure Rust Git backend (recommended: `gix`/gitoxide backend).
- Fast UI for very large repositories (virtualized lists, incremental loading, caching).
- Modular architecture with clear boundaries, to support benchmarking and testing.

### Workspace layout

- `crates/gitgpui-core`: domain types + app/service boundaries (no heavy deps).
- `crates/gitgpui-git`: Git abstraction + no-op backend.
- `crates/gitgpui-git-gix`: `gix`/gitoxide backend implementation.
- `crates/gitgpui-state`: MVU state store + command execution scaffolding.
- `crates/gitgpui-ui`: UI model/state (toolkit-independent).
- `crates/gitgpui-ui-gpui`: gpui views/components.
- `crates/gitgpui-app`: binary entrypoint wiring everything together.

### Getting started

Offline-friendly default build (does not build the UI or the Git backend):

```bash
cargo build
```

To build the actual app you’ll enable features (requires network for dependencies):

```bash
cargo build -p gitgpui-app --features ui,gix
```

To also compile the gpui-based UI crate:

```bash
cargo build -p gitgpui-app --features ui-gpui,gix
```

Run (opens the repo passed as the first arg, or falls back to the current directory):

```bash
cargo run -p gitgpui-app --features ui-gpui,gix -- /path/to/repo
```

### Crash logs

If the app crashes due to a Rust panic, GitGpui writes a crash log to:

- Linux: `$XDG_STATE_HOME/gitgpui/crashes/` (fallback: `~/.local/state/gitgpui/crashes/`)
- macOS: `~/Library/Logs/gitgpui/crashes/`
- Windows: `%LOCALAPPDATA%\\gitgpui\\crashes\\` (fallback: `%APPDATA%\\gitgpui\\crashes\\`)

### Roadmap (high level)

- Open repositories; show status + commit history timeline.
- Branch/remote tracking; pull/push; fetch with progress.
- Stash create/apply/drop; discard changes; stage/unstage.
- Visualize branch/merge topology from refs (commit graph lanes).
- Benchmarks for log/graph/status/diff on large repos.
