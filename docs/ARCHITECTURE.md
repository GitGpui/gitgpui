# Architecture

GitGpui is structured as a small set of crates to keep hot paths testable and to avoid UI concerns leaking into Git logic.

## Design principles

- **Pure Rust** for Git operations (prefer `gix` backend).
- **Incremental** data access: paginate and cache; never load entire history at once.
- **Deterministic** performance: avoid accidental O(N) scans on UI interactions.
- **Clear seams**: all side effects behind traits so we can mock and benchmark.

## Crates

### `gitgpui-core`

Domain types (`Commit`, `Branch`, `Remote`, `RepoSpec`) and the service boundary traits used by the UI.

### `gitgpui-state`

Toolkit-independent app state store implementing a unidirectional “messages → reducer → effects” loop, plus a background task executor to keep the UI responsive.

### `gitgpui-git`

Provides the `GitBackend` selection helpers and a no-op backend (so the rest of the system compiles without external Git deps).

### `gitgpui-git-gix`

Concrete `GitBackend` implementation built on `gix`/gitoxide (pure Rust).

### `gitgpui-ui`

UI model/state that is independent of the concrete UI toolkit.

### `gitgpui-ui-gpui`

gpui views/components. Talks to `gitgpui-core` traits, not directly to `gix`.

### `gitgpui-app`

Entrypoint that selects a backend, wires services, and launches gpui.
