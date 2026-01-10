# Benchmarks

We care about regressions on large repositories. The goal is repeatable, automated measurements for hot paths.

## Hot paths to benchmark

- `open`: open repo + read config/refs
- `status`: baseline worktree status on cold/hot cache
- `log_page`: N commits from `HEAD` (cold/hot)
- `graph_page`: lane layout for N commits
- `diff`: file diff and hunk enumeration
- `fetch/push`: protocol overhead + progress updates (separately)

## Principles

- Bench results should be stable and avoid measuring UI rendering.
- Keep “backend” benchmarks separate from UI to make regressions actionable.
- Track memory/allocations for `log_page` and `graph_page`.

