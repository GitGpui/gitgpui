use super::*;

impl GitGpuiView {
    pub(in super::super) fn render_branch_sidebar_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let theme = this.theme;
        let repo_id = repo.id;
        let rows = Self::branch_sidebar_rows(repo);

        let svg_icon = |path: &'static str, color: gpui::Rgba, size_px: f32| {
            gpui::svg()
                .path(path)
                .w(px(size_px))
                .h(px(size_px))
                .text_color(color)
        };

        fn indent_px(depth: usize) -> Pixels {
            px(6.0 + depth as f32 * 10.0)
        }

        range
            .filter_map(|ix| rows.get(ix).cloned().map(|r| (ix, r)))
            .map(|(ix, row)| match row {
                BranchSidebarRow::SectionHeader {
                    section,
                    top_border,
                } => {
                    let (icon_path, label) = match section {
                        BranchSection::Local => ("icons/computer.svg", "Local"),
                        BranchSection::Remote => ("icons/cloud.svg", "Remote"),
                    };
                    let tooltip: SharedString = match section {
                        BranchSection::Local => "Local branches".into(),
                        BranchSection::Remote => "Remote branches".into(),
                    };

                    div()
                        .id(("branch_section", ix))
                        .h(if section == BranchSection::Local {
                            px(24.0)
                        } else {
                            px(22.0)
                        })
                        .w_full()
                        .px_2()
                        .flex()
                        .items_center()
                        .gap_1()
                        .bg(theme.colors.surface_bg_elevated)
                        .when(top_border, |d| {
                            d.border_t_1().border_color(theme.colors.border)
                        })
                        .child(svg_icon(icon_path, theme.colors.text_muted, 14.0))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.colors.text)
                                .child(label),
                        )
                        .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                            let mut changed = false;
                            if *hovering {
                                changed |= this.set_tooltip_text_if_changed(Some(tooltip.clone()));
                            } else if this.tooltip_text.as_ref() == Some(&tooltip) {
                                changed |= this.set_tooltip_text_if_changed(None);
                            }
                            if changed {
                                cx.notify();
                            }
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                                cx.stop_propagation();
                                this.open_popover_at(
                                    PopoverKind::BranchSectionMenu { repo_id, section },
                                    e.position,
                                    window,
                                    cx,
                                );
                            }),
                        )
                        .into_any_element()
                }
                BranchSidebarRow::SectionSpacer => div()
                    .id(("branch_section_spacer", ix))
                    .h(px(10.0))
                    .w_full()
                    .into_any_element(),
                BranchSidebarRow::StashHeader { top_border } => div()
                    .id(("stash_section", ix))
                    .h(px(22.0))
                    .w_full()
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .bg(theme.colors.surface_bg_elevated)
                    .when(top_border, |d| {
                        d.border_t_1().border_color(theme.colors.border)
                    })
                    .child(svg_icon("icons/box.svg", theme.colors.text_muted, 14.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.colors.text)
                            .child("Stash"),
                    )
                    .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                        let text: SharedString = "Stashes (Apply / Drop)".into();
                        let mut changed = false;
                        if *hovering {
                            changed |= this.set_tooltip_text_if_changed(Some(text));
                        } else if this.tooltip_text.as_ref() == Some(&text) {
                            changed |= this.set_tooltip_text_if_changed(None);
                        }
                        if changed {
                            cx.notify();
                        }
                    }))
                    .into_any_element(),
                BranchSidebarRow::StashPlaceholder { message } => div()
                    .id(("stash_placeholder", ix))
                    .h(px(22.0))
                    .w_full()
                    .px_2()
                    .text_sm()
                    .text_color(theme.colors.text_muted)
                    .child(message)
                    .into_any_element(),
                BranchSidebarRow::StashItem {
                    index,
                    message,
                    created_at: _,
                } => {
                    let repo_id = repo_id;
                    let tooltip: SharedString = if message.is_empty() {
                        "Stash".into()
                    } else {
                        message.clone()
                    };
                    let row_group: SharedString = format!("stash_row_{}", index).into();

                    let apply_tooltip: SharedString = "Apply stash".into();
                    let apply_button = zed::Button::new(
                        format!("stash_sidebar_apply_{index}"),
                        "Apply",
                    )
                    .style(zed::ButtonStyle::Outlined)
                    .on_click(theme, cx, move |this, _e, _w, cx| {
                        this.store.dispatch(Msg::ApplyStash { repo_id, index });
                        cx.notify();
                    })
                    .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                        let mut changed = false;
                        if *hovering {
                            changed |=
                                this.set_tooltip_text_if_changed(Some(apply_tooltip.clone()));
                        } else if this.tooltip_text.as_ref() == Some(&apply_tooltip) {
                            changed |= this.set_tooltip_text_if_changed(None);
                        }
                        if changed {
                            cx.notify();
                        }
                    }));

                    let pop_tooltip: SharedString = "Pop stash".into();
                    let pop_button =
                        zed::Button::new(format!("stash_sidebar_pop_{index}"), "Pop")
                            .style(zed::ButtonStyle::Filled)
                            .on_click(theme, cx, move |this, _e, _w, cx| {
                                this.store.dispatch(Msg::PopStash { repo_id, index });
                                cx.notify();
                            })
                            .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                                let mut changed = false;
                                if *hovering {
                                    changed |=
                                        this.set_tooltip_text_if_changed(Some(pop_tooltip.clone()));
                                } else if this.tooltip_text.as_ref() == Some(&pop_tooltip) {
                                    changed |= this.set_tooltip_text_if_changed(None);
                                }
                                if changed {
                                    cx.notify();
                                }
                            }));

                    let drop_tooltip: SharedString = "Drop stash".into();
                    let drop_button = zed::Button::new(
                        format!("stash_sidebar_drop_{index}"),
                        "Drop",
                    )
                    .style(zed::ButtonStyle::Danger)
                    .on_click(theme, cx, move |this, _e, _w, cx| {
                        this.store.dispatch(Msg::DropStash { repo_id, index });
                        cx.notify();
                    })
                    .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                        let mut changed = false;
                        if *hovering {
                            changed |= this.set_tooltip_text_if_changed(Some(drop_tooltip.clone()));
                        } else if this.tooltip_text.as_ref() == Some(&drop_tooltip) {
                            changed |= this.set_tooltip_text_if_changed(None);
                        }
                        if changed {
                            cx.notify();
                        }
                    }));

                    div()
                        .id(("stash_sidebar_row", index))
                        .relative()
                        .group(row_group.clone())
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py(px(2.0))
                        .w_full()
                        .hover(move |s| s.bg(theme.colors.hover))
                        .active(move |s| s.bg(theme.colors.active))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .flex_1()
                                .min_w(px(0.0))
                                .pr(px(160.0))
                                .child(svg_icon("icons/box.svg", theme.colors.text_muted, 12.0))
                                .child(
                                    div()
                                        .text_sm()
                                        .min_w(px(0.0))
                                        .line_clamp(1)
                                        .child(message.clone()),
                                ),
                        )
                        .child(
                            div()
                                .absolute()
                                .right(px(6.0))
                                .top(px(2.0))
                                .bottom(px(2.0))
                                .flex()
                                .items_center()
                                .gap_2()
                                .invisible()
                                .group_hover(row_group.clone(), |d| d.visible())
                                .child(apply_button)
                                .child(pop_button)
                                .child(drop_button),
                        )
                        .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                            let mut changed = false;
                            if *hovering {
                                changed |= this.set_tooltip_text_if_changed(Some(tooltip.clone()));
                            } else {
                                if this.tooltip_text.as_ref() == Some(&tooltip) {
                                    changed |= this.set_tooltip_text_if_changed(None);
                                }
                            }
                            if changed {
                                cx.notify();
                            }
                        }))
                        .into_any_element()
                }
                BranchSidebarRow::Placeholder {
                    section: _,
                    message,
                } => div()
                    .id(("branch_placeholder", ix))
                    .h(px(22.0))
                    .w_full()
                    .px_2()
                    .text_sm()
                    .text_color(theme.colors.text_muted)
                    .child(message)
                    .into_any_element(),
                BranchSidebarRow::RemoteHeader { name } => div()
                    .id(("branch_remote", ix))
                    .h(px(22.0))
                    .w_full()
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.colors.text)
                    .child(
                        svg_icon("icons/folder.svg", theme.colors.text_muted, 14.0).flex_shrink_0(),
                    )
                    .child(name)
                    .into_any_element(),
                BranchSidebarRow::GroupHeader { label, depth } => div()
                    .id(("branch_group", ix))
                    .h(px(20.0))
                    .w_full()
                    .pl(indent_px(depth))
                    .pr_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.colors.text_muted)
                    .child(
                        svg_icon("icons/folder.svg", theme.colors.text_muted, 14.0).flex_shrink_0(),
                    )
                    .child(label)
                    .into_any_element(),
                BranchSidebarRow::Branch {
                    label,
                    name,
                    section,
                    depth,
                    muted,
                    divergence,
                    is_head,
                    is_upstream,
                } => {
                    let name_for_tooltip: SharedString = name.clone();
                    let full_name_for_checkout = name.to_string();
                    let branch_icon_color = if muted {
                        theme.colors.text_muted
                    } else {
                        theme.colors.text
                    };
                    let mut row = div()
                        .id(("branch_item", ix))
                        .h(if section == BranchSection::Local {
                            px(24.0)
                        } else {
                            px(22.0)
                        })
                        .w_full()
                        .flex()
                        .items_center()
                        .gap_2()
                        .pl(indent_px(depth))
                        .pr_2()
                        .rounded(px(theme.radii.row))
                        .when(is_head, |d| {
                            d.bg(with_alpha(
                                theme.colors.accent,
                                if theme.is_dark { 0.18 } else { 0.12 },
                            ))
                            .border_1()
                            .border_color(with_alpha(theme.colors.accent, 0.90))
                        })
                        .hover(move |s| s.bg(theme.colors.hover))
                        .active(move |s| s.bg(theme.colors.active))
                        .when(muted, |d| d.text_color(theme.colors.text_muted))
                        .child(svg_icon("icons/git_branch.svg", branch_icon_color, 12.0))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .text_sm()
                                .line_clamp(1)
                                .whitespace_nowrap()
                                .child(label),
                        );

                    let mut right = div().flex().items_center().gap_2().ml_auto();
                    let mut has_right = false;

                    if is_upstream && section == BranchSection::Remote {
                        has_right = true;
                        right = right.child(
                            div()
                                .px(px(3.0))
                                .py(px(0.0))
                                .rounded(px(999.0))
                                .text_xs()
                                .text_color(theme.colors.text_muted)
                                .bg(with_alpha(
                                    theme.colors.accent,
                                    if theme.is_dark { 0.16 } else { 0.10 },
                                ))
                                .border_1()
                                .border_color(with_alpha(
                                    theme.colors.accent,
                                    if theme.is_dark { 0.32 } else { 0.22 },
                                ))
                                .child("Upstream"),
                        );
                    }

                    if let Some(divg) = divergence
                        && (divg.ahead > 0 || divg.behind > 0)
                    {
                        has_right = true;
                        if divg.behind > 0 {
                            let color = theme.colors.warning;
                            right = right.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(color)
                                    .child(svg_icon("icons/arrow_down.svg", color, 11.0))
                                    .child(divg.behind.to_string()),
                            );
                        }
                        if divg.ahead > 0 {
                            let color = theme.colors.success;
                            right = right.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(color)
                                    .child(svg_icon("icons/arrow_up.svg", color, 11.0))
                                    .child(divg.ahead.to_string()),
                            );
                        }
                    }

                    if has_right {
                        row = row.child(right);
                    }

                    let upstream_note = if is_upstream && section == BranchSection::Remote {
                        " (upstream for current branch)"
                    } else {
                        ""
                    };
                    let branch_tooltip: SharedString =
                        format!("Branch: {name_for_tooltip}{upstream_note}").into();

                    row = row
                        .on_click(cx.listener(move |this, e: &ClickEvent, _w, cx| {
                            if !e.standard_click() || e.click_count() < 2 {
                                return;
                            }
                            match section {
                                BranchSection::Local => {
                                    this.store.dispatch(Msg::CheckoutBranch {
                                        repo_id,
                                        name: full_name_for_checkout.clone(),
                                    });
                                    this.rebuild_diff_cache();
                                    cx.notify();
                                }
                                BranchSection::Remote => {
                                    if let Some((remote, branch)) =
                                        full_name_for_checkout.split_once('/')
                                    {
                                        this.store.dispatch(Msg::CheckoutRemoteBranch {
                                            repo_id,
                                            remote: remote.to_string(),
                                            name: branch.to_string(),
                                        });
                                        this.rebuild_diff_cache();
                                        cx.notify();
                                    }
                                }
                            }
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                                cx.stop_propagation();
                                this.open_popover_at(
                                    PopoverKind::BranchMenu {
                                        repo_id,
                                        section,
                                        name: name.to_string(),
                                    },
                                    e.position,
                                    window,
                                    cx,
                                );
                            }),
                        )
                        .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                            let mut changed = false;
                            if *hovering {
                                changed |= this
                                    .set_tooltip_text_if_changed(Some(branch_tooltip.clone()));
                            } else if this.tooltip_text.as_ref() == Some(&branch_tooltip) {
                                changed |= this.set_tooltip_text_if_changed(None);
                            }
                            if changed {
                                cx.notify();
                            }
                        }));

                    row.into_any_element()
                }
            })
            .collect()
    }

    pub(in super::super) fn render_commit_file_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let Loadable::Ready(details) = &repo.commit_details else {
            return Vec::new();
        };

        let theme = this.theme;
        let repo_id = repo.id;

        range
            .filter_map(|ix| details.files.get(ix).map(|f| (ix, f)))
            .map(|(ix, f)| {
                let commit_id = details.id.clone();
                let (icon, color) = match f.kind {
                    FileStatusKind::Added => (Some("+"), theme.colors.success),
                    FileStatusKind::Modified => (Some("✎"), theme.colors.warning),
                    FileStatusKind::Deleted => (Some("−"), theme.colors.danger),
                    FileStatusKind::Renamed => (Some("→"), theme.colors.accent),
                    FileStatusKind::Untracked => (Some("?"), theme.colors.warning),
                    FileStatusKind::Conflicted => (Some("!"), theme.colors.danger),
                };

                let path = f.path.clone();
                let selected = repo.diff_target.as_ref().is_some_and(|t| match t {
                    DiffTarget::Commit {
                        commit_id: t_commit_id,
                        path: Some(t_path),
                    } => t_commit_id == &commit_id && t_path == &path,
                    _ => false,
                });
                let commit_id_for_click = commit_id.clone();
                let path_for_click = path.clone();
                let commit_id_for_menu = commit_id.clone();
                let path_for_menu = path.clone();
                let path_label = this.cached_path_display(&path);
                let tooltip = path_label.clone();

                let mut row = div()
                    .id(("commit_file", ix))
                    .h(px(24.0))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .w_full()
                    .rounded(px(theme.radii.row))
                    .hover(move |s| s.bg(theme.colors.hover))
                    .active(move |s| s.bg(theme.colors.active))
                    .child(
                        div()
                            .w(px(16.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when_some(icon, |this, icon| {
                                this.child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(color)
                                        .child(icon),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .text_sm()
                            .line_clamp(1)
                            .whitespace_nowrap()
                            .child(path_label.clone()),
                    )
                    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                        this.store.dispatch(Msg::SelectDiff {
                            repo_id,
                            target: DiffTarget::Commit {
                                commit_id: commit_id_for_click.clone(),
                                path: Some(path_for_click.clone()),
                            },
                        });
                        this.rebuild_diff_cache();
                        cx.notify();
                    }))
                    .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
                        let mut changed = false;
                        if *hovering {
                            changed |= this.set_tooltip_text_if_changed(Some(tooltip.clone()));
                        } else if this.tooltip_text.as_ref() == Some(&tooltip) {
                            changed |= this.set_tooltip_text_if_changed(None);
                        }
                        if changed {
                            cx.notify();
                        }
                    }));
                row = row.on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        this.open_popover_at(
                            PopoverKind::CommitFileMenu {
                                repo_id,
                                commit_id: commit_id_for_menu.clone(),
                                path: path_for_menu.clone(),
                            },
                            e.position,
                            window,
                            cx,
                        );
                    }),
                );

                if selected {
                    row = row.bg(with_alpha(
                        theme.colors.accent,
                        if theme.is_dark { 0.16 } else { 0.10 },
                    ));
                }

                row.into_any_element()
            })
            .collect()
    }
}
