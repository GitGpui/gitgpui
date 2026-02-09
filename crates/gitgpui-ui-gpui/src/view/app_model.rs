use super::*;

#[derive(Debug)]
pub(super) struct AppUiModel {
    pub(super) state: Arc<AppState>,
    pub(super) seq: u64,
}

impl AppUiModel {
    pub(super) fn new(state: Arc<AppState>) -> Self {
        Self { state, seq: 0 }
    }

    pub(super) fn set_state(&mut self, state: Arc<AppState>, cx: &mut gpui::Context<Self>) {
        self.state = state;
        self.seq = self.seq.wrapping_add(1);
        cx.notify();
    }
}
