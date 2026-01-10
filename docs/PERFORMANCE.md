# Performance guidelines

- Prefer **paged history** (`limit`, `cursor`) over full rev-walks.
- Cache by **commit id** and **ref tip id**, not by branch name.
- Use stable, immutable snapshots for UI, and update incrementally.
- Defer expensive work (diff/graph layout) to background tasks and stream results.
- Measure hot paths: `open`, `status`, `log page`, `graph page`, `diff`.

