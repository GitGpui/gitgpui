use super::*;

pub(super) fn panel(
    this: &mut PopoverHost,
    repo_id: RepoId,
    paths: Vec<std::path::PathBuf>,
    cx: &mut gpui::Context<PopoverHost>,
) -> gpui::Div {
    let theme = this.theme;
    let count = paths.len();
    let detail = if count == 1 {
        paths
            .first()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "file".to_string())
    } else {
        format!("{count} files")
    };

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
                .child("Discard changes"),
        )
        .child(div().border_t_1().border_color(theme.colors.border))
        .child(
            div()
                .px_2()
                .py_1()
                .text_sm()
                .text_color(theme.colors.text_muted)
                .child(format!(
                    "This will discard working tree changes for {detail}."
                )),
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
                    zed::Button::new("discard_changes_cancel", "Cancel")
                        .style(zed::ButtonStyle::Outlined)
                        .on_click(theme, cx, |this, _e, _w, cx| {
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }),
                )
                .child(
                    zed::Button::new("discard_changes_go", "Discard")
                        .style(zed::ButtonStyle::Danger)
                        .on_click(theme, cx, move |this, _e, _w, cx| {
                            this.discard_worktree_changes_confirmed(repo_id, paths.clone(), cx);
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }),
                ),
        )
}
