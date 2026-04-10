use super::*;

const SUBMODULE_TRUST_CVE_URL: &str =
    "https://github.blog/open-source/git/git-security-vulnerabilities-announced/#cve-2022-39253";

pub(super) fn panel(
    this: &mut PopoverHost,
    repo_id: RepoId,
    cx: &mut gpui::Context<PopoverHost>,
) -> gpui::Div {
    let theme = this.theme;
    let Some(prompt) = this
        .state
        .submodule_trust_prompt
        .as_ref()
        .filter(|prompt| prompt.repo_id == repo_id)
        .cloned()
    else {
        return div();
    };

    let (title, confirm_label, cancel_label) = match &prompt.operation {
        SubmoduleTrustPromptOperation::Add { .. } => {
            ("Trust local submodule?", "Trust and add", "Back")
        }
        SubmoduleTrustPromptOperation::Update => {
            ("Trust local submodule sources?", "Trust and update", "Cancel")
        }
    };
    let sources = prompt.sources.clone();
    let operation = prompt.operation.clone();

    div()
        .flex()
        .flex_col()
        .w(px(640.0))
        .child(
            div()
                .px_2()
                .py_1()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .child(title),
        )
        .child(div().border_t_1().border_color(theme.colors.border))
        .child(
            div()
                .px_2()
                .pt_1()
                .text_sm()
                .text_color(theme.colors.text_muted)
                .child("Git blocks local file transport for submodules by default. Trusting these sources will allow GitComet to enable file transport only for this repo/source pair."),
        )
        .child(
            div()
                .px_2()
                .pb_1()
                .child(
                    components::Button::new(
                        "submodule_trust_cve_link",
                        "Read about CVE-2022-39253",
                    )
                    .style(components::ButtonStyle::Filled)
                    .borderless()
                    .no_hover_border()
                    .end_slot(div().font_family(UI_MONOSPACE_FONT_FAMILY).child("->"))
                    .on_click(theme, cx, |_this, _e, _window, cx| {
                        cx.open_url(SUBMODULE_TRUST_CVE_URL);
                    }),
                ),
        )
        .children(sources.iter().cloned().map(|source| {
            div()
                .px_2()
                .pb_1()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.colors.text_muted)
                        .child(format!(
                            "Submodule: {}",
                            source.submodule_path.display()
                        )),
                )
                .child(
                    div()
                        .text_sm()
                        .font_family(crate::font_preferences::EDITOR_MONOSPACE_FONT_FAMILY)
                        .child(source.display_source.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .font_family(crate::font_preferences::EDITOR_MONOSPACE_FONT_FAMILY)
                        .text_color(theme.colors.text_muted)
                        .child(format!(
                            "Local path: {}",
                            source.local_source_path.display()
                        )),
                )
        }))
        .child(div().border_t_1().border_color(theme.colors.border))
        .child(
            div()
                .px_2()
                .py_1()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    components::Button::new("submodule_trust_cancel", cancel_label)
                        .style(components::ButtonStyle::Outlined)
                        .on_click(theme, cx, move |this, _e, window, cx| {
                            this.store.dispatch(Msg::CancelSubmoduleTrustPrompt);
                            match &operation {
                                SubmoduleTrustPromptOperation::Add { url, path } => {
                                    let theme = this.theme;
                                    this.submodule_url_input.update(cx, |input, cx| {
                                        input.set_theme(theme, cx);
                                        input.set_text(url, cx);
                                        cx.notify();
                                    });
                                    this.submodule_path_input.update(cx, |input, cx| {
                                        input.set_theme(theme, cx);
                                        input.set_text(&path.display().to_string(), cx);
                                        cx.notify();
                                    });
                                    this.popover = Some(PopoverKind::submodule(
                                        repo_id,
                                        SubmodulePopoverKind::AddPrompt,
                                    ));
                                    let focus = this
                                        .submodule_url_input
                                        .read_with(cx, |input, _| input.focus_handle());
                                    window.focus(&focus);
                                }
                                SubmoduleTrustPromptOperation::Update => {
                                    this.popover = None;
                                    this.popover_anchor = None;
                                }
                            }
                            cx.notify();
                        }),
                )
                .child(
                    components::Button::new("submodule_trust_confirm", confirm_label)
                        .style(components::ButtonStyle::Filled)
                        .on_click(theme, cx, |this, _e, _window, cx| {
                            this.store.dispatch(Msg::ConfirmSubmoduleTrustPrompt);
                            this.popover = None;
                            this.popover_anchor = None;
                            cx.notify();
                        }),
                ),
        )
}
