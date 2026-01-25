use super::diff_text::*;
use super::diff_canvas;
use super::history_canvas;
use super::*;

impl GitGpuiView {
    pub(in super::super) fn render_worktree_preview_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
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

        let syntax_mode = if lines.len() <= MAX_LINES_FOR_SYNTAX_HIGHLIGHTING {
            DiffSyntaxMode::Auto
        } else {
            DiffSyntaxMode::HeuristicOnly
        };
        let language = diff_syntax_language_for_path(path.to_string_lossy().as_ref());

        let highlight_new_file = this.untracked_worktree_preview_path().is_some()
            || this.added_file_preview_abs_path().is_some()
            || this.diff_preview_is_new_file;

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
                            syntax_mode,
                            None,
                        )
                    });

                let line_no = format!("{}", ix + 1);
                diff_canvas::worktree_preview_row_canvas(
                    theme,
                    cx.entity(),
                    ix,
                    highlight_new_file,
                    line_no.into(),
                    styled,
                )
            })
            .collect()
    }

    pub(in super::super) fn render_history_table_rows(
        this: &mut Self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };

        let theme = this.theme;
        let row_pad = window.rem_size() * 0.5;
        let col_branch = this.history_col_branch;
        let col_graph = this.history_col_graph;
        let col_date = this.history_col_date;
        let col_sha = this.history_col_sha;
        let (show_date, show_sha) = this.history_visible_columns();

        let (show_working_tree_summary_row, worktree_counts) = match &repo.status {
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
                let unstaged_counts = count_for(&status.unstaged);
                let staged_counts = count_for(&status.staged);
                (
                    !status.unstaged.is_empty() || !status.staged.is_empty(),
                    (
                        unstaged_counts.0 + staged_counts.0,
                        unstaged_counts.1 + staged_counts.1,
                        unstaged_counts.2 + staged_counts.2,
                    ),
                )
            }
            _ => (false, (0, 0, 0)),
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

        let stash_ids: Option<std::collections::HashSet<&str>> = match &repo.stashes {
            Loadable::Ready(stashes) if !stashes.is_empty() => {
                Some(stashes.iter().map(|s| s.id.as_ref()).collect())
            }
            _ => None,
        };

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
                        worktree_counts,
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
                let mut graph_row_with_incoming;
                let graph_row = if show_working_tree_summary_row && visible_ix == 0 {
                    graph_row_with_incoming = graph_row.clone();
                    if !graph_row_with_incoming
                        .incoming_ids
                        .contains(&graph_row_with_incoming.node_id)
                    {
                        graph_row_with_incoming
                            .incoming_ids
                            .push(graph_row_with_incoming.node_id);
                    }
                    &graph_row_with_incoming
                } else {
                    graph_row
                };
                let refs = commit_refs_parts(repo, commit);
                let when = super::super::format_datetime_utc(commit.time, this.date_time_format);
                let selected = repo.selected_commit.as_ref() == Some(&commit.id);
                let show_graph_color_marker =
                    repo.history_scope == gitgpui_core::domain::LogScope::AllBranches;
                let is_stash_node = stash_ids
                    .as_ref()
                    .is_some_and(|ids| ids.contains(commit.id.as_ref()));

                Some(history_table_row(
                    theme,
                    row_pad,
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
                    is_stash_node,
                    cx,
                ))
            })
            .collect()
    }

}

#[derive(Clone, Debug)]
struct CommitRefsParts {
    branches: String,
    tags: Vec<String>,
}

fn history_table_row(
    theme: AppTheme,
    row_pad: Pixels,
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
    refs: CommitRefsParts,
    when: String,
    selected: bool,
    is_stash_node: bool,
    cx: &mut gpui::Context<GitGpuiView>,
) -> AnyElement {
    let id: &str = commit.id.as_ref();
    let short = id.get(0..8).unwrap_or(id);
    let commit_row = history_canvas::history_commit_row_canvas(
        theme,
        ix,
        col_branch,
        col_graph,
        col_date,
        col_sha,
        show_date,
        show_sha,
        show_graph_color_marker,
        is_stash_node,
        graph_row.clone(),
        SharedString::default(),
        commit.summary.clone().into(),
        when.clone().into(),
        short.to_string().into(),
    );

    let commit_id = commit.id.clone();
    let commit_id_for_menu = commit.id.clone();
    let commit_id_for_tag_menu = commit.id.clone();
    let tag_names = refs.tags.clone();
    let branches_text = refs.branches.clone();

    let tag_chip = |name: String| {
        let commit_id_for_tag_menu = commit_id_for_tag_menu.clone();
        div()
            .px(px(6.0))
            .py(px(2.0))
            .rounded(px(theme.radii.pill))
            .border_1()
            .border_color(with_alpha(theme.colors.accent, 0.35))
            .bg(with_alpha(theme.colors.accent, 0.12))
            .text_xs()
            .text_color(theme.colors.accent)
            .whitespace_nowrap()
            .child(name)
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.open_popover_at(
                        PopoverKind::TagMenu {
                            repo_id,
                            commit_id: commit_id_for_tag_menu.clone(),
                        },
                        e.position,
                        window,
                        cx,
                    );
                }),
            )
    };

    let refs_overlay = div()
        .absolute()
        .top_0()
        .left(row_pad)
        .h_full()
        .w(col_branch.max(px(0.0)))
        .flex()
        .items_center()
        .gap_1()
        .overflow_hidden()
        .whitespace_nowrap()
        .children(tag_names.into_iter().map(tag_chip))
        .when(!branches_text.trim().is_empty(), move |d| {
            d.child(
                div()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .text_xs()
                    .text_color(theme.colors.text_muted)
                    .line_clamp(1)
                    .whitespace_nowrap()
                    .child(branches_text.clone()),
            )
        });
    let mut row = div()
        .id(ix)
        .relative()
        .h(px(24.0))
        .w_full()
        .hover(move |s| s.bg(theme.colors.hover))
        .active(move |s| s.bg(theme.colors.active))
        .child(commit_row)
        .child(refs_overlay)
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
    counts: (usize, usize, usize),
    cx: &mut gpui::Context<GitGpuiView>,
) -> AnyElement {
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

    let (added, modified, deleted) = counts;
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

    let black = gpui::rgba(0x000000ff);
    let circle = gpui::canvas(
        |_, _, _| (),
        move |bounds, _, window, _cx| {
            use gpui::{PathBuilder, fill, point, px, size};
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

            // Connect the working tree node into the history graph below.
            let stroke_width = px(1.6);
            let mut path = PathBuilder::stroke(stroke_width);
            path.move_to(point(center.x, center.y));
            path.line_to(point(center.x, bounds.bottom()));
            if let Ok(p) = path.build() {
                window.paint_path(p, node_color);
            }

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
                .child(div()),
        )
        .child(
            div()
                .w(col_graph)
                .h_full()
                .flex()
                .justify_center()
                .overflow_hidden()
                .child(circle),
        )
        .child(div().flex_1().min_w(px(0.0)).flex().items_center().child({
            let mut summary = div().flex_1().min_w(px(0.0)).flex().items_center().gap_2();
            summary = summary.child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .text_sm()
                    .line_clamp(1)
                    .whitespace_nowrap()
                    .child("Uncommitted changes"),
            );
            if !parts.is_empty() {
                summary = summary.child(div().flex().items_center().gap_2().children(parts));
            }
            summary
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
                    .child("Click to review"),
            )
        })
        .when(show_sha, |row| row.child(div().w(col_sha)))
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.store.dispatch(Msg::ClearCommitSelection { repo_id });
            this.store.dispatch(Msg::ClearDiffSelection { repo_id });
            cx.notify();
        }));

    if selected {
        row = row.bg(with_alpha(theme.colors.accent, 0.15));
    }

    row.into_any_element()
}

fn commit_refs_parts(repo: &RepoState, commit: &Commit) -> CommitRefsParts {
    use std::collections::BTreeSet;

    let mut branches: BTreeSet<String> = BTreeSet::new();
    let mut tags: BTreeSet<String> = BTreeSet::new();
    let mut head_branch_name: Option<String> = None;
    let head_target = match (&repo.head_branch, &repo.branches) {
        (Loadable::Ready(head_name), Loadable::Ready(repo_branches)) => {
            head_branch_name = Some(head_name.clone());
            repo_branches
                .iter()
                .find(|b| b.name == *head_name)
                .map(|b| b.target.clone())
        }
        _ => None,
    };
    if head_target.as_ref() == Some(&commit.id)
        && let Loadable::Ready(head) = &repo.head_branch
    {
        branches.insert(format!("HEAD → {head}"));
    }

    if let Loadable::Ready(repo_branches) = &repo.branches {
        for branch in repo_branches {
            if head_target.as_ref() == Some(&commit.id)
                && head_branch_name.as_ref() == Some(&branch.name)
            {
                continue;
            }
            if branch.target == commit.id {
                branches.insert(branch.name.clone());
            }
        }
    }

    if let Loadable::Ready(repo_tags) = &repo.tags {
        for tag in repo_tags {
            if tag.target == commit.id {
                tags.insert(tag.name.clone());
            }
        }
    }

    CommitRefsParts {
        branches: branches.into_iter().collect::<Vec<_>>().join(", "),
        tags: tags.into_iter().collect::<Vec<_>>(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::format_datetime_utc;
    use super::super::super::DateTimeFormat;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn commit_date_formats_as_yyyy_mm_dd_utc() {
        assert_eq!(
            format_datetime_utc(UNIX_EPOCH, DateTimeFormat::YmdHm),
            "1970-01-01 00:00"
        );
        assert_eq!(
            format_datetime_utc(UNIX_EPOCH + Duration::from_secs(86_400), DateTimeFormat::YmdHm),
            "1970-01-02 00:00"
        );
        assert_eq!(
            format_datetime_utc(UNIX_EPOCH - Duration::from_secs(86_400), DateTimeFormat::YmdHm),
            "1969-12-31 00:00"
        );

        // 2000-02-29 12:34:56 UTC
        assert_eq!(
            format_datetime_utc(
                UNIX_EPOCH + Duration::from_secs(951_782_400 + 12 * 3600 + 34 * 60 + 56),
                DateTimeFormat::YmdHms
            ),
            "2000-02-29 12:34:56"
        );
    }
}
