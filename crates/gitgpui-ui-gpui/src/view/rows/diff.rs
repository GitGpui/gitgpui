use super::diff_text::*;
use super::*;

impl GitGpuiView {
    pub(in super::super) fn render_diff_rows(
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
                            DiffSyntaxMode::Auto,
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
                        DiffSyntaxMode::Auto,
                        word_color,
                    );
                    this.diff_text_segments_cache_set(src_ix, computed);
                }

                let styled: Option<&CachedDiffStyledText> =
                    matches!(click_kind, DiffClickKind::Line)
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

    pub(in super::super) fn render_diff_split_left_rows(
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
                                DiffSyntaxMode::Auto,
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
                            let word_color =
                                this.diff_cache
                                    .get(src_ix)
                                    .and_then(|line| match line.kind {
                                        gitgpui_core::domain::DiffLineKind::Add => {
                                            Some(theme.colors.success)
                                        }
                                        gitgpui_core::domain::DiffLineKind::Remove => {
                                            Some(theme.colors.danger)
                                        }
                                        _ => None,
                                    });

                            let computed = build_cached_diff_styled_text(
                                theme,
                                text,
                                word_ranges,
                                query.as_str(),
                                language,
                                DiffSyntaxMode::Auto,
                                word_color,
                            );
                            this.diff_text_segments_cache_set(src_ix, computed);
                        }

                        let Some(PatchSplitRow::Aligned {
                            row, old_src_ix, ..
                        }) = this.diff_split_cache.get(row_ix)
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

                        let styled =
                            old_src_ix.and_then(|src_ix| this.diff_text_segments_cache_get(src_ix));

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

    pub(in super::super) fn render_diff_split_right_rows(
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
                                DiffSyntaxMode::Auto,
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
                            let word_color =
                                this.diff_cache
                                    .get(src_ix)
                                    .and_then(|line| match line.kind {
                                        gitgpui_core::domain::DiffLineKind::Add => {
                                            Some(theme.colors.success)
                                        }
                                        gitgpui_core::domain::DiffLineKind::Remove => {
                                            Some(theme.colors.danger)
                                        }
                                        _ => None,
                                    });

                            let computed = build_cached_diff_styled_text(
                                theme,
                                text,
                                word_ranges,
                                query.as_str(),
                                language,
                                DiffSyntaxMode::Auto,
                                word_color,
                            );
                            this.diff_text_segments_cache_set(src_ix, computed);
                        }

                        let Some(PatchSplitRow::Aligned {
                            row, new_src_ix, ..
                        }) = this.diff_split_cache.get(row_ix)
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
                .child(div().w(px(1.0)).h_full().bg(theme.colors.border))
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
