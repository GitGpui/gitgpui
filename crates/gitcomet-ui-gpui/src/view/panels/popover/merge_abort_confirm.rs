use super::*;

pub(super) fn panel(
    this: &mut PopoverHost,
    repo_id: RepoId,
    cx: &mut gpui::Context<PopoverHost>,
) -> gpui::Div {
    let theme = this.theme;

    div()
        .flex()
        .flex_col()
        .min_w(px(360.0))
        .child(
            div()
                .px_2()
                .py_1()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .child("Abort merge?"),
        )
        .child(div().border_t_1().border_color(theme.colors.border))
        .child(
            div()
                .px_2()
                .py_1()
                .text_sm()
                .text_color(theme.colors.text_muted)
                .child("This will abort the current merge and restore the pre-merge state. Any resolved conflicts will be lost."),
        )
        .child(
            div()
                .px_2()
                .pb_1()
                .text_xs()
                .font_family("monospace")
                .text_color(theme.colors.text_muted)
                .child("git merge --abort"),
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
                    components::Button::new("merge_abort_cancel", "Cancel")
                        .style(components::ButtonStyle::Outlined)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }),
                )
                .child(
                    components::Button::new("merge_abort_go", "Abort merge")
                        .style(components::ButtonStyle::Danger)
                        .on_click(theme, cx, move |this, _e, _w, cx| {
                            this.store.dispatch(Msg::MergeAbort { repo_id });
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }),
                ),
        )
}
