use super::*;

pub(super) fn panel(this: &mut PopoverHost, cx: &mut gpui::Context<PopoverHost>) -> gpui::Div {
    let theme = this.theme;
    let current = this.date_time_format;
    let preview_now = std::time::SystemTime::now();
    let terminal_program_label: SharedString = this
        .terminal_program
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "System default".to_string())
        .into();

    let row = |id: &'static str, label: &'static str, value: SharedString, open: bool| {
        div()
            .id(id)
            .px_2()
            .py_1()
            .flex()
            .items_center()
            .justify_between()
            .rounded(px(theme.radii.row))
            .hover(move |s| s.bg(theme.colors.hover))
            .active(move |s| s.bg(theme.colors.active))
            .cursor(CursorStyle::PointingHand)
            .child(div().text_sm().child(label))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_sm()
                    .text_color(theme.colors.text_muted)
                    .child(value)
                    .child(
                        div()
                            .font_family("monospace")
                            .child(if open { "▴" } else { "▾" }),
                    ),
            )
    };

    let mut dropdown = div().flex().flex_col().gap_1().px_2().pb_2();

    if this.settings_date_format_open {
        for fmt in DateTimeFormat::all() {
            let selected = *fmt == current;
            let fmt_val = *fmt;
            let preview: SharedString = format_datetime_utc(preview_now, fmt_val).into();
            dropdown = dropdown.child(
                div()
                    .id(("settings_date_format_item", *fmt as usize))
                    .px_2()
                    .py_1()
                    .rounded(px(theme.radii.row))
                    .when(!selected, |d| {
                        d.hover(move |s| s.bg(theme.colors.hover))
                            .active(move |s| s.bg(theme.colors.active))
                    })
                    .when(selected, |d| d.bg(with_alpha(theme.colors.accent, 0.15)))
                    .cursor(CursorStyle::PointingHand)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(div().text_sm().child(fmt.label()))
                            .child(
                                div()
                                    .font_family("monospace")
                                    .text_xs()
                                    .text_color(theme.colors.text_muted)
                                    .child(preview),
                            ),
                    )
                    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                        this.settings_date_format_open = false;
                        this.set_date_time_format(fmt_val, cx);
                        cx.notify();
                    })),
            );
        }
    }

    let header = div()
        .px_2()
        .py_1()
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .child("Settings");

    let section_label = div()
        .px_2()
        .pt(px(6.0))
        .pb(px(4.0))
        .text_xs()
        .text_color(theme.colors.text_muted)
        .child("General");

    let date_row = row(
        "settings_date_format",
        "Date format",
        current.label().into(),
        this.settings_date_format_open,
    )
    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
        this.settings_date_format_open = !this.settings_date_format_open;
        if this.settings_date_format_open {
            this.settings_terminal_program_open = false;
        }
        cx.notify();
    }));

    let terminal_row = row(
        "settings_terminal_program",
        "Terminal program",
        terminal_program_label,
        this.settings_terminal_program_open,
    )
    .on_click(cx.listener(|this, _e: &ClickEvent, window, cx| {
        this.settings_terminal_program_open = !this.settings_terminal_program_open;
        if this.settings_terminal_program_open {
            this.settings_date_format_open = false;
            let theme = this.theme;
            let current = this.terminal_program.clone().unwrap_or_default();
            this.terminal_program_input.update(cx, |input, cx| {
                input.set_theme(theme, cx);
                input.set_text(current, cx);
            });
            let focus = this
                .terminal_program_input
                .read_with(cx, |input, _| input.focus_handle());
            window.focus(&focus);
        }
        cx.notify();
    }));

    zed::context_menu(
        theme,
        div()
            .flex()
            .flex_col()
            .min_w(px(560.0))
            .max_w(px(720.0))
            .child(header)
            .child(div().border_t_1().border_color(theme.colors.border))
            .child(section_label)
            .child(
                div()
                    .px_2()
                    .pb_1()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(date_row)
                    .child(terminal_row),
            )
            .when(this.settings_date_format_open, |d| {
                d.child(
                    div()
                        .px_2()
                        .pb_1()
                        .text_xs()
                        .text_color(theme.colors.text_muted)
                        .child("Choose a format:"),
                )
                .child(dropdown)
            }),
    )
    .when(this.settings_terminal_program_open, |d| {
        let theme = this.theme;

        let save = zed::Button::new("settings_terminal_program_save", "Save")
            .style(zed::ButtonStyle::Filled)
            .on_click(theme, cx, |this, _e, _w, cx| {
                let text = this
                    .terminal_program_input
                    .read_with(cx, |i, _| i.text().trim().to_string());
                let next = if text.is_empty() { None } else { Some(text) };
                this.settings_terminal_program_open = false;
                this.set_terminal_program(next, cx);
                cx.notify();
            });

        let reset = zed::Button::new("settings_terminal_program_default", "System default")
            .style(zed::ButtonStyle::Outlined)
            .on_click(theme, cx, |this, _e, _w, cx| {
                this.settings_terminal_program_open = false;
                this.set_terminal_program(None, cx);
                cx.notify();
            });

        d.child(
            div()
                .px_2()
                .pb_2()
                .flex()
                .flex_col()
                .gap_1()
                .on_key_down(cx.listener(|this, e: &gpui::KeyDownEvent, _w, cx| {
                    let key = e.keystroke.key.as_str();
                    let mods = e.keystroke.modifiers;
                    if mods.control || mods.platform || mods.alt || mods.function {
                        return;
                    }
                    match key {
                        "escape" => {
                            this.settings_terminal_program_open = false;
                            cx.notify();
                        }
                        "enter" => {
                            let text = this
                                .terminal_program_input
                                .read_with(cx, |i, _| i.text().trim().to_string());
                            let next = if text.is_empty() { None } else { Some(text) };
                            this.settings_terminal_program_open = false;
                            this.set_terminal_program(next, cx);
                            cx.notify();
                        }
                        _ => {}
                    }
                }))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.colors.text_muted)
                        .child("Leave empty to use the system default."),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .child(this.terminal_program_input.clone()),
                        )
                        .child(save)
                        .child(reset),
                ),
        )
    })
}
