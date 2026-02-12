mod noop_backend;

pub use noop_backend::NoopBackend;

use gitgpui_core::services::GitBackend;
use gitgpui_core::services::{GitRepository, Result};
use std::path::Path;
use std::sync::Arc;

pub fn default_backend() -> Arc<dyn GitBackend> {
    Arc::new(NoopBackend)
}

pub fn open_repo(workdir: &Path) -> Result<Arc<dyn GitRepository>> {
    default_backend().open(workdir)
}
