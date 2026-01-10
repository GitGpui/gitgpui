# Roadmap

This is an intentionally staged plan to reach “daily-driver” parity with common Git GUI workflows while keeping the codebase modular and fast.

## Phase 1: Repository open + timeline (MVP)

- Open a repository by path (recent list).
- Show working tree status (tracked/untracked/modified/conflicts).
- Show commit history timeline for `HEAD` with paging.
- Basic branch list + checkout.

## Phase 2: Branch graph + refs

- Render commit graph lanes for visible commits.
- Show local branches + remote-tracking branches in the same view.
- Show merge bases and diverged/ahead/behind info.

## Phase 3: Remote workflows

- Fetch with progress and update remote-tracking refs.
- Pull (fast-forward / merge / rebase options).
- Push, including creating upstream tracking.
- Credential handling (platform-appropriate, minimal UX friction).

## Phase 4: Changes + safety workflows

- Stage/unstage files and hunks.
- Diff viewer (unified + split).
- Discard changes (file/hunk) with clear confirmation UX.
- Stash create/list/apply/drop (including untracked).

## Phase 5: Power-user and scale

- Search/filters (author, path, message).
- Background indexing and caches with invalidation.
- Large-repo performance profiling and regression benchmarks.

