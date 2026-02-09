use gitgpui_core::domain::*;
use gitgpui_core::services::BlameLine;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

pub type Shared<T> = Arc<T>;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RepoLoadsInFlight {
    in_flight: u32,
    pending: u32,
    pending_log: Option<PendingLogLoad>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingLogLoad {
    pub scope: LogScope,
    pub limit: usize,
    pub cursor: Option<LogCursor>,
}

impl RepoLoadsInFlight {
    pub const HEAD_BRANCH: u32 = 1 << 0;
    pub const UPSTREAM_DIVERGENCE: u32 = 1 << 1;
    pub const BRANCHES: u32 = 1 << 2;
    pub const TAGS: u32 = 1 << 3;
    pub const REMOTES: u32 = 1 << 4;
    pub const REMOTE_BRANCHES: u32 = 1 << 5;
    pub const STATUS: u32 = 1 << 6;
    pub const STASHES: u32 = 1 << 7;
    pub const REFLOG: u32 = 1 << 8;
    pub const REBASE_STATE: u32 = 1 << 9;
    pub const LOG: u32 = 1 << 10;

    pub fn is_in_flight(&self, flag: u32) -> bool {
        (self.in_flight & flag) != 0
    }

    /// For non-log loads: starts immediately if not in flight, otherwise coalesces by remembering
    /// one pending refresh for the same kind.
    pub fn request(&mut self, flag: u32) -> bool {
        if self.is_in_flight(flag) {
            self.pending |= flag;
            false
        } else {
            self.in_flight |= flag;
            true
        }
    }

    /// For non-log loads: finishes and indicates whether a pending request should be scheduled now.
    pub fn finish(&mut self, flag: u32) -> bool {
        self.in_flight &= !flag;
        if (self.pending & flag) != 0 {
            self.pending &= !flag;
            self.in_flight |= flag;
            true
        } else {
            false
        }
    }

    /// For log loads: coalesce by keeping only the latest requested `(scope, cursor)` while a log
    /// load is already in flight.
    pub fn request_log(
        &mut self,
        scope: LogScope,
        limit: usize,
        cursor: Option<LogCursor>,
    ) -> bool {
        if self.is_in_flight(Self::LOG) {
            let next = PendingLogLoad {
                scope,
                limit,
                cursor,
            };
            match &self.pending_log {
                // Scope changes invalidate older pending requests (including pagination).
                Some(existing) if existing.scope != next.scope => {
                    self.pending_log = Some(next);
                }
                // Don't let a refresh request (cursor=None) clobber a pending pagination request
                // for the same scope.
                Some(existing) if existing.cursor.is_some() && next.cursor.is_none() => {}
                _ => {
                    self.pending_log = Some(next);
                }
            }
            false
        } else {
            self.in_flight |= Self::LOG;
            true
        }
    }

    pub fn finish_log(&mut self) -> Option<PendingLogLoad> {
        self.in_flight &= !Self::LOG;
        if let Some(next) = self.pending_log.take() {
            self.in_flight |= Self::LOG;
            Some(next)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConflictFile {
    pub path: PathBuf,
    pub ours: Option<String>,
    pub theirs: Option<String>,
    pub current: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct AppState {
    pub repos: Vec<RepoState>,
    pub active_repo: Option<RepoId>,
    pub clone: Option<CloneOpState>,
    pub notifications: Vec<AppNotification>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppNotification {
    pub time: SystemTime,
    pub kind: AppNotificationKind,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppNotificationKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CloneOpState {
    pub url: String,
    pub dest: PathBuf,
    pub status: CloneOpStatus,
    pub seq: u64,
    pub output_tail: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CloneOpStatus {
    Running,
    FinishedOk,
    FinishedErr(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandLogEntry {
    pub time: SystemTime,
    pub ok: bool,
    pub command: String,
    pub summary: String,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Clone, Debug)]
pub struct RepoState {
    pub id: RepoId,
    pub spec: RepoSpec,
    pub loads_in_flight: RepoLoadsInFlight,
    pub pull_in_flight: u32,
    pub push_in_flight: u32,

    pub open: Loadable<()>,
    pub history_scope: LogScope,
    pub head_branch: Loadable<String>,
    pub head_branch_rev: u64,
    pub upstream_divergence: Loadable<Option<UpstreamDivergence>>,
    pub branches: Loadable<Vec<Branch>>,
    pub branches_rev: u64,
    pub tags: Loadable<Vec<Tag>>,
    pub tags_rev: u64,
    pub remotes: Loadable<Vec<Remote>>,
    pub remotes_rev: u64,
    pub remote_branches: Loadable<Vec<RemoteBranch>>,
    pub remote_branches_rev: u64,
    pub status: Loadable<Shared<RepoStatus>>,
    pub log: Loadable<Shared<LogPage>>,
    pub log_loading_more: bool,
    pub stashes: Loadable<Vec<StashEntry>>,
    pub stashes_rev: u64,
    pub reflog: Loadable<Vec<ReflogEntry>>,
    pub rebase_in_progress: Loadable<bool>,
    pub file_history_path: Option<PathBuf>,
    pub file_history: Loadable<Shared<LogPage>>,
    pub blame_path: Option<PathBuf>,
    pub blame_rev: Option<String>,
    pub blame: Loadable<Shared<Vec<BlameLine>>>,
    pub worktrees: Loadable<Vec<Worktree>>,
    pub submodules: Loadable<Vec<Submodule>>,

    pub selected_commit: Option<CommitId>,
    pub commit_details: Loadable<Shared<CommitDetails>>,
    pub diff_target: Option<DiffTarget>,
    pub diff_rev: u64,
    pub diff: Loadable<Shared<Diff>>,
    pub diff_file_rev: u64,
    pub diff_file: Loadable<Option<FileDiffText>>,
    pub diff_file_image: Loadable<Option<FileDiffImage>>,

    pub conflict_file_path: Option<PathBuf>,
    pub conflict_file: Loadable<Option<ConflictFile>>,

    pub last_error: Option<String>,
    pub diagnostics: Vec<DiagnosticEntry>,

    pub command_log: Vec<CommandLogEntry>,
}

impl RepoState {
    pub fn new_opening(id: RepoId, spec: RepoSpec) -> Self {
        Self {
            id,
            spec,
            loads_in_flight: RepoLoadsInFlight::default(),
            pull_in_flight: 0,
            push_in_flight: 0,
            open: Loadable::Loading,
            history_scope: LogScope::CurrentBranch,
            head_branch: Loadable::NotLoaded,
            head_branch_rev: 0,
            upstream_divergence: Loadable::NotLoaded,
            branches: Loadable::NotLoaded,
            branches_rev: 0,
            tags: Loadable::NotLoaded,
            tags_rev: 0,
            remotes: Loadable::NotLoaded,
            remotes_rev: 0,
            remote_branches: Loadable::NotLoaded,
            remote_branches_rev: 0,
            status: Loadable::NotLoaded,
            log: Loadable::NotLoaded,
            log_loading_more: false,
            stashes: Loadable::NotLoaded,
            stashes_rev: 0,
            reflog: Loadable::NotLoaded,
            rebase_in_progress: Loadable::NotLoaded,
            file_history_path: None,
            file_history: Loadable::NotLoaded,
            blame_path: None,
            blame_rev: None,
            blame: Loadable::NotLoaded,
            worktrees: Loadable::NotLoaded,
            submodules: Loadable::NotLoaded,
            selected_commit: None,
            commit_details: Loadable::NotLoaded,
            diff_target: None,
            diff_rev: 0,
            diff: Loadable::NotLoaded,
            diff_file_rev: 0,
            diff_file: Loadable::NotLoaded,
            diff_file_image: Loadable::NotLoaded,
            conflict_file_path: None,
            conflict_file: Loadable::NotLoaded,
            last_error: None,
            diagnostics: Vec::new(),
            command_log: Vec::new(),
        }
    }

    pub(crate) fn set_head_branch(&mut self, head_branch: Loadable<String>) {
        self.head_branch = head_branch;
        self.head_branch_rev = self.head_branch_rev.wrapping_add(1);
    }

    pub(crate) fn set_branches(&mut self, branches: Loadable<Vec<Branch>>) {
        self.branches = branches;
        self.branches_rev = self.branches_rev.wrapping_add(1);
    }

    pub(crate) fn set_tags(&mut self, tags: Loadable<Vec<Tag>>) {
        self.tags = tags;
        self.tags_rev = self.tags_rev.wrapping_add(1);
    }

    pub(crate) fn set_remotes(&mut self, remotes: Loadable<Vec<Remote>>) {
        self.remotes = remotes;
        self.remotes_rev = self.remotes_rev.wrapping_add(1);
    }

    pub(crate) fn set_remote_branches(&mut self, remote_branches: Loadable<Vec<RemoteBranch>>) {
        self.remote_branches = remote_branches;
        self.remote_branches_rev = self.remote_branches_rev.wrapping_add(1);
    }

    pub(crate) fn set_stashes(&mut self, stashes: Loadable<Vec<StashEntry>>) {
        self.stashes = stashes;
        self.stashes_rev = self.stashes_rev.wrapping_add(1);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticEntry {
    pub time: SystemTime,
    pub kind: DiagnosticKind,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticKind {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct RepoId(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Loadable<T> {
    NotLoaded,
    Loading,
    Ready(T),
    Error(String),
}

impl<T> Loadable<T> {
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn app_state_clone_shares_heavy_repo_fields_via_arc() {
        let mut state = AppState::default();
        state.repos.push(RepoState::new_opening(
            RepoId(1),
            RepoSpec {
                workdir: PathBuf::from("/tmp/repo"),
            },
        ));

        let repo = &mut state.repos[0];
        repo.status = Loadable::Ready(Arc::new(RepoStatus::default()));
        repo.log = Loadable::Ready(Arc::new(LogPage {
            commits: vec![Commit {
                id: CommitId("c1".to_string()),
                parent_ids: Vec::new(),
                summary: "s1".to_string(),
                author: "a".to_string(),
                time: SystemTime::UNIX_EPOCH,
            }],
            next_cursor: None,
        }));
        repo.file_history = Loadable::Ready(Arc::new(LogPage {
            commits: Vec::new(),
            next_cursor: None,
        }));
        repo.blame = Loadable::Ready(Arc::new(vec![BlameLine {
            commit_id: "c1".to_string(),
            author: "a".to_string(),
            author_time_unix: None,
            summary: "s1".to_string(),
            line: "line".to_string(),
        }]));
        repo.commit_details = Loadable::Ready(Arc::new(CommitDetails {
            id: CommitId("c1".to_string()),
            message: "m".to_string(),
            committed_at: "t".to_string(),
            parent_ids: Vec::new(),
            files: Vec::new(),
        }));
        repo.diff = Loadable::Ready(Arc::new(Diff {
            target: DiffTarget::Commit {
                commit_id: CommitId("c1".to_string()),
                path: None,
            },
            lines: Vec::new(),
        }));

        let cloned = state.clone();

        let repo1 = &state.repos[0];
        let repo2 = &cloned.repos[0];

        let Loadable::Ready(status1) = &repo1.status else {
            panic!("expected status ready");
        };
        let Loadable::Ready(status2) = &repo2.status else {
            panic!("expected status ready");
        };
        assert!(Arc::ptr_eq(status1, status2));
        assert_eq!(Arc::strong_count(status1), 2);

        let Loadable::Ready(log1) = &repo1.log else {
            panic!("expected log ready");
        };
        let Loadable::Ready(log2) = &repo2.log else {
            panic!("expected log ready");
        };
        assert!(Arc::ptr_eq(log1, log2));
        assert_eq!(Arc::strong_count(log1), 2);

        let Loadable::Ready(diff1) = &repo1.diff else {
            panic!("expected diff ready");
        };
        let Loadable::Ready(diff2) = &repo2.diff else {
            panic!("expected diff ready");
        };
        assert!(Arc::ptr_eq(diff1, diff2));
        assert_eq!(Arc::strong_count(diff1), 2);
    }
}
