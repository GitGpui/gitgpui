use super::*;

pub(super) fn panel(
    this: &mut GitGpuiView,
    repo_id: RepoId,
    cx: &mut gpui::Context<GitGpuiView>,
) -> gpui::Div {
    let theme = this.theme;

    div()
        .flex()
        .flex_col()
        .min_w(px(420.0))
        .child(
            div()
                .px_2()
                .py_1()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .child("Add remote"),
        )
        .child(div().border_t_1().border_color(theme.colors.border))
        .child(
            div()
                .px_2()
                .py_1()
                .text_xs()
                .text_color(theme.colors.text_muted)
                .child("Name"),
        )
        .child(
            div()
                .px_2()
                .pb_1()
                .min_w(px(0.0))
                .child(this.remote_name_input.clone()),
        )
        .child(
            div()
                .px_2()
                .py_1()
                .text_xs()
                .text_color(theme.colors.text_muted)
                .child("URL"),
        )
        .child(
            div()
                .px_2()
                .pb_1()
                .min_w(px(0.0))
                .child(this.remote_url_input.clone()),
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
                    zed::Button::new("add_remote_cancel", "Cancel")
                        .style(zed::ButtonStyle::Outlined)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }),
                )
                .child(
                    zed::Button::new("add_remote_go", "Add")
                        .style(zed::ButtonStyle::Filled)
                        .on_click(theme, cx, move |this, _e, _w, cx| {
                            let name = this
                                .remote_name_input
                                .read_with(cx, |i, _| i.text().trim().to_string());
                            let url = this
                                .remote_url_input
                                .read_with(cx, |i, _| i.text().trim().to_string());
                            if name.is_empty() || url.is_empty() {
                                this.push_toast(
                                    zed::ToastKind::Error,
                                    "Remote: name and URL are required".to_string(),
                                    cx,
                                );
                                return;
                            }
                            this.store.dispatch(Msg::AddRemote { repo_id, name, url });
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }),
                ),
        )
}
