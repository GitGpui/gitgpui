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

    let query_ranges = (!query.is_empty())
        .then(|| find_all_ascii_case_insensitive(text, query))
        .unwrap_or_default();

    let mut boundaries: Vec<usize> = Vec::with_capacity(
        2 + word_ranges.len() * 2 + query_ranges.len() * 2 + syntax_tokens.len() * 2,
    );
    boundaries.push(0);
    boundaries.push(text.len());
    for r in word_ranges {
        boundaries.push(r.start.min(text.len()));
        boundaries.push(r.end.min(text.len()));
    }
    for r in &query_ranges {
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
        let in_query = query_ranges.iter().any(|r| a < r.end && b > r.start);

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
        assert_eq!(styled.highlights_hash, 0);
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
        assert_eq!(highlights[0].1.font_weight, None);
        assert!(highlights[0].1.background_color.is_some());

        // Hashing highlights is used for caching shaped layouts; it should be stable for identical
        // highlight sequences within a process.
        let styled = build_cached_diff_styled_text(
            theme,
            "abcdef",
            &[],
            "def",
            None,
            DiffSyntaxMode::Auto,
            None,
        );
        assert_eq!(styled.highlights.len(), 1);
        assert_eq!(styled.highlights[0].0, 3..6);
    }

    #[test]
    fn cached_styled_text_highlights_all_query_occurrences() {
        let theme = AppTheme::zed_ayu_dark();
        let styled = build_cached_diff_styled_text(
            theme,
            "abxxab",
            &[],
            "ab",
            None,
            DiffSyntaxMode::Auto,
            None,
        );
        assert_eq!(styled.highlights.len(), 2);
        assert_eq!(styled.highlights[0].0, 0..2);
        assert_eq!(styled.highlights[1].0, 4..6);
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
    fn syntax_colors_are_softened_for_keywords() {
        let theme = AppTheme::zed_one_light();
        let segments = vec![CachedDiffTextSegment {
            text: "fn".into(),
            in_word: false,
            in_query: false,
            syntax: SyntaxTokenKind::Keyword,
        }];

        let (_text, highlights) = styled_text_for_diff_segments(theme, &segments, None);
        assert_eq!(highlights.len(), 1);
        assert_eq!(highlights[0].0, 0..2);
        assert_ne!(highlights[0].1.color, Some(theme.colors.accent.into()));
    }
}

pub(super) fn selectable_cached_diff_text(
    visible_ix: usize,
    region: DiffTextRegion,
    double_click_kind: DiffClickKind,
    base_fg: gpui::Rgba,
    styled: Option<&CachedDiffStyledText>,
    fallback_text: SharedString,
    cx: &mut gpui::Context<MainPaneView>,
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
            cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                if double_click_kind == DiffClickKind::HunkHeader {
                    return;
                }
                cx.stop_propagation();
                this.open_diff_editor_context_menu(visible_ix, region, e.position, window, cx);
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
            highlights_hash: 0,
        };
    }

    let segments = build_diff_text_segments(text, word_ranges, query, language, syntax_mode);
    let (expanded_text, highlights) = styled_text_for_diff_segments(theme, &segments, word_color);

    if highlights.is_empty() {
        return CachedDiffStyledText {
            text: expanded_text,
            highlights: empty_highlights(),
            highlights_hash: 0,
        };
    }

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    for (range, style) in &highlights {
        range.hash(&mut hasher);
        style.hash(&mut hasher);
    }
    let highlights_hash = hasher.finish();

    CachedDiffStyledText {
        text: expanded_text,
        highlights: Arc::new(highlights),
        highlights_hash,
    }
}

fn styled_text_for_diff_segments(
    theme: AppTheme,
    segments: &[CachedDiffTextSegment],
    word_color: Option<gpui::Rgba>,
) -> (SharedString, Vec<(Range<usize>, gpui::HighlightStyle)>) {
    fn mix_colors(a: gpui::Rgba, b: gpui::Rgba, t: f32) -> gpui::Rgba {
        let t = t.clamp(0.0, 1.0);
        gpui::Rgba {
            r: a.r + (b.r - a.r) * t,
            g: a.g + (b.g - a.g) * t,
            b: a.b + (b.b - a.b) * t,
            a: 1.0,
        }
    }

    fn calm_syntax_color(theme: AppTheme, token: gpui::Rgba) -> gpui::Rgba {
        // Pull token colors towards the base foreground for a less-saturated "calm" look.
        let blend_to_text = if theme.is_dark { 0.42 } else { 0.58 };
        mix_colors(token, theme.colors.text, blend_to_text)
    }

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
            style.background_color = Some(
                with_alpha(theme.colors.accent, if theme.is_dark { 0.22 } else { 0.16 }).into(),
            );
        }

        let syntax_fg = match seg.syntax {
            SyntaxTokenKind::Comment => Some(theme.colors.text_muted),
            SyntaxTokenKind::String => Some(calm_syntax_color(theme, theme.colors.warning)),
            SyntaxTokenKind::Keyword => Some(calm_syntax_color(theme, theme.colors.accent)),
            SyntaxTokenKind::Number => Some(calm_syntax_color(theme, theme.colors.success)),
            SyntaxTokenKind::Function => Some(calm_syntax_color(theme, theme.colors.accent)),
            SyntaxTokenKind::Type => Some(calm_syntax_color(theme, theme.colors.warning)),
            SyntaxTokenKind::Property => Some(calm_syntax_color(theme, theme.colors.accent)),
            SyntaxTokenKind::Constant => Some(calm_syntax_color(theme, theme.colors.success)),
            SyntaxTokenKind::Punctuation => Some(theme.colors.text_muted),
            SyntaxTokenKind::None => None,
        };
        if let Some(fg) = syntax_fg {
            style.color = Some(fg.into());
        }

        if style != gpui::HighlightStyle::default() && offset < next_offset {
            highlights.push((offset..next_offset, style));
        }

        offset = next_offset;
    }

    (combined.into(), highlights)
}

fn find_all_ascii_case_insensitive(haystack: &str, needle: &str) -> Vec<Range<usize>> {
    const MAX_MATCHES: usize = 64;

    let needle_bytes = needle.as_bytes();
    if needle_bytes.is_empty() {
        return Vec::new();
    }

    let haystack_bytes = haystack.as_bytes();
    if needle_bytes.len() > haystack_bytes.len() {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut start = 0usize;
    while start + needle_bytes.len() <= haystack_bytes.len() && out.len() < MAX_MATCHES {
        let mut matched = true;
        for (offset, needle_byte) in needle_bytes.iter().copied().enumerate() {
            let haystack_byte = haystack_bytes[start + offset];
            if !haystack_byte.eq_ignore_ascii_case(&needle_byte) {
                matched = false;
                break;
            }
        }

        if matched {
            out.push(start..(start + needle_bytes.len()));
            start = start.saturating_add(needle_bytes.len().max(1));
        } else {
            start = start.saturating_add(1);
        }
    }

    out
}

pub(super) fn diff_line_colors(
    theme: AppTheme,
    kind: gitgpui_core::domain::DiffLineKind,
) -> (gpui::Rgba, gpui::Rgba, gpui::Rgba) {
    use gitgpui_core::domain::DiffLineKind::*;

    match (theme.is_dark, kind) {
        (_, Header) => (
            theme.colors.window_bg,
            theme.colors.text_muted,
            theme.colors.text_muted,
        ),
        (_, Hunk) => (
            theme.colors.window_bg,
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
            theme.colors.window_bg,
            theme.colors.text,
            theme.colors.text_muted,
        ),
    }
}
