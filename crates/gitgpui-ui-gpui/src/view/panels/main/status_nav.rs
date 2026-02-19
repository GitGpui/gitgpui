use super::*;

impl MainPaneView {
    pub(super) fn status_prev_next_indices(
        entries: &[gitgpui_core::domain::FileStatus],
        path: &std::path::Path,
    ) -> (Option<usize>, Option<usize>) {
        let Some(current_ix) = entries.iter().position(|e| e.path == path) else {
            return (None, None);
        };

        let prev_ix = current_ix.checked_sub(1);
        let next_ix = (current_ix + 1 < entries.len()).then_some(current_ix + 1);
        (prev_ix, next_ix)
    }

    pub(super) fn try_select_adjacent_status_file(
        &mut self,
        repo_id: RepoId,
        direction: i8,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        let Some((area, target_ix, target_path)) = (|| {
            let repo = self.active_repo()?;
            let DiffTarget::WorkingTree { path, area } = repo.diff_target.as_ref()? else {
                return None;
            };
            let area = *area;
            let Loadable::Ready(status) = &repo.status else {
                return None;
            };

            let entries = match area {
                DiffArea::Unstaged => status.unstaged.as_slice(),
                DiffArea::Staged => status.staged.as_slice(),
            };

            let (prev_ix, next_ix) = Self::status_prev_next_indices(entries, path.as_path());
            let target_ix = if direction < 0 { prev_ix } else { next_ix }?;
            let target_path = entries.get(target_ix)?.path.clone();

            Some((area, target_ix, target_path))
        })() else {
            return false;
        };

        window.focus(&self.diff_panel_focus_handle);
        self.clear_status_multi_selection(repo_id, cx);
        self.store.dispatch(Msg::SelectDiff {
            repo_id,
            target: DiffTarget::WorkingTree {
                path: target_path,
                area,
            },
        });
        self.scroll_status_list_to_ix(area, target_ix, cx);

        true
    }
}
