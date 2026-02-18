use super::*;

pub(super) struct Poller {
    _task: gpui::Task<()>,
}

impl Poller {
    pub(super) fn start(
        store: Arc<AppStore>,
        events: smol::channel::Receiver<StoreEvent>,
        model: WeakEntity<AppUiModel>,
        window: &mut Window,
        cx: &mut gpui::Context<GitGpuiView>,
    ) -> Poller {
        let task = window.spawn(cx, async move |cx| {
            loop {
                if events.recv().await.is_err() {
                    break;
                }
                while events.try_recv().is_ok() {}

                // Avoid blocking the UI thread on cloning large state.
                // This still does a full snapshot clone today, but now it happens only when the
                // store reports state changes (no 10ms polling loop), and coalescing ensures
                // we do at most one pending update.
                let snapshot = smol::unblock({
                    let store = Arc::clone(&store);
                    move || store.snapshot()
                })
                .await;

                let _ = model.update(cx, |model, cx| model.set_state(Arc::new(snapshot), cx));
            }
        });

        Poller { _task: task }
    }
}
