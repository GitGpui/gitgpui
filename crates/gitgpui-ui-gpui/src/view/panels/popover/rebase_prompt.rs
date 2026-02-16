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
        .w(px(420.0))
        .child(
            div()
                .px_2()
                .py_1()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .child("Rebase"),
        )
        .child(div().border_t_1().border_color(theme.colors.border))
        .child(
            div()
                .px_2()
                .py_1()
                .text_xs()
                .text_color(theme.colors.text_muted)
                .child("Rebase current branch onto"),
        )
        .child(
            div()
                .px_2()
                .pb_1()
                .w_full()
                .min_w(px(0.0))
                .child(this.rebase_onto_input.clone()),
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
                    zed::Button::new("rebase_cancel", "Cancel")
                        .style(zed::ButtonStyle::Outlined)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }),
                )
                .child(
                    zed::Button::new("rebase_go", "Rebase")
                        .style(zed::ButtonStyle::Filled)
                        .on_click(theme, cx, move |this, _e, _w, cx| {
                            let onto = this
                                .rebase_onto_input
                                .read_with(cx, |i, _| i.text().trim().to_string());
                            if onto.is_empty() {
                                this.push_toast(
                                    zed::ToastKind::Error,
                                    "Rebase: target is required".to_string(),
                                    cx,
                                );
                                return;
                            }
                            this.store.dispatch(Msg::Rebase { repo_id, onto });
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }),
                ),
        )
}
