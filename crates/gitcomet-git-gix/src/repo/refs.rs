use super::GixRepo;
use super::git_ops::{GitOpMode, GitOps};
use gitcomet_core::domain::Branch;
use gitcomet_core::services::Result;

impl GixRepo {
    pub(super) fn current_branch_impl(&self) -> Result<String> {
        GitOps::new(self).current_branch(GitOpMode::PreferGixWithFallback)
    }

    pub(super) fn list_branches_impl(&self) -> Result<Vec<Branch>> {
        GitOps::new(self).list_branches(GitOpMode::PreferGixWithFallback)
    }
}
