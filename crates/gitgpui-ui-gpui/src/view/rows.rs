use super::*;
use std::cell::RefCell;
use std::sync::{Arc, OnceLock};
use tree_sitter::StreamingIterator;

const MAX_LINES_FOR_SYNTAX_HIGHLIGHTING: usize = 4_000;
const MAX_TREESITTER_LINE_BYTES: usize = 512;

thread_local! {
    static LINE_NUMBER_STRINGS: RefCell<Vec<SharedString>> =
        RefCell::new(vec![SharedString::default()]);
}

fn line_number_string(n: Option<u32>) -> SharedString {
    let Some(n) = n else {
        return SharedString::default();
    };
    let ix = n as usize;
    LINE_NUMBER_STRINGS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() <= ix {
            let start = cache.len();
            cache.reserve(ix + 1 - start);
            for v in start..=ix {
                cache.push(v.to_string().into());
            }
        }
        cache[ix].clone()
    })
}

impl GitGpuiView {
    pub(super) fn render_branch_sidebar_rows(
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

        fn indent_px(depth: usize) -> Pixels {
            px(8.0 + depth as f32 * 12.0)
        }

        range
            .filter_map(|ix| rows.get(ix).cloned().map(|r| (ix, r)))
            .map(|(ix, row)| match row {
                BranchSidebarRow::SectionHeader { section, top_border } => {
                    let (icon, label) = match section {
                        BranchSection::Local => ("🖥", "Local"),
                        BranchSection::Remote => ("☁", "Remote"),
                    };

                    div()
                        .id(("branch_section", ix))
                        .px_2()
                        .py_1()
                        .flex()
                        .items_center()
                        .gap_1()
                        .bg(theme.colors.surface_bg_elevated)
                        .when(top_border, |d| d.border_t_1().border_color(theme.colors.border))
                        .border_b_1()
                        .border_color(theme.colors.border)
                        .child(div().text_sm().text_color(theme.colors.text_muted).child(icon))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.colors.text)
                                .child(label),
                        )
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                                cx.stop_propagation();
                                this.open_popover_at(
                                    PopoverKind::BranchSectionMenu {
                                        repo_id,
                                        section,
                                    },
                                    e.position,
                                    window,
                                    cx,
                                );
                            }),
                        )
                        .into_any_element()
                }
                BranchSidebarRow::Placeholder { section: _, message } => div()
                    .id(("branch_placeholder", ix))
                    .px_2()
                    .py_1()
                    .text_sm()
                    .text_color(theme.colors.text_muted)
                    .child(message)
                    .into_any_element(),
                BranchSidebarRow::RemoteHeader { name } => div()
                    .id(("branch_remote", ix))
                    .px_2()
                    .py_1()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.colors.text)
                    .child(name)
                    .into_any_element(),
                BranchSidebarRow::GroupHeader { label, depth } => div()
                    .id(("branch_group", ix))
                    .py_1()
                    .pl(indent_px(depth))
                    .pr_2()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.colors.text_muted)
                    .child(label)
                    .into_any_element(),
                BranchSidebarRow::Branch {
                    label,
                    name,
                    section,
                    depth,
                    muted,
                } => div()
                    .id(("branch_item", ix))
                    .py_1()
                    .pl(indent_px(depth))
                    .pr_2()
                    .rounded(px(theme.radii.row))
                    .hover(move |s| s.bg(theme.colors.hover))
                    .active(move |s| s.bg(theme.colors.active))
                    .text_sm()
                    .when(muted, |d| d.text_color(theme.colors.text_muted))
                    .child(label)
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
                    .into_any_element(),
            })
            .collect()
    }

    pub(super) fn render_commit_file_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let Some(selected_id) = repo.selected_commit.as_ref() else {
            return Vec::new();
        };
        let Loadable::Ready(details) = &repo.commit_details else {
            return Vec::new();
        };
        if &details.id != selected_id {
            return Vec::new();
        }

        let theme = this.theme;
        let repo_id = repo.id;

        range
            .filter_map(|ix| details.files.get(ix).map(|f| (ix, f)))
            .map(|(ix, f)| {
                let commit_id = details.id.clone();
                let (icon, color) = match f.kind {
                    FileStatusKind::Added => (Some("+"), theme.colors.success),
                    FileStatusKind::Modified => (Some("✎"), theme.colors.warning),
                    FileStatusKind::Deleted => (None, theme.colors.text_muted),
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

                let mut row = div()
                    .id(("commit_file", ix))
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
                            .child(path.display().to_string()),
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
                    row = row.bg(with_alpha(theme.colors.accent, if theme.is_dark { 0.16 } else { 0.10 }));
                }

                row.into_any_element()
            })
            .collect()
    }

    pub(super) fn render_worktree_preview_rows(
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

    pub(super) fn render_history_table_rows(
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

        let (show_working_tree_summary_row, unstaged_counts, staged_counts) =
            match &repo.status {
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
        let cache = this
            .history_cache
            .as_ref()
            .filter(|c| c.repo_id == repo.id);

        range
            .filter_map(|list_ix| {
                if show_working_tree_summary_row && list_ix == 0 {
                    return Some(working_tree_summary_history_row(
                        theme,
                        col_branch,
                        col_graph,
                        col_date,
                        col_sha,
                        show_date,
                        show_sha,
                        repo.id,
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

                Some(history_table_row(
                    theme,
                    col_branch,
                    col_graph,
                    col_date,
                    col_sha,
                    show_date,
                    show_sha,
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
            .map(|(ix, entry)| {
                let show_stage_button = this.hovered_status_row.as_ref().is_some_and(|(r, a, p)| {
                    *r == repo.id && *a == DiffArea::Unstaged && p == &entry.path
                });
                status_row(
                    theme,
                    ix,
                    entry,
                    DiffArea::Unstaged,
                    repo.id,
                    show_stage_button,
                    cx,
                )
            })
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
            .map(|(ix, entry)| {
                let show_stage_button = this.hovered_status_row.as_ref().is_some_and(|(r, a, p)| {
                    *r == repo.id && *a == DiffArea::Staged && p == &entry.path
                });
                status_row(
                    theme,
                    ix,
                    entry,
                    DiffArea::Staged,
                    repo.id,
                    show_stage_button,
                    cx,
                )
            })
            .collect()
    }

    pub(super) fn render_diff_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        if this.is_file_diff_view_active() {
            let theme = this.theme;
            if this.diff_text_segments_cache_query != this.diff_visible_query {
                this.diff_text_segments_cache_query = this.diff_visible_query.clone();
                this.diff_text_segments_cache.clear();
            }
            let query = this.diff_visible_query.clone();
            let empty_ranges: &[Range<usize>] = &[];
            let language = (this.file_diff_inline_cache.len() <= MAX_LINES_FOR_SYNTAX_HIGHLIGHTING)
                .then(|| {
                    this.file_diff_cache_path
                        .as_ref()
                        .and_then(|p| diff_syntax_language_for_path(p.to_string_lossy().as_ref()))
                })
                .flatten();

            return range
                .map(|visible_ix| {
                    let selected = this
                        .diff_selection_range
                        .is_some_and(|(a, b)| visible_ix >= a.min(b) && visible_ix <= a.max(b));

                    let Some(inline_ix) = this.diff_visible_indices.get(visible_ix).copied() else {
                        return div()
                            .id(("diff_missing", visible_ix))
                            .h(px(20.0))
                            .px_2()
                            .font_family("monospace")
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .child("…")
                            .into_any_element();
                    };

                    let word_ranges: &[Range<usize>] = this
                        .file_diff_inline_word_highlights
                        .get(inline_ix)
                        .and_then(|r| r.as_ref().map(Vec::as_slice))
                        .unwrap_or(empty_ranges);

                    if this.diff_text_segments_cache_get(inline_ix).is_none() {
                        let Some(line) = this.file_diff_inline_cache.get(inline_ix) else {
                            return div()
                                .id(("diff_oob", visible_ix))
                                .h(px(20.0))
                                .px_2()
                                .font_family("monospace")
                                .text_xs()
                                .text_color(theme.colors.text_muted)
                                .child("…")
                                .into_any_element();
                        };

                        let word_color = match line.kind {
                            gitgpui_core::domain::DiffLineKind::Add => Some(theme.colors.success),
                            gitgpui_core::domain::DiffLineKind::Remove => Some(theme.colors.danger),
                            _ => None,
                        };

                        // Avoid per-line syntax work on context/header lines; changed lines get syntax.
                        let language = matches!(
                            line.kind,
                            gitgpui_core::domain::DiffLineKind::Add
                                | gitgpui_core::domain::DiffLineKind::Remove
                        )
                        .then_some(language)
                        .flatten();

                        let computed = build_cached_diff_styled_text(
                            theme,
                            diff_content_text(line),
                            word_ranges,
                            query.as_str(),
                            language,
                            DiffSyntaxMode::HeuristicOnly,
                            word_color,
                        );
                        this.diff_text_segments_cache_set(inline_ix, computed);
                    }

                    let Some(line) = this.file_diff_inline_cache.get(inline_ix) else {
                        return div()
                            .id(("diff_oob", visible_ix))
                            .h(px(20.0))
                            .px_2()
                            .font_family("monospace")
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .child("…")
                            .into_any_element();
                    };
                    let styled = this
                        .diff_text_segments_cache_get(inline_ix)
                        .expect("cache populated above");

                    diff_row(
                        theme,
                        visible_ix,
                        DiffClickKind::Line,
                        selected,
                        DiffViewMode::Inline,
                        line,
                        None,
                        Some(styled),
                        cx,
                    )
                })
                .collect();
        }

        let theme = this.theme;
        if this.diff_text_segments_cache_query != this.diff_visible_query {
            this.diff_text_segments_cache_query = this.diff_visible_query.clone();
            this.diff_text_segments_cache.clear();
        }
        let query = this.diff_visible_query.clone();
        let syntax_enabled = this.diff_cache.len() <= MAX_LINES_FOR_SYNTAX_HIGHLIGHTING;
        range
            .map(|visible_ix| {
                let selected = this
                    .diff_selection_range
                    .is_some_and(|(a, b)| visible_ix >= a.min(b) && visible_ix <= a.max(b));

                let Some(src_ix) = this.diff_visible_indices.get(visible_ix).copied() else {
                    return div()
                        .id(("diff_missing", visible_ix))
                        .h(px(20.0))
                        .px_2()
                        .font_family("monospace")
                        .text_xs()
                        .text_color(theme.colors.text_muted)
                        .child("…")
                        .into_any_element();
                };
                let click_kind = {
                    let Some(line) = this.diff_cache.get(src_ix) else {
                        return div()
                            .id(("diff_oob", visible_ix))
                            .h(px(20.0))
                            .px_2()
                            .font_family("monospace")
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .child("…")
                            .into_any_element();
                    };

                    if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Hunk) {
                        DiffClickKind::HunkHeader
                    } else if matches!(line.kind, gitgpui_core::domain::DiffLineKind::Header)
                        && line.text.starts_with("diff --git ")
                    {
                        DiffClickKind::FileHeader
                    } else {
                        DiffClickKind::Line
                    }
                };

                let word_ranges: &[Range<usize>] = this
                    .diff_word_highlights
                    .get(src_ix)
                    .and_then(|r| r.as_ref().map(Vec::as_slice))
                    .unwrap_or(&[]);

                let file_stat = this.diff_file_stats.get(src_ix).and_then(|s| *s);

                let language = if syntax_enabled {
                    this.diff_file_for_src_ix
                        .get(src_ix)
                        .and_then(|p| p.as_deref())
                        .and_then(diff_syntax_language_for_path)
                } else {
                    None
                };

                if matches!(click_kind, DiffClickKind::Line)
                    && this.diff_text_segments_cache_get(src_ix).is_none()
                {
                    let Some(line) = this.diff_cache.get(src_ix) else {
                        return div()
                            .id(("diff_oob", visible_ix))
                            .h(px(20.0))
                            .px_2()
                            .font_family("monospace")
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .child("…")
                            .into_any_element();
                    };

                    let word_color = match line.kind {
                        gitgpui_core::domain::DiffLineKind::Add => Some(theme.colors.success),
                        gitgpui_core::domain::DiffLineKind::Remove => Some(theme.colors.danger),
                        _ => None,
                    };

                    // Only syntax-highlight changed lines; for context lines this avoids extra work
                    // while scrolling (word-level highlights still apply).
                    let language = matches!(
                        line.kind,
                        gitgpui_core::domain::DiffLineKind::Add
                            | gitgpui_core::domain::DiffLineKind::Remove
                    )
                    .then_some(language)
                    .flatten();

                    let computed = build_cached_diff_styled_text(
                        theme,
                        diff_content_text(line),
                        word_ranges,
                        query.as_str(),
                        language,
                        DiffSyntaxMode::HeuristicOnly,
                        word_color,
                    );
                    this.diff_text_segments_cache_set(src_ix, computed);
                }

                let styled: Option<&CachedDiffStyledText> = matches!(click_kind, DiffClickKind::Line)
                    .then(|| this.diff_text_segments_cache_get(src_ix))
                    .flatten();

                let Some(line) = this.diff_cache.get(src_ix) else {
                    return div()
                        .id(("diff_oob", visible_ix))
                        .h(px(20.0))
                        .px_2()
                        .font_family("monospace")
                        .text_xs()
                        .text_color(theme.colors.text_muted)
                        .child("…")
                        .into_any_element();
                };

                diff_row(
                    theme,
                    visible_ix,
                    click_kind,
                    selected,
                    DiffViewMode::Inline,
                    line,
                    file_stat,
                    styled,
                    cx,
                )
            })
            .collect()
    }

    pub(super) fn render_diff_split_left_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        if this.is_file_diff_view_active() {
            let theme = this.theme;
            if this.diff_text_segments_cache_query != this.diff_visible_query {
                this.diff_text_segments_cache_query = this.diff_visible_query.clone();
                this.diff_text_segments_cache.clear();
            }
            let query = this.diff_visible_query.clone();
            let empty_ranges: &[Range<usize>] = &[];
            let language = (this.file_diff_cache_rows.len() <= MAX_LINES_FOR_SYNTAX_HIGHLIGHTING)
                .then(|| {
                    this.file_diff_cache_path
                        .as_ref()
                        .and_then(|p| diff_syntax_language_for_path(p.to_string_lossy().as_ref()))
                })
                .flatten();

            return range
                .map(|visible_ix| {
                    let selected = this
                        .diff_selection_range
                        .is_some_and(|(a, b)| visible_ix >= a.min(b) && visible_ix <= a.max(b));

                    let Some(row_ix) = this.diff_visible_indices.get(visible_ix).copied() else {
                        return div()
                            .id(("diff_split_left_missing", visible_ix))
                            .h(px(20.0))
                            .px_2()
                            .font_family("monospace")
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .child("…")
                            .into_any_element();
                    };
                    let key = row_ix * 2;
                    if this.diff_text_segments_cache_get(key).is_none() {
                        let Some(row) = this.file_diff_cache_rows.get(row_ix) else {
                            return div()
                                .id(("diff_split_left_oob", visible_ix))
                                .h(px(20.0))
                                .px_2()
                                .font_family("monospace")
                                .text_xs()
                                .text_color(theme.colors.text_muted)
                                .child("…")
                                .into_any_element();
                        };

                        if let Some(text) = row.old.as_deref() {
                            let word_color = matches!(
                                row.kind,
                                gitgpui_core::file_diff::FileDiffRowKind::Remove
                                    | gitgpui_core::file_diff::FileDiffRowKind::Modify
                            )
                            .then_some(theme.colors.danger);

                            let language = (!matches!(
                                row.kind,
                                gitgpui_core::file_diff::FileDiffRowKind::Context
                            ))
                            .then_some(language)
                            .flatten();

                            let word_ranges: &[Range<usize>] = this
                                .file_diff_split_word_highlights_old
                                .get(row_ix)
                                .and_then(|r| r.as_ref().map(Vec::as_slice))
                                .unwrap_or(empty_ranges);

                            let computed = build_cached_diff_styled_text(
                                theme,
                                text,
                                word_ranges,
                                query.as_str(),
                                language,
                                DiffSyntaxMode::HeuristicOnly,
                                word_color,
                            );
                            this.diff_text_segments_cache_set(key, computed);
                        }
                    }

                    let Some(row) = this.file_diff_cache_rows.get(row_ix) else {
                        return div()
                            .id(("diff_split_left_oob", visible_ix))
                            .h(px(20.0))
                            .px_2()
                            .font_family("monospace")
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .child("…")
                            .into_any_element();
                    };
                    let styled: Option<&CachedDiffStyledText> = row
                        .old
                        .is_some()
                        .then(|| this.diff_text_segments_cache_get(key))
                        .flatten();

                    patch_split_column_row(
                        theme,
                        PatchSplitColumn::Left,
                        visible_ix,
                        selected,
                        row,
                        styled,
                        cx,
                    )
                })
                .collect();
        }

        let theme = this.theme;
        if this.diff_text_segments_cache_query != this.diff_visible_query {
            this.diff_text_segments_cache_query = this.diff_visible_query.clone();
            this.diff_text_segments_cache.clear();
        }
        let query = this.diff_visible_query.clone();
        let syntax_enabled = this.diff_cache.len() <= MAX_LINES_FOR_SYNTAX_HIGHLIGHTING;
        let empty_ranges: &[Range<usize>] = &[];
        range
            .map(|visible_ix| {
                let selected = this
                    .diff_selection_range
                    .is_some_and(|(a, b)| visible_ix >= a.min(b) && visible_ix <= a.max(b));

                let Some(row_ix) = this.diff_visible_indices.get(visible_ix).copied() else {
                    return div()
                        .id(("diff_split_left_missing", visible_ix))
                        .h(px(20.0))
                        .px_2()
                        .font_family("monospace")
                        .text_xs()
                        .text_color(theme.colors.text_muted)
                        .child("…")
                        .into_any_element();
                };
                let Some(row) = this.diff_split_cache.get(row_ix) else {
                    return div()
                        .id(("diff_split_left_oob", visible_ix))
                        .h(px(20.0))
                        .px_2()
                        .font_family("monospace")
                        .text_xs()
                        .text_color(theme.colors.text_muted)
                        .child("…")
                        .into_any_element();
                };

                match row {
                    PatchSplitRow::Aligned { old_src_ix, .. } => {
                        if let Some(src_ix) = *old_src_ix
                            && this.diff_text_segments_cache_get(src_ix).is_none()
                        {
                            let Some(PatchSplitRow::Aligned { row, .. }) =
                                this.diff_split_cache.get(row_ix)
                            else {
                                return div()
                                    .id(("diff_split_left_oob", visible_ix))
                                    .h(px(20.0))
                                    .px_2()
                                    .font_family("monospace")
                                    .text_xs()
                                    .text_color(theme.colors.text_muted)
                                    .child("…")
                                    .into_any_element();
                            };

                            let text = row.old.as_deref().unwrap_or("");
                            let language = if syntax_enabled {
                                this.diff_file_for_src_ix
                                    .get(src_ix)
                                    .and_then(|p| p.as_deref())
                                    .and_then(diff_syntax_language_for_path)
                            } else {
                                None
                            };
                            let language = this
                                .diff_cache
                                .get(src_ix)
                                .is_some_and(|l| {
                                    matches!(
                                        l.kind,
                                        gitgpui_core::domain::DiffLineKind::Add
                                            | gitgpui_core::domain::DiffLineKind::Remove
                                    )
                                })
                                .then_some(language)
                                .flatten();
                            let word_ranges: &[Range<usize>] = this
                                .diff_word_highlights
                                .get(src_ix)
                                .and_then(|r| r.as_ref().map(Vec::as_slice))
                                .unwrap_or(empty_ranges);
                            let word_color = this.diff_cache.get(src_ix).and_then(|line| match line.kind {
                                gitgpui_core::domain::DiffLineKind::Add => Some(theme.colors.success),
                                gitgpui_core::domain::DiffLineKind::Remove => Some(theme.colors.danger),
                                _ => None,
                            });

                            let computed = build_cached_diff_styled_text(
                                theme,
                                text,
                                word_ranges,
                                query.as_str(),
                                language,
                                DiffSyntaxMode::HeuristicOnly,
                                word_color,
                            );
                            this.diff_text_segments_cache_set(src_ix, computed);
                        }

                        let Some(PatchSplitRow::Aligned { row, old_src_ix, .. }) =
                            this.diff_split_cache.get(row_ix)
                        else {
                            return div()
                                .id(("diff_split_left_oob", visible_ix))
                                .h(px(20.0))
                                .px_2()
                                .font_family("monospace")
                                .text_xs()
                                .text_color(theme.colors.text_muted)
                                .child("…")
                                .into_any_element();
                        };

                        let styled = old_src_ix.and_then(|src_ix| this.diff_text_segments_cache_get(src_ix));

                        patch_split_column_row(
                            theme,
                            PatchSplitColumn::Left,
                            visible_ix,
                            selected,
                            row,
                            styled,
                            cx,
                        )
                    }
                    PatchSplitRow::Raw { src_ix, click_kind } => {
                        let Some(line) = this.diff_cache.get(*src_ix) else {
                            return div()
                                .id(("diff_split_left_src_oob", visible_ix))
                                .h(px(20.0))
                                .px_2()
                                .font_family("monospace")
                                .text_xs()
                                .text_color(theme.colors.text_muted)
                                .child("…")
                                .into_any_element();
                        };
                        let file_stat = this.diff_file_stats.get(*src_ix).and_then(|s| *s);
                        patch_split_header_row(
                            theme,
                            PatchSplitColumn::Left,
                            visible_ix,
                            *click_kind,
                            selected,
                            line,
                            file_stat,
                            cx,
                        )
                    }
                }
            })
            .collect()
    }

    pub(super) fn render_diff_split_right_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        if this.is_file_diff_view_active() {
            let theme = this.theme;
            if this.diff_text_segments_cache_query != this.diff_visible_query {
                this.diff_text_segments_cache_query = this.diff_visible_query.clone();
                this.diff_text_segments_cache.clear();
            }
            let query = this.diff_visible_query.clone();
            let empty_ranges: &[Range<usize>] = &[];
            let language = (this.file_diff_cache_rows.len() <= MAX_LINES_FOR_SYNTAX_HIGHLIGHTING)
                .then(|| {
                    this.file_diff_cache_path
                        .as_ref()
                        .and_then(|p| diff_syntax_language_for_path(p.to_string_lossy().as_ref()))
                })
                .flatten();

            return range
                .map(|visible_ix| {
                    let selected = this
                        .diff_selection_range
                        .is_some_and(|(a, b)| visible_ix >= a.min(b) && visible_ix <= a.max(b));

                    let Some(row_ix) = this.diff_visible_indices.get(visible_ix).copied() else {
                        return div()
                            .id(("diff_split_right_missing", visible_ix))
                            .h(px(20.0))
                            .px_2()
                            .font_family("monospace")
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .child("…")
                            .into_any_element();
                    };
                    let key = row_ix * 2 + 1;
                    if this.diff_text_segments_cache_get(key).is_none() {
                        let Some(row) = this.file_diff_cache_rows.get(row_ix) else {
                            return div()
                                .id(("diff_split_right_oob", visible_ix))
                                .h(px(20.0))
                                .px_2()
                                .font_family("monospace")
                                .text_xs()
                                .text_color(theme.colors.text_muted)
                                .child("…")
                                .into_any_element();
                        };

                        if let Some(text) = row.new.as_deref() {
                            let word_color = matches!(
                                row.kind,
                                gitgpui_core::file_diff::FileDiffRowKind::Add
                                    | gitgpui_core::file_diff::FileDiffRowKind::Modify
                            )
                            .then_some(theme.colors.success);

                            let language = (!matches!(
                                row.kind,
                                gitgpui_core::file_diff::FileDiffRowKind::Context
                            ))
                            .then_some(language)
                            .flatten();

                            let word_ranges: &[Range<usize>] = this
                                .file_diff_split_word_highlights_new
                                .get(row_ix)
                                .and_then(|r| r.as_ref().map(Vec::as_slice))
                                .unwrap_or(empty_ranges);

                            let computed = build_cached_diff_styled_text(
                                theme,
                                text,
                                word_ranges,
                                query.as_str(),
                                language,
                                DiffSyntaxMode::HeuristicOnly,
                                word_color,
                            );
                            this.diff_text_segments_cache_set(key, computed);
                        }
                    }

                    let Some(row) = this.file_diff_cache_rows.get(row_ix) else {
                        return div()
                            .id(("diff_split_right_oob", visible_ix))
                            .h(px(20.0))
                            .px_2()
                            .font_family("monospace")
                            .text_xs()
                            .text_color(theme.colors.text_muted)
                            .child("…")
                            .into_any_element();
                    };
                    let styled: Option<&CachedDiffStyledText> = row
                        .new
                        .is_some()
                        .then(|| this.diff_text_segments_cache_get(key))
                        .flatten();

                    patch_split_column_row(
                        theme,
                        PatchSplitColumn::Right,
                        visible_ix,
                        selected,
                        row,
                        styled,
                        cx,
                    )
                })
                .collect();
        }

        let theme = this.theme;
        if this.diff_text_segments_cache_query != this.diff_visible_query {
            this.diff_text_segments_cache_query = this.diff_visible_query.clone();
            this.diff_text_segments_cache.clear();
        }
        let query = this.diff_visible_query.clone();
        let syntax_enabled = this.diff_cache.len() <= MAX_LINES_FOR_SYNTAX_HIGHLIGHTING;
        let empty_ranges: &[Range<usize>] = &[];
        range
            .map(|visible_ix| {
                let selected = this
                    .diff_selection_range
                    .is_some_and(|(a, b)| visible_ix >= a.min(b) && visible_ix <= a.max(b));

                let Some(row_ix) = this.diff_visible_indices.get(visible_ix).copied() else {
                    return div()
                        .id(("diff_split_right_missing", visible_ix))
                        .h(px(20.0))
                        .px_2()
                        .font_family("monospace")
                        .text_xs()
                        .text_color(theme.colors.text_muted)
                        .child("…")
                        .into_any_element();
                };
                let Some(row) = this.diff_split_cache.get(row_ix) else {
                    return div()
                        .id(("diff_split_right_oob", visible_ix))
                        .h(px(20.0))
                        .px_2()
                        .font_family("monospace")
                        .text_xs()
                        .text_color(theme.colors.text_muted)
                        .child("…")
                        .into_any_element();
                };

                match row {
                    PatchSplitRow::Aligned { new_src_ix, .. } => {
                        if let Some(src_ix) = *new_src_ix
                            && this.diff_text_segments_cache_get(src_ix).is_none()
                        {
                            let Some(PatchSplitRow::Aligned { row, .. }) =
                                this.diff_split_cache.get(row_ix)
                            else {
                                return div()
                                    .id(("diff_split_right_oob", visible_ix))
                                    .h(px(20.0))
                                    .px_2()
                                    .font_family("monospace")
                                    .text_xs()
                                    .text_color(theme.colors.text_muted)
                                    .child("…")
                                    .into_any_element();
                            };

                            let text = row.new.as_deref().unwrap_or("");
                            let language = if syntax_enabled {
                                this.diff_file_for_src_ix
                                    .get(src_ix)
                                    .and_then(|p| p.as_deref())
                                    .and_then(diff_syntax_language_for_path)
                            } else {
                                None
                            };
                            let language = this
                                .diff_cache
                                .get(src_ix)
                                .is_some_and(|l| {
                                    matches!(
                                        l.kind,
                                        gitgpui_core::domain::DiffLineKind::Add
                                            | gitgpui_core::domain::DiffLineKind::Remove
                                    )
                                })
                                .then_some(language)
                                .flatten();
                            let word_ranges: &[Range<usize>] = this
                                .diff_word_highlights
                                .get(src_ix)
                                .and_then(|r| r.as_ref().map(Vec::as_slice))
                                .unwrap_or(empty_ranges);
                            let word_color = this.diff_cache.get(src_ix).and_then(|line| match line.kind {
                                gitgpui_core::domain::DiffLineKind::Add => Some(theme.colors.success),
                                gitgpui_core::domain::DiffLineKind::Remove => Some(theme.colors.danger),
                                _ => None,
                            });

                            let computed = build_cached_diff_styled_text(
                                theme,
                                text,
                                word_ranges,
                                query.as_str(),
                                language,
                                DiffSyntaxMode::HeuristicOnly,
                                word_color,
                            );
                            this.diff_text_segments_cache_set(src_ix, computed);
                        }

                        let Some(PatchSplitRow::Aligned { row, new_src_ix, .. }) =
                            this.diff_split_cache.get(row_ix)
                        else {
                            return div()
                                .id(("diff_split_right_oob", visible_ix))
                                .h(px(20.0))
                                .px_2()
                                .font_family("monospace")
                                .text_xs()
                                .text_color(theme.colors.text_muted)
                                .child("…")
                                .into_any_element();
                        };

                        let styled =
                            new_src_ix.and_then(|src_ix| this.diff_text_segments_cache_get(src_ix));

                        patch_split_column_row(
                            theme,
                            PatchSplitColumn::Right,
                            visible_ix,
                            selected,
                            row,
                            styled,
                            cx,
                        )
                    }
                    PatchSplitRow::Raw { src_ix, click_kind } => {
                        let Some(line) = this.diff_cache.get(*src_ix) else {
                            return div()
                                .id(("diff_split_right_src_oob", visible_ix))
                                .h(px(20.0))
                                .px_2()
                                .font_family("monospace")
                                .text_xs()
                                .text_color(theme.colors.text_muted)
                                .child("…")
                                .into_any_element();
                        };
                        let file_stat = this.diff_file_stats.get(*src_ix).and_then(|s| *s);
                        patch_split_header_row(
                            theme,
                            PatchSplitColumn::Right,
                            visible_ix,
                            *click_kind,
                            selected,
                            line,
                            file_stat,
                            cx,
                        )
                    }
                }
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
                .child(graph),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .gap_2()
                .child(div().w(px(3.0)).h_full().bg(node_color))
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
    repo_id: RepoId,
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
            .child(div().text_xs().text_color(theme.colors.text_muted).child(count.to_string()))
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

    div()
        .id(("history_worktree_summary", repo_id.0))
        .h(px(28.0))
        .flex()
        .w_full()
        .items_center()
        .gap_2()
        .px_2()
        .bg(theme.colors.surface_bg_elevated)
        .border_1()
        .border_color(theme.colors.border)
        .rounded(px(theme.radii.row))
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
        .child(div().w(col_graph).h_full())
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
                )
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
            this.store
                .dispatch(Msg::ClearCommitSelection { repo_id });
            cx.notify();
        }))
        .into_any_element()
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

fn status_row(
    theme: AppTheme,
    ix: usize,
    entry: &FileStatus,
    area: DiffArea,
    repo_id: RepoId,
    show_stage_button: bool,
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

    let hover_area = area;
    div()
        .id(ix)
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .px_2()
        .py_1()
        .w_full()
        .rounded(px(theme.radii.row))
        .hover(move |s| s.bg(theme.colors.hover))
        .active(move |s| s.bg(theme.colors.active))
        .on_hover(cx.listener(move |this, hovering: &bool, _w, cx| {
            if *hovering {
                this.hovered_status_row = Some((repo_id, hover_area, path_for_hover.clone()));
            } else if this.hovered_status_row.as_ref().is_some_and(|(r, a, p)| {
                *r == repo_id && *a == hover_area && p == &path_for_hover
            }) {
                this.hovered_status_row = None;
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
                        .line_clamp(1)
                        .child(path.display().to_string()),
                ),
        )
        .child(
            div()
                .flex_none()
                .w(px(78.0))
                .flex()
                .justify_end()
                .when(show_stage_button, |this| {
                    this.child(
                        zed::Button::new(format!("stage_btn_{ix}"), stage_label)
                            .style(zed::ButtonStyle::Outlined)
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
    visible_ix: usize,
    click_kind: DiffClickKind,
    selected: bool,
    mode: DiffViewMode,
    line: &AnnotatedDiffLine,
    file_stat: Option<(usize, usize)>,
    styled: Option<&CachedDiffStyledText>,
    cx: &mut gpui::Context<GitGpuiView>,
) -> AnyElement {
    let on_click = cx.listener(move |this, e: &ClickEvent, _w, cx| {
        if this.consume_suppress_click_after_drag() {
            cx.notify();
            return;
        }
        this.handle_patch_row_click(visible_ix, click_kind, e.modifiers().shift);
        cx.notify();
    });

    if matches!(click_kind, DiffClickKind::FileHeader) {
        let file = parse_diff_git_header_path(&line.text).unwrap_or_else(|| line.text.clone());
        let mut row = div()
            .id(("diff_file_hdr", visible_ix))
            .h(px(28.0))
            .flex()
            .items_center()
            .justify_between()
            .px_2()
            .bg(theme.colors.surface_bg_elevated)
            .border_b_1()
            .border_color(theme.colors.border)
            .font_family("monospace")
            .text_sm()
            .font_weight(FontWeight::BOLD)
            .child(selectable_cached_diff_text(
                visible_ix,
                DiffTextRegion::Inline,
                DiffClickKind::FileHeader,
                theme.colors.text,
                None,
                file.into(),
                cx,
            ))
            .when(file_stat.is_some_and(|(a, r)| a > 0 || r > 0), |this| {
                let (a, r) = file_stat.unwrap_or_default();
                this.child(zed::diff_stat(theme, a, r))
            })
            .on_click(on_click);

        if selected {
            row = row
                .border_1()
                .border_color(with_alpha(theme.colors.accent, 0.55));
        }

        return row.into_any_element();
    }

    if matches!(click_kind, DiffClickKind::HunkHeader) {
        let display = parse_unified_hunk_header_for_display(&line.text)
            .map(|p| {
                let heading = p.heading.unwrap_or_default();
                if heading.is_empty() {
                    format!("{} {}", p.old, p.new)
                } else {
                    format!("{} {}  {heading}", p.old, p.new)
                }
            })
            .unwrap_or_else(|| line.text.clone());

        let mut row = div()
            .id(("diff_hunk_hdr", visible_ix))
            .h(px(24.0))
            .flex()
            .items_center()
            .px_2()
            .bg(with_alpha(
                theme.colors.accent,
                if theme.is_dark { 0.10 } else { 0.07 },
            ))
            .border_b_1()
            .border_color(with_alpha(
                theme.colors.accent,
                if theme.is_dark { 0.28 } else { 0.22 },
            ))
            .font_family("monospace")
            .text_xs()
            .text_color(theme.colors.text_muted)
            .child(selectable_cached_diff_text(
                visible_ix,
                DiffTextRegion::Inline,
                DiffClickKind::HunkHeader,
                theme.colors.text_muted,
                None,
                display.into(),
                cx,
            ))
            .on_click(on_click);

        if selected {
            row = row
                .border_1()
                .border_color(with_alpha(theme.colors.accent, 0.55));
        }

        return row.into_any_element();
    }

    let (bg, fg, gutter_fg) = diff_line_colors(theme, line.kind);

    let old = line_number_string(line.old_line);
    let new = line_number_string(line.new_line);

    match mode {
        DiffViewMode::Inline => {
            let mut row = div()
                .id(("diff_row", visible_ix))
                .h(px(20.0))
                .flex()
                .items_center()
                .bg(bg)
                .font_family("monospace")
                .text_xs()
                .on_click(on_click)
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

            if selected {
                row = row
                    .border_1()
                    .border_color(with_alpha(theme.colors.accent, 0.55));
            }

            row.child(
                div()
                    .flex_1()
                    .px_2()
                    .text_color(fg)
                    .whitespace_nowrap()
                    .child(selectable_cached_diff_text(
                        visible_ix,
                        DiffTextRegion::Inline,
                        DiffClickKind::Line,
                        fg,
                        styled,
                        SharedString::default(),
                        cx,
                    )),
            )
            .into_any_element()
        }
        DiffViewMode::Split => {
            let left_kind = match line.kind {
                gitgpui_core::domain::DiffLineKind::Remove => {
                    gitgpui_core::domain::DiffLineKind::Remove
                }
                gitgpui_core::domain::DiffLineKind::Add => {
                    gitgpui_core::domain::DiffLineKind::Context
                }
                _ => gitgpui_core::domain::DiffLineKind::Context,
            };
            let right_kind = match line.kind {
                gitgpui_core::domain::DiffLineKind::Add => gitgpui_core::domain::DiffLineKind::Add,
                gitgpui_core::domain::DiffLineKind::Remove => {
                    gitgpui_core::domain::DiffLineKind::Context
                }
                _ => gitgpui_core::domain::DiffLineKind::Context,
            };

            let (left_bg, left_fg, left_gutter) = diff_line_colors(theme, left_kind);
            let (right_bg, right_fg, right_gutter) = diff_line_colors(theme, right_kind);

            let (left_text, right_text) = match line.kind {
                gitgpui_core::domain::DiffLineKind::Remove => (styled, None),
                gitgpui_core::domain::DiffLineKind::Add => (None, styled),
                gitgpui_core::domain::DiffLineKind::Context => (styled, styled),
                _ => (styled, None),
            };

            let mut row = div()
                .id(("diff_row", visible_ix))
                .h(px(20.0))
                .flex()
                .items_center()
                .font_family("monospace")
                .text_xs()
                .on_click(on_click)
                .child(
                    div()
                        .bg(left_bg)
                        .flex_1()
                        .min_w(px(0.0))
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .w(px(44.0))
                                .px_2()
                                .text_color(left_gutter)
                                .whitespace_nowrap()
                                .child(old),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .px_2()
                                .text_color(left_fg)
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .child(selectable_cached_diff_text(
                                    visible_ix,
                                    DiffTextRegion::SplitLeft,
                                    DiffClickKind::Line,
                                    left_fg,
                                    left_text,
                                    SharedString::default(),
                                    cx,
                                )),
                        ),
                )
                .child(
                    div()
                        .w(px(1.0))
                        .h_full()
                        .bg(theme.colors.border),
                )
                .child(
                    div()
                        .bg(right_bg)
                        .flex_1()
                        .min_w(px(0.0))
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .w(px(44.0))
                                .px_2()
                                .text_color(right_gutter)
                                .whitespace_nowrap()
                                .child(new),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .px_2()
                                .text_color(right_fg)
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .child(selectable_cached_diff_text(
                                    visible_ix,
                                    DiffTextRegion::SplitRight,
                                    DiffClickKind::Line,
                                    right_fg,
                                    right_text,
                                    SharedString::default(),
                                    cx,
                                )),
                        ),
                );

            if selected {
                row = row
                    .border_1()
                    .border_color(with_alpha(theme.colors.accent, 0.55));
            }

            row.into_any_element()
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PatchSplitColumn {
    Left,
    Right,
}

fn patch_split_column_row(
    theme: AppTheme,
    column: PatchSplitColumn,
    visible_ix: usize,
    selected: bool,
    row: &gitgpui_core::file_diff::FileDiffRow,
    styled: Option<&CachedDiffStyledText>,
    cx: &mut gpui::Context<GitGpuiView>,
) -> AnyElement {
    let on_click = cx.listener(move |this, e: &ClickEvent, _w, cx| {
        if this.consume_suppress_click_after_drag() {
            cx.notify();
            return;
        }
        this.handle_patch_row_click(visible_ix, DiffClickKind::Line, e.modifiers().shift);
        cx.notify();
    });

    let (ctx_bg, ctx_fg, ctx_gutter) =
        diff_line_colors(theme, gitgpui_core::domain::DiffLineKind::Context);
    let (add_bg, add_fg, add_gutter) =
        diff_line_colors(theme, gitgpui_core::domain::DiffLineKind::Add);
    let (rem_bg, rem_fg, rem_gutter) =
        diff_line_colors(theme, gitgpui_core::domain::DiffLineKind::Remove);

    let (bg, fg, gutter_fg) = match (column, row.kind) {
        (
            PatchSplitColumn::Left,
            gitgpui_core::file_diff::FileDiffRowKind::Remove
            | gitgpui_core::file_diff::FileDiffRowKind::Modify,
        ) => (rem_bg, rem_fg, rem_gutter),
        (
            PatchSplitColumn::Right,
            gitgpui_core::file_diff::FileDiffRowKind::Add
            | gitgpui_core::file_diff::FileDiffRowKind::Modify,
        ) => (add_bg, add_fg, add_gutter),
        _ => (ctx_bg, ctx_fg, ctx_gutter),
    };

    let line_no = match column {
        PatchSplitColumn::Left => line_number_string(row.old_line),
        PatchSplitColumn::Right => line_number_string(row.new_line),
    };

    let mut el = div()
        .id((
            match column {
                PatchSplitColumn::Left => "diff_split_left_row",
                PatchSplitColumn::Right => "diff_split_right_row",
            },
            visible_ix,
        ))
        .h(px(20.0))
        .flex()
        .items_center()
        .font_family("monospace")
        .text_xs()
        .on_click(on_click)
        .child(
            div()
                .bg(bg)
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .child(
                    div()
                        .w(px(44.0))
                        .px_2()
                        .text_color(gutter_fg)
                        .whitespace_nowrap()
                        .child(line_no),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .px_2()
                        .text_color(fg)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(selectable_cached_diff_text(
                            visible_ix,
                            match column {
                                PatchSplitColumn::Left => DiffTextRegion::SplitLeft,
                                PatchSplitColumn::Right => DiffTextRegion::SplitRight,
                            },
                            DiffClickKind::Line,
                            fg,
                            styled,
                            SharedString::default(),
                            cx,
                        )),
                ),
        );

    if selected {
        el = el
            .border_1()
            .border_color(with_alpha(theme.colors.accent, 0.55));
    }

    el.into_any_element()
}

fn patch_split_header_row(
    theme: AppTheme,
    column: PatchSplitColumn,
    visible_ix: usize,
    click_kind: DiffClickKind,
    selected: bool,
    line: &AnnotatedDiffLine,
    file_stat: Option<(usize, usize)>,
    cx: &mut gpui::Context<GitGpuiView>,
) -> AnyElement {
    let on_click = cx.listener(move |this, e: &ClickEvent, _w, cx| {
        if this.consume_suppress_click_after_drag() {
            cx.notify();
            return;
        }
        this.handle_patch_row_click(visible_ix, click_kind, e.modifiers().shift);
        cx.notify();
    });
    let region = match column {
        PatchSplitColumn::Left => DiffTextRegion::SplitLeft,
        PatchSplitColumn::Right => DiffTextRegion::SplitRight,
    };

    match click_kind {
        DiffClickKind::FileHeader => {
            let file = parse_diff_git_header_path(&line.text).unwrap_or_else(|| line.text.clone());
            let mut row = div()
                .id((
                    match column {
                        PatchSplitColumn::Left => "diff_split_left_file_hdr",
                        PatchSplitColumn::Right => "diff_split_right_file_hdr",
                    },
                    visible_ix,
                ))
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_between()
                .px_2()
                .bg(theme.colors.surface_bg_elevated)
                .border_b_1()
                .border_color(theme.colors.border)
                .font_family("monospace")
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .child(selectable_cached_diff_text(
                    visible_ix,
                    region,
                    DiffClickKind::FileHeader,
                    theme.colors.text,
                    None,
                    file.into(),
                    cx,
                ))
                .when(file_stat.is_some_and(|(a, r)| a > 0 || r > 0), |this| {
                    let (a, r) = file_stat.unwrap_or_default();
                    this.child(zed::diff_stat(theme, a, r))
                })
                .on_click(on_click);

            if selected {
                row = row
                    .border_1()
                    .border_color(with_alpha(theme.colors.accent, 0.55));
            }

            row.into_any_element()
        }
        DiffClickKind::HunkHeader => {
            let display = parse_unified_hunk_header_for_display(&line.text)
                .map(|p| {
                    let heading = p.heading.unwrap_or_default();
                    if heading.is_empty() {
                        format!("{} {}", p.old, p.new)
                    } else {
                        format!("{} {}  {heading}", p.old, p.new)
                    }
                })
                .unwrap_or_else(|| line.text.clone());

            let mut row = div()
                .id((
                    match column {
                        PatchSplitColumn::Left => "diff_split_left_hunk_hdr",
                        PatchSplitColumn::Right => "diff_split_right_hunk_hdr",
                    },
                    visible_ix,
                ))
                .h(px(24.0))
                .flex()
                .items_center()
                .px_2()
                .bg(with_alpha(
                    theme.colors.accent,
                    if theme.is_dark { 0.10 } else { 0.07 },
                ))
                .border_b_1()
                .border_color(with_alpha(
                    theme.colors.accent,
                    if theme.is_dark { 0.28 } else { 0.22 },
                ))
                .font_family("monospace")
                .text_xs()
                .text_color(theme.colors.text_muted)
                .child(selectable_cached_diff_text(
                    visible_ix,
                    region,
                    DiffClickKind::HunkHeader,
                    theme.colors.text_muted,
                    None,
                    display.into(),
                    cx,
                ))
                .on_click(on_click);

            if selected {
                row = row
                    .border_1()
                    .border_color(with_alpha(theme.colors.accent, 0.55));
            }

            row.into_any_element()
        }
        DiffClickKind::Line => patch_split_meta_row(theme, column, visible_ix, selected, line, cx),
    }
}

fn patch_split_meta_row(
    theme: AppTheme,
    column: PatchSplitColumn,
    visible_ix: usize,
    selected: bool,
    line: &AnnotatedDiffLine,
    cx: &mut gpui::Context<GitGpuiView>,
) -> AnyElement {
    let on_click = cx.listener(move |this, e: &ClickEvent, _w, cx| {
        if this.consume_suppress_click_after_drag() {
            cx.notify();
            return;
        }
        this.handle_patch_row_click(visible_ix, DiffClickKind::Line, e.modifiers().shift);
        cx.notify();
    });
    let region = match column {
        PatchSplitColumn::Left => DiffTextRegion::SplitLeft,
        PatchSplitColumn::Right => DiffTextRegion::SplitRight,
    };

    let (bg, fg, _) = diff_line_colors(theme, line.kind);
    let mut row = div()
        .id((
            match column {
                PatchSplitColumn::Left => "diff_split_left_meta",
                PatchSplitColumn::Right => "diff_split_right_meta",
            },
            visible_ix,
        ))
        .h(px(20.0))
        .flex()
        .items_center()
        .px_2()
        .font_family("monospace")
        .text_xs()
        .bg(bg)
        .text_color(fg)
        .whitespace_nowrap()
        .child(selectable_cached_diff_text(
            visible_ix,
            region,
            DiffClickKind::Line,
            fg,
            None,
            line.text.clone().into(),
            cx,
        ))
        .on_click(on_click);

    if selected {
        row = row
            .border_1()
            .border_color(with_alpha(theme.colors.accent, 0.55));
    }

    row.into_any_element()
}

fn maybe_expand_tabs(s: &str) -> SharedString {
    if !s.contains('\t') {
        return s.to_string().into();
    }

    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\t' => out.push_str("    "),
            _ => out.push(ch),
        }
    }
    out.into()
}

fn build_diff_text_segments(
    text: &str,
    word_ranges: &[Range<usize>],
    query: &str,
    language: Option<DiffSyntaxLanguage>,
    syntax_mode: DiffSyntaxMode,
) -> Vec<CachedDiffTextSegment> {
    if text.is_empty() {
        return Vec::new();
    }

    let query = query.trim();
    if word_ranges.is_empty() && query.is_empty() && language.is_none() {
        return vec![CachedDiffTextSegment {
            text: maybe_expand_tabs(text),
            in_word: false,
            in_query: false,
            syntax: SyntaxTokenKind::None,
        }];
    }

    let syntax_tokens = language
        .map(|language| syntax_tokens_for_line(text, language, syntax_mode))
        .unwrap_or_default();

    let query_range = (!query.is_empty())
        .then(|| find_ascii_case_insensitive(text, query))
        .flatten();

    let mut boundaries: Vec<usize> = Vec::with_capacity(
        2 + word_ranges.len() * 2
            + query_range.as_ref().map(|_| 2).unwrap_or(0)
            + syntax_tokens.len() * 2,
    );
    boundaries.push(0);
    boundaries.push(text.len());
    for r in word_ranges {
        boundaries.push(r.start.min(text.len()));
        boundaries.push(r.end.min(text.len()));
    }
    if let Some(r) = &query_range {
        boundaries.push(r.start);
        boundaries.push(r.end);
    }
    for t in &syntax_tokens {
        boundaries.push(t.range.start.min(text.len()));
        boundaries.push(t.range.end.min(text.len()));
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    let mut token_ix = 0usize;
    let mut segments = Vec::with_capacity(boundaries.len().saturating_sub(1));
    for w in boundaries.windows(2) {
        let (a, b) = (w[0], w[1]);
        if a >= b || a >= text.len() {
            continue;
        }
        let b = b.min(text.len());
        let Some(seg) = text.get(a..b) else {
            // Defensive fallback: if any boundary isn't a UTF-8 char boundary, avoid panicking and
            // render the whole line without highlights.
            return vec![CachedDiffTextSegment {
                text: maybe_expand_tabs(text),
                in_word: false,
                in_query: false,
                syntax: SyntaxTokenKind::None,
            }];
        };

        while token_ix < syntax_tokens.len() && syntax_tokens[token_ix].range.end <= a {
            token_ix += 1;
        }
        let syntax = syntax_tokens
            .get(token_ix)
            .filter(|t| t.range.start <= a && t.range.end >= b)
            .map(|t| t.kind)
            .unwrap_or(SyntaxTokenKind::None);

        let in_word = word_ranges.iter().any(|r| a < r.end && b > r.start);
        let in_query = query_range
            .as_ref()
            .is_some_and(|r| a < r.end && b > r.start);

        segments.push(CachedDiffTextSegment {
            text: maybe_expand_tabs(seg),
            in_word,
            in_query,
            syntax,
        });
    }

    segments
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffSyntaxLanguage {
    Plain,
    Html,
    Css,
    Hcl,
    Bicep,
    Lua,
    Makefile,
    Kotlin,
    Zig,
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Tsx,
    Go,
    C,
    Cpp,
    CSharp,
    FSharp,
    VisualBasic,
    Java,
    Php,
    Ruby,
    Json,
    Toml,
    Yaml,
    Bash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffSyntaxMode {
    Auto,
    HeuristicOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SyntaxToken {
    range: Range<usize>,
    kind: SyntaxTokenKind,
}

fn diff_syntax_language_for_path(path: &str) -> Option<DiffSyntaxLanguage> {
    let p = std::path::Path::new(path);
    let ext = p
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let file_name = p
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    Some(match ext.as_str() {
        "html" | "htm" => DiffSyntaxLanguage::Html,
        // Use HTML highlighting for XML-ish formats as a pragmatic baseline.
        "xml" | "svg" | "xsl" | "xslt" | "xsd" => DiffSyntaxLanguage::Html,
        "css" | "less" | "sass" | "scss" => DiffSyntaxLanguage::Css,
        "hcl" | "tf" | "tfvars" => DiffSyntaxLanguage::Hcl,
        "bicep" => DiffSyntaxLanguage::Bicep,
        "lua" => DiffSyntaxLanguage::Lua,
        "mk" => DiffSyntaxLanguage::Makefile,
        "kt" | "kts" => DiffSyntaxLanguage::Kotlin,
        "zig" => DiffSyntaxLanguage::Zig,
        "rs" => DiffSyntaxLanguage::Rust,
        "py" => DiffSyntaxLanguage::Python,
        "js" | "jsx" | "mjs" | "cjs" => DiffSyntaxLanguage::JavaScript,
        "ts" | "cts" | "mts" => DiffSyntaxLanguage::TypeScript,
        "tsx" => DiffSyntaxLanguage::Tsx,
        "go" => DiffSyntaxLanguage::Go,
        "c" | "h" => DiffSyntaxLanguage::C,
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => DiffSyntaxLanguage::Cpp,
        "cs" => DiffSyntaxLanguage::CSharp,
        "fs" | "fsx" | "fsi" => DiffSyntaxLanguage::FSharp,
        "vb" | "vbs" => DiffSyntaxLanguage::VisualBasic,
        "java" => DiffSyntaxLanguage::Java,
        "php" | "phtml" => DiffSyntaxLanguage::Php,
        "rb" => DiffSyntaxLanguage::Ruby,
        "json" => DiffSyntaxLanguage::Json,
        "toml" => DiffSyntaxLanguage::Toml,
        "yaml" | "yml" => DiffSyntaxLanguage::Yaml,
        "sh" | "bash" | "zsh" => DiffSyntaxLanguage::Bash,
        _ => {
            if file_name == "makefile" || file_name == "gnumakefile" {
                DiffSyntaxLanguage::Makefile
            } else {
                return None;
            }
        }
    })
}

fn syntax_tokens_for_line(
    text: &str,
    language: DiffSyntaxLanguage,
    mode: DiffSyntaxMode,
) -> Vec<SyntaxToken> {
    match mode {
        DiffSyntaxMode::HeuristicOnly => syntax_tokens_for_line_heuristic(text, language),
        DiffSyntaxMode::Auto => {
            if !should_use_treesitter_for_line(text) {
                return syntax_tokens_for_line_heuristic(text, language);
            }
            if let Some(tokens) = syntax_tokens_for_line_treesitter(text, language) {
                return tokens;
            }
            syntax_tokens_for_line_heuristic(text, language)
        }
    }
}

fn should_use_treesitter_for_line(text: &str) -> bool {
    text.len() <= MAX_TREESITTER_LINE_BYTES
}

fn syntax_tokens_for_line_treesitter(
    text: &str,
    language: DiffSyntaxLanguage,
) -> Option<Vec<SyntaxToken>> {
    let ts_language = tree_sitter_language(language)?;
    let query = tree_sitter_highlight_query(language)?;

    thread_local! {
        static TS_PARSER: RefCell<tree_sitter::Parser> = RefCell::new(tree_sitter::Parser::new());
    }

    let mut input = String::with_capacity(text.len() + 1);
    input.push_str(text);
    input.push('\n');

    let tree = TS_PARSER.with(|parser| {
        let mut parser = parser.borrow_mut();
        parser.set_language(&ts_language).ok()?;
        parser.parse(&input, None)
    })?;
    let mut cursor = tree_sitter::QueryCursor::new();

    let mut tokens: Vec<SyntaxToken> = Vec::new();
    let mut captures = cursor.captures(query, tree.root_node(), input.as_bytes());
    tree_sitter::StreamingIterator::advance(&mut captures);
    while let Some((m, capture_ix)) = captures.get() {
        let capture = m.captures.get(*capture_ix)?;
        let name = query.capture_names().get(capture.index as usize)?;
        if let Some(kind) = syntax_kind_from_capture_name(name) {
            let mut range = capture.node.byte_range();
            range.start = range.start.min(text.len());
            range.end = range.end.min(text.len());
            if range.start < range.end {
                tokens.push(SyntaxToken { range, kind });
            }
        }
        tree_sitter::StreamingIterator::advance(&mut captures);
    }

    if tokens.is_empty() {
        return Some(tokens);
    }

    tokens.sort_by(|a, b| a.range.start.cmp(&b.range.start).then(a.range.end.cmp(&b.range.end)));

    // Ensure non-overlapping tokens so the segment splitter can pick a single style per range.
    let mut out: Vec<SyntaxToken> = Vec::with_capacity(tokens.len());
    for mut token in tokens {
        if let Some(prev) = out.last()
            && token.range.start < prev.range.end
        {
            if token.range.end <= prev.range.end {
                continue;
            }
            token.range.start = prev.range.end;
            if token.range.start >= token.range.end {
                continue;
            }
        }
        out.push(token);
    }

    Some(out)
}

fn tree_sitter_language(language: DiffSyntaxLanguage) -> Option<tree_sitter::Language> {
    Some(match language {
        DiffSyntaxLanguage::Plain => return None,
        DiffSyntaxLanguage::Html => tree_sitter_html::LANGUAGE.into(),
        DiffSyntaxLanguage::Css => tree_sitter_css::LANGUAGE.into(),
        DiffSyntaxLanguage::Hcl => return None,
        DiffSyntaxLanguage::Bicep => return None,
        DiffSyntaxLanguage::Lua => return None,
        DiffSyntaxLanguage::Makefile => return None,
        DiffSyntaxLanguage::Kotlin => return None,
        DiffSyntaxLanguage::Zig => return None,
        DiffSyntaxLanguage::Rust => tree_sitter_rust::LANGUAGE.into(),
        DiffSyntaxLanguage::Python => tree_sitter_python::LANGUAGE.into(),
        DiffSyntaxLanguage::Go => tree_sitter_go::LANGUAGE.into(),
        DiffSyntaxLanguage::C => return None,
        DiffSyntaxLanguage::Cpp => return None,
        DiffSyntaxLanguage::CSharp => return None,
        DiffSyntaxLanguage::FSharp => return None,
        DiffSyntaxLanguage::VisualBasic => return None,
        DiffSyntaxLanguage::Java => return None,
        DiffSyntaxLanguage::Php => return None,
        DiffSyntaxLanguage::Ruby => return None,
        DiffSyntaxLanguage::Json => tree_sitter_json::LANGUAGE.into(),
        DiffSyntaxLanguage::Yaml => tree_sitter_yaml::language(),
        DiffSyntaxLanguage::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        DiffSyntaxLanguage::Tsx | DiffSyntaxLanguage::JavaScript => {
            tree_sitter_typescript::LANGUAGE_TSX.into()
        }
        DiffSyntaxLanguage::Bash => tree_sitter_bash::LANGUAGE.into(),
        DiffSyntaxLanguage::Toml => return None,
    })
}

fn tree_sitter_highlight_query(language: DiffSyntaxLanguage) -> Option<&'static tree_sitter::Query> {
    static HTML_QUERY: OnceLock<tree_sitter::Query> = OnceLock::new();
    static CSS_QUERY: OnceLock<tree_sitter::Query> = OnceLock::new();
    static RUST_QUERY: OnceLock<tree_sitter::Query> = OnceLock::new();
    static PY_QUERY: OnceLock<tree_sitter::Query> = OnceLock::new();
    static GO_QUERY: OnceLock<tree_sitter::Query> = OnceLock::new();
    static JSON_QUERY: OnceLock<tree_sitter::Query> = OnceLock::new();
    static YAML_QUERY: OnceLock<tree_sitter::Query> = OnceLock::new();
    static TS_QUERY: OnceLock<tree_sitter::Query> = OnceLock::new();
    static TSX_QUERY: OnceLock<tree_sitter::Query> = OnceLock::new();
    static JS_QUERY: OnceLock<tree_sitter::Query> = OnceLock::new();
    static BASH_QUERY: OnceLock<tree_sitter::Query> = OnceLock::new();

    Some(match language {
        DiffSyntaxLanguage::Plain => return None,
        DiffSyntaxLanguage::Html => HTML_QUERY.get_or_init(|| {
            tree_sitter::Query::new(
                &tree_sitter_html::LANGUAGE.into(),
                include_str!("../syntax/html_highlights.scm"),
            )
            .expect("html highlights.scm should compile")
        }),
        DiffSyntaxLanguage::Css => CSS_QUERY.get_or_init(|| {
            tree_sitter::Query::new(
                &tree_sitter_css::LANGUAGE.into(),
                include_str!("../../../../zed/crates/languages/src/css/highlights.scm"),
            )
            .expect("css highlights.scm should compile")
        }),
        DiffSyntaxLanguage::Hcl => return None,
        DiffSyntaxLanguage::Bicep => return None,
        DiffSyntaxLanguage::Lua => return None,
        DiffSyntaxLanguage::Makefile => return None,
        DiffSyntaxLanguage::Kotlin => return None,
        DiffSyntaxLanguage::Zig => return None,
        DiffSyntaxLanguage::Rust => RUST_QUERY.get_or_init(|| {
            tree_sitter::Query::new(
                &tree_sitter_rust::LANGUAGE.into(),
                include_str!("../../../../zed/crates/languages/src/rust/highlights.scm"),
            )
            .expect("rust highlights.scm should compile")
        }),
        DiffSyntaxLanguage::Python => PY_QUERY.get_or_init(|| {
            tree_sitter::Query::new(
                &tree_sitter_python::LANGUAGE.into(),
                include_str!("../../../../zed/crates/languages/src/python/highlights.scm"),
            )
            .expect("python highlights.scm should compile")
        }),
        DiffSyntaxLanguage::Go => GO_QUERY.get_or_init(|| {
            tree_sitter::Query::new(
                &tree_sitter_go::LANGUAGE.into(),
                include_str!("../../../../zed/crates/languages/src/go/highlights.scm"),
            )
            .expect("go highlights.scm should compile")
        }),
        DiffSyntaxLanguage::C => return None,
        DiffSyntaxLanguage::Cpp => return None,
        DiffSyntaxLanguage::CSharp => return None,
        DiffSyntaxLanguage::FSharp => return None,
        DiffSyntaxLanguage::VisualBasic => return None,
        DiffSyntaxLanguage::Java => return None,
        DiffSyntaxLanguage::Php => return None,
        DiffSyntaxLanguage::Ruby => return None,
        DiffSyntaxLanguage::Json => JSON_QUERY.get_or_init(|| {
            tree_sitter::Query::new(
                &tree_sitter_json::LANGUAGE.into(),
                include_str!("../../../../zed/crates/languages/src/json/highlights.scm"),
            )
            .expect("json highlights.scm should compile")
        }),
        DiffSyntaxLanguage::Yaml => YAML_QUERY.get_or_init(|| {
            tree_sitter::Query::new(
                &tree_sitter_yaml::language(),
                include_str!("../../../../zed/crates/languages/src/yaml/highlights.scm"),
            )
            .expect("yaml highlights.scm should compile")
        }),
        DiffSyntaxLanguage::TypeScript => TS_QUERY.get_or_init(|| {
            tree_sitter::Query::new(
                &tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                include_str!("../../../../zed/crates/languages/src/typescript/highlights.scm"),
            )
            .expect("typescript highlights.scm should compile")
        }),
        DiffSyntaxLanguage::Tsx => TSX_QUERY.get_or_init(|| {
            tree_sitter::Query::new(
                &tree_sitter_typescript::LANGUAGE_TSX.into(),
                include_str!("../../../../zed/crates/languages/src/tsx/highlights.scm"),
            )
            .expect("tsx highlights.scm should compile")
        }),
        DiffSyntaxLanguage::JavaScript => JS_QUERY.get_or_init(|| {
            tree_sitter::Query::new(
                &tree_sitter_typescript::LANGUAGE_TSX.into(),
                include_str!("../../../../zed/crates/languages/src/javascript/highlights.scm"),
            )
            .expect("javascript highlights.scm should compile")
        }),
        DiffSyntaxLanguage::Bash => BASH_QUERY.get_or_init(|| {
            tree_sitter::Query::new(
                &tree_sitter_bash::LANGUAGE.into(),
                include_str!("../../../../zed/crates/languages/src/bash/highlights.scm"),
            )
            .expect("bash highlights.scm should compile")
        }),
        DiffSyntaxLanguage::Toml => return None,
    })
}

fn syntax_kind_from_capture_name(name: &str) -> Option<SyntaxTokenKind> {
    let base = name.split('.').next().unwrap_or(name);
    Some(match base {
        "comment" => SyntaxTokenKind::Comment,
        "string" | "character" => SyntaxTokenKind::String,
        "keyword" => SyntaxTokenKind::Keyword,
        "include" | "preproc" => SyntaxTokenKind::Keyword,
        "number" => SyntaxTokenKind::Number,
        "boolean" => SyntaxTokenKind::Constant,
        "function" | "constructor" | "method" => SyntaxTokenKind::Function,
        "type" => SyntaxTokenKind::Type,
        // Tree-sitter highlight queries often capture most identifiers as `variable.*`.
        // Coloring these makes Rust diffs look like "everything is blue", so we skip them.
        "variable" => return None,
        "property" | "field" | "attribute" => SyntaxTokenKind::Property,
        "tag" | "namespace" | "selector" => SyntaxTokenKind::Type,
        "constant" => SyntaxTokenKind::Constant,
        "punctuation" | "operator" => SyntaxTokenKind::Punctuation,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn treesitter_line_length_guard() {
        assert!(should_use_treesitter_for_line("fn main() {}"));
        assert!(!should_use_treesitter_for_line(
            &"a".repeat(MAX_TREESITTER_LINE_BYTES + 1)
        ));
    }

    #[test]
    fn build_segments_fast_path_skips_syntax_work() {
        let segments = build_diff_text_segments("a\tb", &[], "", None, DiffSyntaxMode::Auto);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text.as_ref(), "a    b");
        assert!(!segments[0].in_word);
        assert!(!segments[0].in_query);
        assert_eq!(segments[0].syntax, SyntaxTokenKind::None);
    }

    #[test]
    fn build_cached_styled_text_plain_has_no_highlights() {
        let theme = AppTheme::zed_ayu_dark();
        let styled =
            build_cached_diff_styled_text(theme, "a\tb", &[], "", None, DiffSyntaxMode::Auto, None);
        assert_eq!(styled.text.as_ref(), "a    b");
        assert!(styled.highlights.is_empty());
    }

    #[test]
    fn build_segments_does_not_panic_on_non_char_boundary_ranges() {
        // This can happen if token ranges are computed in bytes that don't align to UTF-8
        // boundaries. We should never panic during diff rendering.
        let text = "aé"; // 'é' is 2 bytes in UTF-8
        let segments = build_diff_text_segments(text, &[1..2], "", None, DiffSyntaxMode::Auto);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text.as_ref(), text);
    }

    #[test]
    fn styled_text_highlights_cover_combined_ranges() {
        let theme = AppTheme::zed_ayu_dark();
        let segments = vec![
            CachedDiffTextSegment {
                text: "abc".into(),
                in_word: false,
                in_query: false,
                syntax: SyntaxTokenKind::None,
            },
            CachedDiffTextSegment {
                text: "def".into(),
                in_word: false,
                in_query: true,
                syntax: SyntaxTokenKind::Keyword,
            },
        ];

        let (text, highlights) = styled_text_for_diff_segments(theme, &segments, None);
        assert_eq!(text.as_ref(), "abcdef");
        assert_eq!(highlights.len(), 1);
        assert_eq!(highlights[0].0, 3..6);
        assert_eq!(highlights[0].1.font_weight, Some(FontWeight::BOLD));
        assert_eq!(highlights[0].1.color, Some(theme.colors.accent.into()));
    }

    #[test]
    fn styled_text_word_highlight_sets_background() {
        let theme = AppTheme::zed_ayu_dark();
        let segments = vec![CachedDiffTextSegment {
            text: "x".into(),
            in_word: true,
            in_query: false,
            syntax: SyntaxTokenKind::None,
        }];
        let (text, highlights) =
            styled_text_for_diff_segments(theme, &segments, Some(theme.colors.danger));
        assert_eq!(text.as_ref(), "x");
        assert_eq!(highlights.len(), 1);
        assert!(highlights[0].1.background_color.is_some());
    }

    #[test]
    fn xml_uses_html_highlighting() {
        assert_eq!(
            diff_syntax_language_for_path("foo.xml"),
            Some(DiffSyntaxLanguage::Html)
        );
    }

    #[test]
    fn treesitter_variable_capture_is_not_colored() {
        assert_eq!(syntax_kind_from_capture_name("variable"), None);
        assert_eq!(syntax_kind_from_capture_name("variable.parameter"), None);
    }
}

fn syntax_tokens_for_line_heuristic(text: &str, language: DiffSyntaxLanguage) -> Vec<SyntaxToken> {
    let mut tokens: Vec<SyntaxToken> = Vec::new();
    let len = text.len();
    let mut i = 0usize;

    let is_ident_start = |ch: char| ch == '_' || ch.is_ascii_alphabetic();
    let is_ident_continue = |ch: char| ch == '_' || ch.is_ascii_alphanumeric();
    let is_digit = |ch: char| ch.is_ascii_digit();

    while i < len {
        let rest = &text[i..];

        if matches!(language, DiffSyntaxLanguage::Html) && rest.starts_with("<!--") {
            let end = rest.find("-->").map(|ix| i + ix + 3).unwrap_or(len);
            tokens.push(SyntaxToken {
                range: i..end,
                kind: SyntaxTokenKind::Comment,
            });
            i = end;
            continue;
        }

        if matches!(language, DiffSyntaxLanguage::FSharp) && rest.starts_with("(*") {
            let end = rest.find("*)").map(|ix| i + ix + 2).unwrap_or(len);
            tokens.push(SyntaxToken {
                range: i..end,
                kind: SyntaxTokenKind::Comment,
            });
            i = end;
            continue;
        }

        if matches!(language, DiffSyntaxLanguage::Lua) && rest.starts_with("--") {
            if rest.starts_with("--[[") {
                let end = rest.find("]]").map(|ix| i + ix + 2).unwrap_or(len);
                tokens.push(SyntaxToken {
                    range: i..end,
                    kind: SyntaxTokenKind::Comment,
                });
                i = end;
                continue;
            }
            tokens.push(SyntaxToken {
                range: i..len,
                kind: SyntaxTokenKind::Comment,
            });
            break;
        }

        let (line_comment, hash_comment, block_comment) = match language {
            DiffSyntaxLanguage::Python | DiffSyntaxLanguage::Toml | DiffSyntaxLanguage::Yaml => {
                (None, Some('#'), false)
            }
            DiffSyntaxLanguage::Bash => (None, Some('#'), false),
            DiffSyntaxLanguage::Makefile => (None, Some('#'), false),
            DiffSyntaxLanguage::Rust
            | DiffSyntaxLanguage::JavaScript
            | DiffSyntaxLanguage::TypeScript
            | DiffSyntaxLanguage::Tsx
            | DiffSyntaxLanguage::Go
            | DiffSyntaxLanguage::C
            | DiffSyntaxLanguage::Cpp
            | DiffSyntaxLanguage::CSharp
            | DiffSyntaxLanguage::Java
            | DiffSyntaxLanguage::Kotlin
            | DiffSyntaxLanguage::Zig
            | DiffSyntaxLanguage::Bicep => (Some("//"), None, true),
            DiffSyntaxLanguage::Hcl => (Some("//"), Some('#'), true),
            DiffSyntaxLanguage::Php => (Some("//"), Some('#'), true),
            DiffSyntaxLanguage::Ruby
            | DiffSyntaxLanguage::FSharp
            | DiffSyntaxLanguage::VisualBasic
            | DiffSyntaxLanguage::Html
            | DiffSyntaxLanguage::Css => (None, None, false),
            DiffSyntaxLanguage::Json => (None, None, false),
            DiffSyntaxLanguage::Plain => (Some("//"), Some('#'), true),
            DiffSyntaxLanguage::Lua => (None, None, false),
        };

        if let Some(prefix) = line_comment
            && rest.starts_with(prefix)
        {
            tokens.push(SyntaxToken {
                range: i..len,
                kind: SyntaxTokenKind::Comment,
            });
            break;
        }

        if block_comment && rest.starts_with("/*") {
            let end = rest.find("*/").map(|ix| i + ix + 2).unwrap_or(len);
            tokens.push(SyntaxToken {
                range: i..end,
                kind: SyntaxTokenKind::Comment,
            });
            i = end;
            continue;
        }

        if matches!(language, DiffSyntaxLanguage::Ruby) && rest.starts_with('#') {
            tokens.push(SyntaxToken {
                range: i..len,
                kind: SyntaxTokenKind::Comment,
            });
            break;
        }

        if matches!(language, DiffSyntaxLanguage::VisualBasic)
            && (rest.starts_with('\'') || rest.to_ascii_lowercase().starts_with("rem "))
        {
            tokens.push(SyntaxToken {
                range: i..len,
                kind: SyntaxTokenKind::Comment,
            });
            break;
        }

        if let Some('#') = hash_comment
            && rest.starts_with('#')
        {
            tokens.push(SyntaxToken {
                range: i..len,
                kind: SyntaxTokenKind::Comment,
            });
            break;
        }

        let Some(ch) = rest.chars().next() else {
            break;
        };

        if ch == '"' || ch == '\'' || (ch == '`' && matches!(language, DiffSyntaxLanguage::JavaScript | DiffSyntaxLanguage::TypeScript | DiffSyntaxLanguage::Tsx | DiffSyntaxLanguage::Go | DiffSyntaxLanguage::Bash | DiffSyntaxLanguage::Plain)) {
            let quote = ch;
            let mut j = i + quote.len_utf8();
            let mut escaped = false;
            while j < len {
                let Some(next) = text[j..].chars().next() else {
                    break;
                };
                let next_len = next.len_utf8();
                if escaped {
                    escaped = false;
                    j += next_len;
                    continue;
                }
                if next == '\\' {
                    escaped = true;
                    j += next_len;
                    continue;
                }
                j += next_len;
                if next == quote {
                    break;
                }
            }
            tokens.push(SyntaxToken {
                range: i..j.min(len),
                kind: SyntaxTokenKind::String,
            });
            i = j.min(len);
            continue;
        }

        if is_digit(ch) {
            let mut j = i + ch.len_utf8();
            while j < len {
                let Some(next) = text[j..].chars().next() else {
                    break;
                };
                if next.is_ascii_digit() || matches!(next, '.' | '_' | 'x' | 'X' | 'a'..='f' | 'A'..='F')
                {
                    j += next.len_utf8();
                } else {
                    break;
                }
            }
            tokens.push(SyntaxToken {
                range: i..j,
                kind: SyntaxTokenKind::Number,
            });
            i = j;
            continue;
        }

        if is_ident_start(ch) {
            let mut j = i + ch.len_utf8();
            while j < len {
                let Some(next) = text[j..].chars().next() else {
                    break;
                };
                if is_ident_continue(next) {
                    j += next.len_utf8();
                } else {
                    break;
                }
            }
            let ident = &text[i..j];
            let mut kind = if is_keyword(language, ident) {
                Some(SyntaxTokenKind::Keyword)
            } else {
                None
            };

            if matches!(language, DiffSyntaxLanguage::Css) {
                let mut k = j;
                while k < len && text[k..].starts_with(char::is_whitespace) {
                    k += text[k..].chars().next().unwrap().len_utf8();
                }
                if k < len && text[k..].starts_with(':') {
                    kind = Some(SyntaxTokenKind::Property);
                }
            }

            if matches!(language, DiffSyntaxLanguage::Hcl | DiffSyntaxLanguage::Plain) {
                let mut k = j;
                while k < len && text[k..].starts_with(char::is_whitespace) {
                    k += text[k..].chars().next().unwrap().len_utf8();
                }
                if k < len && text[k..].starts_with('=') {
                    kind = Some(SyntaxTokenKind::Property);
                }
            }

            if matches!(language, DiffSyntaxLanguage::Html) && i > 0 {
                // Best-effort: highlight common attribute names (`foo=`) as properties.
                let mut k = j;
                while k < len && text[k..].starts_with(char::is_whitespace) {
                    k += text[k..].chars().next().unwrap().len_utf8();
                }
                if k < len && text[k..].starts_with('=') {
                    kind = Some(SyntaxTokenKind::Property);
                }
            }

            if let Some(kind) = kind {
                tokens.push(SyntaxToken {
                    range: i..j,
                    kind,
                });
            }
            i = j;
            continue;
        }

        if matches!(language, DiffSyntaxLanguage::Css) && ch == '@' {
            let mut j = i + 1;
            while j < len {
                let Some(next) = text[j..].chars().next() else {
                    break;
                };
                if next.is_ascii_alphanumeric() || next == '-' {
                    j += next.len_utf8();
                } else {
                    break;
                }
            }
            if j > i + 1 {
                tokens.push(SyntaxToken {
                    range: i..j,
                    kind: SyntaxTokenKind::Keyword,
                });
                i = j;
                continue;
            }
        }

        i += ch.len_utf8();
    }

    tokens
}

fn is_keyword(language: DiffSyntaxLanguage, ident: &str) -> bool {
    match language {
        DiffSyntaxLanguage::Plain => matches!(
            ident.to_ascii_lowercase().as_str(),
            "if"
                | "else"
                | "for"
                | "while"
                | "do"
                | "break"
                | "continue"
                | "return"
                | "fn"
                | "function"
                | "let"
                | "var"
                | "const"
                | "class"
                | "struct"
                | "enum"
                | "import"
                | "from"
                | "package"
                | "public"
                | "private"
                | "protected"
                | "static"
                | "new"
                | "true"
                | "false"
                | "null"
        ),
        DiffSyntaxLanguage::Html => matches!(
            ident.to_ascii_lowercase().as_str(),
            "doctype" | "html" | "head" | "body" | "script" | "style"
        ),
        DiffSyntaxLanguage::Css => matches!(
            ident.to_ascii_lowercase().as_str(),
            "@media"
                | "@import"
                | "@supports"
                | "@keyframes"
                | "@font-face"
                | "@layer"
                | "url"
                | "important"
        ),
        DiffSyntaxLanguage::Hcl => matches!(
            ident.to_ascii_lowercase().as_str(),
            "variable"
                | "locals"
                | "resource"
                | "data"
                | "provider"
                | "module"
                | "output"
                | "terraform"
                | "true"
                | "false"
                | "null"
        ),
        DiffSyntaxLanguage::Bicep => matches!(
            ident.to_ascii_lowercase().as_str(),
            "param"
                | "var"
                | "resource"
                | "module"
                | "output"
                | "existing"
                | "import"
                | "targetScope"
                | "true"
                | "false"
                | "null"
                | "if"
                | "for"
        ),
        DiffSyntaxLanguage::Lua => matches!(
            ident,
            "and"
                | "break"
                | "do"
                | "else"
                | "elseif"
                | "end"
                | "false"
                | "for"
                | "function"
                | "goto"
                | "if"
                | "in"
                | "local"
                | "nil"
                | "not"
                | "or"
                | "repeat"
                | "return"
                | "then"
                | "true"
                | "until"
                | "while"
        ),
        DiffSyntaxLanguage::Makefile => matches!(
            ident.to_ascii_lowercase().as_str(),
            "include"
                | "define"
                | "endef"
                | "ifeq"
                | "ifneq"
                | "ifdef"
                | "ifndef"
                | "else"
                | "endif"
                | "export"
                | "override"
        ),
        DiffSyntaxLanguage::Kotlin => matches!(
            ident,
            "package"
                | "import"
                | "as"
                | "class"
                | "interface"
                | "object"
                | "fun"
                | "val"
                | "var"
                | "typealias"
                | "data"
                | "sealed"
                | "enum"
                | "when"
                | "if"
                | "else"
                | "for"
                | "while"
                | "do"
                | "return"
                | "break"
                | "continue"
                | "try"
                | "catch"
                | "finally"
                | "throw"
                | "this"
                | "super"
                | "true"
                | "false"
                | "null"
        ),
        DiffSyntaxLanguage::Zig => matches!(
            ident,
            "const"
                | "var"
                | "fn"
                | "pub"
                | "comptime"
                | "asm"
                | "if"
                | "else"
                | "for"
                | "while"
                | "switch"
                | "return"
                | "break"
                | "continue"
                | "try"
                | "catch"
                | "defer"
                | "errdefer"
                | "struct"
                | "enum"
                | "union"
                | "opaque"
                | "true"
                | "false"
                | "null"
                | "undefined"
        ),
        DiffSyntaxLanguage::Rust => matches!(
            ident,
            "fn"
                | "let"
                | "mut"
                | "pub"
                | "struct"
                | "enum"
                | "impl"
                | "trait"
                | "use"
                | "mod"
                | "crate"
                | "super"
                | "self"
                | "Self"
                | "const"
                | "static"
                | "match"
                | "if"
                | "else"
                | "for"
                | "while"
                | "loop"
                | "in"
                | "move"
                | "async"
                | "await"
                | "return"
                | "break"
                | "continue"
                | "where"
                | "type"
                | "ref"
                | "unsafe"
                | "extern"
                | "dyn"
                | "as"
                | "true"
                | "false"
        ),
        DiffSyntaxLanguage::Python => matches!(
            ident,
            "def"
                | "class"
                | "return"
                | "if"
                | "elif"
                | "else"
                | "for"
                | "while"
                | "break"
                | "continue"
                | "pass"
                | "import"
                | "from"
                | "as"
                | "with"
                | "try"
                | "except"
                | "finally"
                | "raise"
                | "yield"
                | "lambda"
                | "True"
                | "False"
                | "None"
                | "async"
                | "await"
        ),
        DiffSyntaxLanguage::JavaScript | DiffSyntaxLanguage::TypeScript | DiffSyntaxLanguage::Tsx => matches!(
            ident,
            "function"
                | "const"
                | "let"
                | "var"
                | "return"
                | "if"
                | "else"
                | "for"
                | "while"
                | "break"
                | "continue"
                | "class"
                | "extends"
                | "import"
                | "from"
                | "export"
                | "default"
                | "new"
                | "this"
                | "super"
                | "try"
                | "catch"
                | "finally"
                | "throw"
                | "async"
                | "await"
                | "typeof"
                | "instanceof"
                | "in"
                | "of"
                | "true"
                | "false"
                | "null"
                | "undefined"
        ),
        DiffSyntaxLanguage::Go => matches!(
            ident,
            "func"
                | "package"
                | "import"
                | "return"
                | "if"
                | "else"
                | "for"
                | "range"
                | "switch"
                | "case"
                | "default"
                | "break"
                | "continue"
                | "go"
                | "defer"
                | "struct"
                | "interface"
                | "type"
                | "map"
                | "chan"
                | "select"
                | "var"
                | "const"
                | "true"
                | "false"
                | "nil"
        ),
        DiffSyntaxLanguage::Json => matches!(ident, "true" | "false" | "null"),
        DiffSyntaxLanguage::Toml | DiffSyntaxLanguage::Yaml => matches!(ident, "true" | "false" | "null"),
        DiffSyntaxLanguage::C => matches!(
            ident,
            "auto"
                | "break"
                | "case"
                | "char"
                | "const"
                | "continue"
                | "default"
                | "do"
                | "double"
                | "else"
                | "enum"
                | "extern"
                | "float"
                | "for"
                | "goto"
                | "if"
                | "inline"
                | "int"
                | "long"
                | "register"
                | "restrict"
                | "return"
                | "short"
                | "signed"
                | "sizeof"
                | "static"
                | "struct"
                | "switch"
                | "typedef"
                | "union"
                | "unsigned"
                | "void"
                | "volatile"
                | "while"
                | "true"
                | "false"
                | "NULL"
        ),
        DiffSyntaxLanguage::Cpp => matches!(
            ident,
            "alignas"
                | "alignof"
                | "and"
                | "and_eq"
                | "asm"
                | "auto"
                | "bitand"
                | "bitor"
                | "bool"
                | "break"
                | "case"
                | "catch"
                | "char"
                | "char16_t"
                | "char32_t"
                | "class"
                | "const"
                | "constexpr"
                | "const_cast"
                | "continue"
                | "decltype"
                | "default"
                | "delete"
                | "do"
                | "double"
                | "dynamic_cast"
                | "else"
                | "enum"
                | "explicit"
                | "export"
                | "extern"
                | "false"
                | "float"
                | "for"
                | "friend"
                | "goto"
                | "if"
                | "inline"
                | "int"
                | "long"
                | "mutable"
                | "namespace"
                | "new"
                | "noexcept"
                | "not"
                | "not_eq"
                | "nullptr"
                | "operator"
                | "or"
                | "or_eq"
                | "private"
                | "protected"
                | "public"
                | "register"
                | "reinterpret_cast"
                | "return"
                | "short"
                | "signed"
                | "sizeof"
                | "static"
                | "static_assert"
                | "static_cast"
                | "struct"
                | "switch"
                | "template"
                | "this"
                | "thread_local"
                | "throw"
                | "true"
                | "try"
                | "typedef"
                | "typeid"
                | "typename"
                | "union"
                | "unsigned"
                | "using"
                | "virtual"
                | "void"
                | "volatile"
                | "wchar_t"
                | "while"
                | "xor"
                | "xor_eq"
        ),
        DiffSyntaxLanguage::CSharp => matches!(
            ident,
            "abstract"
                | "as"
                | "base"
                | "bool"
                | "break"
                | "byte"
                | "case"
                | "catch"
                | "char"
                | "checked"
                | "class"
                | "const"
                | "continue"
                | "decimal"
                | "default"
                | "delegate"
                | "do"
                | "double"
                | "else"
                | "enum"
                | "event"
                | "explicit"
                | "extern"
                | "false"
                | "finally"
                | "fixed"
                | "float"
                | "for"
                | "foreach"
                | "goto"
                | "if"
                | "implicit"
                | "in"
                | "int"
                | "interface"
                | "internal"
                | "is"
                | "lock"
                | "long"
                | "namespace"
                | "new"
                | "null"
                | "object"
                | "operator"
                | "out"
                | "override"
                | "params"
                | "private"
                | "protected"
                | "public"
                | "readonly"
                | "ref"
                | "return"
                | "sbyte"
                | "sealed"
                | "short"
                | "sizeof"
                | "stackalloc"
                | "static"
                | "string"
                | "struct"
                | "switch"
                | "this"
                | "throw"
                | "true"
                | "try"
                | "typeof"
                | "uint"
                | "ulong"
                | "unchecked"
                | "unsafe"
                | "ushort"
                | "using"
                | "virtual"
                | "void"
                | "volatile"
                | "while"
        ),
        DiffSyntaxLanguage::FSharp => matches!(
            ident,
            "let"
                | "mutable"
                | "use"
                | "match"
                | "with"
                | "function"
                | "type"
                | "member"
                | "interface"
                | "inherit"
                | "abstract"
                | "override"
                | "static"
                | "if"
                | "then"
                | "else"
                | "for"
                | "while"
                | "do"
                | "done"
                | "true"
                | "false"
                | "null"
        ),
        DiffSyntaxLanguage::VisualBasic => matches!(
            ident.to_ascii_lowercase().as_str(),
            "dim"
                | "as"
                | "function"
                | "sub"
                | "end"
                | "if"
                | "then"
                | "else"
                | "elseif"
                | "for"
                | "each"
                | "while"
                | "do"
                | "loop"
                | "select"
                | "case"
                | "return"
                | "true"
                | "false"
                | "nothing"
        ),
        DiffSyntaxLanguage::Java => matches!(
            ident,
            "abstract"
                | "assert"
                | "boolean"
                | "break"
                | "byte"
                | "case"
                | "catch"
                | "char"
                | "class"
                | "const"
                | "continue"
                | "default"
                | "do"
                | "double"
                | "else"
                | "enum"
                | "extends"
                | "final"
                | "finally"
                | "float"
                | "for"
                | "goto"
                | "if"
                | "implements"
                | "import"
                | "instanceof"
                | "int"
                | "interface"
                | "long"
                | "native"
                | "new"
                | "null"
                | "package"
                | "private"
                | "protected"
                | "public"
                | "return"
                | "short"
                | "static"
                | "strictfp"
                | "super"
                | "switch"
                | "synchronized"
                | "this"
                | "throw"
                | "throws"
                | "transient"
                | "true"
                | "false"
                | "try"
                | "void"
                | "volatile"
                | "while"
        ),
        DiffSyntaxLanguage::Php => matches!(
            ident.to_ascii_lowercase().as_str(),
            "function"
                | "class"
                | "public"
                | "private"
                | "protected"
                | "static"
                | "final"
                | "abstract"
                | "extends"
                | "implements"
                | "use"
                | "namespace"
                | "return"
                | "if"
                | "else"
                | "elseif"
                | "for"
                | "foreach"
                | "while"
                | "do"
                | "switch"
                | "case"
                | "default"
                | "try"
                | "catch"
                | "finally"
                | "throw"
                | "new"
                | "true"
                | "false"
                | "null"
        ),
        DiffSyntaxLanguage::Ruby => matches!(
            ident,
            "def"
                | "class"
                | "module"
                | "end"
                | "if"
                | "elsif"
                | "else"
                | "unless"
                | "case"
                | "when"
                | "while"
                | "until"
                | "for"
                | "in"
                | "do"
                | "break"
                | "next"
                | "redo"
                | "retry"
                | "return"
                | "yield"
                | "super"
                | "self"
                | "true"
                | "false"
                | "nil"
        ),
        DiffSyntaxLanguage::Bash => matches!(
            ident,
            "if"
                | "then"
                | "else"
                | "elif"
                | "fi"
                | "for"
                | "in"
                | "do"
                | "done"
                | "case"
                | "esac"
                | "while"
                | "function"
                | "return"
                | "break"
                | "continue"
        ),
    }
}

fn render_cached_diff_styled_text(
    base_fg: gpui::Rgba,
    styled: Option<&CachedDiffStyledText>,
) -> AnyElement {
    let Some(styled) = styled else {
        return div().into_any_element();
    };
    if styled.text.is_empty() {
        return div().into_any_element();
    }

    if styled.highlights.is_empty() {
        return div()
            .min_w(px(0.0))
            .overflow_hidden()
            .whitespace_nowrap()
            .text_color(base_fg)
            .child(styled.text.clone())
            .into_any_element();
    }

    div()
        .flex()
        .items_center()
        .min_w(px(0.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_color(base_fg)
        .child(
            gpui::StyledText::new(styled.text.clone())
                .with_highlights(styled.highlights.as_ref().iter().cloned()),
        )
        .into_any_element()
}

fn selectable_cached_diff_text(
    visible_ix: usize,
    region: DiffTextRegion,
    double_click_kind: DiffClickKind,
    base_fg: gpui::Rgba,
    styled: Option<&CachedDiffStyledText>,
    fallback_text: SharedString,
    cx: &mut gpui::Context<GitGpuiView>,
) -> AnyElement {
    let view = cx.entity();
    let (text, highlights) = if let Some(styled) = styled {
        (styled.text.clone(), Arc::clone(&styled.highlights))
    } else {
        (fallback_text, empty_highlights())
    };

    let overlay_text = text.clone();
    let overlay = div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .bottom_0()
        .child(DiffTextSelectionOverlay {
            view: view.clone(),
            visible_ix,
            region,
            text: overlay_text,
        });

    let content = if text.is_empty() {
        div().into_any_element()
    } else if highlights.is_empty() {
        div().min_w(px(0.0)).overflow_hidden().child(text.clone()).into_any_element()
    } else {
        div()
            .min_w(px(0.0))
            .overflow_hidden()
            .child(gpui::StyledText::new(text.clone()).with_highlights(highlights.iter().cloned()))
            .into_any_element()
    };

    div()
        .relative()
        .min_w(px(0.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_color(base_fg)
        .cursor(CursorStyle::IBeam)
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, window, cx| {
            window.focus(&this.diff_panel_focus_handle);
            if e.click_count >= 2 {
                cx.stop_propagation();
                this.double_click_select_diff_text(visible_ix, region, double_click_kind);
                cx.notify();
                return;
            }
            this.begin_diff_text_selection(visible_ix, region, e.position);
            cx.notify();
        }))
        .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _w, cx| {
            this.update_diff_text_selection_from_mouse(e.position);
            cx.notify();
        }))
        .on_mouse_up(MouseButton::Left, cx.listener(|this, _e: &MouseUpEvent, _w, cx| {
            this.end_diff_text_selection();
            cx.notify();
        }))
        .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _e: &MouseUpEvent, _w, cx| {
            this.end_diff_text_selection();
            cx.notify();
        }))
        .on_mouse_down(MouseButton::Right, cx.listener(move |this, _e: &MouseDownEvent, _w, cx| {
            cx.stop_propagation();
            this.copy_diff_text_selection_or_region_line_to_clipboard(visible_ix, region, cx);
        }))
        .child(overlay)
        .child(content)
        .into_any_element()
}

fn empty_highlights() -> Arc<Vec<(Range<usize>, gpui::HighlightStyle)>> {
    type Highlights = Vec<(Range<usize>, gpui::HighlightStyle)>;
    type HighlightsRef = Arc<Highlights>;

    static EMPTY: OnceLock<HighlightsRef> = OnceLock::new();
    Arc::clone(EMPTY.get_or_init(|| Arc::new(Vec::new())))
}

fn build_cached_diff_styled_text(
    theme: AppTheme,
    text: &str,
    word_ranges: &[Range<usize>],
    query: &str,
    language: Option<DiffSyntaxLanguage>,
    syntax_mode: DiffSyntaxMode,
    word_color: Option<gpui::Rgba>,
) -> CachedDiffStyledText {
    if text.is_empty() {
        return CachedDiffStyledText {
            text: "".into(),
            highlights: empty_highlights(),
        };
    }

    let query = query.trim();

    let mut tab_positions: Vec<usize> = Vec::new();
    let expanded_text: SharedString = if text.contains('\t') {
        let mut out = String::with_capacity(text.len());
        for (byte_ix, ch) in text.char_indices() {
            if ch == '\t' {
                tab_positions.push(byte_ix);
                out.push_str("    ");
            } else {
                out.push(ch);
            }
        }
        out.into()
    } else {
        text.to_string().into()
    };

    if word_ranges.is_empty() && query.is_empty() && language.is_none() {
        return CachedDiffStyledText {
            text: expanded_text,
            highlights: empty_highlights(),
        };
    }

    let syntax_tokens = language
        .map(|language| syntax_tokens_for_line(text, language, syntax_mode))
        .unwrap_or_default();

    let query_range = (!query.is_empty())
        .then(|| find_ascii_case_insensitive(text, query))
        .flatten();

    let mut boundaries: Vec<usize> = Vec::with_capacity(
        2 + word_ranges.len() * 2
            + query_range.as_ref().map(|_| 2).unwrap_or(0)
            + syntax_tokens.len() * 2,
    );
    boundaries.push(0);
    boundaries.push(text.len());
    for r in word_ranges {
        boundaries.push(r.start.min(text.len()));
        boundaries.push(r.end.min(text.len()));
    }
    if let Some(r) = &query_range {
        boundaries.push(r.start);
        boundaries.push(r.end);
    }
    for t in &syntax_tokens {
        boundaries.push(t.range.start.min(text.len()));
        boundaries.push(t.range.end.min(text.len()));
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    let map_ix = |ix: usize| -> usize {
        if tab_positions.is_empty() {
            return ix;
        }
        let tabs_before = tab_positions.partition_point(|&p| p < ix);
        ix + tabs_before * 3
    };

    let mut token_ix = 0usize;
    let mut word_ix = 0usize;
    let mut highlights: Vec<(Range<usize>, gpui::HighlightStyle)> =
        Vec::with_capacity(boundaries.len().saturating_sub(1));

    for w in boundaries.windows(2) {
        let (a, b) = (w[0], w[1]);
        if a >= b || a >= text.len() {
            continue;
        }
        let b = b.min(text.len());

        if text.get(a..b).is_none() {
            return CachedDiffStyledText {
                text: expanded_text,
                highlights: empty_highlights(),
            };
        }

        while token_ix < syntax_tokens.len() && syntax_tokens[token_ix].range.end <= a {
            token_ix += 1;
        }
        let syntax = syntax_tokens
            .get(token_ix)
            .filter(|t| t.range.start <= a && t.range.end >= b)
            .map(|t| t.kind)
            .unwrap_or(SyntaxTokenKind::None);

        while word_ix < word_ranges.len() && word_ranges[word_ix].end <= a {
            word_ix += 1;
        }
        let in_word = word_ranges
            .get(word_ix)
            .is_some_and(|r| a < r.end && b > r.start);
        let in_query = query_range
            .as_ref()
            .is_some_and(|r| a < r.end && b > r.start);

        let mut style = gpui::HighlightStyle::default();
        if in_word && let Some(mut c) = word_color {
            c.a = if theme.is_dark { 0.22 } else { 0.16 };
            style.background_color = Some(c.into());
        }

        if in_query {
            style.color = Some(theme.colors.accent.into());
            style.font_weight = Some(FontWeight::BOLD);
        } else {
            let syntax_fg = match syntax {
                SyntaxTokenKind::Comment => Some(theme.colors.text_muted),
                SyntaxTokenKind::String => Some(theme.colors.warning),
                SyntaxTokenKind::Keyword => Some(theme.colors.accent),
                SyntaxTokenKind::Number => Some(theme.colors.success),
                SyntaxTokenKind::Function => Some(theme.colors.accent),
                SyntaxTokenKind::Type => Some(theme.colors.warning),
                SyntaxTokenKind::Property => Some(theme.colors.accent),
                SyntaxTokenKind::Constant => Some(theme.colors.success),
                SyntaxTokenKind::Punctuation => Some(theme.colors.text_muted),
                SyntaxTokenKind::None => None,
            };
            if let Some(fg) = syntax_fg {
                style.color = Some(fg.into());
            }
        }

        if style == gpui::HighlightStyle::default() {
            continue;
        }

        let start = map_ix(a);
        let end = map_ix(b);
        if start >= end || end > expanded_text.len() {
            continue;
        }

        if let Some((prev_range, prev_style)) = highlights.last_mut()
            && *prev_style == style
            && prev_range.end == start
        {
            prev_range.end = end;
            continue;
        }
        highlights.push((start..end, style));
    }

    if highlights.is_empty() {
        return CachedDiffStyledText {
            text: expanded_text,
            highlights: empty_highlights(),
        };
    }

    CachedDiffStyledText {
        text: expanded_text,
        highlights: Arc::new(highlights),
    }
}

fn styled_text_for_diff_segments(
    theme: AppTheme,
    segments: &[CachedDiffTextSegment],
    word_color: Option<gpui::Rgba>,
) -> (SharedString, Vec<(Range<usize>, gpui::HighlightStyle)>) {
    let combined_len: usize = segments.iter().map(|s| s.text.len()).sum();
    let mut combined = String::with_capacity(combined_len);
    let mut highlights: Vec<(Range<usize>, gpui::HighlightStyle)> =
        Vec::with_capacity(segments.len());

    let mut offset = 0usize;
    for seg in segments {
        combined.push_str(seg.text.as_ref());
        let next_offset = offset + seg.text.len();

        let mut style = gpui::HighlightStyle::default();

        if seg.in_word && let Some(mut c) = word_color {
            c.a = if theme.is_dark { 0.22 } else { 0.16 };
            style.background_color = Some(c.into());
        }

        if seg.in_query {
            style.color = Some(theme.colors.accent.into());
            style.font_weight = Some(FontWeight::BOLD);
        } else {
            let syntax_fg = match seg.syntax {
                SyntaxTokenKind::Comment => Some(theme.colors.text_muted),
                SyntaxTokenKind::String => Some(theme.colors.warning),
                SyntaxTokenKind::Keyword => Some(theme.colors.accent),
                SyntaxTokenKind::Number => Some(theme.colors.success),
                SyntaxTokenKind::Function => Some(theme.colors.accent),
                SyntaxTokenKind::Type => Some(theme.colors.warning),
                SyntaxTokenKind::Property => Some(theme.colors.accent),
                SyntaxTokenKind::Constant => Some(theme.colors.success),
                SyntaxTokenKind::Punctuation => Some(theme.colors.text_muted),
                SyntaxTokenKind::None => None,
            };
            if let Some(fg) = syntax_fg {
                style.color = Some(fg.into());
            }
        }

        if style != gpui::HighlightStyle::default() && offset < next_offset {
            highlights.push((offset..next_offset, style));
        }

        offset = next_offset;
    }

    (combined.into(), highlights)
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<Range<usize>> {
    if needle.is_empty() {
        return Some(0..0);
    }

    let haystack_bytes = haystack.as_bytes();
    let needle_bytes = needle.as_bytes();
    if needle_bytes.len() > haystack_bytes.len() {
        return None;
    }

    'outer: for start in 0..=(haystack_bytes.len() - needle_bytes.len()) {
        for (offset, needle_byte) in needle_bytes.iter().copied().enumerate() {
            let haystack_byte = haystack_bytes[start + offset];
            if !haystack_byte.eq_ignore_ascii_case(&needle_byte) {
                continue 'outer;
            }
        }
        return Some(start..(start + needle_bytes.len()));
    }

    None
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
