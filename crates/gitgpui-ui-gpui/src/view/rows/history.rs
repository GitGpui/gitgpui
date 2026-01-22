use super::diff_text::*;
use super::*;

impl GitGpuiView {
    pub(in super::super) fn render_worktree_preview_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let theme = this.theme;
        let Some(path) = this.worktree_preview_path.as_ref() else {
            return Vec::new();
        };
        let Loadable::Ready(lines) = &this.worktree_preview else {
            return Vec::new();
        };

        let should_clear_cache = match this.worktree_preview_segments_cache_path.as_ref() {
            Some(p) => p != path,
            None => true,
        };
        if should_clear_cache {
            this.worktree_preview_segments_cache_path = Some(path.clone());
            this.worktree_preview_segments_cache.clear();
        }

        let language = (lines.len() <= MAX_LINES_FOR_SYNTAX_HIGHLIGHTING)
            .then(|| diff_syntax_language_for_path(path.to_string_lossy().as_ref()))
            .flatten();

        range
            .map(|ix| {
                let line = lines.get(ix).map(String::as_str).unwrap_or("");

                let styled = this
                    .worktree_preview_segments_cache
                    .entry(ix)
                    .or_insert_with(|| {
                        build_cached_diff_styled_text(
                            theme,
                            line,
                            &[],
                            "",
                            language,
                            DiffSyntaxMode::Auto,
                            None,
                        )
                    });

                let line_no = format!("{}", ix + 1);

                div()
                    .id(("worktree_preview_row", ix))
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .font_family("monospace")
                    .text_xs()
                    .bg(theme.colors.surface_bg)
                    .child(
                        div()
                            .w(px(44.0))
                            .px_2()
                            .text_color(theme.colors.text_muted)
                            .whitespace_nowrap()
                            .child(line_no),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .px_2()
                            .text_color(theme.colors.text)
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(render_cached_diff_styled_text(
                                theme.colors.text,
                                Some(styled),
                            )),
                    )
                    .into_any_element()
            })
            .collect()
    }

    pub(in super::super) fn render_history_table_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };

        let theme = this.theme;
        let col_branch = this.history_col_branch;
        let col_graph = this.history_col_graph;
        let col_date = this.history_col_date;
        let col_sha = this.history_col_sha;
        let (show_date, show_sha) = this.history_visible_columns();

        let (show_working_tree_summary_row, unstaged_counts, staged_counts) = match &repo.status {
            Loadable::Ready(status) => {
                let count_for = |entries: &[FileStatus]| {
                    let mut added = 0usize;
                    let mut modified = 0usize;
                    let mut deleted = 0usize;
                    for e in entries {
                        match e.kind {
                            FileStatusKind::Untracked | FileStatusKind::Added => added += 1,
                            FileStatusKind::Deleted => deleted += 1,
                            FileStatusKind::Modified
                            | FileStatusKind::Renamed
                            | FileStatusKind::Conflicted => modified += 1,
                        }
                    }
                    (added, modified, deleted)
                };
                (
                    !status.unstaged.is_empty(),
                    count_for(&status.unstaged),
                    count_for(&status.staged),
                )
            }
            _ => (false, (0, 0, 0), (0, 0, 0)),
        };

        let page = match &repo.log {
            Loadable::Ready(page) => Some(page),
            _ => None,
        };
        let cache = this.history_cache.as_ref().filter(|c| c.repo_id == repo.id);
        let worktree_node_color = cache
            .and_then(|c| c.graph_rows.first())
            .and_then(|row| row.lanes_now.get(row.node_col).map(|l| l.color))
            .unwrap_or(theme.colors.accent);

        range
            .filter_map(|list_ix| {
                if show_working_tree_summary_row && list_ix == 0 {
                    let selected = repo.selected_commit.is_none();
                    return Some(working_tree_summary_history_row(
                        theme,
                        col_branch,
                        col_graph,
                        col_date,
                        col_sha,
                        show_date,
                        show_sha,
                        worktree_node_color,
                        repo.id,
                        selected,
                        unstaged_counts,
                        staged_counts,
                        cx,
                    ));
                }

                let offset = usize::from(show_working_tree_summary_row);
                let visible_ix = list_ix.checked_sub(offset)?;

                let page = page?;
                let cache = cache?;

                let commit_ix = cache.visible_indices.get(visible_ix).copied()?;
                let commit = page.commits.get(commit_ix)?;
                let graph_row = cache.graph_rows.get(visible_ix)?;
                let refs = commit_refs(repo, commit);
                let when = format_relative_time(commit.time);
                let selected = repo.selected_commit.as_ref() == Some(&commit.id);
                let show_graph_color_marker =
                    repo.history_scope == gitgpui_core::domain::LogScope::AllBranches;

                Some(history_table_row(
                    theme,
                    col_branch,
                    col_graph,
                    col_date,
                    col_sha,
                    show_date,
                    show_sha,
                    show_graph_color_marker,
                    list_ix,
                    repo.id,
                    commit,
                    graph_row,
                    refs,
                    when,
                    selected,
                    cx,
                ))
            })
            .collect()
    }

    pub(in super::super) fn render_stash_rows(
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
                    .active(move |s| s.bg(theme.colors.active))
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
                                zed::Button::new(format!("stash_apply_{index}"), "Apply")
                                    .style(zed::ButtonStyle::Outlined)
                                    .on_click(theme, cx, move |this, _e, _w, cx| {
                                        this.store.dispatch(Msg::ApplyStash { repo_id, index });
                                        cx.notify();
                                    }),
                            )
                            .child(
                                zed::Button::new(format!("stash_drop_{index}"), "Drop")
                                    .style(zed::ButtonStyle::Danger)
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

    pub(in super::super) fn render_reflog_rows(
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
                    .active(move |s| s.bg(theme.colors.active))
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
}

fn history_table_row(
    theme: AppTheme,
    col_branch: Pixels,
    col_graph: Pixels,
    col_date: Pixels,
    col_sha: Pixels,
    show_date: bool,
    show_sha: bool,
    show_graph_color_marker: bool,
    ix: usize,
    repo_id: RepoId,
    commit: &Commit,
    graph_row: &history_graph::GraphRow,
    refs: String,
    when: String,
    selected: bool,
    cx: &mut gpui::Context<GitGpuiView>,
) -> AnyElement {
    let id: &str = commit.id.as_ref();
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
        let max_pills = if col_branch <= px(80.0) {
            1usize
        } else if col_branch <= px(110.0) {
            2usize
        } else {
            3usize
        };

        let mut pills = Vec::new();
        let mut extra = 0usize;
        for label in refs.split(", ").map(str::trim).filter(|s| !s.is_empty()) {
            if pills.len() < max_pills {
                pills.push(
                    div()
                        .px_1()
                        .py(px(1.0))
                        .rounded(px(999.0))
                        .text_xs()
                        .text_color(theme.colors.text)
                        .bg(with_alpha(
                            node_color,
                            if theme.is_dark { 0.22 } else { 0.16 },
                        ))
                        .border_1()
                        .border_color(with_alpha(
                            node_color,
                            if theme.is_dark { 0.48 } else { 0.36 },
                        ))
                        .child(label.to_string()),
                );
            } else {
                extra += 1;
            }
        }

        if extra > 0 {
            pills.push(
                div()
                    .px_1()
                    .py(px(1.0))
                    .rounded(px(999.0))
                    .text_xs()
                    .text_color(theme.colors.text_muted)
                    .bg(with_alpha(
                        node_color,
                        if theme.is_dark { 0.14 } else { 0.10 },
                    ))
                    .border_1()
                    .border_color(with_alpha(
                        node_color,
                        if theme.is_dark { 0.32 } else { 0.24 },
                    ))
                    .child(format!("+{extra}")),
            );
        }

        div()
            .flex()
            .items_center()
            .gap_1()
            .whitespace_nowrap()
            .overflow_hidden()
            .children(pills)
            .into_any_element()
    };

    let commit_id = commit.id.clone();
    let commit_id_for_menu = commit.id.clone();
    let summary_for_tooltip: SharedString = commit.summary.clone().into();
    let mut row = div()
        .id(ix)
        .h(px(24.0))
        .flex()
        .w_full()
        .items_center()
        .px_2()
        .hover(move |s| s.bg(theme.colors.hover))
        .active(move |s| s.bg(theme.colors.active))
        .child(
            div()
                .w(col_branch)
                .text_xs()
                .text_color(theme.colors.text_muted)
                .line_clamp(1)
                .whitespace_nowrap()
                .child(refs),
        )
        .child(
            div()
                .w(col_graph)
                .h_full()
                .flex()
                .justify_center()
                .overflow_hidden()
                .child(graph),
        )
        .child(div().flex_1().min_w(px(0.0)).flex().items_center().child({
            let mut summary = div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .gap_2()
                .text_sm()
                .line_clamp(1)
                .whitespace_nowrap();
            if show_graph_color_marker {
                summary = summary.child(
                    div()
                        .w(px(2.0))
                        .h(px(12.0))
                        .rounded(px(999.0))
                        .bg(node_color)
                        .flex_none(),
                );
            }
            summary.child(commit.summary.clone())
        }))
        .when(show_date, |row| {
            row.child(
                div()
                    .w(col_date)
                    .flex()
                    .justify_end()
                    .text_xs()
                    .text_color(theme.colors.text_muted)
                    .whitespace_nowrap()
                    .child(when),
            )
        })
        .when(show_sha, |row| {
            row.child(
                div()
                    .w(col_sha)
                    .flex()
                    .justify_end()
                    .text_xs()
                    .text_color(theme.colors.text_muted)
                    .whitespace_nowrap()
                    .child(short.to_string()),
            )
        })
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            let selection_changed =
                this.active_repo().and_then(|r| r.selected_commit.as_ref()) != Some(&commit_id);
            if selection_changed {
                this.commit_scroll.set_offset(point(px(0.0), px(0.0)));
            }
            this.store.dispatch(Msg::SelectCommit {
                repo_id,
                commit_id: commit_id.clone(),
            });
            cx.notify();
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                this.open_popover_at(
                    PopoverKind::CommitMenu {
                        repo_id,
                        commit_id: commit_id_for_menu.clone(),
                    },
                    e.position,
                    window,
                    cx,
                );
            }),
        );
    row = row.on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
        if *hovering {
            this.tooltip_text = Some(summary_for_tooltip.clone());
        } else if this.tooltip_text.as_ref() == Some(&summary_for_tooltip) {
            this.tooltip_text = None;
        }
        cx.notify();
    }));

    if selected {
        row = row.bg(with_alpha(theme.colors.accent, 0.15));
    }

    row.into_any_element()
}

fn working_tree_summary_history_row(
    theme: AppTheme,
    col_branch: Pixels,
    col_graph: Pixels,
    col_date: Pixels,
    col_sha: Pixels,
    show_date: bool,
    show_sha: bool,
    node_color: gpui::Rgba,
    repo_id: RepoId,
    selected: bool,
    unstaged: (usize, usize, usize),
    staged: (usize, usize, usize),
    cx: &mut gpui::Context<GitGpuiView>,
) -> AnyElement {
    let staged_total = staged.0 + staged.1 + staged.2;
    let icon_count = |icon: &'static str, color: gpui::Rgba, count: usize| {
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(color)
                    .child(icon),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.colors.text_muted)
                    .child(count.to_string()),
            )
            .into_any_element()
    };

    let group = |label: &'static str, (added, modified, deleted): (usize, usize, usize)| {
        let mut parts: Vec<AnyElement> = Vec::new();
        if modified > 0 {
            parts.push(icon_count("✎", theme.colors.warning, modified));
        }
        if added > 0 {
            parts.push(icon_count("+", theme.colors.success, added));
        }
        if deleted > 0 {
            parts.push(icon_count("−", theme.colors.danger, deleted));
        }
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .text_color(theme.colors.text_muted)
                    .whitespace_nowrap()
                    .child(label),
            )
            .children(parts)
            .into_any_element()
    };

    let black = gpui::rgba(0x000000ff);
    let circle = gpui::canvas(
        |_, _, _| (),
        move |bounds, _, window, _cx| {
            let r = px(3.0);
            let border = px(1.0);
            let outer = r + border;
            let margin_x = px(HISTORY_GRAPH_MARGIN_X_PX);
            let col_gap = px(HISTORY_GRAPH_COL_GAP_PX);
            let node_x = margin_x + col_gap * 0.0;
            let center = point(
                bounds.left() + node_x,
                bounds.top() + bounds.size.height / 2.0,
            );
            window.paint_quad(
                fill(
                    gpui::Bounds::new(
                        point(center.x - outer, center.y - outer),
                        size(outer * 2.0, outer * 2.0),
                    ),
                    node_color,
                )
                .corner_radii(outer),
            );
            window.paint_quad(
                fill(
                    gpui::Bounds::new(point(center.x - r, center.y - r), size(r * 2.0, r * 2.0)),
                    black,
                )
                .corner_radii(r),
            );
        },
    )
    .w_full()
    .h_full();

    let mut row = div()
        .id(("history_worktree_summary", repo_id.0))
        .h(px(28.0))
        .flex()
        .w_full()
        .items_center()
        .gap_2()
        .px_2()
        .hover(move |s| s.bg(theme.colors.hover))
        .active(move |s| s.bg(theme.colors.active))
        .child(
            div()
                .w(col_branch)
                .text_xs()
                .text_color(theme.colors.text_muted)
                .whitespace_nowrap()
                .child("Working tree"),
        )
        .child(div().w(col_graph).h_full().child(circle))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .min_w(px(0.0))
                        .child(
                            div()
                                .text_sm()
                                .line_clamp(1)
                                .whitespace_nowrap()
                                .child("Uncommitted changes"),
                        )
                        .child(group("Unstaged", unstaged))
                        .when(staged_total > 0, |this| this.child(group("Staged", staged))),
                ),
        )
        .when(show_date, |row| {
            row.child(
                div()
                    .w(col_date)
                    .flex()
                    .justify_end()
                    .text_xs()
                    .text_color(theme.colors.text_muted)
                    .whitespace_nowrap()
                    .child("Click to review"),
            )
        })
        .when(show_sha, |row| row.child(div().w(col_sha)))
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.store.dispatch(Msg::ClearCommitSelection { repo_id });
            this.store.dispatch(Msg::ClearDiffSelection { repo_id });
            cx.notify();
        }))
        ;

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
            window.paint_layer(bounds, |window| {
                if row.lanes_now.is_empty() {
                    return;
                }

                let col_gap = px(HISTORY_GRAPH_COL_GAP_PX);
                let margin_x = px(HISTORY_GRAPH_MARGIN_X_PX);
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
                let black = gpui::rgba(0x000000ff);
                window.paint_quad(
                    fill(
                        gpui::Bounds::new(
                            point(bounds.left() + node_x - outer_r, y_center - outer_r),
                            size(outer_r * 2.0, outer_r * 2.0),
                        ),
                        black,
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
            });
        },
    )
    .w_full()
    .h_full()
}

fn commit_refs(repo: &RepoState, commit: &Commit) -> String {
    use std::collections::BTreeSet;

    let mut refs: BTreeSet<String> = BTreeSet::new();
    let mut head_branch_name: Option<String> = None;
    let head_target = match (&repo.head_branch, &repo.branches) {
        (Loadable::Ready(head_name), Loadable::Ready(branches)) => {
            head_branch_name = Some(head_name.clone());
            branches
                .iter()
                .find(|b| b.name == *head_name)
                .map(|b| b.target.clone())
        }
        _ => None,
    };
    if head_target.as_ref() == Some(&commit.id)
        && let Loadable::Ready(head) = &repo.head_branch
    {
        refs.insert(format!("HEAD → {head}"));
    }

    if let Loadable::Ready(branches) = &repo.branches {
        for branch in branches {
            if head_target.as_ref() == Some(&commit.id)
                && head_branch_name.as_ref() == Some(&branch.name)
            {
                continue;
            }
            if branch.target == commit.id {
                refs.insert(branch.name.clone());
            }
        }
    }

    if let Loadable::Ready(tags) = &repo.tags {
        for tag in tags {
            if tag.target == commit.id {
                refs.insert(tag.name.clone());
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
