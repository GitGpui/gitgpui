use super::*;

impl GitGpuiView {
    pub(in super::super) fn render_unstaged_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let Loadable::Ready(RepoStatus { unstaged, .. }) = &repo.status else {
            return Vec::new();
        };
        let selected = repo.diff_target.as_ref();
        let theme = this.theme;
        range
            .filter_map(|ix| unstaged.get(ix).map(|e| (ix, e)))
            .map(|(ix, entry)| {
                let show_stage_button =
                    this.hovered_status_row.as_ref().is_some_and(|(r, a, p)| {
                        *r == repo.id && *a == DiffArea::Unstaged && p == &entry.path
                    });
                let is_selected = selected.is_some_and(|t| match t {
                    DiffTarget::WorkingTree { path, area } => {
                        *area == DiffArea::Unstaged && path == &entry.path
                    }
                    _ => false,
                });
                status_row(
                    theme,
                    ix,
                    entry,
                    DiffArea::Unstaged,
                    repo.id,
                    show_stage_button,
                    is_selected,
                    cx,
                )
            })
            .collect()
    }

    pub(in super::super) fn render_staged_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let Loadable::Ready(RepoStatus { staged, .. }) = &repo.status else {
            return Vec::new();
        };
        let selected = repo.diff_target.as_ref();
        let theme = this.theme;
        range
            .filter_map(|ix| staged.get(ix).map(|e| (ix, e)))
            .map(|(ix, entry)| {
                let show_stage_button =
                    this.hovered_status_row.as_ref().is_some_and(|(r, a, p)| {
                        *r == repo.id && *a == DiffArea::Staged && p == &entry.path
                    });
                let is_selected = selected.is_some_and(|t| match t {
                    DiffTarget::WorkingTree { path, area } => {
                        *area == DiffArea::Staged && path == &entry.path
                    }
                    _ => false,
                });
                status_row(
                    theme,
                    ix,
                    entry,
                    DiffArea::Staged,
                    repo.id,
                    show_stage_button,
                    is_selected,
                    cx,
                )
            })
            .collect()
    }
}

fn status_row(
    theme: AppTheme,
    ix: usize,
    entry: &FileStatus,
    area: DiffArea,
    repo_id: RepoId,
    show_stage_button: bool,
    selected: bool,
    cx: &mut gpui::Context<GitGpuiView>,
) -> AnyElement {
    let (icon, color) = match entry.kind {
        FileStatusKind::Untracked => match area {
            DiffArea::Unstaged => ("+", theme.colors.success),
            DiffArea::Staged => ("?", theme.colors.warning),
        },
        FileStatusKind::Modified => ("✎", theme.colors.warning),
        FileStatusKind::Added => ("+", theme.colors.success),
        FileStatusKind::Deleted => ("−", theme.colors.danger),
        FileStatusKind::Renamed => ("→", theme.colors.accent),
        FileStatusKind::Conflicted => ("!", theme.colors.danger),
    };

    let path = entry.path.clone();
    let path_for_stage = path.clone();
    let path_for_row = path.clone();
    let path_for_hover = path.clone();
    let path_for_menu = path.clone();
    let stage_label = match area {
        DiffArea::Unstaged => "Stage",
        DiffArea::Staged => "Unstage",
    };
    let row_tooltip: SharedString = path.display().to_string().into();

    let hover_area = area;
    let stage_button = zed::Button::new(format!("stage_btn_{ix}"), stage_label)
        .style(zed::ButtonStyle::Outlined)
        .on_click(theme, cx, move |this, _e, window, cx| {
            cx.stop_propagation();
            window.focus(&this.diff_panel_focus_handle);

            let next_path_in_area = (|| {
                let repo = this.active_repo()?;
                let Loadable::Ready(status) = &repo.status else {
                    return None;
                };
                let entries = match area {
                    DiffArea::Unstaged => status.unstaged.as_slice(),
                    DiffArea::Staged => status.staged.as_slice(),
                };
                let Some(current_ix) = entries.iter().position(|e| e.path == path_for_stage) else {
                    return None;
                };
                if entries.len() <= 1 {
                    return None;
                }
                let next_ix = if current_ix + 1 < entries.len() {
                    current_ix + 1
                } else {
                    current_ix.saturating_sub(1)
                };
                entries.get(next_ix).map(|e| e.path.clone())
            })();

            match area {
                DiffArea::Unstaged => this.store.dispatch(Msg::StagePath {
                    repo_id,
                    path: path_for_stage.clone(),
                }),
                DiffArea::Staged => this.store.dispatch(Msg::UnstagePath {
                    repo_id,
                    path: path_for_stage.clone(),
                }),
            }

            if let Some(next_path) = next_path_in_area {
                this.store.dispatch(Msg::SelectDiff {
                    repo_id,
                    target: DiffTarget::WorkingTree {
                        path: next_path,
                        area,
                    },
                });
            } else {
                this.store.dispatch(Msg::ClearDiffSelection { repo_id });
            }

            this.rebuild_diff_cache();
            cx.notify();
        })
        .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
            let text: SharedString = format!("{stage_label} file").into();
            if *hovering {
                this.tooltip_text = Some(text);
            } else if this.tooltip_text.as_ref() == Some(&text) {
                this.tooltip_text = None;
            }
            cx.notify();
        }));

    div()
        .id(ix)
        .relative()
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .w_full()
        .rounded(px(theme.radii.row))
        .when(selected, |s| s.bg(theme.colors.hover))
        .hover(move |s| s.bg(theme.colors.hover))
        .active(move |s| s.bg(theme.colors.active))
        .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
            if *hovering {
                this.hovered_status_row = Some((repo_id, hover_area, path_for_hover.clone()));
                this.tooltip_text = Some(row_tooltip.clone());
            } else if this
                .hovered_status_row
                .as_ref()
                .is_some_and(|(r, a, p)| *r == repo_id && *a == hover_area && p == &path_for_hover)
            {
                this.hovered_status_row = None;
                if this.tooltip_text.as_ref() == Some(&row_tooltip) {
                    this.tooltip_text = None;
                }
            }
            cx.notify();
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                this.open_popover_at(
                    PopoverKind::StatusFileMenu {
                        repo_id,
                        area,
                        path: path_for_menu.clone(),
                    },
                    e.position,
                    window,
                    cx,
                );
            }),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .flex_1()
                .min_w(px(0.0))
                .pr(if show_stage_button { px(92.0) } else { px(0.0) })
                .child(
                    div()
                        .w(px(16.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(color)
                                .child(icon),
                        ),
                )
                .child(
                    div()
                        .text_sm()
                        .flex_1()
                        .min_w(px(0.0))
                        .line_clamp(1)
                        .child(path.display().to_string()),
                ),
        )
        .when(show_stage_button, |row| {
            row.child(
                div()
                    .absolute()
                    .right(px(6.0))
                    .top_0()
                    .bottom_0()
                    .flex()
                    .items_center()
                    .child(stage_button),
            )
        })
        .on_click(cx.listener(move |this, _e: &ClickEvent, window, cx| {
            window.focus(&this.diff_panel_focus_handle);
            this.store.dispatch(Msg::SelectDiff {
                repo_id,
                target: DiffTarget::WorkingTree {
                    path: path_for_row.clone(),
                    area,
                },
            });
            this.rebuild_diff_cache();
            cx.notify();
        }))
        .into_any_element()
}
