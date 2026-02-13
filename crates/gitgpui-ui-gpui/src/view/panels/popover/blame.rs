use super::*;

pub(super) fn panel(
    this: &mut PopoverHost,
    repo_id: RepoId,
    path: std::path::PathBuf,
    rev: Option<String>,
    cx: &mut gpui::Context<PopoverHost>,
) -> gpui::Div {
    let theme = this.theme;
    let repo = this.state.repos.iter().find(|r| r.id == repo_id);
    let title: SharedString = path.display().to_string().into();
    let subtitle: SharedString = rev
        .clone()
        .map(|r| format!("rev: {r}").into())
        .unwrap_or_else(|| "rev: HEAD".into());

    let header = div()
        .px_2()
        .py_1()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .flex_col()
                .min_w(px(0.0))
                .child(div().text_sm().font_weight(FontWeight::BOLD).child("Blame"))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.colors.text_muted)
                        .line_clamp(1)
                        .whitespace_nowrap()
                        .child(title),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.colors.text_muted)
                        .line_clamp(1)
                        .whitespace_nowrap()
                        .child(subtitle),
                ),
        )
        .child(
            zed::Button::new("blame_close", "Close")
                .style(zed::ButtonStyle::Outlined)
                .on_click(theme, cx, |this, _e, _w, cx| {
                    this.popover = None;
                    this.popover_anchor = None;
                    cx.notify();
                }),
        );

    let body: AnyElement = match repo.map(|r| &r.blame) {
        None => zed::context_menu_label(theme, "No repository").into_any_element(),
        Some(Loadable::Loading) => zed::context_menu_label(theme, "Loading").into_any_element(),
        Some(Loadable::Error(e)) => zed::context_menu_label(theme, e.clone()).into_any_element(),
        Some(Loadable::NotLoaded) => {
            zed::context_menu_label(theme, "Not loaded").into_any_element()
        }
        Some(Loadable::Ready(lines)) => {
            let count = lines.len();
            let list = uniform_list(
                "blame_popover",
                count,
                cx.processor(PopoverHost::render_blame_popover_rows),
            )
            .h(px(360.0))
            .track_scroll(this.blame_scroll.clone());
            let scroll_handle = {
                let state = this.blame_scroll.0.borrow();
                state.base_handle.clone()
            };

            div()
                .relative()
                .child(list)
                .child(zed::Scrollbar::new("blame_popover_scrollbar", scroll_handle).render(theme))
                .into_any_element()
        }
    };

    div()
        .flex()
        .flex_col()
        .min_w(px(720.0))
        .max_w(px(980.0))
        .child(header)
        .child(div().border_t_1().border_color(theme.colors.border))
        .child(body)
}
