use super::*;

impl GitGpuiView {
    pub(super) fn render_branch_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let Loadable::Ready(branches) = &repo.branches else {
            return Vec::new();
        };
        let theme = this.theme;
        range
            .filter_map(|ix| branches.get(ix).map(|b| (ix, b)))
            .map(|(ix, branch)| {
                div()
                    .id(ix)
                    .px_2()
                    .py_1()
                    .rounded(px(theme.radii.row))
                    .hover(move |s| s.bg(theme.colors.hover))
                    .child(branch.name.clone())
                    .into_any_element()
            })
            .collect()
    }

    pub(super) fn render_remote_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let rows = Self::remote_rows(repo);
        let theme = this.theme;
        range
            .filter_map(|ix| rows.get(ix).cloned().map(|r| (ix, r)))
            .map(|(ix, row)| match row {
                RemoteRow::Header(name) => div()
                    .id(("remote_hdr", ix))
                    .px_2()
                    .py_1()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.colors.text)
                    .child(name)
                    .into_any_element(),
                RemoteRow::Branch { remote: _, name } => div()
                    .id(("remote_branch", ix))
                    .px_2()
                    .py_1()
                    .pl_4()
                    .rounded(px(theme.radii.row))
                    .hover(move |s| s.bg(theme.colors.hover))
                    .text_sm()
                    .text_color(theme.colors.text_muted)
                    .child(name)
                    .into_any_element(),
            })
            .collect()
    }

    pub(super) fn render_diagnostic_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let theme = this.theme;

        range
            .filter_map(|ix| repo.diagnostics.get(ix).map(|d| (ix, d)))
            .map(|(ix, d)| {
                let (label, color) = match d.kind {
                    DiagnosticKind::Info => ("Info", theme.colors.accent),
                    DiagnosticKind::Warning => ("Warning", theme.colors.warning),
                    DiagnosticKind::Error => ("Error", theme.colors.danger),
                };

                div()
                    .id(("diag", ix))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded(px(theme.radii.row))
                    .hover(move |s| s.bg(theme.colors.hover))
                    .child(components::pill(theme, label, color))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.colors.text_muted)
                            .line_clamp(2)
                            .child(d.message.clone()),
                    )
                    .into_any_element()
            })
            .collect()
    }

    pub(super) fn render_history_table_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let Loadable::Ready(page) = &repo.log else {
            return Vec::new();
        };
        let Some(cache) = this.history_cache.as_ref() else {
            return Vec::new();
        };
        if cache.repo_id != repo.id {
            return Vec::new();
        }

        let theme = this.theme;
        range
            .filter_map(|visible_ix| {
                let commit_ix = cache.visible_indices.get(visible_ix).copied()?;
                let commit = page.commits.get(commit_ix)?;
                let graph_row = cache.graph_rows.get(visible_ix)?;
                Some((visible_ix, commit, graph_row))
            })
            .map(|(visible_ix, commit, graph_row)| {
                let is_head = page
                    .commits
                    .first()
                    .is_some_and(|head| head.id == commit.id);
                let refs = commit_refs(repo, is_head, commit);
                let when = format_relative_time(commit.time);
                let selected = repo.selected_commit.as_ref() == Some(&commit.id);
                history_table_row(
                    theme, visible_ix, repo.id, commit, graph_row, refs, when, selected, cx,
                )
            })
            .collect()
    }

    pub(super) fn render_stash_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let Loadable::Ready(stashes) = &repo.stashes else {
            return Vec::new();
        };

        let theme = this.theme;
        range
            .filter_map(|ix| stashes.get(ix).map(|s| (ix, s)))
            .map(|(ix, stash)| {
                let repo_id = repo.id;
                let index = stash.index;
                let when = stash
                    .created_at
                    .map(format_relative_time)
                    .unwrap_or_else(|| "—".to_string());

                div()
                    .id(("stash_row", ix))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded(px(theme.radii.row))
                    .hover(move |s| s.bg(theme.colors.hover))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .min_w(px(0.0))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.text_muted)
                                    .whitespace_nowrap()
                                    .child(format!("stash@{{{index}}}")),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .min_w(px(0.0))
                                    .line_clamp(1)
                                    .child(stash.message.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.text_muted)
                                    .whitespace_nowrap()
                                    .child(when),
                            )
                            .child(
                                kit::Button::new(format!("stash_apply_{index}"), "Apply")
                                    .style(kit::ButtonStyle::Secondary)
                                    .on_click(theme, cx, move |this, _e, _w, cx| {
                                        this.store.dispatch(Msg::ApplyStash { repo_id, index });
                                        cx.notify();
                                    }),
                            )
                            .child(
                                kit::Button::new(format!("stash_drop_{index}"), "Drop")
                                    .style(kit::ButtonStyle::Danger)
                                    .on_click(theme, cx, move |this, _e, _w, cx| {
                                        this.store.dispatch(Msg::DropStash { repo_id, index });
                                        cx.notify();
                                    }),
                            ),
                    )
                    .into_any_element()
            })
            .collect()
    }

    pub(super) fn render_reflog_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let Loadable::Ready(reflog) = &repo.reflog else {
            return Vec::new();
        };

        let theme = this.theme;
        range
            .filter_map(|ix| reflog.get(ix).map(|e| (ix, e)))
            .map(|(ix, entry)| {
                let repo_id = repo.id;
                let commit_id = entry.new_id.clone();
                let when = entry
                    .time
                    .map(format_relative_time)
                    .unwrap_or_else(|| "—".to_string());

                div()
                    .id(("reflog_row", ix))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded(px(theme.radii.row))
                    .hover(move |s| s.bg(theme.colors.hover))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .min_w(px(0.0))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.text_muted)
                                    .whitespace_nowrap()
                                    .child(entry.selector.clone()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .min_w(px(0.0))
                                    .line_clamp(1)
                                    .child(entry.message.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .whitespace_nowrap()
                            .child(when),
                    )
                    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                        this.store.dispatch(Msg::SelectCommit {
                            repo_id,
                            commit_id: commit_id.clone(),
                        });
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect()
    }

    pub(super) fn render_unstaged_rows(
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
        let theme = this.theme;
        range
            .filter_map(|ix| unstaged.get(ix).map(|e| (ix, e)))
            .map(|(ix, entry)| status_row(theme, ix, entry, DiffArea::Unstaged, repo.id, cx))
            .collect()
    }

    pub(super) fn render_staged_rows(
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
        let theme = this.theme;
        range
            .filter_map(|ix| staged.get(ix).map(|e| (ix, e)))
            .map(|(ix, entry)| status_row(theme, ix, entry, DiffArea::Staged, repo.id, cx))
            .collect()
    }

    pub(super) fn render_diff_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let theme = this.theme;
        range
            .filter_map(|ix| this.diff_cache.get(ix).map(|l| (ix, l)))
            .map(|(ix, line)| diff_row(theme, ix, this.diff_view, line))
            .collect()
    }
}

fn history_table_row(
    theme: AppTheme,
    ix: usize,
    repo_id: RepoId,
    commit: &Commit,
    graph_row: &history_graph::GraphRow,
    refs: String,
    when: String,
    selected: bool,
    cx: &mut gpui::Context<GitGpuiView>,
) -> AnyElement {
    let id: &str = <CommitId as AsRef<str>>::as_ref(&commit.id);
    let short = id.get(0..8).unwrap_or(id);
    let graph = history_graph_cell(theme, graph_row);
    let node_color = graph_row
        .lanes_now
        .get(graph_row.node_col)
        .map(|l| l.color)
        .unwrap_or(theme.colors.text_muted);

    let refs = if refs.trim().is_empty() {
        div().into_any_element()
    } else {
        let pills = refs
            .split(", ")
            .filter(|s| !s.trim().is_empty())
            .map(|label| {
                div()
                    .px_2()
                    .py(px(1.0))
                    .rounded(px(999.0))
                    .text_xs()
                    .text_color(theme.colors.text)
                    .bg(with_alpha(node_color, if theme.is_dark { 0.22 } else { 0.16 }))
                    .border_1()
                    .border_color(with_alpha(node_color, if theme.is_dark { 0.48 } else { 0.36 }))
                    .child(label.to_string())
            })
            .collect::<Vec<_>>();

        div()
            .flex()
            .items_center()
            .gap_1()
            .children(pills)
            .into_any_element()
    };

    let commit_id = commit.id.clone();
    let commit_id_for_menu = commit.id.clone();
    let mut row = div()
        .id(ix)
        .h(px(24.0))
        .flex()
        .w_full()
        .items_center()
        .gap_2()
        .px_2()
        .rounded(px(theme.radii.row))
        .hover(move |s| s.bg(theme.colors.hover))
        .child(
            div()
                .w(px(HISTORY_COL_BRANCH_PX))
                .text_xs()
                .text_color(theme.colors.text_muted)
                .line_clamp(1)
                .whitespace_nowrap()
                .child(refs),
        )
        .child(
            div()
                .w(px(HISTORY_COL_GRAPH_PX))
                .h_full()
                .flex()
                .justify_center()
                .child(graph),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .w(px(3.0))
                        .h_full()
                        .bg(node_color),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .text_sm()
                        .line_clamp(1)
                        .whitespace_nowrap()
                        .child(commit.summary.clone()),
                ),
        )
        .child(
            div()
                .w(px(HISTORY_COL_DATE_PX))
                .flex()
                .justify_end()
                .text_xs()
                .text_color(theme.colors.text_muted)
                .whitespace_nowrap()
                .child(when),
        )
        .child(
            div()
                .w(px(HISTORY_COL_SHA_PX))
                .flex()
                .justify_end()
                .text_xs()
                .text_color(theme.colors.text_muted)
                .whitespace_nowrap()
                .child(short.to_string()),
        )
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.store.dispatch(Msg::SelectCommit {
                repo_id,
                commit_id: commit_id.clone(),
            });
            cx.notify();
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, e: &MouseDownEvent, _w, cx| {
                this.popover = Some(PopoverKind::CommitMenu {
                    repo_id,
                    commit_id: commit_id_for_menu.clone(),
                });
                this.popover_anchor = Some(e.position);
                cx.notify();
            }),
        );

    if selected {
        row = row.bg(with_alpha(theme.colors.accent, 0.15));
    }

    row.into_any_element()
}

fn history_graph_cell(theme: AppTheme, row: &history_graph::GraphRow) -> impl IntoElement {
    use gpui::{PathBuilder, canvas, fill, point, px, size};

    let row = row.clone();
    let stroke_width = px(1.6);

    canvas(
        |_, _, _| (),
        move |bounds, _, window, _cx| {
            if row.lanes_now.is_empty() {
                return;
            }

            let col_gap = px(16.0);
            let margin_x = px(10.0);
            let node_radius = if row.is_merge { px(3.5) } else { px(3.0) };

            let y_top = bounds.top();
            let y_center = bounds.top() + bounds.size.height / 2.0;
            let y_bottom = bounds.bottom();

            let x_for_col = |col: usize| margin_x + col_gap * (col as f32);
            let node_x = x_for_col(row.node_col);

            let mut col_now: std::collections::HashMap<history_graph::LaneId, usize> =
                std::collections::HashMap::new();
            for (ix, lane) in row.lanes_now.iter().enumerate() {
                col_now.insert(lane.id, ix);
            }

            let mut col_next: std::collections::HashMap<history_graph::LaneId, usize> =
                std::collections::HashMap::new();
            for (ix, lane) in row.lanes_next.iter().enumerate() {
                col_next.insert(lane.id, ix);
            }

            // Lane background bands (per-row, but forms continuous columns across rows).
            let lane_width = col_gap * 0.9;
            let lane_alpha = if theme.is_dark { 0.10 } else { 0.07 };
            for (ix, lane) in row.lanes_now.iter().enumerate() {
                let x = x_for_col(ix);
                window.paint_quad(fill(
                    gpui::Bounds::new(
                        point(bounds.left() + x - lane_width / 2.0, y_top),
                        size(lane_width, bounds.size.height),
                    ),
                    with_alpha(lane.color, lane_alpha),
                ));
            }

            // Incoming vertical segments.
            for lane in row.lanes_now.iter() {
                let Some(col) = col_now.get(&lane.id).copied() else {
                    continue;
                };
                if !row.incoming_ids.contains(&lane.id) {
                    continue;
                }
                let x = x_for_col(col);
                let mut path = PathBuilder::stroke(stroke_width);
                path.move_to(point(bounds.left() + x, y_top));
                path.line_to(point(bounds.left() + x, y_center));
                if let Ok(p) = path.build() {
                    window.paint_path(p, lane.color);
                }
            }

            // Incoming join edges into the node (used both for merge commits and fork points).
            for edge in row.joins_in.iter() {
                if edge.from_col == edge.to_col {
                    continue;
                }
                let x_from = x_for_col(edge.from_col);
                let x_to = x_for_col(edge.to_col);
                let mut path = PathBuilder::stroke(stroke_width);
                path.move_to(point(bounds.left() + x_from, y_center));
                if (x_from - x_to).abs() < px(0.5) {
                    path.line_to(point(bounds.left() + x_to, y_center));
                } else {
                    let ctrl = px(8.0);
                    path.cubic_bezier_to(
                        point(bounds.left() + x_to, y_center),
                        point(bounds.left() + x_from + ctrl, y_center),
                        point(bounds.left() + x_to - ctrl, y_center),
                    );
                }
                if let Ok(p) = path.build() {
                    window.paint_path(p, edge.color);
                }
            }

            // Continuations from current row to next row.
            for lane in row.lanes_next.iter() {
                let Some(out_col) = col_next.get(&lane.id).copied() else {
                    continue;
                };
                let x_out = x_for_col(out_col);

                let x_from = match col_now.get(&lane.id).copied() {
                    Some(now_col) => x_for_col(now_col),
                    None => node_x,
                };

                let mut path = PathBuilder::stroke(stroke_width);
                path.move_to(point(bounds.left() + x_from, y_center));
                if (x_from - x_out).abs() < px(0.5) {
                    path.line_to(point(bounds.left() + x_out, y_bottom));
                } else {
                    let y_mid = y_center + (y_bottom - y_center) * 0.5;
                    path.cubic_bezier_to(
                        point(bounds.left() + x_out, y_bottom),
                        point(bounds.left() + x_from, y_mid),
                        point(bounds.left() + x_out, y_mid),
                    );
                }
                if let Ok(p) = path.build() {
                    window.paint_path(p, lane.color);
                }
            }

            // Additional merge edges from the node into lanes that were re-targeted to secondary parents.
            for edge in row.edges_out.iter() {
                if edge.from_col == edge.to_col {
                    continue;
                }
                let x_to = x_for_col(edge.to_col);
                let mut path = PathBuilder::stroke(stroke_width);
                path.move_to(point(bounds.left() + node_x, y_center));
                if (node_x - x_to).abs() < px(0.5) {
                    path.line_to(point(bounds.left() + x_to, y_bottom));
                } else {
                    let y_mid = y_center + (y_bottom - y_center) * 0.5;
                    path.cubic_bezier_to(
                        point(bounds.left() + x_to, y_bottom),
                        point(bounds.left() + node_x, y_mid),
                        point(bounds.left() + x_to, y_mid),
                    );
                }
                if let Ok(p) = path.build() {
                    window.paint_path(p, edge.color);
                }
            }

            let node_color = row
                .lanes_now
                .get(row.node_col)
                .map(|l| l.color)
                .unwrap_or(theme.colors.text_muted);
            let node_border = px(1.0);
            let outer_r = node_radius + node_border;
            window.paint_quad(
                fill(
                    gpui::Bounds::new(
                        point(bounds.left() + node_x - outer_r, y_center - outer_r),
                        size(outer_r * 2.0, outer_r * 2.0),
                    ),
                    theme.colors.surface_bg,
                )
                .corner_radii(outer_r),
            );
            window.paint_quad(
                fill(
                    gpui::Bounds::new(
                        point(bounds.left() + node_x - node_radius, y_center - node_radius),
                        size(node_radius * 2.0, node_radius * 2.0),
                    ),
                    node_color,
                )
                .corner_radii(node_radius),
            );
        },
    )
    .w_full()
    .h_full()
}

fn commit_refs(repo: &RepoState, is_head: bool, commit: &Commit) -> String {
    use std::collections::BTreeSet;

    let mut refs: BTreeSet<String> = BTreeSet::new();
    if is_head {
        if let Loadable::Ready(head) = &repo.head_branch {
            refs.insert(format!("HEAD → {head}"));
        }
    }

    if let Loadable::Ready(branches) = &repo.branches {
        for branch in branches {
            if branch.target == commit.id {
                refs.insert(branch.name.clone());
            }
        }
    }

    refs.into_iter().collect::<Vec<_>>().join(", ")
}

fn format_relative_time(time: std::time::SystemTime) -> String {
    use std::time::SystemTime;

    let Ok(elapsed) = SystemTime::now().duration_since(time) else {
        return "in the future".to_string();
    };

    fn fmt(n: u64, unit: &str) -> String {
        if n == 1 {
            format!("1 {unit} ago")
        } else {
            format!("{n} {unit}s ago")
        }
    }

    let secs = elapsed.as_secs();
    if secs < 60 {
        return fmt(secs.max(1), "second");
    }
    let mins = secs / 60;
    if mins < 60 {
        return fmt(mins, "minute");
    }
    let hours = mins / 60;
    if hours < 24 {
        return fmt(hours, "hour");
    }
    let days = hours / 24;
    if days < 30 {
        return fmt(days, "day");
    }
    let months = days / 30;
    if months < 12 {
        return fmt(months, "month");
    }
    let years = months / 12;
    fmt(years, "year")
}

fn status_row(
    theme: AppTheme,
    ix: usize,
    entry: &FileStatus,
    area: DiffArea,
    repo_id: RepoId,
    cx: &mut gpui::Context<GitGpuiView>,
) -> AnyElement {
    let (label, color) = match entry.kind {
        FileStatusKind::Untracked => ("Untracked", theme.colors.warning),
        FileStatusKind::Modified => ("Modified", theme.colors.accent),
        FileStatusKind::Added => ("Added", theme.colors.success),
        FileStatusKind::Deleted => ("Deleted", theme.colors.danger),
        FileStatusKind::Renamed => ("Renamed", theme.colors.accent),
        FileStatusKind::Conflicted => ("Conflicted", theme.colors.danger),
    };

    let path = entry.path.clone();
    let path_for_stage = path.clone();
    let path_for_row = path.clone();
    let stage_label = match area {
        DiffArea::Unstaged => "Stage",
        DiffArea::Staged => "Unstage",
    };

    div()
        .id(ix)
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .px_2()
        .py_1()
        .rounded(px(theme.radii.row))
        .hover(move |s| s.bg(theme.colors.hover))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(components::pill(theme, label, color))
                .child(
                    div()
                        .text_sm()
                        .line_clamp(1)
                        .child(path.display().to_string()),
                ),
        )
        .child(
            kit::Button::new(format!("stage_btn_{ix}"), stage_label)
                .style(kit::ButtonStyle::Secondary)
                .on_click(theme, cx, move |this, _e, _w, cx| {
                    this.store.dispatch(Msg::SelectDiff {
                        repo_id,
                        target: DiffTarget::WorkingTree {
                            path: path_for_stage.clone(),
                            area,
                        },
                    });
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
                    cx.notify();
                }),
        )
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
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

fn diff_row(
    theme: AppTheme,
    ix: usize,
    mode: DiffViewMode,
    line: &AnnotatedDiffLine,
) -> AnyElement {
    let (bg, fg, gutter_fg) = diff_line_colors(theme, line.kind);

    let text = match line.kind {
        gitgpui_core::domain::DiffLineKind::Add => {
            line.text.strip_prefix('+').unwrap_or(&line.text)
        }
        gitgpui_core::domain::DiffLineKind::Remove => {
            line.text.strip_prefix('-').unwrap_or(&line.text)
        }
        gitgpui_core::domain::DiffLineKind::Context => {
            line.text.strip_prefix(' ').unwrap_or(&line.text)
        }
        _ => &line.text,
    };

    let old = line.old_line.map(|n| n.to_string()).unwrap_or_default();
    let new = line.new_line.map(|n| n.to_string()).unwrap_or_default();

    let (left_text, right_text) = match (mode, line.kind) {
        (DiffViewMode::Split, gitgpui_core::domain::DiffLineKind::Remove) => {
            (text.to_string(), String::new())
        }
        (DiffViewMode::Split, gitgpui_core::domain::DiffLineKind::Add) => {
            (String::new(), text.to_string())
        }
        (DiffViewMode::Split, gitgpui_core::domain::DiffLineKind::Context) => {
            (text.to_string(), text.to_string())
        }
        (DiffViewMode::Split, _) => (text.to_string(), String::new()),
        (DiffViewMode::Inline, _) => (text.to_string(), String::new()),
    };

    let row = div()
        .id(ix)
        .h(px(20.0))
        .flex()
        .items_center()
        .bg(bg)
        .font_family("monospace")
        .text_xs()
        .child(
            div()
                .w(px(44.0))
                .px_2()
                .text_color(gutter_fg)
                .whitespace_nowrap()
                .child(old),
        )
        .child(
            div()
                .w(px(44.0))
                .px_2()
                .text_color(gutter_fg)
                .whitespace_nowrap()
                .child(new),
        );

    match mode {
        DiffViewMode::Inline => row
            .child(
                div()
                    .flex_1()
                    .px_2()
                    .text_color(fg)
                    .whitespace_nowrap()
                    .child(left_text),
            )
            .into_any_element(),
        DiffViewMode::Split => row
            .child(
                div()
                    .flex_1()
                    .px_2()
                    .text_color(fg)
                    .whitespace_nowrap()
                    .child(left_text),
            )
            .child(
                div()
                    .flex_1()
                    .px_2()
                    .text_color(fg)
                    .whitespace_nowrap()
                    .child(right_text),
            )
            .into_any_element(),
    }
}

fn diff_line_colors(
    theme: AppTheme,
    kind: gitgpui_core::domain::DiffLineKind,
) -> (gpui::Rgba, gpui::Rgba, gpui::Rgba) {
    use gitgpui_core::domain::DiffLineKind::*;

    match (theme.is_dark, kind) {
        (_, Header) => (
            theme.colors.surface_bg,
            theme.colors.text_muted,
            theme.colors.text_muted,
        ),
        (_, Hunk) => (
            theme.colors.surface_bg_elevated,
            theme.colors.accent,
            theme.colors.text_muted,
        ),
        (true, Add) => (
            gpui::rgb(0x0B2E1C),
            gpui::rgb(0xBBF7D0),
            gpui::rgb(0x86EFAC),
        ),
        (true, Remove) => (
            gpui::rgb(0x3A0D13),
            gpui::rgb(0xFECACA),
            gpui::rgb(0xFCA5A5),
        ),
        (false, Add) => (
            gpui::rgba(0xe6ffedff),
            gpui::rgba(0x22863aff),
            theme.colors.text_muted,
        ),
        (false, Remove) => (
            gpui::rgba(0xffeef0ff),
            gpui::rgba(0xcb2431ff),
            theme.colors.text_muted,
        ),
        (_, Context) => (
            theme.colors.surface_bg_elevated,
            theme.colors.text,
            theme.colors.text_muted,
        ),
    }
}
