use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RepoPath(Arc<PathBuf>);

impl RepoPath {
    pub fn new(path: PathBuf) -> Self {
        Self(Arc::new(path))
    }

    pub fn from_shared(path: Arc<PathBuf>) -> Self {
        Self(path)
    }

    pub fn as_path(&self) -> &Path {
        self.0.as_path()
    }
}

impl AsRef<Path> for RepoPath {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl From<PathBuf> for RepoPath {
    fn from(path: PathBuf) -> Self {
        Self::new(path)
    }
}

impl From<Arc<PathBuf>> for RepoPath {
    fn from(path: Arc<PathBuf>) -> Self {
        Self::from_shared(path)
    }
}
