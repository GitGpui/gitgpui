use super::*;

pub(super) fn panel(
    this: &mut GitGpuiView,
    repo_id: RepoId,
    target: String,
    mode: ResetMode,
    cx: &mut gpui::Context<GitGpuiView>,
) -> gpui::Div {
    let theme = this.theme;
    let mode_label = match mode {
        ResetMode::Soft => "--soft",
        ResetMode::Mixed => "--mixed",
        ResetMode::Hard => "--hard",
    };

    div()
        .flex()
        .flex_col()
        .min_w(px(380.0))
        .child(
            div()
                .px_2()
                .py_1()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .child("Reset"),
        )
        .child(div().border_t_1().border_color(theme.colors.border))
        .child(
            div()
                .px_2()
                .py_1()
                .text_sm()
                .text_color(theme.colors.text_muted)
                .child(format!("{mode_label} → {target}")),
        )
        .child(
            div()
                .px_2()
                .pb_1()
                .text_xs()
                .text_color(theme.colors.text_muted)
                .child(match mode {
                    ResetMode::Hard => "Hard reset updates index + working tree (destructive).",
                    ResetMode::Mixed => "Mixed reset updates index only.",
                    ResetMode::Soft => "Soft reset moves HEAD only.",
                }),
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
                    zed::Button::new("reset_cancel", "Cancel")
                        .style(zed::ButtonStyle::Outlined)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }),
                )
                .child(
                    zed::Button::new("reset_go", "Reset")
                        .style(zed::ButtonStyle::Filled)
                        .on_click(theme, cx, move |this, _e, _w, cx| {
                            this.store.dispatch(Msg::Reset {
                                repo_id,
                                target: target.clone(),
                                mode,
                            });
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }),
                ),
        )
}
