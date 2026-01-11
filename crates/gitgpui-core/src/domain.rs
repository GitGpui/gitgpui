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
pub struct CommitDetails {
    pub id: CommitId,
    pub message: String,
    pub committed_at: String,
    pub parent_ids: Vec<CommitId>,
    pub files: Vec<PathBuf>,
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
pub struct RemoteBranch {
    pub remote: String,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileStatus {
    pub path: PathBuf,
    pub kind: FileStatusKind,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RepoStatus {
    pub staged: Vec<FileStatus>,
    pub unstaged: Vec<FileStatus>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiffArea {
    Staged,
    Unstaged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffTarget {
    pub path: PathBuf,
    pub area: DiffArea,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diff {
    pub target: DiffTarget,
    pub lines: Vec<DiffLine>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiffLineKind {
    Header,
    Hunk,
    Add,
    Remove,
    Context,
}

impl Diff {
    pub fn from_unified(target: DiffTarget, text: &str) -> Self {
        let mut lines = Vec::new();

        for raw in text.lines() {
            let kind = if raw.starts_with("@@") {
                DiffLineKind::Hunk
            } else if raw.starts_with("diff ")
                || raw.starts_with("index ")
                || raw.starts_with("--- ")
                || raw.starts_with("+++ ")
                || raw.starts_with("new file mode ")
                || raw.starts_with("deleted file mode ")
                || raw.starts_with("similarity index ")
                || raw.starts_with("rename from ")
                || raw.starts_with("rename to ")
                || raw.starts_with("Binary files ")
            {
                DiffLineKind::Header
            } else if raw.starts_with('+') && !raw.starts_with("+++") {
                DiffLineKind::Add
            } else if raw.starts_with('-') && !raw.starts_with("---") {
                DiffLineKind::Remove
            } else {
                DiffLineKind::Context
            };

            lines.push(DiffLine {
                kind,
                text: raw.to_string(),
            });
        }

        Self { target, lines }
    }
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
