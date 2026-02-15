use super::*;

pub(super) fn panel(
    this: &mut PopoverHost,
    repo_id: RepoId,
    kind: RemoteUrlKind,
    cx: &mut gpui::Context<PopoverHost>,
) -> gpui::Div {
    let theme = this.theme;
    let close = cx.listener(|this, _e: &ClickEvent, _w, cx| this.close_popover(cx));

    let remotes = this
        .active_repo()
        .and_then(|r| match &r.remotes {
            Loadable::Ready(remotes) => Some(remotes.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let items = remotes
        .iter()
        .map(|r| r.name.clone().into())
        .collect::<Vec<_>>();
    let names = remotes.iter().map(|r| r.name.clone()).collect::<Vec<_>>();

    if let Some(search) = this.remote_picker_search_input.clone() {
        zed::context_menu(
            theme,
            zed::PickerPrompt::new(search)
                .items(items)
                .empty_text("No remotes")
                .max_height(px(260.0))
                .render(theme, cx, move |this, ix, e, window, cx| {
                    let Some(name) = names.get(ix).cloned() else {
                        return;
                    };
                    let url = this
                        .active_repo()
                        .and_then(|r| match &r.remotes {
                            Loadable::Ready(remotes) => remotes
                                .iter()
                                .find(|rr| rr.name == name)
                                .and_then(|rr| rr.url.clone()),
                            _ => None,
                        })
                        .unwrap_or_default();
                    this.remote_url_edit_input
                        .update(cx, |i, cx| i.set_text(url, cx));
                    this.open_popover_at(
                        PopoverKind::RemoteEditUrlPrompt {
                            repo_id,
                            name,
                            kind,
                        },
                        e.position(),
                        window,
                        cx,
                    );
                }),
        )
        .min_w(px(420.0))
        .max_w(px(820.0))
    } else {
        let mut menu = div().flex().flex_col().min_w(px(420.0)).max_w(px(820.0));
        for (ix, item) in items.into_iter().enumerate() {
            let name = names.get(ix).cloned().unwrap_or_default();
            menu = menu.child(
                div()
                    .id(("remote_url_item", ix))
                    .px_2()
                    .py_1()
                    .hover(move |s| s.bg(theme.colors.hover))
                    .child(div().text_sm().line_clamp(1).child(item))
                    .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                        let url = this
                            .active_repo()
                            .and_then(|r| match &r.remotes {
                                Loadable::Ready(remotes) => remotes
                                    .iter()
                                    .find(|rr| rr.name == name)
                                    .and_then(|rr| rr.url.clone()),
                                _ => None,
                            })
                            .unwrap_or_default();
                        this.remote_url_edit_input
                            .update(cx, |i, cx| i.set_text(url, cx));
                        this.open_popover_at(
                            PopoverKind::RemoteEditUrlPrompt {
                                repo_id,
                                name: name.clone(),
                                kind,
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
                .id("remote_url_close")
                .px_2()
                .py_1()
                .hover(move |s| s.bg(theme.colors.hover))
                .child("Close")
                .on_click(close),
        )
    }
}
