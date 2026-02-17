use super::*;

pub(super) fn panel(
    this: &mut PopoverHost,
    repo_id: RepoId,
    remote: Option<String>,
    cx: &mut gpui::Context<PopoverHost>,
) -> gpui::Div {
    let theme = this.theme;

    let Some(repo) = this.state.repos.iter().find(|r| r.id == repo_id) else {
        return zed::context_menu(
            theme,
            zed::context_menu_label(theme, "Repository not found"),
        )
        .w(px(520.0))
        .max_w(px(820.0));
    };

    let branches = match &repo.remote_branches {
        Loadable::Ready(branches) => branches,
        Loadable::Loading => {
            return zed::context_menu(
                theme,
                zed::context_menu_label(theme, "Loading remote branches"),
            )
            .w(px(520.0))
            .max_w(px(820.0));
        }
        Loadable::NotLoaded => {
            return zed::context_menu(
                theme,
                zed::context_menu_label(theme, "Remote branches not loaded"),
            )
            .w(px(520.0))
            .max_w(px(820.0));
        }
        Loadable::Error(e) => {
            return zed::context_menu(theme, zed::context_menu_label(theme, e.clone()))
                .w(px(520.0))
                .max_w(px(820.0));
        }
    };

    let mut entries: Vec<(SharedString, String, String)> = branches
        .iter()
        .filter(|branch| {
            remote
                .as_ref()
                .is_none_or(|filter| filter.as_str() == branch.remote.as_str())
        })
        .map(|branch| {
            let full: SharedString = format!("{}/{}", branch.remote, branch.name).into();
            (full, branch.remote.clone(), branch.name.clone())
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let items: Vec<SharedString> = entries.iter().map(|(label, ..)| label.clone()).collect();

    if let Some(search) = this.branch_picker_search_input.clone() {
        zed::context_menu(
            theme,
            zed::PickerPrompt::new(search)
                .items(items)
                .empty_text("No remote branches")
                .max_height(px(260.0))
                .render(theme, cx, move |this, ix, e, window, cx| {
                    let Some((_, remote, branch)) = entries.get(ix).cloned() else {
                        return;
                    };
                    this.open_popover_at(
                        PopoverKind::DeleteRemoteBranchConfirm {
                            repo_id,
                            remote,
                            branch,
                        },
                        e.position(),
                        window,
                        cx,
                    );
                }),
        )
        .w(px(520.0))
        .max_w(px(820.0))
    } else {
        let close = cx.listener(|this, _e: &ClickEvent, _w, cx| this.close_popover(cx));

        let mut menu = div().flex().flex_col().min_w(px(520.0)).max_w(px(820.0));
        for (ix, (label, remote, branch)) in entries.into_iter().enumerate() {
            menu = menu.child(
                div()
                    .id(("remote_branch_delete_item", ix))
                    .px_2()
                    .py_1()
                    .hover(move |s| s.bg(theme.colors.hover))
                    .child(div().text_sm().line_clamp(1).child(label))
                    .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                        this.open_popover_at(
                            PopoverKind::DeleteRemoteBranchConfirm {
                                repo_id,
                                remote: remote.clone(),
                                branch: branch.clone(),
                            },
                            e.position(),
                            window,
                            cx,
                        );
                    })),
            );
        }
        menu.child(
            div()
                .id("remote_branch_delete_close")
                .px_2()
                .py_1()
                .hover(move |s| s.bg(theme.colors.hover))
                .child("Close")
                .on_click(close),
        )
    }
}
