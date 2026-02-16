use super::*;

pub(super) fn panel(
    this: &mut PopoverHost,
    repo_id: RepoId,
    cx: &mut gpui::Context<PopoverHost>,
) -> gpui::Div {
    let theme = this.theme;

    if let Some(repo) = this.state.repos.iter().find(|r| r.id == repo_id) {
        match &repo.submodules {
            Loadable::Loading => zed::context_menu_label(theme, "Loading"),
            Loadable::NotLoaded => zed::context_menu_label(theme, "Not loaded"),
            Loadable::Error(e) => zed::context_menu_label(theme, e.clone()),
            Loadable::Ready(subs) => {
                let items = subs
                    .iter()
                    .map(|s| s.path.display().to_string().into())
                    .collect::<Vec<SharedString>>();
                let paths = subs.iter().map(|s| s.path.clone()).collect::<Vec<_>>();

                if let Some(search) = this.submodule_picker_search_input.clone() {
                    zed::context_menu(
                        theme,
                        zed::PickerPrompt::new(search)
                            .items(items)
                            .empty_text("No submodules")
                            .max_height(px(260.0))
                            .render(theme, cx, move |this, ix, e, window, cx| {
                                let Some(path) = paths.get(ix).cloned() else {
                                    return;
                                };
                                this.open_popover_at(
                                    PopoverKind::SubmoduleRemoveConfirm { repo_id, path },
                                    e.position(),
                                    window,
                                    cx,
                                );
                            }),
                    )
                    .w(px(520.0))
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
