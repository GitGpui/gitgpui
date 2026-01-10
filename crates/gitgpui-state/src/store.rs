use crate::model::{AppState, Loadable, RepoId, RepoState};
use crate::msg::{Effect, Msg, StoreEvent};
use gitgpui_core::domain::RepoSpec;
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::{GitBackend, GitRepository};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, RwLock};
use std::thread;

pub struct AppStore {
    state: Arc<RwLock<AppState>>,
    msg_tx: mpsc::Sender<Msg>,
}

impl Clone for AppStore {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            msg_tx: self.msg_tx.clone(),
        }
    }
}

impl AppStore {
    pub fn new(backend: Arc<dyn GitBackend>) -> (Self, mpsc::Receiver<StoreEvent>) {
        let state = Arc::new(RwLock::new(AppState::default()));
        let (msg_tx, msg_rx) = mpsc::channel::<Msg>();
        let (event_tx, event_rx) = mpsc::channel::<StoreEvent>();

        let thread_state = Arc::clone(&state);
        let thread_msg_tx = msg_tx.clone();

        thread::spawn(move || {
            let executor = TaskExecutor::new(default_worker_threads());
            let mut repos: HashMap<RepoId, Arc<dyn GitRepository>> = HashMap::new();
            let id_alloc = AtomicU64::new(1);

            while let Ok(msg) = msg_rx.recv() {
                let effects = {
                    let mut app_state = thread_state
                        .write()
                        .expect("state lock poisoned (write)");

                    reduce(&mut repos, &id_alloc, &mut app_state, msg)
                };

                let _ = event_tx.send(StoreEvent::StateChanged);

                for effect in effects {
                    schedule_effect(
                        &executor,
                        &backend,
                        &repos,
                        thread_msg_tx.clone(),
                        effect,
                    );
                }
            }
        });

        (Self { state, msg_tx }, event_rx)
    }

    pub fn dispatch(&self, msg: Msg) {
        let _ = self.msg_tx.send(msg);
    }

    pub fn snapshot(&self) -> AppState {
        self.state
            .read()
            .expect("state lock poisoned (read)")
            .clone()
    }
}

fn reduce(
    repos: &mut HashMap<RepoId, Arc<dyn GitRepository>>,
    id_alloc: &AtomicU64,
    state: &mut AppState,
    msg: Msg,
) -> Vec<Effect> {
    match msg {
        Msg::OpenRepo(path) => {
            let repo_id = RepoId(id_alloc.fetch_add(1, Ordering::Relaxed));
            let spec = RepoSpec { workdir: path };

            state.current_repo = Some(RepoState::new_opening(repo_id, spec.clone()));
            vec![Effect::OpenRepo {
                repo_id,
                path: spec.workdir.clone(),
            }]
        }

        Msg::RepoOpenedOk {
            repo_id,
            spec,
            repo,
        } => {
            repos.insert(repo_id, repo);

            if let Some(repo_state) = state.current_repo.as_mut()
                && repo_state.id == repo_id
            {
                repo_state.spec = spec;
                repo_state.open = Loadable::Ready(());
                repo_state.branches = Loadable::Loading;
                repo_state.remotes = Loadable::Loading;
                repo_state.status = Loadable::Loading;
                repo_state.log = Loadable::Loading;
            }

            vec![
                Effect::LoadBranches { repo_id },
                Effect::LoadRemotes { repo_id },
                Effect::LoadStatus { repo_id },
                Effect::LoadHeadLog {
                    repo_id,
                    limit: 200,
                    cursor: None,
                },
            ]
        }

        Msg::RepoOpenedErr {
            repo_id,
            spec,
            error,
        } => {
            if let Some(repo_state) = state.current_repo.as_mut()
                && repo_state.id == repo_id
            {
                repo_state.spec = spec;
                repo_state.open = Loadable::Error(error.to_string());
            }
            Vec::new()
        }

        Msg::BranchesLoaded { repo_id, result } => {
            if let Some(repo_state) = state.current_repo.as_mut()
                && repo_state.id == repo_id
            {
                repo_state.branches = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => Loadable::Error(e.to_string()),
                };
            }
            Vec::new()
        }

        Msg::RemotesLoaded { repo_id, result } => {
            if let Some(repo_state) = state.current_repo.as_mut()
                && repo_state.id == repo_id
            {
                repo_state.remotes = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => Loadable::Error(e.to_string()),
                };
            }
            Vec::new()
        }

        Msg::StatusLoaded { repo_id, result } => {
            if let Some(repo_state) = state.current_repo.as_mut()
                && repo_state.id == repo_id
            {
                repo_state.status = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => Loadable::Error(e.to_string()),
                };
            }
            Vec::new()
        }

        Msg::LogLoaded { repo_id, result } => {
            if let Some(repo_state) = state.current_repo.as_mut()
                && repo_state.id == repo_id
            {
                repo_state.log = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => Loadable::Error(e.to_string()),
                };
            }
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let repo_state = state.current_repo.expect("repo state to be set");
        assert_eq!(repo_state.id.0, 1);
        assert!(repo_state.open.is_loading());
        assert!(matches!(effects.as_slice(), [Effect::OpenRepo { .. }]));
    }
}

fn schedule_effect(
    executor: &TaskExecutor,
    backend: &Arc<dyn GitBackend>,
    repos: &HashMap<RepoId, Arc<dyn GitRepository>>,
    msg_tx: mpsc::Sender<Msg>,
    effect: Effect,
) {
    match effect {
        Effect::OpenRepo { repo_id, path } => {
            let backend = Arc::clone(backend);
            executor.spawn(move || {
                let spec = RepoSpec { workdir: path };
                match backend.open(&spec.workdir) {
                    Ok(repo) => {
                        let _ = msg_tx.send(Msg::RepoOpenedOk { repo_id, spec, repo });
                    }
                    Err(error) => {
                        let _ = msg_tx.send(Msg::RepoOpenedErr {
                            repo_id,
                            spec,
                            error,
                        });
                    }
                }
            });
        }

        Effect::LoadBranches { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::BranchesLoaded {
                        repo_id,
                        result: repo.list_branches(),
                    });
                });
            }
        }

        Effect::LoadRemotes { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RemotesLoaded {
                        repo_id,
                        result: repo.list_remotes(),
                    });
                });
            }
        }

        Effect::LoadStatus { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::StatusLoaded {
                        repo_id,
                        result: repo.status(),
                    });
                });
            }
        }

        Effect::LoadHeadLog {
            repo_id,
            limit,
            cursor,
        } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let cursor_ref = cursor.as_ref();
                    let _ = msg_tx.send(Msg::LogLoaded {
                        repo_id,
                        result: repo.log_head_page(limit, cursor_ref),
                    });
                });
            }
        }
    }
}

fn default_worker_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get().clamp(1, 8))
        .unwrap_or(2)
}

struct TaskExecutor {
    tx: mpsc::Sender<Box<dyn FnOnce() + Send + 'static>>,
    _threads: Vec<thread::JoinHandle<()>>,
}

impl TaskExecutor {
    fn new(threads: usize) -> Self {
        let (tx, rx) = mpsc::channel::<Box<dyn FnOnce() + Send + 'static>>();
        let rx = Arc::new(std::sync::Mutex::new(rx));

        let mut worker_threads = Vec::with_capacity(threads);
        for _ in 0..threads {
            let rx = Arc::clone(&rx);
            worker_threads.push(thread::spawn(move || loop {
                let task = {
                    let rx = rx.lock().expect("executor lock poisoned");
                    rx.recv()
                };
                match task {
                    Ok(task) => task(),
                    Err(_) => break,
                }
            }));
        }

        Self {
            tx,
            _threads: worker_threads,
        }
    }

    fn spawn(&self, task: impl FnOnce() + Send + 'static) {
        let _ = self.tx.send(Box::new(task));
    }
}

#[allow(dead_code)]
fn validate_repo_path(path: &Path) -> Result<PathBuf, Error> {
    path.canonicalize()
        .map_err(|e| Error::new(ErrorKind::Io(e.kind())))
}
