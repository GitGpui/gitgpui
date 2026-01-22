use super::*;
use std::sync::{Arc, OnceLock};

mod syntax;

pub(super) use syntax::{DiffSyntaxLanguage, DiffSyntaxMode, diff_syntax_language_for_path};

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
        .map(|language| syntax::syntax_tokens_for_line(text, language, syntax_mode))
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let ranges = vec![1..2];
        let segments = build_diff_text_segments(text, &ranges, "", None, DiffSyntaxMode::Auto);
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
}

pub(super) fn render_cached_diff_styled_text(
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

pub(super) fn selectable_cached_diff_text(
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
        div()
            .min_w(px(0.0))
            .overflow_hidden()
            .child(text.clone())
            .into_any_element()
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
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                window.focus(&this.diff_panel_focus_handle);
                if e.click_count >= 2 {
                    cx.stop_propagation();
                    this.double_click_select_diff_text(visible_ix, region, double_click_kind);
                    cx.notify();
                    return;
                }
                this.begin_diff_text_selection(visible_ix, region, e.position);
                cx.notify();
            }),
        )
        .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _w, cx| {
            this.update_diff_text_selection_from_mouse(e.position);
            cx.notify();
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseUpEvent, _w, cx| {
                this.end_diff_text_selection();
                cx.notify();
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseUpEvent, _w, cx| {
                this.end_diff_text_selection();
                cx.notify();
            }),
        )
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, _e: &MouseDownEvent, _w, cx| {
                cx.stop_propagation();
                this.copy_diff_text_selection_or_region_line_to_clipboard(visible_ix, region, cx);
            }),
        )
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

pub(super) fn build_cached_diff_styled_text(
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

    let segments = build_diff_text_segments(text, word_ranges, query, language, syntax_mode);
    let (expanded_text, highlights) = styled_text_for_diff_segments(theme, &segments, word_color);

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

        if seg.in_word
            && let Some(mut c) = word_color
        {
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

pub(super) fn diff_line_colors(
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
