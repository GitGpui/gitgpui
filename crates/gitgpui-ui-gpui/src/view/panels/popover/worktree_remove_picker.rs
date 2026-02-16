use super::*;

pub(super) fn panel(
    this: &mut PopoverHost,
    repo_id: RepoId,
    cx: &mut gpui::Context<PopoverHost>,
) -> gpui::Div {
    let theme = this.theme;

    if let Some(repo) = this.state.repos.iter().find(|r| r.id == repo_id) {
        match &repo.worktrees {
            Loadable::Loading => zed::context_menu_label(theme, "Loading"),
            Loadable::NotLoaded => zed::context_menu_label(theme, "Not loaded"),
            Loadable::Error(e) => zed::context_menu_label(theme, e.clone()),
            Loadable::Ready(worktrees) => {
                let workdir = repo.spec.workdir.clone();
                let items = worktrees
                    .iter()
                    .filter(|w| w.path != workdir)
                    .map(|w| w.path.display().to_string().into())
                    .collect::<Vec<SharedString>>();
                let paths = worktrees
                    .iter()
                    .filter(|w| w.path != workdir)
                    .map(|w| w.path.clone())
                    .collect::<Vec<_>>();

                if let Some(search) = this.worktree_picker_search_input.clone() {
                    zed::context_menu(
                        theme,
                        zed::PickerPrompt::new(search)
                            .items(items)
                            .empty_text("No worktrees")
                            .max_height(px(260.0))
                            .render(theme, cx, move |this, ix, e, window, cx| {
                                let Some(path) = paths.get(ix).cloned() else {
                                    return;
                                };
                                this.open_popover_at(
                                    PopoverKind::WorktreeRemoveConfirm { repo_id, path },
                                    e.position(),
                                    window,
                                    cx,
                                );
                            }),
                    )
                    .min_w(px(520.0))
                    .max_w(px(820.0))
                } else {
                    zed::context_menu_label(theme, "Search input not initialized")
                }
            }
        }
    } else {
        zed::context_menu_label(theme, "No repository")
    }
}
