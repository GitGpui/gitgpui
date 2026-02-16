use super::*;

pub(super) fn panel(this: &mut PopoverHost, cx: &mut gpui::Context<PopoverHost>) -> gpui::Div {
    let theme = this.theme;

    div()
        .flex()
        .flex_col()
        .w(px(420.0))
        .child(
            div()
                .px_2()
                .py_1()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .child("Clone repository"),
        )
        .child(div().border_t_1().border_color(theme.colors.border))
        .child(
            div()
                .px_2()
                .py_1()
                .text_xs()
                .text_color(theme.colors.text_muted)
                .child("Repository URL / Path"),
        )
        .child(
            div()
                .px_2()
                .pb_1()
                .w_full()
                .min_w(px(0.0))
                .child(this.clone_repo_url_input.clone()),
        )
        .child(
            div()
                .px_2()
                .py_1()
                .text_xs()
                .text_color(theme.colors.text_muted)
                .child("Destination parent folder"),
        )
        .child(
            div()
                .px_2()
                .pb_1()
                .w_full()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .child(this.clone_repo_parent_dir_input.clone()),
                )
                .child(
                    zed::Button::new("clone_repo_browse", "Browse")
                        .style(zed::ButtonStyle::Outlined)
                        .on_click(theme, cx, |_this, _e, window, cx| {
                            cx.stop_propagation();
                            let view = cx.weak_entity();
                            let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
                                files: false,
                                directories: true,
                                multiple: false,
                                prompt: Some("Clone into folder".into()),
                            });

                            window
                                .spawn(cx, async move |cx| {
                                    let result = rx.await;
                                    let paths = match result {
                                        Ok(Ok(Some(paths))) => paths,
                                        Ok(Ok(None)) => return,
                                        Ok(Err(_)) | Err(_) => return,
                                    };
                                    let Some(path) = paths.into_iter().next() else {
                                        return;
                                    };
                                    let _ = view.update(cx, |this, cx| {
                                        this.clone_repo_parent_dir_input.update(cx, |input, cx| {
                                            input.set_text(path.display().to_string(), cx);
                                        });
                                        cx.notify();
                                    });
                                })
                                .detach();
                        }),
                ),
        )
        .child(div().border_t_1().border_color(theme.colors.border))
        .child(
            div()
                .px_2()
                .py_1()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    zed::Button::new("clone_repo_cancel", "Cancel")
                        .style(zed::ButtonStyle::Outlined)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }),
                )
                .child(
                    zed::Button::new("clone_repo_go", "Clone")
                        .style(zed::ButtonStyle::Filled)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            let url = this
                                .clone_repo_url_input
                                .read_with(cx, |i, _| i.text().trim().to_string());
                            let parent = this
                                .clone_repo_parent_dir_input
                                .read_with(cx, |i, _| i.text().trim().to_string());
                            if url.is_empty() || parent.is_empty() {
                                this.push_toast(
                                    zed::ToastKind::Error,
                                    "Clone: URL and destination are required".to_string(),
                                    cx,
                                );
                                return;
                            }

                            let repo_name = clone_repo_name_from_url(&url);
                            let dest = std::path::PathBuf::from(parent).join(repo_name);
                            this.store.dispatch(Msg::CloneRepo { url, dest });
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }),
                ),
        )
}
