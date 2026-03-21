use super::*;

pub(super) fn panel(this: &mut PopoverHost, cx: &mut gpui::Context<PopoverHost>) -> gpui::Div {
    let theme = this.theme;
    let can_create = this.can_submit_create_branch(cx);

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
                .child("Create branch"),
        )
        .child(div().border_t_1().border_color(theme.colors.border))
        .child(
            div()
                .px_2()
                .py_1()
                .w_full()
                .min_w(px(0.0))
                .child(this.create_branch_input.clone()),
        )
        .child(
            div()
                .px_2()
                .py_1()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    components::Button::new("create_branch_cancel", "Cancel")
                        .style(components::ButtonStyle::Outlined)
                        .on_click(theme, cx, |this, _e, window, cx| {
                            this.dismiss_inline_popover(window, cx);
                        }),
                )
                .child(
                    components::Button::new("create_branch_go", "Create")
                        .style(components::ButtonStyle::Filled)
                        .disabled(!can_create)
                        .on_click(theme, cx, |this, _e, window, cx| {
                            this.submit_create_branch(window, cx);
                        }),
                ),
        )
}
