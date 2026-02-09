use super::*;

pub(super) fn panel(
    this: &mut GitGpuiView,
    repo_id: RepoId,
    remote: String,
    cx: &mut gpui::Context<GitGpuiView>,
) -> gpui::Div {
    let theme = this.theme;
    let remote_for_action = remote.clone();

    div()
        .flex()
        .flex_col()
        .min_w(px(320.0))
        .child(
            div()
                .px_2()
                .py_1()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .child("Set upstream and push"),
        )
        .child(div().border_t_1().border_color(theme.colors.border))
        .child(
            div()
                .px_2()
                .py_1()
                .text_sm()
                .text_color(theme.colors.text_muted)
                .child(format!("Remote: {remote}")),
        )
        .child(
            div()
                .px_2()
                .py_1()
                .w_full()
                .min_w(px(0.0))
                .child(this.push_upstream_branch_input.clone()),
        )
        .child(
            div()
                .px_2()
                .py_1()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    zed::Button::new("push_upstream_cancel", "Cancel")
                        .style(zed::ButtonStyle::Outlined)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }),
                )
                .child(
                    zed::Button::new("push_upstream_go", "Push")
                        .style(zed::ButtonStyle::Filled)
                        .on_click(theme, cx, move |this, _e, _w, cx| {
                            let branch = this
                                .push_upstream_branch_input
                                .read_with(cx, |i, _| i.text().trim().to_string());
                            if branch.is_empty() {
                                return;
                            }
                            this.store.dispatch(Msg::PushSetUpstream {
                                repo_id,
                                remote: remote_for_action.clone(),
                                branch,
                            });
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }),
                ),
        )
}
