use super::diff_canvas;
use super::diff_text::*;
use super::*;

impl MainPaneView {
    pub(in super::super) fn render_conflict_compare_diff_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        match this.diff_view {
            DiffViewMode::Split => range
                .map(|row_ix| this.render_conflict_compare_split_row(row_ix, cx))
                .collect(),
            DiffViewMode::Inline => range
                .map(|ix| this.render_conflict_compare_inline_row(ix, cx))
                .collect(),
        }
    }

    pub(in super::super) fn render_conflict_resolver_diff_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        match this.conflict_resolver.diff_mode {
            ConflictDiffMode::Split => range
                .map(|row_ix| this.render_conflict_resolver_split_row(row_ix, cx))
                .collect(),
            ConflictDiffMode::Inline => range
                .map(|ix| this.render_conflict_resolver_inline_row(ix, cx))
                .collect(),
        }
    }

    pub(in super::super) fn render_conflict_resolved_preview_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let theme = this.theme;
        let Some(path) = this.conflict_resolver.path.as_ref() else {
            return Vec::new();
        };
        if this.conflict_resolved_preview_lines.is_empty() {
            return Vec::new();
        }

        let syntax_mode =
            if this.conflict_resolved_preview_lines.len() <= MAX_LINES_FOR_SYNTAX_HIGHLIGHTING {
                DiffSyntaxMode::Auto
            } else {
                DiffSyntaxMode::HeuristicOnly
            };
        let language = diff_syntax_language_for_path(path.to_string_lossy().as_ref());

        range
            .map(|ix| {
                let line = this
                    .conflict_resolved_preview_lines
                    .get(ix)
                    .map(String::as_str)
                    .unwrap_or("");

                let styled = this
                    .conflict_resolved_preview_segments_cache
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
                    None,
                    line_no.into(),
                    styled,
                )
            })
            .collect()
    }

    fn render_conflict_compare_split_row(
        &mut self,
        row_ix: usize,
        _cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let Some(row) = self.conflict_resolver.diff_rows.get(row_ix) else {
            return div()
                .id(("conflict_compare_split_oob", row_ix))
                .h(px(20.0))
                .px_2()
                .font_family("monospace")
                .text_xs()
                .text_color(theme.colors.text_muted)
                .child("")
                .into_any_element();
        };

        let left_text: SharedString = row.old.clone().unwrap_or_default().into();
        let right_text: SharedString = row.new.clone().unwrap_or_default().into();

        let query = if self.diff_search_active {
            self.diff_search_query.clone()
        } else {
            SharedString::default()
        };
        let query = query.as_ref().trim();
        let should_style = !query.is_empty();
        if should_style {
            if let Some(text) = row.old.as_deref() {
                self.conflict_diff_segments_cache_split
                    .entry((row_ix, ConflictPickSide::Ours))
                    .or_insert_with(|| {
                        build_cached_diff_styled_text(
                            theme,
                            text,
                            &[],
                            query,
                            None,
                            DiffSyntaxMode::HeuristicOnly,
                            None,
                        )
                    });
            }
            if let Some(text) = row.new.as_deref() {
                self.conflict_diff_segments_cache_split
                    .entry((row_ix, ConflictPickSide::Theirs))
                    .or_insert_with(|| {
                        build_cached_diff_styled_text(
                            theme,
                            text,
                            &[],
                            query,
                            None,
                            DiffSyntaxMode::HeuristicOnly,
                            None,
                        )
                    });
            }
        }
        let left_styled = should_style
            .then(|| {
                self.conflict_diff_segments_cache_split
                    .get(&(row_ix, ConflictPickSide::Ours))
            })
            .flatten();
        let right_styled = should_style
            .then(|| {
                self.conflict_diff_segments_cache_split
                    .get(&(row_ix, ConflictPickSide::Theirs))
            })
            .flatten();

        let left_bg = split_cell_bg(theme, row.kind, ConflictPickSide::Ours, false);
        let right_bg = split_cell_bg(theme, row.kind, ConflictPickSide::Theirs, false);

        let left = div()
            .id(("conflict_compare_split_ours", row_ix))
            .h(px(20.0))
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .font_family("monospace")
            .text_xs()
            .bg(left_bg)
            .text_color(if row.old.is_some() {
                theme.colors.text
            } else {
                theme.colors.text_muted
            })
            .whitespace_nowrap()
            .child(
                div()
                    .w(px(38.0))
                    .text_color(theme.colors.text_muted)
                    .child(line_number_string(row.old_line)),
            )
            .child(conflict_diff_text_cell(left_text.clone(), left_styled));

        let right = div()
            .id(("conflict_compare_split_theirs", row_ix))
            .h(px(20.0))
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .font_family("monospace")
            .text_xs()
            .bg(right_bg)
            .text_color(if row.new.is_some() {
                theme.colors.text
            } else {
                theme.colors.text_muted
            })
            .whitespace_nowrap()
            .child(
                div()
                    .w(px(38.0))
                    .text_color(theme.colors.text_muted)
                    .child(line_number_string(row.new_line)),
            )
            .child(conflict_diff_text_cell(right_text.clone(), right_styled));

        div()
            .id(("conflict_compare_split_row", row_ix))
            .flex()
            .child(left)
            .child(div().w(px(1.0)).h_full().bg(theme.colors.border))
            .child(right)
            .into_any_element()
    }

    fn render_conflict_compare_inline_row(
        &mut self,
        ix: usize,
        _cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let Some(row) = self.conflict_resolver.inline_rows.get(ix) else {
            return div()
                .id(("conflict_compare_inline_oob", ix))
                .h(px(20.0))
                .px_2()
                .font_family("monospace")
                .text_xs()
                .text_color(theme.colors.text_muted)
                .child("")
                .into_any_element();
        };

        let query = if self.diff_search_active {
            self.diff_search_query.clone()
        } else {
            SharedString::default()
        };
        let query = query.as_ref().trim();
        let should_style = !query.is_empty();
        if should_style && !row.content.is_empty() {
            self.conflict_diff_segments_cache_inline
                .entry(ix)
                .or_insert_with(|| {
                    build_cached_diff_styled_text(
                        theme,
                        row.content.as_str(),
                        &[],
                        query,
                        None,
                        DiffSyntaxMode::HeuristicOnly,
                        None,
                    )
                });
        }
        let styled = should_style
            .then(|| self.conflict_diff_segments_cache_inline.get(&ix))
            .flatten();

        let bg = inline_row_bg(theme, row.kind, row.side, false);
        let prefix = match row.kind {
            gitgpui_core::domain::DiffLineKind::Add => "+",
            gitgpui_core::domain::DiffLineKind::Remove => "-",
            gitgpui_core::domain::DiffLineKind::Context => " ",
            gitgpui_core::domain::DiffLineKind::Header => " ",
            gitgpui_core::domain::DiffLineKind::Hunk => " ",
        };

        div()
            .id(("conflict_compare_inline", ix))
            .h(px(20.0))
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .font_family("monospace")
            .text_xs()
            .bg(bg)
            .text_color(theme.colors.text)
            .whitespace_nowrap()
            .child(
                div()
                    .w(px(38.0))
                    .text_color(theme.colors.text_muted)
                    .child(line_number_string(row.old_line)),
            )
            .child(
                div()
                    .w(px(38.0))
                    .text_color(theme.colors.text_muted)
                    .child(line_number_string(row.new_line)),
            )
            .child(
                div()
                    .w(px(12.0))
                    .text_color(theme.colors.text_muted)
                    .child(prefix),
            )
            .child(conflict_diff_text_cell(row.content.clone().into(), styled))
            .into_any_element()
    }

    fn render_conflict_resolver_split_row(
        &mut self,
        row_ix: usize,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let Some(row) = self.conflict_resolver.diff_rows.get(row_ix) else {
            return div()
                .id(("conflict_diff_split_oob", row_ix))
                .h(px(20.0))
                .px_2()
                .font_family("monospace")
                .text_xs()
                .text_color(theme.colors.text_muted)
                .child("")
                .into_any_element();
        };

        let left_text: SharedString = row.old.clone().unwrap_or_default().into();
        let right_text: SharedString = row.new.clone().unwrap_or_default().into();

        let query = if self.diff_search_active {
            self.diff_search_query.clone()
        } else {
            SharedString::default()
        };
        let query = query.as_ref().trim();
        let should_style = !query.is_empty();
        if should_style {
            if let Some(text) = row.old.as_deref() {
                self.conflict_diff_segments_cache_split
                    .entry((row_ix, ConflictPickSide::Ours))
                    .or_insert_with(|| {
                        build_cached_diff_styled_text(
                            theme,
                            text,
                            &[],
                            query,
                            None,
                            DiffSyntaxMode::HeuristicOnly,
                            None,
                        )
                    });
            }
            if let Some(text) = row.new.as_deref() {
                self.conflict_diff_segments_cache_split
                    .entry((row_ix, ConflictPickSide::Theirs))
                    .or_insert_with(|| {
                        build_cached_diff_styled_text(
                            theme,
                            text,
                            &[],
                            query,
                            None,
                            DiffSyntaxMode::HeuristicOnly,
                            None,
                        )
                    });
            }
        }
        let left_styled = should_style
            .then(|| {
                self.conflict_diff_segments_cache_split
                    .get(&(row_ix, ConflictPickSide::Ours))
            })
            .flatten();
        let right_styled = should_style
            .then(|| {
                self.conflict_diff_segments_cache_split
                    .get(&(row_ix, ConflictPickSide::Theirs))
            })
            .flatten();

        let left_selected = self
            .conflict_resolver
            .split_selected
            .contains(&(row_ix, ConflictPickSide::Ours));
        let right_selected = self
            .conflict_resolver
            .split_selected
            .contains(&(row_ix, ConflictPickSide::Theirs));

        let left_bg = split_cell_bg(theme, row.kind, ConflictPickSide::Ours, left_selected);
        let right_bg = split_cell_bg(theme, row.kind, ConflictPickSide::Theirs, right_selected);

        let left_click = cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.conflict_resolver_toggle_split_selected(row_ix, ConflictPickSide::Ours, cx);
        });
        let right_click = cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.conflict_resolver_toggle_split_selected(row_ix, ConflictPickSide::Theirs, cx);
        });

        let mut left = div()
            .id(("conflict_diff_split_ours", row_ix))
            .h(px(20.0))
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .font_family("monospace")
            .text_xs()
            .bg(left_bg)
            .text_color(theme.colors.text)
            .whitespace_nowrap()
            .child(
                div()
                    .w(px(38.0))
                    .text_color(theme.colors.text_muted)
                    .child(line_number_string(row.old_line)),
            )
            .child(conflict_diff_text_cell(left_text.clone(), left_styled));
        if row.old.is_some() {
            left = left
                .cursor(CursorStyle::PointingHand)
                .hover(move |s| s.bg(with_alpha(theme.colors.hover, 0.7)))
                .active(move |s| s.bg(with_alpha(theme.colors.active, 0.7)))
                .on_click(left_click);
        } else {
            left = left.text_color(theme.colors.text_muted);
        }

        let mut right = div()
            .id(("conflict_diff_split_theirs", row_ix))
            .h(px(20.0))
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .font_family("monospace")
            .text_xs()
            .bg(right_bg)
            .text_color(theme.colors.text)
            .whitespace_nowrap()
            .child(
                div()
                    .w(px(38.0))
                    .text_color(theme.colors.text_muted)
                    .child(line_number_string(row.new_line)),
            )
            .child(conflict_diff_text_cell(right_text.clone(), right_styled));
        if row.new.is_some() {
            right = right
                .cursor(CursorStyle::PointingHand)
                .hover(move |s| s.bg(with_alpha(theme.colors.hover, 0.7)))
                .active(move |s| s.bg(with_alpha(theme.colors.active, 0.7)))
                .on_click(right_click);
        } else {
            right = right.text_color(theme.colors.text_muted);
        }

        div()
            .id(("conflict_diff_split_row", row_ix))
            .flex()
            .child(left)
            .child(div().w(px(1.0)).h_full().bg(theme.colors.border))
            .child(right)
            .into_any_element()
    }

    fn render_conflict_resolver_inline_row(
        &mut self,
        ix: usize,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let Some(row) = self.conflict_resolver.inline_rows.get(ix) else {
            return div()
                .id(("conflict_diff_inline_oob", ix))
                .h(px(20.0))
                .px_2()
                .font_family("monospace")
                .text_xs()
                .text_color(theme.colors.text_muted)
                .child("")
                .into_any_element();
        };

        let query = if self.diff_search_active {
            self.diff_search_query.clone()
        } else {
            SharedString::default()
        };
        let query = query.as_ref().trim();
        let should_style = !query.is_empty();
        if should_style && !row.content.is_empty() {
            self.conflict_diff_segments_cache_inline
                .entry(ix)
                .or_insert_with(|| {
                    build_cached_diff_styled_text(
                        theme,
                        row.content.as_str(),
                        &[],
                        query,
                        None,
                        DiffSyntaxMode::HeuristicOnly,
                        None,
                    )
                });
        }
        let styled = should_style
            .then(|| self.conflict_diff_segments_cache_inline.get(&ix))
            .flatten();

        let selected = self.conflict_resolver.inline_selected.contains(&ix);
        let bg = inline_row_bg(theme, row.kind, row.side, selected);
        let prefix = match row.kind {
            gitgpui_core::domain::DiffLineKind::Add => "+",
            gitgpui_core::domain::DiffLineKind::Remove => "-",
            gitgpui_core::domain::DiffLineKind::Context => " ",
            gitgpui_core::domain::DiffLineKind::Header => " ",
            gitgpui_core::domain::DiffLineKind::Hunk => " ",
        };

        let mut base = div()
            .id(("conflict_diff_inline", ix))
            .h(px(20.0))
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .font_family("monospace")
            .text_xs()
            .bg(bg)
            .text_color(theme.colors.text)
            .whitespace_nowrap()
            .child(
                div()
                    .w(px(38.0))
                    .text_color(theme.colors.text_muted)
                    .child(line_number_string(row.old_line)),
            )
            .child(
                div()
                    .w(px(38.0))
                    .text_color(theme.colors.text_muted)
                    .child(line_number_string(row.new_line)),
            )
            .child(
                div()
                    .w(px(12.0))
                    .text_color(theme.colors.text_muted)
                    .child(prefix),
            )
            .child(conflict_diff_text_cell(row.content.clone().into(), styled));

        if !row.content.is_empty() {
            base = base
                .cursor(CursorStyle::PointingHand)
                .hover(move |s| s.bg(with_alpha(theme.colors.hover, 0.7)))
                .active(move |s| s.bg(with_alpha(theme.colors.active, 0.7)))
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.conflict_resolver_toggle_inline_selected(ix, cx);
                }));
        }

        base.into_any_element()
    }
}

fn conflict_diff_text_cell(text: SharedString, styled: Option<&CachedDiffStyledText>) -> AnyElement {
    let Some(styled) = styled else {
        return div()
            .flex_1()
            .min_w(px(0.0))
            .overflow_hidden()
            .child(text)
            .into_any_element();
    };

    if styled.highlights.is_empty() {
        return div()
            .flex_1()
            .min_w(px(0.0))
            .overflow_hidden()
            .child(styled.text.clone())
            .into_any_element();
    }

    div()
        .flex_1()
        .min_w(px(0.0))
        .overflow_hidden()
        .child(
            gpui::StyledText::new(styled.text.clone())
                .with_highlights(styled.highlights.iter().cloned()),
        )
        .into_any_element()
}

fn split_cell_bg(
    theme: AppTheme,
    kind: gitgpui_core::file_diff::FileDiffRowKind,
    side: ConflictPickSide,
    selected: bool,
) -> gpui::Rgba {
    let base = match (kind, side) {
        (gitgpui_core::file_diff::FileDiffRowKind::Add, ConflictPickSide::Theirs)
        | (gitgpui_core::file_diff::FileDiffRowKind::Modify, ConflictPickSide::Theirs) => {
            with_alpha(
                theme.colors.success,
                if theme.is_dark { 0.10 } else { 0.08 },
            )
        }
        (gitgpui_core::file_diff::FileDiffRowKind::Remove, ConflictPickSide::Ours)
        | (gitgpui_core::file_diff::FileDiffRowKind::Modify, ConflictPickSide::Ours) => {
            with_alpha(theme.colors.danger, if theme.is_dark { 0.10 } else { 0.08 })
        }
        _ => with_alpha(theme.colors.surface_bg_elevated, 0.0),
    };
    if selected {
        with_alpha(theme.colors.accent, if theme.is_dark { 0.14 } else { 0.10 })
    } else {
        base
    }
}

fn inline_row_bg(
    theme: AppTheme,
    kind: gitgpui_core::domain::DiffLineKind,
    side: ConflictPickSide,
    selected: bool,
) -> gpui::Rgba {
    let base = match (kind, side) {
        (gitgpui_core::domain::DiffLineKind::Add, _) => with_alpha(
            theme.colors.success,
            if theme.is_dark { 0.10 } else { 0.08 },
        ),
        (gitgpui_core::domain::DiffLineKind::Remove, _) => {
            with_alpha(theme.colors.danger, if theme.is_dark { 0.10 } else { 0.08 })
        }
        _ => with_alpha(theme.colors.surface_bg_elevated, 0.0),
    };
    if selected {
        with_alpha(theme.colors.accent, if theme.is_dark { 0.14 } else { 0.10 })
    } else {
        base
    }
}
