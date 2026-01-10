use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct RepoSpec {
    pub workdir: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct CommitId(pub String);

impl AsRef<str> for CommitId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Commit {
    pub id: CommitId,
    pub parent_ids: Vec<CommitId>,
    pub summary: String,
    pub author: String,
    pub time: SystemTime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Branch {
    pub name: String,
    pub target: CommitId,
    pub upstream: Option<Upstream>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Upstream {
    pub remote: String,
    pub branch: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Remote {
    pub name: String,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileStatus {
    pub path: PathBuf,
    pub kind: FileStatusKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileStatusKind {
    Untracked,
    Modified,
    Added,
    Deleted,
    Renamed,
    Conflicted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StashEntry {
    pub index: usize,
    pub message: String,
    pub created_at: Option<SystemTime>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogPage {
    pub commits: Vec<Commit>,
    pub next_cursor: Option<LogCursor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogCursor {
    pub last_seen: CommitId,
}
