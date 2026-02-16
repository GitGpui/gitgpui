use super::*;

pub(super) fn panel(
    this: &mut PopoverHost,
    repo_id: RepoId,
    path: std::path::PathBuf,
    cx: &mut gpui::Context<PopoverHost>,
) -> gpui::Div {
    let theme = this.theme;
    let repo = this.state.repos.iter().find(|r| r.id == repo_id);
    let title: SharedString = path.display().to_string().into();

    let header = div()
        .px_2()
        .py_1()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .flex_col()
                .min_w(px(0.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .child("File history"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.colors.text_muted)
                        .line_clamp(1)
                        .whitespace_nowrap()
                        .child(title),
                ),
        )
        .child(
            zed::Button::new("file_history_close", "Close")
                .style(zed::ButtonStyle::Outlined)
                .on_click(theme, cx, |this, _e, _w, cx| {
                    this.popover = None;
                    this.popover_anchor = None;
                    cx.notify();
                }),
        );

    let body: AnyElement = match repo.map(|r| &r.file_history) {
        None => zed::context_menu_label(theme, "No repository").into_any_element(),
        Some(Loadable::Loading) => zed::context_menu_label(theme, "Loading").into_any_element(),
        Some(Loadable::Error(e)) => zed::context_menu_label(theme, e.clone()).into_any_element(),
        Some(Loadable::NotLoaded) => {
            zed::context_menu_label(theme, "Not loaded").into_any_element()
        }
        Some(Loadable::Ready(page)) => {
            let commit_ids = page
                .commits
                .iter()
                .map(|c| c.id.clone())
                .collect::<Vec<_>>();
            let items = page
                .commits
                .iter()
                .map(|c| {
                    let sha = c.id.as_ref();
                    let short = sha.get(0..8).unwrap_or(sha);
                    format!("{short}  {}", c.summary).into()
                })
                .collect::<Vec<SharedString>>();

            if let Some(search) = this.file_history_search_input.clone() {
                zed::PickerPrompt::new(search)
                    .items(items)
                    .empty_text("No commits")
                    .max_height(px(340.0))
                    .render(theme, cx, move |this, ix, _e, _w, cx| {
                        let Some(commit_id) = commit_ids.get(ix).cloned() else {
                            return;
                        };
                        this.store.dispatch(Msg::SelectCommit {
                            repo_id,
                            commit_id: commit_id.clone(),
                        });
                        this.store.dispatch(Msg::SelectDiff {
                            repo_id,
                            target: DiffTarget::Commit {
                                commit_id,
                                path: Some(path.clone()),
                            },
                        });
                        this.popover = None;
                        this.popover_anchor = None;
                        cx.notify();
                    })
                    .into_any_element()
            } else {
                zed::context_menu_label(theme, "Search input not initialized").into_any_element()
            }
        }
    };

    zed::context_menu(
        theme,
        div()
            .flex()
            .flex_col()
            .w(px(520.0))
            .max_w(px(820.0))
            .child(header)
            .child(div().border_t_1().border_color(theme.colors.border))
            .child(body),
    )
}
