use super::*;

pub(super) fn panel(
    _this: &mut GitGpuiView,
    repo_id: RepoId,
    cx: &mut gpui::Context<GitGpuiView>,
) -> gpui::Div {
    let theme = _this.theme;

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
                .child("Force push"),
        )
        .child(div().border_t_1().border_color(theme.colors.border))
        .child(
            div()
                .px_2()
                .py_1()
                .text_sm()
                .text_color(theme.colors.text_muted)
                .child("This will overwrite remote history if your branch has diverged."),
        )
        .child(
            div()
                .px_2()
                .pb_1()
                .text_xs()
                .font_family("monospace")
                .text_color(theme.colors.text_muted)
                .child("git push --force-with-lease"),
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
                    zed::Button::new("force_push_cancel", "Cancel")
                        .style(zed::ButtonStyle::Outlined)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }),
                )
                .child(
                    zed::Button::new("force_push_go", "Force push")
                        .style(zed::ButtonStyle::Danger)
                        .on_click(theme, cx, move |this, _e, _w, cx| {
                            this.store.dispatch(Msg::ForcePush { repo_id });
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }),
                ),
        )
}
