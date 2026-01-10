use crate::model::{AppState, Loadable, RepoId, RepoState};
use crate::msg::{Effect, Msg, StoreEvent};
use gitgpui_core::domain::{Diff, RepoSpec};
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

            state.repos.push(RepoState::new_opening(repo_id, spec.clone()));
            state.active_repo = Some(repo_id);
            vec![Effect::OpenRepo {
                repo_id,
                path: spec.workdir.clone(),
            }]
        }

        Msg::CloseRepo { repo_id } => {
            state.repos.retain(|r| r.id != repo_id);
            repos.remove(&repo_id);
            if state.active_repo == Some(repo_id) {
                state.active_repo = state.repos.first().map(|r| r.id);
            }
            Vec::new()
        }

        Msg::SetActiveRepo { repo_id } => {
            if state.repos.iter().any(|r| r.id == repo_id) {
                state.active_repo = Some(repo_id);
            }
            Vec::new()
        }

        Msg::ReloadRepo { repo_id } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };

            repo_state.head_branch = Loadable::Loading;
            repo_state.branches = Loadable::Loading;
            repo_state.remotes = Loadable::Loading;
            repo_state.remote_branches = Loadable::Loading;
            repo_state.status = Loadable::Loading;
            repo_state.log = Loadable::Loading;

            refresh_effects(repo_id)
        }

        Msg::SelectDiff { repo_id, target } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };

            repo_state.diff_target = Some(target.clone());
            repo_state.diff = Loadable::Loading;
            vec![Effect::LoadDiff { repo_id, target }]
        }

        Msg::ClearDiffSelection { repo_id } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };

            repo_state.diff_target = None;
            repo_state.diff = Loadable::NotLoaded;
            Vec::new()
        }

        Msg::CheckoutBranch { repo_id, name } => vec![Effect::CheckoutBranch { repo_id, name }],
        Msg::CreateBranch { repo_id, name } => vec![Effect::CreateBranch { repo_id, name }],
        Msg::StagePath { repo_id, path } => vec![Effect::StagePath { repo_id, path }],
        Msg::UnstagePath { repo_id, path } => vec![Effect::UnstagePath { repo_id, path }],
        Msg::Commit { repo_id, message } => vec![Effect::Commit { repo_id, message }],
        Msg::FetchAll { repo_id } => vec![Effect::FetchAll { repo_id }],
        Msg::Pull { repo_id, mode } => vec![Effect::Pull { repo_id, mode }],
        Msg::Push { repo_id } => vec![Effect::Push { repo_id }],
        Msg::Stash {
            repo_id,
            message,
            include_untracked,
        } => vec![Effect::Stash {
            repo_id,
            message,
            include_untracked,
        }],

        Msg::RepoOpenedOk {
            repo_id,
            spec,
            repo,
        } => {
            repos.insert(repo_id, repo);

            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.spec = spec;
                repo_state.open = Loadable::Ready(());
                repo_state.head_branch = Loadable::Loading;
                repo_state.branches = Loadable::Loading;
                repo_state.remotes = Loadable::Loading;
                repo_state.remote_branches = Loadable::Loading;
                repo_state.status = Loadable::Loading;
                repo_state.log = Loadable::Loading;
                repo_state.diff_target = None;
                repo_state.diff = Loadable::NotLoaded;
                repo_state.last_error = None;
            }

            refresh_effects(repo_id)
        }

        Msg::RepoOpenedErr {
            repo_id,
            spec,
            error,
        } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.spec = spec;
                repo_state.open = Loadable::Error(error.to_string());
                repo_state.last_error = Some(error.to_string());
            }
            Vec::new()
        }

        Msg::BranchesLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.branches = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => Loadable::Error(e.to_string()),
                };
            }
            Vec::new()
        }

        Msg::RemotesLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.remotes = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => Loadable::Error(e.to_string()),
                };
            }
            Vec::new()
        }

        Msg::RemoteBranchesLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.remote_branches = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => Loadable::Error(e.to_string()),
                };
            }
            Vec::new()
        }

        Msg::StatusLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.status = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => Loadable::Error(e.to_string()),
                };
            }
            Vec::new()
        }

        Msg::HeadBranchLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.head_branch = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => Loadable::Error(e.to_string()),
                };
            }
            Vec::new()
        }

        Msg::LogLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.log = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => Loadable::Error(e.to_string()),
                };
            }
            Vec::new()
        }

        Msg::DiffLoaded {
            repo_id,
            target,
            result,
        } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id)
                && repo_state.diff_target.as_ref() == Some(&target)
            {
                repo_state.diff = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => Loadable::Error(e.to_string()),
                };
            }
            Vec::new()
        }

        Msg::RepoActionFinished { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                match result {
                    Ok(()) => repo_state.last_error = None,
                    Err(e) => repo_state.last_error = Some(e.to_string()),
                }
            }
            refresh_effects(repo_id)
        }
    }
}

fn refresh_effects(repo_id: RepoId) -> Vec<Effect> {
    vec![
        Effect::LoadHeadBranch { repo_id },
        Effect::LoadBranches { repo_id },
        Effect::LoadRemotes { repo_id },
        Effect::LoadRemoteBranches { repo_id },
        Effect::LoadStatus { repo_id },
        Effect::LoadHeadLog {
            repo_id,
            limit: 200,
            cursor: None,
        },
    ]
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

        assert_eq!(state.active_repo, Some(RepoId(1)));
        let repo_state = state.repos.first().expect("repo state to be set");
        assert_eq!(repo_state.id.0, 1);
        assert!(repo_state.open.is_loading());
        assert!(matches!(effects.as_slice(), [Effect::OpenRepo { .. }]));
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

        let target = gitgpui_core::domain::DiffTarget {
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
        assert!(matches!(
            effects.as_slice(),
            [Effect::LoadDiff { repo_id: RepoId(1), target: t }] if t == &target
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
        repo_state.diff_target = Some(gitgpui_core::domain::DiffTarget {
            path: PathBuf::from("src/lib.rs"),
            area: gitgpui_core::domain::DiffArea::Unstaged,
        });
        repo_state.diff = Loadable::Loading;
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
        assert!(effects.is_empty());
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

        Effect::LoadRemoteBranches { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RemoteBranchesLoaded {
                        repo_id,
                        result: repo.list_remote_branches(),
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

        Effect::LoadHeadBranch { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::HeadBranchLoaded {
                        repo_id,
                        result: repo.current_branch(),
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

        Effect::LoadDiff { repo_id, target } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let result = repo
                        .diff_unified(&target)
                        .map(|text| Diff::from_unified(target.clone(), &text));
                    let _ = msg_tx.send(Msg::DiffLoaded {
                        repo_id,
                        target,
                        result,
                    });
                });
            }
        }

        Effect::CheckoutBranch { repo_id, name } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.checkout_branch(&name),
                    });
                });
            }
        }

        Effect::CreateBranch { repo_id, name } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let target = gitgpui_core::domain::CommitId("HEAD".to_string());
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.create_branch(&name, &target),
                    });
                });
            }
        }

        Effect::StagePath { repo_id, path } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let path_ref: &Path = &path;
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.stage(&[path_ref]),
                    });
                });
            }
        }

        Effect::UnstagePath { repo_id, path } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let path_ref: &Path = &path;
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.unstage(&[path_ref]),
                    });
                });
            }
        }

        Effect::Commit { repo_id, message } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.commit(&message),
                    });
                });
            }
        }

        Effect::FetchAll { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.fetch_all(),
                    });
                });
            }
        }

        Effect::Pull { repo_id, mode } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.pull(mode),
                    });
                });
            }
        }

        Effect::Push { repo_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.push(),
                    });
                });
            }
        }

        Effect::Stash {
            repo_id,
            message,
            include_untracked,
        } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::RepoActionFinished {
                        repo_id,
                        result: repo.stash_create(&message, include_untracked),
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
