use gitgpui_core::services::GitBackend;
use std::sync::Arc;

pub fn run(_backend: Arc<dyn GitBackend>) {
    // This crate holds UI-adjacent state that is intentionally independent of the concrete UI
    // toolkit. The gpui implementation lives in `gitgpui-ui-gpui`.
    eprintln!("GitGpui UI core is wired. Build with `--features ui-gpui` to enable gpui.");
}
