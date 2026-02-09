use super::*;

pub(super) fn panel(
    this: &mut GitGpuiView,
    repo_id: RepoId,
    cx: &mut gpui::Context<GitGpuiView>,
) -> gpui::Div {
    let theme = this.theme;

    if let Some(repo) = this.state.repos.iter().find(|r| r.id == repo_id) {
        match &repo.submodules {
            Loadable::Loading => zed::context_menu_label(theme, "Loading"),
            Loadable::NotLoaded => zed::context_menu_label(theme, "Not loaded"),
            Loadable::Error(e) => zed::context_menu_label(theme, e.clone()),
            Loadable::Ready(subs) => {
                let base = repo.spec.workdir.clone();
                let items = subs
                    .iter()
                    .map(|s| s.path.display().to_string().into())
                    .collect::<Vec<SharedString>>();
                let paths = subs.iter().map(|s| base.join(&s.path)).collect::<Vec<_>>();

                if let Some(search) = this.submodule_picker_search_input.clone() {
                    zed::context_menu(
                        theme,
                        zed::PickerPrompt::new(search)
                            .items(items)
                            .empty_text("No submodules")
                            .max_height(px(260.0))
                            .render(theme, cx, move |this, ix, _e, _w, cx| {
                                let Some(path) = paths.get(ix).cloned() else {
                                    return;
                                };
                                this.store.dispatch(Msg::OpenRepo(path));
                                this.popover = None;
                                this.popover_anchor = None;
                                cx.notify();
                            }),
                    )
                    .min_w(px(420.0))
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
