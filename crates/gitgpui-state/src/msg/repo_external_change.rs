#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepoExternalChange {
    Worktree,
    GitState,
    Both,
}
