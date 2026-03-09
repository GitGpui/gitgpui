use super::GixRepo;
use super::git_ops::GitOps;
use gitcomet_core::domain::Branch;
use gitcomet_core::services::Result;

impl GixRepo {
    pub(super) fn current_branch_impl(&self) -> Result<String> {
        GitOps::new(self).current_branch()
    }

    pub(super) fn list_branches_impl(&self) -> Result<Vec<Branch>> {
        GitOps::new(self).list_branches()
    }
}
