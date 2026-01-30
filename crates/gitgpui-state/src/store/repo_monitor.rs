use crate::model::RepoId;
use crate::msg::{Msg, RepoExternalChange};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DebouncedChange {
    pending: Option<RepoExternalChange>,
    first_event_at: Option<Instant>,
    last_event_at: Option<Instant>,
    debounce: Duration,
    max_delay: Duration,
}

impl DebouncedChange {
    fn new(debounce: Duration, max_delay: Duration) -> Self {
        Self {
            pending: None,
            first_event_at: None,
            last_event_at: None,
            debounce,
            max_delay,
        }
    }

    fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    fn push(&mut self, change: RepoExternalChange, now: Instant) -> Option<RepoExternalChange> {
        self.pending = Some(merge_change(self.pending.unwrap_or(change), change));
        self.first_event_at.get_or_insert(now);
        self.last_event_at = Some(now);
        self.take_if_max_delay_elapsed(now)
    }

    fn take_if_max_delay_elapsed(&mut self, now: Instant) -> Option<RepoExternalChange> {
        let Some(first) = self.first_event_at else {
            return None;
        };
        if now.duration_since(first) >= self.max_delay {
            self.take()
        } else {
            None
        }
    }

    fn next_timeout(&self, now: Instant) -> Option<Duration> {
        let (first, last) = (self.first_event_at?, self.last_event_at?);
        let due_by_debounce = last + self.debounce;
        let due_by_max = first + self.max_delay;
        let due = if due_by_debounce <= due_by_max {
            due_by_debounce
        } else {
            due_by_max
        };
        Some(due.saturating_duration_since(now))
    }

    fn take_if_due(&mut self, now: Instant) -> Option<RepoExternalChange> {
        if !self.is_pending() {
            return None;
        }
        let timeout = self.next_timeout(now).unwrap_or(Duration::from_secs(0));
        if timeout.is_zero() {
            self.take()
        } else {
            None
        }
    }

    fn take(&mut self) -> Option<RepoExternalChange> {
        let pending = self.pending.take();
        self.first_event_at = None;
        self.last_event_at = None;
        pending
    }
}

pub(super) struct RepoMonitorManager {
    handles: HashMap<RepoId, RepoMonitorHandle>,
}

impl RepoMonitorManager {
    pub(super) fn new() -> Self {
        Self {
            handles: HashMap::new(),
        }
    }

    pub(super) fn stop_all(&mut self) {
        let repo_ids = self.handles.keys().copied().collect::<Vec<_>>();
        for repo_id in repo_ids {
            self.stop(repo_id);
        }
    }

    pub(super) fn stop(&mut self, repo_id: RepoId) {
        let Some(handle) = self.handles.remove(&repo_id) else {
            return;
        };
        let _ = handle.stop_tx.send(());
        let _ = handle.join.join();
    }

    pub(super) fn running_repo_ids(&self) -> Vec<RepoId> {
        self.handles.keys().copied().collect()
    }

    pub(super) fn start(
        &mut self,
        repo_id: RepoId,
        workdir: PathBuf,
        msg_tx: mpsc::Sender<Msg>,
        active_repo_id: Arc<AtomicU64>,
    ) {
        if self.handles.contains_key(&repo_id) {
            return;
        }
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let join = thread::spawn(move || {
            repo_monitor_thread(repo_id, workdir, msg_tx, stop_rx, active_repo_id)
        });
        self.handles.insert(repo_id, RepoMonitorHandle { stop_tx, join });
    }
}

struct RepoMonitorHandle {
    stop_tx: mpsc::Sender<()>,
    join: thread::JoinHandle<()>,
}

fn repo_monitor_thread(
    repo_id: RepoId,
    workdir: PathBuf,
    msg_tx: mpsc::Sender<Msg>,
    stop_rx: mpsc::Receiver<()>,
    active_repo_id: Arc<AtomicU64>,
) {
    let workdir = workdir.canonicalize().unwrap_or(workdir);
    let git_dir = resolve_git_dir(&workdir);

    let (event_tx, event_rx) = mpsc::channel::<notify::Result<notify::Event>>();

    let watcher = notify::recommended_watcher({
        let event_tx = event_tx.clone();
        move |res| {
            let _ = event_tx.send(res);
        }
    });

    let mut watcher: RecommendedWatcher = match watcher {
        Ok(w) => w,
        Err(_) => return,
    };

    if watcher
        .watch(&workdir, RecursiveMode::Recursive)
        .or_else(|_| watcher.watch(&workdir, RecursiveMode::NonRecursive))
        .is_err()
    {
        return;
    }

    if let Some(git_dir) = &git_dir {
        let _ = watcher
            .watch(git_dir, RecursiveMode::Recursive)
            .or_else(|_| watcher.watch(git_dir, RecursiveMode::NonRecursive));
    }

    let debounce = Duration::from_millis(250);
    let max_delay = Duration::from_secs(2);
    let idle_tick = Duration::from_secs(30);

    let mut debouncer = DebouncedChange::new(debounce, max_delay);

    let flush = |change: RepoExternalChange| {
        if active_repo_id.load(Ordering::Relaxed) == repo_id.0 {
            let _ = msg_tx.send(Msg::RepoExternallyChanged { repo_id, change });
        }
    };

    let flush_if_active = |pending: Option<RepoExternalChange>| {
        if let Some(change) = pending
            && active_repo_id.load(Ordering::Relaxed) == repo_id.0
        {
            let _ = msg_tx.send(Msg::RepoExternallyChanged { repo_id, change });
        }
    };

    loop {
        if stop_rx.try_recv().is_ok() {
            break;
        }

        let now = Instant::now();
        let timeout = debouncer.next_timeout(now).unwrap_or(idle_tick);

        match event_rx.recv_timeout(timeout) {
            Ok(Ok(event)) => {
                let change = classify_repo_change(&workdir, git_dir.as_deref(), &event);
                let now = Instant::now();
                if let Some(to_flush) = debouncer.push(change, now) {
                    flush(to_flush);
                }
            }
            Ok(Err(_)) => {
                let now = Instant::now();
                if let Some(to_flush) = debouncer.push(RepoExternalChange::Both, now) {
                    flush(to_flush);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let now = Instant::now();
                flush_if_active(debouncer.take_if_due(now));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn resolve_git_dir(workdir: &Path) -> Option<PathBuf> {
    let dot_git = workdir.join(".git");
    let md = fs::metadata(&dot_git).ok()?;

    if md.is_dir() {
        return Some(dot_git);
    }

    if !md.is_file() {
        return None;
    }

    let contents = fs::read_to_string(&dot_git).ok()?;
    let line = contents.lines().next()?.trim();
    let gitdir = line.strip_prefix("gitdir:")?.trim();
    if gitdir.is_empty() {
        return None;
    }

    let path = PathBuf::from(gitdir);
    if path.is_absolute() {
        Some(path)
    } else {
        Some(workdir.join(path))
    }
}

fn merge_change(a: RepoExternalChange, b: RepoExternalChange) -> RepoExternalChange {
    use RepoExternalChange::*;
    match (a, b) {
        (Both, _) | (_, Both) => Both,
        (Worktree, GitState) | (GitState, Worktree) => Both,
        (Worktree, Worktree) => Worktree,
        (GitState, GitState) => GitState,
    }
}

fn classify_repo_change(
    workdir: &Path,
    git_dir: Option<&Path>,
    event: &notify::Event,
) -> RepoExternalChange {
    if event.paths.is_empty() {
        return RepoExternalChange::Both;
    }

    let mut saw_worktree = false;
    let mut saw_git = false;

    for path in &event.paths {
        if is_git_related_path(workdir, git_dir, path) {
            saw_git = true;
        } else {
            saw_worktree = true;
        }
        if saw_git && saw_worktree {
            return RepoExternalChange::Both;
        }
    }

    if saw_git {
        RepoExternalChange::GitState
    } else {
        RepoExternalChange::Worktree
    }
}

fn is_git_related_path(workdir: &Path, git_dir: Option<&Path>, path: &Path) -> bool {
    let dot_git = workdir.join(".git");
    if path == dot_git || path.starts_with(&dot_git) {
        return true;
    }
    git_dir.is_some_and(|git_dir| path.starts_with(git_dir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;
    use notify::EventKind;

    #[test]
    fn resolve_git_dir_handles_dot_git_directory() {
        let dir = std::env::temp_dir().join(format!(
            "gitgpui-monitor-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let workdir = dir.join("repo");
        let _ = fs::create_dir_all(workdir.join(".git"));

        assert_eq!(resolve_git_dir(&workdir), Some(workdir.join(".git")));
    }

    #[test]
    fn resolve_git_dir_parses_dot_git_file() {
        let dir = std::env::temp_dir().join(format!(
            "gitgpui-monitor-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let workdir = dir.join("repo");
        let gitdir = dir.join("actual-git-dir");
        let _ = fs::create_dir_all(&workdir);
        let _ = fs::create_dir_all(&gitdir);

        fs::write(workdir.join(".git"), format!("gitdir: {}\n", gitdir.display()))
            .expect("write .git file");

        assert_eq!(resolve_git_dir(&workdir), Some(gitdir));
    }

    #[test]
    fn merge_change_coalesces_to_both() {
        assert_eq!(
            merge_change(RepoExternalChange::Worktree, RepoExternalChange::GitState),
            RepoExternalChange::Both
        );
        assert_eq!(
            merge_change(RepoExternalChange::GitState, RepoExternalChange::Worktree),
            RepoExternalChange::Both
        );
        assert_eq!(
            merge_change(RepoExternalChange::Both, RepoExternalChange::Worktree),
            RepoExternalChange::Both
        );
        assert_eq!(
            merge_change(RepoExternalChange::GitState, RepoExternalChange::GitState),
            RepoExternalChange::GitState
        );
    }

    #[test]
    fn classify_repo_change_distinguishes_gitdir_from_worktree() {
        let dir = std::env::temp_dir().join(format!(
            "gitgpui-monitor-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let workdir = dir.join("repo");
        let _ = fs::create_dir_all(workdir.join(".git"));

        let event = notify::Event {
            kind: EventKind::Any,
            paths: vec![workdir.join(".git").join("index")],
            attrs: Default::default(),
        };
        assert_eq!(
            classify_repo_change(&workdir, Some(&workdir.join(".git")), &event),
            RepoExternalChange::GitState
        );

        let event = notify::Event {
            kind: EventKind::Any,
            paths: vec![workdir.join("file.txt")],
            attrs: Default::default(),
        };
        assert_eq!(
            classify_repo_change(&workdir, Some(&workdir.join(".git")), &event),
            RepoExternalChange::Worktree
        );

        let event = notify::Event {
            kind: EventKind::Any,
            paths: vec![workdir.join(".git").join("HEAD"), workdir.join("file.txt")],
            attrs: Default::default(),
        };
        assert_eq!(
            classify_repo_change(&workdir, Some(&workdir.join(".git")), &event),
            RepoExternalChange::Both
        );
    }

    #[test]
    fn debouncer_flushes_on_debounce_or_max_delay() {
        let base = Instant::now();
        let mut d = DebouncedChange::new(Duration::from_millis(100), Duration::from_millis(250));

        assert_eq!(d.push(RepoExternalChange::Worktree, base), None);
        assert!(d.is_pending());

        // Another event resets debounce window.
        assert_eq!(
            d.push(RepoExternalChange::Worktree, base + Duration::from_millis(50)),
            None
        );
        assert!(d.next_timeout(base + Duration::from_millis(50)).is_some());

        // Not yet due at 149ms from base.
        assert_eq!(
            d.take_if_due(base + Duration::from_millis(149)),
            None
        );

        // Due by debounce at 150ms from base (last at 50ms + 100ms).
        assert_eq!(
            d.take_if_due(base + Duration::from_millis(150)),
            Some(RepoExternalChange::Worktree)
        );
        assert!(!d.is_pending());

        // Continuous events should flush by max_delay.
        assert_eq!(d.push(RepoExternalChange::GitState, base), None);
        assert_eq!(
            d.push(RepoExternalChange::GitState, base + Duration::from_millis(300)),
            Some(RepoExternalChange::GitState)
        );
        assert!(!d.is_pending());
    }
}
