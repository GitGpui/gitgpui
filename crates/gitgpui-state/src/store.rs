use crate::model::{AppState, DiagnosticEntry, DiagnosticKind, Loadable, RepoId, RepoState};
use crate::msg::{Effect, Msg, StoreEvent};
use crate::session;
use gitgpui_core::domain::{Diff, RepoSpec};
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::{GitBackend, GitRepository};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock, mpsc};
use std::thread;
use std::time::SystemTime;

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
                    let mut app_state = thread_state.write().expect("state lock poisoned (write)");

                    reduce(&mut repos, &id_alloc, &mut app_state, msg)
                };

                let _ = event_tx.send(StoreEvent::StateChanged);

                for effect in effects {
                    schedule_effect(&executor, &backend, &repos, thread_msg_tx.clone(), effect);
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
            let path = normalize_repo_path(path);
            let repo_id = RepoId(id_alloc.fetch_add(1, Ordering::Relaxed));
            let spec = RepoSpec { workdir: path };

            state
                .repos
                .push(RepoState::new_opening(repo_id, spec.clone()));
            state.active_repo = Some(repo_id);
            let effects = vec![Effect::OpenRepo {
                repo_id,
                path: spec.workdir.clone(),
            }];
            let _ = session::persist_from_state(state);
            effects
        }

        Msg::RestoreSession {
            open_repos,
            active_repo,
        } => {
            repos.clear();
            state.repos.clear();
            state.active_repo = None;

            let active_repo = active_repo.map(normalize_repo_path);
            let mut active_repo_id: Option<RepoId> = None;

            let mut effects = Vec::new();
            for path in dedup_paths_in_order(open_repos).into_iter().map(normalize_repo_path) {
                if state.repos.iter().any(|r| r.spec.workdir == path) {
                    continue;
                }
                let repo_id = RepoId(id_alloc.fetch_add(1, Ordering::Relaxed));
                let spec = RepoSpec { workdir: path };
                if active_repo_id.is_none()
                    && active_repo.as_ref().is_some_and(|active| active == &spec.workdir)
                {
                    active_repo_id = Some(repo_id);
                }

                state
                    .repos
                    .push(RepoState::new_opening(repo_id, spec.clone()));
                effects.push(Effect::OpenRepo {
                    repo_id,
                    path: spec.workdir.clone(),
                });
            }

            state.active_repo = if let Some(active_repo_id) = active_repo_id {
                Some(active_repo_id)
            } else {
                state.repos.last().map(|r| r.id)
            };

            let _ = session::persist_from_state(state);
            effects
        }

        Msg::CloseRepo { repo_id } => {
            state.repos.retain(|r| r.id != repo_id);
            repos.remove(&repo_id);
            if state.active_repo == Some(repo_id) {
                state.active_repo = state.repos.first().map(|r| r.id);
            }
            let _ = session::persist_from_state(state);
            Vec::new()
        }

        Msg::SetActiveRepo { repo_id } => {
            if state.repos.iter().any(|r| r.id == repo_id) {
                state.active_repo = Some(repo_id);
                let _ = session::persist_from_state(state);
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
            repo_state.selected_commit = None;
            repo_state.commit_details = Loadable::NotLoaded;

            refresh_effects(repo_id)
        }

        Msg::SelectCommit { repo_id, commit_id } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };

            repo_state.selected_commit = Some(commit_id.clone());
            repo_state.commit_details = Loadable::Loading;
            vec![Effect::LoadCommitDetails { repo_id, commit_id }]
        }

        Msg::ClearCommitSelection { repo_id } => {
            let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) else {
                return Vec::new();
            };

            repo_state.selected_commit = None;
            repo_state.commit_details = Loadable::NotLoaded;
            Vec::new()
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

            let spec = RepoSpec {
                workdir: normalize_repo_path(spec.workdir),
            };
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.spec = spec;
                repo_state.open = Loadable::Ready(());
                repo_state.head_branch = Loadable::Loading;
                repo_state.branches = Loadable::Loading;
                repo_state.remotes = Loadable::Loading;
                repo_state.remote_branches = Loadable::Loading;
                repo_state.status = Loadable::Loading;
                repo_state.log = Loadable::Loading;
                repo_state.selected_commit = None;
                repo_state.commit_details = Loadable::NotLoaded;
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
            let spec = RepoSpec {
                workdir: normalize_repo_path(spec.workdir),
            };
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.spec = spec;
                repo_state.open = Loadable::Error(error.to_string());
                repo_state.last_error = Some(error.to_string());
                push_diagnostic(repo_state, DiagnosticKind::Error, error.to_string());
            }
            Vec::new()
        }

        Msg::BranchesLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.branches = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::RemotesLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.remotes = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::RemoteBranchesLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.remote_branches = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::StatusLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.status = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::HeadBranchLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.head_branch = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::LogLoaded { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                repo_state.log = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::CommitDetailsLoaded {
            repo_id,
            commit_id,
            result,
        } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id)
                && repo_state.selected_commit.as_ref() == Some(&commit_id)
            {
                repo_state.commit_details = match result {
                    Ok(v) => Loadable::Ready(v),
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
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
                    Err(e) => {
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                        Loadable::Error(e.to_string())
                    }
                };
            }
            Vec::new()
        }

        Msg::RepoActionFinished { repo_id, result } => {
            if let Some(repo_state) = state.repos.iter_mut().find(|r| r.id == repo_id) {
                match result {
                    Ok(()) => repo_state.last_error = None,
                    Err(e) => {
                        repo_state.last_error = Some(e.to_string());
                        push_diagnostic(repo_state, DiagnosticKind::Error, e.to_string());
                    }
                }
            }
            refresh_effects(repo_id)
        }
    }
}

fn dedup_paths_in_order(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for p in paths {
        if out.iter().any(|x| x == &p) {
            continue;
        }
        out.push(p);
    }
    out
}

fn normalize_repo_path(path: PathBuf) -> PathBuf {
    let path = if path.is_relative() {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    } else {
        path
    };

    std::fs::canonicalize(&path).unwrap_or(path)
}

fn push_diagnostic(repo_state: &mut RepoState, kind: DiagnosticKind, message: String) {
    const MAX_DIAGNOSTICS: usize = 200;
    repo_state.diagnostics.push(DiagnosticEntry {
        time: SystemTime::now(),
        kind,
        message,
    });
    if repo_state.diagnostics.len() > MAX_DIAGNOSTICS {
        let extra = repo_state.diagnostics.len() - MAX_DIAGNOSTICS;
        repo_state.diagnostics.drain(0..extra);
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
    use gitgpui_core::domain::{
        Branch, CommitDetails, CommitId, DiffTarget, LogCursor, LogPage, Remote, RemoteBranch,
        RepoStatus, StashEntry,
    };
    use gitgpui_core::services::{PullMode, Result};
    use std::sync::Arc;

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

        assert_eq!(active_workdir, super::normalize_repo_path(repo_a));
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
        assert!(repo_state.remotes.is_loading());
        assert!(repo_state.remote_branches.is_loading());
        assert!(repo_state.status.is_loading());
        assert!(repo_state.log.is_loading());
        assert!(matches!(
            effects.as_slice(),
            [
                Effect::LoadHeadBranch { .. },
                Effect::LoadBranches { .. },
                Effect::LoadRemotes { .. },
                Effect::LoadRemoteBranches { .. },
                Effect::LoadStatus { .. },
                Effect::LoadHeadLog { .. }
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
        let target = DiffTarget {
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
            push_diagnostic(&mut repo_state, DiagnosticKind::Error, format!("err-{i}"));
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
        assert!(repo_state.remotes.is_loading());
        assert!(repo_state.remote_branches.is_loading());
        assert!(repo_state.status.is_loading());
        assert!(repo_state.log.is_loading());
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::LoadStatus { repo_id: RepoId(1) }))
        );
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
                        let _ = msg_tx.send(Msg::RepoOpenedOk {
                            repo_id,
                            spec,
                            repo,
                        });
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

        Effect::LoadCommitDetails { repo_id, commit_id } => {
            if let Some(repo) = repos.get(&repo_id).cloned() {
                executor.spawn(move || {
                    let _ = msg_tx.send(Msg::CommitDetailsLoaded {
                        repo_id,
                        commit_id: commit_id.clone(),
                        result: repo.commit_details(&commit_id),
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
            worker_threads.push(thread::spawn(move || {
                loop {
                    let task = {
                        let rx = rx.lock().expect("executor lock poisoned");
                        rx.recv()
                    };
                    match task {
                        Ok(task) => task(),
                        Err(_) => break,
                    }
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
