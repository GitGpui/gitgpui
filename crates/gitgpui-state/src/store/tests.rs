use super::*;
use crate::model::{CloneOpStatus, DiagnosticKind, Loadable, RepoState};
use crate::msg::Effect;
use gitgpui_core::domain::{
    Branch, Commit, CommitDetails, CommitId, DiffTarget, LogCursor, LogPage, LogScope, ReflogEntry,
    Remote, RemoteBranch, RepoSpec, RepoStatus, StashEntry,
};
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::{CommandOutput, PullMode, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

struct DummyRepo {
    spec: RepoSpec,
}

impl DummyRepo {
    fn new(path: &str) -> Self {
        Self {
            spec: RepoSpec {
                workdir: PathBuf::from(path),
            },
        }
    }
}

impl GitRepository for DummyRepo {
    fn spec(&self) -> &RepoSpec {
        &self.spec
    }

    fn log_head_page(&self, _limit: usize, _cursor: Option<&LogCursor>) -> Result<LogPage> {
        unimplemented!()
    }
    fn commit_details(&self, _id: &CommitId) -> Result<CommitDetails> {
        unimplemented!()
    }
    fn reflog_head(&self, _limit: usize) -> Result<Vec<ReflogEntry>> {
        unimplemented!()
    }
    fn current_branch(&self) -> Result<String> {
        unimplemented!()
    }
    fn list_branches(&self) -> Result<Vec<Branch>> {
        unimplemented!()
    }
    fn list_remotes(&self) -> Result<Vec<Remote>> {
        unimplemented!()
    }
    fn list_remote_branches(&self) -> Result<Vec<RemoteBranch>> {
        unimplemented!()
    }
    fn status(&self) -> Result<RepoStatus> {
        unimplemented!()
    }
    fn diff_unified(&self, _target: &DiffTarget) -> Result<String> {
        unimplemented!()
    }

    fn create_branch(&self, _name: &str, _target: &CommitId) -> Result<()> {
        unimplemented!()
    }
    fn delete_branch(&self, _name: &str) -> Result<()> {
        unimplemented!()
    }
    fn checkout_branch(&self, _name: &str) -> Result<()> {
        unimplemented!()
    }
    fn checkout_commit(&self, _id: &CommitId) -> Result<()> {
        unimplemented!()
    }
    fn cherry_pick(&self, _id: &CommitId) -> Result<()> {
        unimplemented!()
    }
    fn revert(&self, _id: &CommitId) -> Result<()> {
        unimplemented!()
    }

    fn stash_create(&self, _message: &str, _include_untracked: bool) -> Result<()> {
        unimplemented!()
    }
    fn stash_list(&self) -> Result<Vec<StashEntry>> {
        unimplemented!()
    }
    fn stash_apply(&self, _index: usize) -> Result<()> {
        unimplemented!()
    }
    fn stash_drop(&self, _index: usize) -> Result<()> {
        unimplemented!()
    }

    fn stage(&self, _paths: &[&Path]) -> Result<()> {
        unimplemented!()
    }
    fn unstage(&self, _paths: &[&Path]) -> Result<()> {
        unimplemented!()
    }
    fn commit(&self, _message: &str) -> Result<()> {
        unimplemented!()
    }
    fn fetch_all(&self) -> Result<()> {
        unimplemented!()
    }
    fn pull(&self, _mode: PullMode) -> Result<()> {
        unimplemented!()
    }
    fn push(&self) -> Result<()> {
        unimplemented!()
    }

    fn discard_worktree_changes(&self, _paths: &[&Path]) -> Result<()> {
        unimplemented!()
    }
}

fn run_git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .status()
        .expect("git command to run");
    assert!(status.success(), "git {:?} failed", args);
}

#[test]
fn open_repo_sets_opening_and_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::OpenRepo(PathBuf::from("/tmp/repo")),
    );

    assert_eq!(state.active_repo, Some(RepoId(1)));
    let repo_state = state.repos.first().expect("repo state to be set");
    assert_eq!(repo_state.id.0, 1);
    assert!(repo_state.open.is_loading());
    assert!(matches!(effects.as_slice(), [Effect::OpenRepo { .. }]));
}

#[test]
fn clone_repo_sets_running_state_and_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::CloneRepo {
            url: "file:///tmp/example.git".to_string(),
            dest: PathBuf::from("/tmp/example"),
        },
    );

    let op = state.clone.as_ref().expect("clone op set");
    assert!(matches!(op.status, CloneOpStatus::Running));
    assert_eq!(op.seq, 0);
    assert!(matches!(effects.as_slice(), [Effect::CloneRepo { .. }]));
}

#[test]
fn clone_repo_effect_clones_local_repo_and_emits_finished_and_open_repo() {
    struct Backend;
    impl GitBackend for Backend {
        fn open(&self, _path: &Path) -> std::result::Result<Arc<dyn GitRepository>, Error> {
            Err(Error::new(ErrorKind::Unsupported("test backend")))
        }
    }

    let base = std::env::temp_dir().join(format!(
        "gitgpui-clone-effect-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let _ = std::fs::create_dir_all(&base);

    let src = base.join("src");
    let dest = base.join("dest");
    let _ = std::fs::create_dir_all(&src);

    run_git(&src, &["init"]);
    run_git(&src, &["config", "user.email", "you@example.com"]);
    run_git(&src, &["config", "user.name", "You"]);
    run_git(&src, &["config", "commit.gpgsign", "false"]);
    std::fs::write(src.join("a.txt"), "one\n").unwrap();
    run_git(&src, &["add", "a.txt"]);
    run_git(&src, &["-c", "commit.gpgsign=false", "commit", "-m", "init"]);

    let executor = super::executor::TaskExecutor::new(1);
    let backend: Arc<dyn GitBackend> = Arc::new(Backend);
    let repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let (msg_tx, msg_rx) = std::sync::mpsc::channel::<Msg>();

    super::effects::schedule_effect(
        &executor,
        &backend,
        &repos,
        msg_tx,
        Effect::CloneRepo {
            url: src.display().to_string(),
            dest: dest.clone(),
        },
    );

    let start = Instant::now();
    let mut saw_finished_ok = false;
    let mut saw_open_repo = false;
    while start.elapsed() < Duration::from_secs(15) {
        let msg = match msg_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(m) => m,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(e) => panic!("channel closed: {e:?}"),
        };

        match msg {
            Msg::CloneRepoFinished {
                dest: finished_dest,
                result,
                ..
            } if finished_dest == dest => {
                assert!(result.is_ok(), "clone failed: {result:?}");
                saw_finished_ok = true;
            }
            Msg::OpenRepo(path) if path == dest => {
                saw_open_repo = true;
            }
            _ => {}
        }

        if saw_finished_ok && saw_open_repo {
            break;
        }
    }

    assert!(saw_finished_ok, "did not observe CloneRepoFinished");
    assert!(saw_open_repo, "did not observe OpenRepo after clone");
    assert!(dest.join(".git").exists(), "expected .git at cloned dest");
}

#[test]
fn close_repo_removes_and_moves_active() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(10);
    let mut state = AppState::default();

    reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::OpenRepo(PathBuf::from("/tmp/repo1")),
    );
    reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::OpenRepo(PathBuf::from("/tmp/repo2")),
    );

    assert_eq!(state.repos.len(), 2);
    assert_eq!(state.active_repo, Some(RepoId(11)));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::CloseRepo {
            repo_id: RepoId(11),
        },
    );

    assert!(effects.is_empty());
    assert_eq!(state.repos.len(), 1);
    assert_eq!(state.active_repo, Some(RepoId(10)));
}

#[test]
fn commit_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::Commit {
            repo_id: RepoId(1),
            message: "hello".to_string(),
        },
    );

    assert!(matches!(
        effects.as_slice(),
        [Effect::Commit { repo_id: RepoId(1), message } ] if message == "hello"
    ));
}

#[test]
fn reset_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::Reset {
            repo_id: RepoId(1),
            target: "HEAD~1".to_string(),
            mode: gitgpui_core::services::ResetMode::Hard,
        },
    );

    assert!(matches!(
        effects.as_slice(),
        [Effect::Reset { repo_id: RepoId(1), target, mode: gitgpui_core::services::ResetMode::Hard }]
            if target == "HEAD~1"
    ));
}

#[test]
fn revert_commit_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::RevertCommit {
            repo_id: RepoId(1),
            commit_id: gitgpui_core::domain::CommitId("deadbeef".to_string()),
        },
    );

    assert!(matches!(
        effects.as_slice(),
        [Effect::RevertCommit { repo_id: RepoId(1), commit_id: _ }]
    ));
}

#[test]
fn commit_amend_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::CommitAmend {
            repo_id: RepoId(1),
            message: "amended".to_string(),
        },
    );

    assert!(matches!(
        effects.as_slice(),
        [Effect::CommitAmend { repo_id: RepoId(1), message }] if message == "amended"
    ));
}

#[test]
fn merge_ref_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::MergeRef {
            repo_id: RepoId(1),
            reference: "feature".to_string(),
        },
    );

    assert!(matches!(
        effects.as_slice(),
        [Effect::MergeRef { repo_id: RepoId(1), reference }] if reference == "feature"
    ));
}

#[test]
fn rebase_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::Rebase {
            repo_id: RepoId(1),
            onto: "master".to_string(),
        },
    );

    assert!(matches!(
        effects.as_slice(),
        [Effect::Rebase { repo_id: RepoId(1), onto }] if onto == "master"
    ));
}

#[test]
fn create_and_delete_branch_emit_effects() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::CreateBranch {
            repo_id: RepoId(1),
            name: "feature".to_string(),
        },
    );
    assert!(matches!(
        effects.as_slice(),
        [Effect::CreateBranch { repo_id: RepoId(1), name }] if name == "feature"
    ));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::DeleteBranch {
            repo_id: RepoId(1),
            name: "feature".to_string(),
        },
    );
    assert!(matches!(
        effects.as_slice(),
        [Effect::DeleteBranch { repo_id: RepoId(1), name }] if name == "feature"
    ));
}

#[test]
fn create_and_delete_tag_emit_effects() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::CreateTag {
            repo_id: RepoId(1),
            name: "v1.0.0".to_string(),
            target: "HEAD".to_string(),
        },
    );
    assert!(matches!(
        effects.as_slice(),
        [Effect::CreateTag { repo_id: RepoId(1), name, target }] if name == "v1.0.0" && target == "HEAD"
    ));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::DeleteTag {
            repo_id: RepoId(1),
            name: "v1.0.0".to_string(),
        },
    );
    assert!(matches!(
        effects.as_slice(),
        [Effect::DeleteTag { repo_id: RepoId(1), name }] if name == "v1.0.0"
    ));
}

#[test]
fn remote_branches_loaded_sets_state() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(2);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::RemoteBranchesLoaded {
            repo_id: RepoId(1),
            result: Ok(vec![RemoteBranch {
                remote: "origin".to_string(),
                name: "main".to_string(),
            }]),
        },
    );

    let repo = state.repos.iter().find(|r| r.id == RepoId(1)).unwrap();
    match &repo.remote_branches {
        Loadable::Ready(branches) => {
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].remote, "origin");
            assert_eq!(branches[0].name, "main");
        }
        other => panic!("expected Ready remote_branches, got {other:?}"),
    }
}

#[test]
fn apply_drop_and_pop_stash_emit_effects() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let apply = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::ApplyStash {
            repo_id: RepoId(1),
            index: 0,
        },
    );
    assert!(matches!(
        apply.as_slice(),
        [Effect::ApplyStash { repo_id: RepoId(1), index: 0 }]
    ));

    let drop = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::DropStash {
            repo_id: RepoId(1),
            index: 0,
        },
    );
    assert!(matches!(
        drop.as_slice(),
        [Effect::DropStash { repo_id: RepoId(1), index: 0 }]
    ));

    let pop = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::PopStash {
            repo_id: RepoId(1),
            index: 0,
        },
    );
    assert!(matches!(
        pop.as_slice(),
        [Effect::PopStash { repo_id: RepoId(1), index: 0 }]
    ));
}

#[test]
fn checkout_commit_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::CheckoutCommit {
            repo_id: RepoId(1),
            commit_id: gitgpui_core::domain::CommitId("deadbeef".to_string()),
        },
    );

    assert!(matches!(
        effects.as_slice(),
        [Effect::CheckoutCommit { repo_id: RepoId(1), commit_id: _ }]
    ));
}

#[test]
fn pop_stash_effect_applies_then_drops() {
    use std::sync::Mutex;

    struct RecordingRepo {
        spec: RepoSpec,
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl GitRepository for RecordingRepo {
        fn spec(&self) -> &RepoSpec {
            &self.spec
        }

        fn log_head_page(&self, _limit: usize, _cursor: Option<&LogCursor>) -> Result<LogPage> {
            unimplemented!()
        }
        fn commit_details(&self, _id: &CommitId) -> Result<CommitDetails> {
            unimplemented!()
        }
        fn reflog_head(&self, _limit: usize) -> Result<Vec<ReflogEntry>> {
            unimplemented!()
        }
        fn current_branch(&self) -> Result<String> {
            unimplemented!()
        }
        fn list_branches(&self) -> Result<Vec<Branch>> {
            unimplemented!()
        }
        fn list_remotes(&self) -> Result<Vec<Remote>> {
            unimplemented!()
        }
        fn list_remote_branches(&self) -> Result<Vec<RemoteBranch>> {
            unimplemented!()
        }
        fn status(&self) -> Result<RepoStatus> {
            unimplemented!()
        }
        fn diff_unified(&self, _target: &DiffTarget) -> Result<String> {
            unimplemented!()
        }

        fn create_branch(&self, _name: &str, _target: &CommitId) -> Result<()> {
            unimplemented!()
        }
        fn delete_branch(&self, _name: &str) -> Result<()> {
            unimplemented!()
        }
        fn checkout_branch(&self, _name: &str) -> Result<()> {
            unimplemented!()
        }
        fn checkout_commit(&self, _id: &CommitId) -> Result<()> {
            unimplemented!()
        }
        fn cherry_pick(&self, _id: &CommitId) -> Result<()> {
            unimplemented!()
        }
        fn revert(&self, _id: &CommitId) -> Result<()> {
            unimplemented!()
        }

        fn stash_create(&self, _message: &str, _include_untracked: bool) -> Result<()> {
            unimplemented!()
        }
        fn stash_list(&self) -> Result<Vec<StashEntry>> {
            unimplemented!()
        }
        fn stash_apply(&self, index: usize) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("apply {index}"));
            Ok(())
        }
        fn stash_drop(&self, index: usize) -> Result<()> {
            self.calls.lock().unwrap().push(format!("drop {index}"));
            Ok(())
        }

        fn stage(&self, _paths: &[&Path]) -> Result<()> {
            unimplemented!()
        }
        fn unstage(&self, _paths: &[&Path]) -> Result<()> {
            unimplemented!()
        }
        fn commit(&self, _message: &str) -> Result<()> {
            unimplemented!()
        }
        fn fetch_all(&self) -> Result<()> {
            unimplemented!()
        }
        fn pull(&self, _mode: PullMode) -> Result<()> {
            unimplemented!()
        }
        fn push(&self) -> Result<()> {
            unimplemented!()
        }
        fn discard_worktree_changes(&self, _paths: &[&Path]) -> Result<()> {
            unimplemented!()
        }
    }

    let calls: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let repo: Arc<RecordingRepo> = Arc::new(RecordingRepo {
        spec: RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
        calls: Arc::clone(&calls),
    });

    struct Backend;
    impl GitBackend for Backend {
        fn open(&self, _workdir: &Path) -> std::result::Result<Arc<dyn GitRepository>, Error> {
            Err(Error::new(ErrorKind::Unsupported("test backend")))
        }
    }

    let executor = super::executor::TaskExecutor::new(1);
    let backend: Arc<dyn GitBackend> = Arc::new(Backend);
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    repos.insert(RepoId(1), repo);
    let (msg_tx, msg_rx) = std::sync::mpsc::channel::<Msg>();

    super::effects::schedule_effect(
        &executor,
        &backend,
        &repos,
        msg_tx,
        Effect::PopStash {
            repo_id: RepoId(1),
            index: 0,
        },
    );

    let msg = msg_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("expected RepoActionFinished");
    assert!(matches!(
        msg,
        Msg::RepoActionFinished {
            repo_id: RepoId(1),
            result: Ok(())
        }
    ));

    assert_eq!(
        *calls.lock().unwrap(),
        vec!["apply 0".to_string(), "drop 0".to_string()]
    );
}

#[test]
fn restore_session_opens_all_and_selects_active_repo() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();

    let dir = std::env::temp_dir().join(format!(
        "gitgpui-restore-session-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let _ = std::fs::create_dir_all(&dir);

    let repo_a = dir.join("repo-a");
    let repo_b = dir.join("repo-b");
    let _ = std::fs::create_dir_all(&repo_a);
    let _ = std::fs::create_dir_all(&repo_b);

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::RestoreSession {
            open_repos: vec![repo_a.clone(), repo_b],
            active_repo: Some(repo_a.clone()),
        },
    );

    assert_eq!(state.repos.len(), 2);
    assert!(matches!(
        effects.as_slice(),
        [Effect::OpenRepo { .. }, Effect::OpenRepo { .. }]
    ));

    let active_repo_id = state.active_repo.expect("active repo is set");
    let active_workdir = state
        .repos
        .iter()
        .find(|r| r.id == active_repo_id)
        .expect("active repo exists")
        .spec
        .workdir
        .clone();

    assert_eq!(active_workdir, super::reducer::normalize_repo_path(repo_a));
}

#[test]
fn set_active_repo_refreshes_repo_state_and_selected_diff() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();

    reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::OpenRepo(PathBuf::from("/tmp/repo1")),
    );
    reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::OpenRepo(PathBuf::from("/tmp/repo2")),
    );

    let repo1 = RepoId(1);
    let repo2 = RepoId(2);
    assert_eq!(state.active_repo, Some(repo2));

    let repo1_state = state
        .repos
        .iter_mut()
        .find(|r| r.id == repo1)
        .expect("repo1 exists");
    repo1_state.diff_target = Some(DiffTarget::WorkingTree {
        path: PathBuf::from("src/lib.rs"),
        area: gitgpui_core::domain::DiffArea::Unstaged,
    });

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::SetActiveRepo { repo_id: repo1 },
    );

    assert_eq!(state.active_repo, Some(repo1));

    let has_status = effects
        .iter()
        .any(|e| matches!(e, Effect::LoadStatus { repo_id } if *repo_id == repo1));
    let has_log = effects.iter().any(|e| {
        matches!(e, Effect::LoadLog { repo_id, scope: _, limit: _, cursor: _ } if *repo_id == repo1)
    });
    let has_diff = effects
        .iter()
        .any(|e| matches!(e, Effect::LoadDiff { repo_id, target: _ } if *repo_id == repo1));
    let has_diff_file = effects
        .iter()
        .any(|e| matches!(e, Effect::LoadDiffFile { repo_id, target: _ } if *repo_id == repo1));

    assert!(has_status, "expected status refresh on activation");
    assert!(has_log, "expected log refresh on activation");
    assert!(has_diff, "expected diff refresh on activation");
    assert!(has_diff_file, "expected diff-file refresh on activation");
}

#[test]
fn repo_opened_ok_sets_loading_and_emits_refresh_effects() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();

    reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::OpenRepo(PathBuf::from("/tmp/repo")),
    );

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::RepoOpenedOk {
            repo_id: RepoId(1),
            spec: RepoSpec {
                workdir: PathBuf::from("/tmp/repo"),
            },
            repo: Arc::new(DummyRepo::new("/tmp/repo")),
        },
    );

    let repo_state = state.repos.first().unwrap();
    assert!(matches!(repo_state.open, Loadable::Ready(())));
    assert!(repo_state.head_branch.is_loading());
    assert!(repo_state.branches.is_loading());
    assert!(repo_state.tags.is_loading());
    assert!(repo_state.remotes.is_loading());
    assert!(repo_state.remote_branches.is_loading());
    assert!(repo_state.status.is_loading());
    assert!(repo_state.log.is_loading());
    assert!(repo_state.stashes.is_loading());
    assert!(repo_state.reflog.is_loading());
    assert!(repo_state.upstream_divergence.is_loading());
    assert!(repo_state.rebase_in_progress.is_loading());
    assert!(matches!(repo_state.file_history, Loadable::NotLoaded));
    assert!(matches!(repo_state.blame, Loadable::NotLoaded));
    assert!(matches!(
        effects.as_slice(),
        [
            Effect::LoadHeadBranch { .. },
            Effect::LoadUpstreamDivergence { .. },
            Effect::LoadBranches { .. },
            Effect::LoadTags { .. },
            Effect::LoadRemotes { .. },
            Effect::LoadRemoteBranches { .. },
            Effect::LoadStatus { .. },
            Effect::LoadStashes { .. },
            Effect::LoadReflog { .. },
            Effect::LoadRebaseState { .. },
            Effect::LoadLog { .. }
        ]
    ));
}

#[test]
fn repo_action_finished_clears_error_and_refreshes() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));
    state.repos[0].last_error = Some("boom".to_string());

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::RepoActionFinished {
            repo_id: RepoId(1),
            result: Ok(()),
        },
    );

    assert!(state.repos[0].last_error.is_none());
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::LoadStatus { repo_id: RepoId(1) }))
    );
}

#[test]
fn repo_action_finished_err_records_diagnostic() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let error = Error::new(ErrorKind::Backend("boom".to_string()));
    reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::RepoActionFinished {
            repo_id: RepoId(1),
            result: Err(error),
        },
    );

    let repo_state = &state.repos[0];
    assert!(
        repo_state
            .last_error
            .as_deref()
            .is_some_and(|s| s.contains("boom"))
    );
    assert!(
        repo_state
            .diagnostics
            .iter()
            .any(|d| d.message.contains("boom"))
    );
}

#[test]
fn repo_opened_err_records_diagnostic() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();

    reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::OpenRepo(PathBuf::from("/tmp/repo")),
    );

    let error = Error::new(ErrorKind::Backend("nope".to_string()));
    reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::RepoOpenedErr {
            repo_id: RepoId(1),
            spec: RepoSpec {
                workdir: PathBuf::from("/tmp/repo"),
            },
            error,
        },
    );

    let repo_state = &state.repos[0];
    assert!(
        repo_state
            .last_error
            .as_deref()
            .is_some_and(|s| s.contains("nope"))
    );
    assert!(
        repo_state
            .diagnostics
            .iter()
            .any(|d| d.message.contains("nope"))
    );
}

#[test]
fn select_diff_sets_loading_and_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(2);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let target = gitgpui_core::domain::DiffTarget::WorkingTree {
        path: PathBuf::from("src/lib.rs"),
        area: gitgpui_core::domain::DiffArea::Unstaged,
    };

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::SelectDiff {
            repo_id: RepoId(1),
            target: target.clone(),
        },
    );

    let repo_state = state.repos.first().expect("repo state to exist");
    assert_eq!(repo_state.diff_target, Some(target.clone()));
    assert!(repo_state.diff.is_loading());
    assert!(repo_state.diff_file.is_loading());
    assert!(matches!(
        effects.as_slice(),
        [
            Effect::LoadDiff { repo_id: RepoId(1), target: a },
            Effect::LoadDiffFile { repo_id: RepoId(1), target: b },
        ] if a == &target && b == &target
    ));
}

#[test]
fn select_diff_for_image_sets_loading_and_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(2);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let target = gitgpui_core::domain::DiffTarget::WorkingTree {
        path: PathBuf::from("img.png"),
        area: gitgpui_core::domain::DiffArea::Unstaged,
    };

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::SelectDiff {
            repo_id: RepoId(1),
            target: target.clone(),
        },
    );

    let repo_state = state.repos.first().expect("repo state to exist");
    assert_eq!(repo_state.diff_target, Some(target.clone()));
    assert!(repo_state.diff.is_loading());
    assert!(matches!(repo_state.diff_file, Loadable::NotLoaded));
    assert!(repo_state.diff_file_image.is_loading());
    assert!(matches!(
        effects.as_slice(),
        [
            Effect::LoadDiff { repo_id: RepoId(1), target: a },
            Effect::LoadDiffFileImage { repo_id: RepoId(1), target: b },
        ] if a == &target && b == &target
    ));
}

#[test]
fn stage_hunk_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(2);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::StageHunk {
            repo_id: RepoId(1),
            patch: "diff --git a/a.txt b/a.txt\n".to_string(),
        },
    );

    assert!(matches!(
        effects.as_slice(),
        [Effect::StageHunk { repo_id: RepoId(1), patch: _ }]
    ));
}

#[test]
fn unstage_hunk_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(2);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::UnstageHunk {
            repo_id: RepoId(1),
            patch: "diff --git a/a.txt b/a.txt\n".to_string(),
        },
    );

    assert!(matches!(
        effects.as_slice(),
        [Effect::UnstageHunk { repo_id: RepoId(1), patch: _ }]
    ));
}

#[test]
fn stage_hunk_command_finished_reloads_current_diff() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(2);
    let mut state = AppState::default();
    let mut repo_state = RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    );
    repo_state.diff_target = Some(DiffTarget::WorkingTree {
        path: PathBuf::from("a.txt"),
        area: gitgpui_core::domain::DiffArea::Unstaged,
    });
    repo_state.diff = Loadable::NotLoaded;
    repo_state.diff_file = Loadable::NotLoaded;
    state.repos.push(repo_state);
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::RepoCommandFinished {
            repo_id: RepoId(1),
            command: crate::msg::RepoCommandKind::StageHunk,
            result: Ok(CommandOutput::default()),
        },
    );

    let repo_state = state.repos.iter().find(|r| r.id == RepoId(1)).unwrap();
    assert!(repo_state.diff.is_loading());
    assert!(repo_state.diff_file.is_loading());
    assert!(effects.iter().any(|e| {
        matches!(e, Effect::LoadDiff { repo_id: RepoId(1), target: DiffTarget::WorkingTree { path, area: gitgpui_core::domain::DiffArea::Unstaged } } if path == &PathBuf::from("a.txt"))
    }));
    assert!(effects.iter().any(|e| matches!(e, Effect::LoadDiffFile { repo_id: RepoId(1), target: _ })));
}

#[test]
fn discard_worktree_changes_path_emits_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(2);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::DiscardWorktreeChangesPath {
            repo_id: RepoId(1),
            path: PathBuf::from("a.txt"),
        },
    );

    assert!(matches!(
        effects.as_slice(),
        [Effect::DiscardWorktreeChangesPath { repo_id: RepoId(1), path: _ }]
    ));
}

#[test]
fn clear_diff_selection_resets_diff_state() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(2);
    let mut state = AppState::default();
    let mut repo_state = RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    );
    repo_state.diff_target = Some(gitgpui_core::domain::DiffTarget::WorkingTree {
        path: PathBuf::from("src/lib.rs"),
        area: gitgpui_core::domain::DiffArea::Unstaged,
    });
    repo_state.diff = Loadable::Loading;
    repo_state.diff_file = Loadable::Loading;
    state.repos.push(repo_state);
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::ClearDiffSelection { repo_id: RepoId(1) },
    );

    let repo_state = state.repos.first().expect("repo state to exist");
    assert!(repo_state.diff_target.is_none());
    assert!(matches!(repo_state.diff, Loadable::NotLoaded));
    assert!(matches!(repo_state.diff_file, Loadable::NotLoaded));
    assert!(effects.is_empty());
}

#[test]
fn set_active_repo_ignores_unknown_repo() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();

    reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::OpenRepo(PathBuf::from("/tmp/repo1")),
    );
    reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::OpenRepo(PathBuf::from("/tmp/repo2")),
    );
    assert_eq!(state.active_repo, Some(RepoId(2)));

    reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::SetActiveRepo {
            repo_id: RepoId(999),
        },
    );
    assert_eq!(state.active_repo, Some(RepoId(2)));
}

#[test]
fn diff_loaded_err_records_diagnostic_when_target_matches() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    let mut repo_state = RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    );
    let target = DiffTarget::WorkingTree {
        path: PathBuf::from("src/lib.rs"),
        area: gitgpui_core::domain::DiffArea::Unstaged,
    };
    repo_state.diff_target = Some(target.clone());
    repo_state.diff = Loadable::Loading;
    state.repos.push(repo_state);
    state.active_repo = Some(RepoId(1));

    let error = Error::new(ErrorKind::Backend("diff failed".to_string()));
    reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::DiffLoaded {
            repo_id: RepoId(1),
            target,
            result: Err(error),
        },
    );

    let repo_state = &state.repos[0];
    assert!(matches!(repo_state.diff, Loadable::Error(_)));
    assert!(
        repo_state
            .diagnostics
            .iter()
            .any(|d| d.message.contains("diff failed"))
    );
}

#[test]
fn diagnostics_are_capped() {
    let mut repo_state = RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    );

    for i in 0..205 {
        super::reducer::push_diagnostic(&mut repo_state, DiagnosticKind::Error, format!("err-{i}"));
    }

    assert_eq!(repo_state.diagnostics.len(), 200);
    assert_eq!(repo_state.diagnostics[0].message, "err-5");
    assert_eq!(repo_state.diagnostics.last().unwrap().message, "err-204");
}

#[test]
fn reload_repo_sets_sections_loading_and_emits_refresh_effects() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::ReloadRepo { repo_id: RepoId(1) },
    );

    let repo_state = &state.repos[0];
    assert!(repo_state.head_branch.is_loading());
    assert!(repo_state.branches.is_loading());
    assert!(repo_state.tags.is_loading());
    assert!(repo_state.remotes.is_loading());
    assert!(repo_state.remote_branches.is_loading());
    assert!(repo_state.status.is_loading());
    assert!(repo_state.log.is_loading());
    assert!(!repo_state.log_loading_more);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::LoadStatus { repo_id: RepoId(1) }))
    );
}

#[test]
fn load_more_history_emits_paginated_load_log_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let repo_state = &mut state.repos[0];
    repo_state.history_scope = LogScope::CurrentBranch;
    repo_state.log = Loadable::Ready(LogPage {
        commits: vec![Commit {
            id: CommitId("c1".to_string()),
            parent_ids: Vec::new(),
            summary: "s1".to_string(),
            author: "a".to_string(),
            time: SystemTime::UNIX_EPOCH,
        }],
        next_cursor: Some(LogCursor {
            last_seen: CommitId("c1".to_string()),
        }),
    });
    repo_state.log_loading_more = false;

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::LoadMoreHistory { repo_id: RepoId(1) },
    );

    let repo_state = &state.repos[0];
    assert!(repo_state.log_loading_more);
    assert!(matches!(
        effects.as_slice(),
        [Effect::LoadLog {
            repo_id: RepoId(1),
            scope: LogScope::CurrentBranch,
            limit: 200,
            cursor: Some(_)
        }]
    ));
}

#[test]
fn set_history_scope_to_all_branches_emits_load_log_all_branches_effect() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let repo_state = &mut state.repos[0];
    repo_state.history_scope = LogScope::CurrentBranch;
    repo_state.log = Loadable::Ready(LogPage {
        commits: vec![],
        next_cursor: None,
    });

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::SetHistoryScope {
            repo_id: RepoId(1),
            scope: LogScope::AllBranches,
        },
    );

    let repo_state = &state.repos[0];
    assert_eq!(repo_state.history_scope, LogScope::AllBranches);
    assert!(repo_state.log.is_loading());
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::LoadLog {
                repo_id: RepoId(1),
                scope: LogScope::AllBranches,
                ..
            }
        )),
        "expected a LoadLog(AllBranches) effect, got {effects:?}"
    );
}

#[test]
fn load_more_history_noops_when_no_next_cursor() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let repo_state = &mut state.repos[0];
    repo_state.log = Loadable::Ready(LogPage {
        commits: vec![Commit {
            id: CommitId("c1".to_string()),
            parent_ids: Vec::new(),
            summary: "s1".to_string(),
            author: "a".to_string(),
            time: SystemTime::UNIX_EPOCH,
        }],
        next_cursor: None,
    });

    let effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::LoadMoreHistory { repo_id: RepoId(1) },
    );

    let repo_state = &state.repos[0];
    assert!(!repo_state.log_loading_more);
    assert!(effects.is_empty());
}

#[test]
fn log_loaded_appends_when_loading_more() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let repo_state = &mut state.repos[0];
    repo_state.history_scope = LogScope::CurrentBranch;
    repo_state.log = Loadable::Ready(LogPage {
        commits: vec![Commit {
            id: CommitId("c1".to_string()),
            parent_ids: Vec::new(),
            summary: "s1".to_string(),
            author: "a".to_string(),
            time: SystemTime::UNIX_EPOCH,
        }],
        next_cursor: Some(LogCursor {
            last_seen: CommitId("c1".to_string()),
        }),
    });
    repo_state.log_loading_more = true;

    let _effects = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::LogLoaded {
            repo_id: RepoId(1),
            scope: LogScope::CurrentBranch,
            result: Ok(LogPage {
                commits: vec![Commit {
                    id: CommitId("c2".to_string()),
                    parent_ids: Vec::new(),
                    summary: "s2".to_string(),
                    author: "a".to_string(),
                    time: SystemTime::UNIX_EPOCH,
                }],
                next_cursor: None,
            }),
        },
    );

    let repo_state = &state.repos[0];
    assert!(!repo_state.log_loading_more);
    let Loadable::Ready(page) = &repo_state.log else {
        panic!("expected log ready");
    };
    assert_eq!(page.commits.len(), 2);
    assert_eq!(page.commits[0].id.as_ref(), "c1");
    assert_eq!(page.commits[1].id.as_ref(), "c2");
    assert_eq!(page.next_cursor, None);
}

#[test]
fn repo_operations_emit_effects() {
    let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
    let id_alloc = AtomicU64::new(1);
    let mut state = AppState::default();
    state.repos.push(RepoState::new_opening(
        RepoId(1),
        RepoSpec {
            workdir: PathBuf::from("/tmp/repo"),
        },
    ));
    state.active_repo = Some(RepoId(1));

    let stage = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::StagePath {
            repo_id: RepoId(1),
            path: PathBuf::from("a.txt"),
        },
    );
    assert!(matches!(
        stage.as_slice(),
        [Effect::StagePath {
            repo_id: RepoId(1),
            ..
        }]
    ));

    let unstage = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::UnstagePath {
            repo_id: RepoId(1),
            path: PathBuf::from("a.txt"),
        },
    );
    assert!(matches!(
        unstage.as_slice(),
        [Effect::UnstagePath {
            repo_id: RepoId(1),
            ..
        }]
    ));

    let commit = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::Commit {
            repo_id: RepoId(1),
            message: "m".to_string(),
        },
    );
    assert!(matches!(
        commit.as_slice(),
        [Effect::Commit {
            repo_id: RepoId(1),
            ..
        }]
    ));

    let pull = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::Pull {
            repo_id: RepoId(1),
            mode: PullMode::Rebase,
        },
    );
    assert!(matches!(
        pull.as_slice(),
        [Effect::Pull {
            repo_id: RepoId(1),
            ..
        }]
    ));

    let push = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::Push { repo_id: RepoId(1) },
    );
    assert!(matches!(
        push.as_slice(),
        [Effect::Push { repo_id: RepoId(1) }]
    ));

    let force_push = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::ForcePush { repo_id: RepoId(1) },
    );
    assert!(matches!(
        force_push.as_slice(),
        [Effect::ForcePush { repo_id: RepoId(1) }]
    ));

    let push_set_upstream = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::PushSetUpstream {
            repo_id: RepoId(1),
            remote: "origin".to_string(),
            branch: "feature/foo".to_string(),
        },
    );
    assert!(matches!(
        push_set_upstream.as_slice(),
        [Effect::PushSetUpstream {
            repo_id: RepoId(1),
            ..
        }]
    ));

    let stash = reduce(
        &mut repos,
        &id_alloc,
        &mut state,
        Msg::Stash {
            repo_id: RepoId(1),
            message: "wip".to_string(),
            include_untracked: true,
        },
    );
    assert!(matches!(
        stash.as_slice(),
        [Effect::Stash {
            repo_id: RepoId(1),
            ..
        }]
    ));
}
