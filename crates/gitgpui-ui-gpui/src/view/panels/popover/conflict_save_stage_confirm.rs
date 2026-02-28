use super::*;

pub(super) fn panel(
    this: &mut PopoverHost,
    repo_id: RepoId,
    path: &std::path::Path,
    cx: &mut gpui::Context<PopoverHost>,
) -> gpui::Div {
    let theme = this.theme;
    let path = path.to_path_buf();

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
                .child("Unresolved conflict markers detected"),
        )
        .child(div().border_t_1().border_color(theme.colors.border))
        .child(
            div()
                .px_2()
                .py_1()
                .text_sm()
                .text_color(theme.colors.text_muted)
                .child("The resolved text still contains conflict markers (<<<<<<<, =======, >>>>>>>). Staging this file may leave it in a broken state."),
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
                    zed::Button::new("conflict_stage_cancel", "Cancel")
                        .style(zed::ButtonStyle::Outlined)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }),
                )
                .child(
                    zed::Button::new("conflict_stage_anyway", "Stage anyway")
                        .style(zed::ButtonStyle::Danger)
                        .on_click(theme, cx, move |this, _e, _w, cx| {
                            let text = this
                                .main_pane
                                .read_with(cx, |main, cx| {
                                    main.conflict_resolver_input
                                        .read_with(cx, |i, _| i.text().to_string())
                                });
                            this.store.dispatch(Msg::SaveWorktreeFile {
                                repo_id,
                                path: path.clone(),
                                contents: text,
                                stage: true,
                            });
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }),
                ),
        )
}
