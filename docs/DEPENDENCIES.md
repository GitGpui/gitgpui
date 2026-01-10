# Dependency notes

## `gpui` on Linux (ashpd + zvariant)

At the time of bootstrapping this repo, `gpui` pulls in `ashpd` (directly and via `oo7`) and the crates.io releases of `ashpd` require a small compatibility fix with newer `zvariant` versions.

We vendor patched copies of:

- `vendor/ashpd-0.11.0`
- `vendor/ashpd-0.12.0`

and use Cargo `[replace]` in `Cargo.toml` to make `--features ui-gpui` compile.

## Async runtime

For now, `gitgpui-state` uses a small `std::thread` worker pool to execute effects. When we wire actual gpui views, we can switch to `gpui::Context::spawn` or a dedicated async runtime behind a feature flag without changing the UI→state message flow.
