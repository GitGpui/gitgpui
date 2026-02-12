use super::GixRepo;
use crate::util::run_git_with_output;
use gitgpui_core::domain::{CommitId, Tag};
use gitgpui_core::error::{Error, ErrorKind};
use gitgpui_core::services::{CommandOutput, Result};
use gix::bstr::ByteSlice as _;
use std::process::Command;

impl GixRepo {
    pub(super) fn list_tags_impl(&self) -> Result<Vec<Tag>> {
        let repo = self._repo.to_thread_local();

        let refs = repo
            .references()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix references: {e}"))))?;

        let iter = refs
            .tags()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix tags: {e}"))))?
            .peeled()
            .map_err(|e| Error::new(ErrorKind::Backend(format!("gix peel refs: {e}"))))?;

        let mut tags = Vec::new();
        for reference in iter {
            let reference = reference
                .map_err(|e| Error::new(ErrorKind::Backend(format!("gix ref iter: {e}"))))?;
            let name = reference.name().shorten().to_str_lossy().into_owned();
            let target = CommitId(reference.id().detach().to_string());
            tags.push(Tag { name, target });
        }

        tags.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(tags)
    }

    pub(super) fn create_tag_with_output_impl(
        &self,
        name: &str,
        target: &str,
    ) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("-c")
            .arg("alias.tag=")
            .arg("-c")
            .arg("tag.gpgsign=false")
            .arg("-c")
            .arg("tag.forcesignannotated=false")
            .arg("tag")
            .arg("-m")
            .arg(name)
            .arg(name)
            .arg(target);
        run_git_with_output(cmd, &format!("git tag -m {name} {name} {target}"))
    }

    pub(super) fn delete_tag_with_output_impl(&self, name: &str) -> Result<CommandOutput> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.spec.workdir)
            .arg("-c")
            .arg("alias.tag=")
            .arg("tag")
            .arg("-d")
            .arg(name);
        run_git_with_output(cmd, &format!("git tag -d {name}"))
    }
}
