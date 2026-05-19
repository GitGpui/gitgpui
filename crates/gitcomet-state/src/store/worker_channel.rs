use crate::msg::Msg;
#[cfg(any(test, feature = "test-support"))]
use gitcomet_core::services::GitRepository;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};

#[cfg(any(test, feature = "test-support"))]
use super::RepoId;
use super::send_diagnostics::{self, SendFailureKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StoreInstanceId(u64);

impl StoreInstanceId {
    pub(super) fn next() -> Self {
        static NEXT_STORE_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_STORE_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub(super) fn get(self) -> u64 {
        self.0
    }
}

pub(super) enum StoreWorkerCommand {
    Msg(Msg),
    Shutdown,
    #[cfg(any(test, feature = "test-support"))]
    InsertRepoForTest {
        repo_id: RepoId,
        repo: Arc<dyn GitRepository>,
    },
}

#[derive(Clone)]
enum StoreWorkerSenderInner {
    Command(mpsc::Sender<StoreWorkerCommand>),
    #[cfg(test)]
    MsgForTest(mpsc::Sender<Msg>),
}

#[derive(Clone)]
pub(super) struct StoreWorkerSender {
    inner: StoreWorkerSenderInner,
    alive: Arc<AtomicBool>,
    store_id: StoreInstanceId,
}

impl StoreWorkerSender {
    pub(super) fn new(
        tx: mpsc::Sender<StoreWorkerCommand>,
        alive: Arc<AtomicBool>,
        store_id: StoreInstanceId,
    ) -> Self {
        Self {
            inner: StoreWorkerSenderInner::Command(tx),
            alive,
            store_id,
        }
    }

    #[cfg(test)]
    pub(super) fn for_test_msg_sender(tx: mpsc::Sender<Msg>) -> Self {
        Self {
            inner: StoreWorkerSenderInner::MsgForTest(tx),
            alive: Arc::new(AtomicBool::new(true)),
            store_id: StoreInstanceId(0),
        }
    }

    pub(super) fn store_id(&self) -> StoreInstanceId {
        self.store_id
    }

    pub(super) fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    pub(super) fn dispatch(&self, msg: Msg) {
        self.send_or_log(
            msg,
            SendFailureKind::StoreDispatch,
            "AppStore::dispatch",
            false,
        );
    }

    pub(super) fn send_effect_or_log(&self, msg: Msg, context: &'static str) {
        self.send_or_log(msg, SendFailureKind::EffectMessage, context, true);
    }

    pub(super) fn send_repo_monitor_or_log(&self, msg: Msg, context: &'static str) {
        self.send_or_log(msg, SendFailureKind::RepoMonitorMessage, context, true);
    }

    fn send_or_log(
        &self,
        msg: Msg,
        kind: SendFailureKind,
        context: &'static str,
        suppress_after_shutdown: bool,
    ) {
        if suppress_after_shutdown && !self.is_alive() {
            return;
        }

        match &self.inner {
            StoreWorkerSenderInner::Command(tx) => {
                send_diagnostics::send_or_log(tx, StoreWorkerCommand::Msg(msg), kind, context)
            }
            #[cfg(test)]
            StoreWorkerSenderInner::MsgForTest(tx) => {
                send_diagnostics::send_or_log(tx, msg, kind, context)
            }
        }
    }

    pub(super) fn shutdown(&self) {
        if !self.alive.swap(false, Ordering::AcqRel) {
            return;
        }

        match &self.inner {
            StoreWorkerSenderInner::Command(tx) => {
                let _ = tx.send(StoreWorkerCommand::Shutdown);
            }
            #[cfg(test)]
            StoreWorkerSenderInner::MsgForTest(_) => {}
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn insert_repo_for_test(&self, repo_id: RepoId, repo: Arc<dyn GitRepository>) {
        if !self.is_alive() {
            return;
        }

        match &self.inner {
            StoreWorkerSenderInner::Command(tx) => {
                let _ = tx.send(StoreWorkerCommand::InsertRepoForTest { repo_id, repo });
            }
            #[cfg(test)]
            StoreWorkerSenderInner::MsgForTest(_) => {}
        }
    }
}
