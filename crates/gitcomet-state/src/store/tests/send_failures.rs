use super::*;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Barrier, Condvar, LazyLock, Mutex, MutexGuard, RwLock,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

static SEND_FAILURE_COUNTER_TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn send_failure_counter_test_lock() -> MutexGuard<'static, ()> {
    SEND_FAILURE_COUNTER_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn store_event_failure_count() -> u64 {
    super::send_diagnostics::send_failure_count(
        super::send_diagnostics::SendFailureKind::StoreEvent,
    )
}

struct BlockingOpenBackend {
    started_tx: mpsc::Sender<()>,
    release_rx: Mutex<mpsc::Receiver<()>>,
}

impl GitBackend for BlockingOpenBackend {
    fn open(&self, _path: &Path) -> std::result::Result<Arc<dyn GitRepository>, Error> {
        let _ = self.started_tx.send(());
        let _ = self
            .release_rx
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .recv_timeout(Duration::from_secs(2));
        Err(Error::new(ErrorKind::Unsupported("blocking open released")))
    }
}

struct BlockingDiffRepo {
    spec: RepoSpec,
    started_tx: mpsc::Sender<&'static str>,
    release: Arc<(Mutex<bool>, Condvar)>,
}

impl BlockingDiffRepo {
    fn wait_for_release(&self, name: &'static str) {
        let _ = self.started_tx.send(name);
        let (lock, condvar) = &*self.release;
        let mut released = lock.lock().unwrap_or_else(|e| e.into_inner());
        while !*released {
            released = condvar.wait(released).unwrap_or_else(|e| e.into_inner());
        }
    }
}

impl GitRepository for BlockingDiffRepo {
    fn spec(&self) -> &RepoSpec {
        &self.spec
    }

    fn log_head_page(&self, _limit: usize, _cursor: Option<&LogCursor>) -> Result<LogPage> {
        Ok(LogPage {
            commits: Vec::new(),
            next_cursor: None,
        })
    }

    fn commit_details(&self, id: &CommitId) -> Result<CommitDetails> {
        Ok(CommitDetails {
            id: id.clone(),
            message: String::new(),
            committed_at: String::new(),
            parent_ids: Vec::new(),
            files: Vec::new(),
        })
    }

    fn reflog_head(&self, _limit: usize) -> Result<Vec<ReflogEntry>> {
        Ok(Vec::new())
    }

    fn current_branch(&self) -> Result<String> {
        Ok("main".to_string())
    }

    fn list_branches(&self) -> Result<Vec<Branch>> {
        Ok(Vec::new())
    }

    fn list_remotes(&self) -> Result<Vec<Remote>> {
        Ok(Vec::new())
    }

    fn list_remote_branches(&self) -> Result<Vec<RemoteBranch>> {
        Ok(Vec::new())
    }

    fn status(&self) -> Result<RepoStatus> {
        Ok(RepoStatus::default())
    }

    fn diff_unified(&self, _target: &DiffTarget) -> Result<String> {
        self.wait_for_release("diff_unified");
        Ok("diff --git a/tracked.txt b/tracked.txt\n--- a/tracked.txt\n+++ b/tracked.txt\n@@ -1 +1 @@\n-old\n+new\n".to_string())
    }

    fn diff_file_text(
        &self,
        _target: &DiffTarget,
    ) -> Result<Option<gitcomet_core::domain::FileDiffText>> {
        self.wait_for_release("diff_file_text");
        Ok(Some(gitcomet_core::domain::FileDiffText::new(
            PathBuf::from("tracked.txt"),
            Some("old\n".to_string()),
            Some("new\n".to_string()),
        )))
    }

    fn create_branch(&self, _name: &str, _target: &CommitId) -> Result<()> {
        Ok(())
    }
    fn delete_branch(&self, _name: &str) -> Result<()> {
        Ok(())
    }
    fn checkout_branch(&self, _name: &str) -> Result<()> {
        Ok(())
    }
    fn checkout_commit(&self, _id: &CommitId) -> Result<()> {
        Ok(())
    }
    fn cherry_pick(&self, _id: &CommitId) -> Result<()> {
        Ok(())
    }
    fn revert(&self, _id: &CommitId) -> Result<()> {
        Ok(())
    }
    fn stash_create(&self, _message: &str, _include_untracked: bool) -> Result<()> {
        Ok(())
    }
    fn stash_list(&self) -> Result<Vec<StashEntry>> {
        Ok(Vec::new())
    }
    fn stash_apply(&self, _index: usize) -> Result<()> {
        Ok(())
    }
    fn stash_drop(&self, _index: usize) -> Result<()> {
        Ok(())
    }
    fn stage(&self, _paths: &[&Path]) -> Result<()> {
        Ok(())
    }
    fn unstage(&self, _paths: &[&Path]) -> Result<()> {
        Ok(())
    }
    fn commit(&self, _message: &str) -> Result<()> {
        Ok(())
    }
    fn fetch_all(&self) -> Result<()> {
        Ok(())
    }
    fn pull(&self, _mode: PullMode) -> Result<()> {
        Ok(())
    }
    fn push(&self) -> Result<()> {
        Ok(())
    }
    fn discard_worktree_changes(&self, _paths: &[&Path]) -> Result<()> {
        Ok(())
    }
}

#[test]
fn dispatch_increments_failure_counter_when_channel_is_disconnected() {
    let _guard = send_failure_counter_test_lock();
    let before = super::send_diagnostics::send_failure_count(
        super::send_diagnostics::SendFailureKind::StoreDispatch,
    );

    let (command_tx, command_rx) = mpsc::channel::<super::worker_channel::StoreWorkerCommand>();
    drop(command_rx);
    let msg_tx = super::worker_channel::StoreWorkerSender::new(
        command_tx,
        Arc::new(std::sync::atomic::AtomicBool::new(true)),
        super::worker_channel::StoreInstanceId::next(),
    );

    let store = AppStore {
        state: Arc::new(RwLock::new(Arc::new(AppState::default()))),
        msg_tx: msg_tx.clone(),
        public_lifetime: Arc::new(StorePublicLifetime::new(msg_tx)),
    };

    store.dispatch(Msg::OpenRepo(PathBuf::from("/tmp/repo")));

    let after = super::send_diagnostics::send_failure_count(
        super::send_diagnostics::SendFailureKind::StoreDispatch,
    );
    assert!(after > before);
}

#[test]
fn concurrent_last_app_store_drops_shutdown_worker_once() {
    let (command_tx, command_rx) = mpsc::channel::<super::worker_channel::StoreWorkerCommand>();
    let alive = Arc::new(AtomicBool::new(true));
    let msg_tx = super::worker_channel::StoreWorkerSender::new(
        command_tx,
        Arc::clone(&alive),
        super::worker_channel::StoreInstanceId::next(),
    );
    let store = AppStore {
        state: Arc::new(RwLock::new(Arc::new(AppState::default()))),
        msg_tx: msg_tx.clone(),
        public_lifetime: Arc::new(StorePublicLifetime::new(msg_tx)),
    };
    let store_a = store.clone();
    let store_b = store.clone();
    drop(store);

    let barrier = Arc::new(Barrier::new(3));
    let thread_a_barrier = Arc::clone(&barrier);
    let thread_a = std::thread::spawn(move || {
        thread_a_barrier.wait();
        drop(store_a);
    });
    let thread_b_barrier = Arc::clone(&barrier);
    let thread_b = std::thread::spawn(move || {
        thread_b_barrier.wait();
        drop(store_b);
    });

    barrier.wait();
    thread_a.join().expect("first store drop thread panicked");
    thread_b.join().expect("second store drop thread panicked");

    match command_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("expected final AppStore drop to send worker shutdown")
    {
        super::worker_channel::StoreWorkerCommand::Shutdown => {}
        super::worker_channel::StoreWorkerCommand::Msg(_) => {
            panic!("expected shutdown command, got message command")
        }
        #[cfg(any(test, feature = "test-support"))]
        super::worker_channel::StoreWorkerCommand::InsertRepoForTest { .. } => {
            panic!("expected shutdown command, got test repo insertion command")
        }
    }
    assert!(
        !alive.load(Ordering::Acquire),
        "expected final AppStore drop to close the store worker sender"
    );
}

#[test]
fn executor_increments_failure_counter_when_worker_queue_disconnects() {
    let _guard = send_failure_counter_test_lock();
    let before = super::send_diagnostics::send_failure_count(
        super::send_diagnostics::SendFailureKind::ExecutorQueue,
    );

    let executor = super::executor::TaskExecutor::new(1);
    let (started_tx, started_rx) = mpsc::channel::<()>();
    executor.spawn(move || {
        let _ = started_tx.send(());
        panic!("intentional panic to drop executor worker");
    });

    started_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("worker task did not start");

    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        // The worker panic may race with this test thread; keep attempting to enqueue
        // until the sender observes the disconnected queue and diagnostics increment.
        executor.spawn(|| {});

        let after = super::send_diagnostics::send_failure_count(
            super::send_diagnostics::SendFailureKind::ExecutorQueue,
        );
        if after > before {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "expected executor queue send failure count to increase"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn effect_message_send_increments_failure_counter_when_disconnected() {
    let _guard = send_failure_counter_test_lock();
    let before = super::send_diagnostics::send_failure_count(
        super::send_diagnostics::SendFailureKind::EffectMessage,
    );

    let (msg_tx, msg_rx) = mpsc::channel::<Msg>();
    drop(msg_rx);

    super::send_diagnostics::send_or_log(
        &msg_tx,
        Msg::RefreshBranches { repo_id: RepoId(1) },
        super::send_diagnostics::SendFailureKind::EffectMessage,
        "test effect pipeline send",
    );

    let after = super::send_diagnostics::send_failure_count(
        super::send_diagnostics::SendFailureKind::EffectMessage,
    );
    assert!(after > before);
}

#[test]
fn store_event_send_increments_failure_counter_when_receiver_closed() {
    let _guard = send_failure_counter_test_lock();
    let before = store_event_failure_count();

    let (event_tx, event_rx) = smol::channel::bounded::<StoreEvent>(1);
    drop(event_rx);

    super::send_diagnostics::try_send_state_changed_or_log(
        &event_tx,
        "test state event send",
        super::worker_channel::StoreInstanceId::next(),
        true,
    );

    let after = store_event_failure_count();
    assert!(after > before);
}

#[test]
fn closed_receiver_while_store_is_alive_increments_store_event_failure_counter() {
    let _guard = send_failure_counter_test_lock();
    let before = store_event_failure_count();

    let backend: Arc<dyn GitBackend> = Arc::new(FailingBackend);
    let (store, event_rx) = AppStore::new(backend);
    drop(event_rx);

    store.dispatch(Msg::DismissBannerError);

    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let after = store_event_failure_count();
        if after > before {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "expected active store receiver loss to be recorded"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn dropping_receiver_then_last_store_suppresses_late_open_result_store_event_failures() {
    let _guard = send_failure_counter_test_lock();
    let before = store_event_failure_count();
    let (started_tx, started_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let backend: Arc<dyn GitBackend> = Arc::new(BlockingOpenBackend {
        started_tx,
        release_rx: Mutex::new(release_rx),
    });
    let (store, event_rx) = AppStore::new(backend);

    store.dispatch(Msg::OpenRepo(PathBuf::from("/tmp/gitcomet-blocking-open")));
    started_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("open effect did not start");

    drop(event_rx);
    drop(store);
    let _ = release_tx.send(());
    std::thread::sleep(Duration::from_millis(100));

    assert_eq!(store_event_failure_count(), before);
}

#[test]
fn selected_diff_results_after_store_drop_do_not_emit_store_event_failures() {
    let _guard = send_failure_counter_test_lock();
    let before = store_event_failure_count();
    let backend: Arc<dyn GitBackend> = Arc::new(FailingBackend);
    let (store, event_rx) = AppStore::new(backend);

    let repo_id = RepoId(1);
    let spec = RepoSpec {
        workdir: PathBuf::from("/tmp/gitcomet-blocking-diff"),
    };
    let mut state = AppState::default();
    state.active_repo = Some(repo_id);
    state
        .repos
        .push(RepoState::new_opening(repo_id, spec.clone()));
    store.replace_snapshot_for_test(Arc::new(state));

    let (started_tx, started_rx) = mpsc::channel::<&'static str>();
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let repo: Arc<dyn GitRepository> = Arc::new(BlockingDiffRepo {
        spec,
        started_tx,
        release: Arc::clone(&release),
    });
    store.insert_repo_for_test(repo_id, repo);

    store.dispatch(Msg::SelectDiff {
        repo_id,
        target: DiffTarget::WorkingTree {
            path: PathBuf::from("tracked.txt"),
            area: DiffArea::Unstaged,
        },
    });

    started_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("selected diff load did not start");

    drop(event_rx);
    drop(store);
    {
        let (lock, condvar) = &*release;
        let mut released = lock.lock().unwrap_or_else(|e| e.into_inner());
        *released = true;
        condvar.notify_all();
    }
    std::thread::sleep(Duration::from_millis(100));

    assert_eq!(store_event_failure_count(), before);
}
