use crate::model::{AppState, RepoId};
use crate::msg::{Msg, StoreEvent};
use gitgpui_core::services::{GitBackend, GitRepository};
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock, mpsc};
use std::thread;

mod effects;
mod executor;
mod reducer;

use effects::schedule_effect;
use executor::{TaskExecutor, default_worker_threads};
use reducer::reduce;

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

#[cfg(test)]
mod tests;
