//! Picker prompt port.
//!
//! Adapted from Zed's `git_ui::picker_prompt` and simplified to avoid pulling in
//! Zed's `picker`, `ui`, and `workspace` crates.

use crate::{theme::AppTheme, zed_port::TextInput};
use gpui::prelude::*;
use gpui::{ClickEvent, CursorStyle, Div, Entity, FontWeight, SharedString, Window, div, px};
use std::ops::Range;
use std::sync::Arc;

pub struct PickerPrompt {
    query_input: Entity<TextInput>,
    items: Vec<SharedString>,
    empty_text: SharedString,
    max_height: gpui::Pixels,
}

impl PickerPrompt {
    pub fn new(query_input: Entity<TextInput>) -> Self {
        Self {
            query_input,
            items: Vec::new(),
            empty_text: "No matches".into(),
            max_height: px(360.0),
        }
    }

    pub fn items(mut self, items: impl IntoIterator<Item = SharedString>) -> Self {
        self.items = items.into_iter().collect();
        self
    }

    pub fn empty_text(mut self, text: impl Into<SharedString>) -> Self {
        self.empty_text = text.into();
        self
    }

    pub fn max_height(mut self, height: gpui::Pixels) -> Self {
        self.max_height = height;
        self
    }

    pub fn render<V: 'static>(
        self,
        theme: AppTheme,
        cx: &gpui::Context<V>,
        on_select: impl Fn(&mut V, usize, &ClickEvent, &mut Window, &mut gpui::Context<V>) + 'static,
    ) -> Div {
        let on_select: Arc<
            dyn Fn(&mut V, usize, &ClickEvent, &mut Window, &mut gpui::Context<V>) + 'static,
        > = Arc::new(on_select);

        let query = self
            .query_input
            .read_with(cx, |input, _| input.text().trim().to_string());
        let matches = match_items(&self.items, &query);

        let body = div()
            .flex()
            .flex_col()
            .child(div().p_2().child(self.query_input.clone()))
            .child(div().border_t_1().border_color(theme.colors.border));

        let mut list = div()
            .id("picker_prompt_list")
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .max_h(self.max_height);

        if matches.is_empty() {
            list = list.child(
                div()
                    .px_3()
                    .py_2()
                    .text_sm()
                    .text_color(theme.colors.text_muted)
                    .child(self.empty_text),
            );
        } else {
            for m in matches {
                let label = highlighted_label(theme, &self.items[m.index], &query, m.range);
                let on_select = Arc::clone(&on_select);
                let original_index = m.index;
                list = list.child(
                    div()
                        .id(("picker_prompt_item", original_index))
                        .px_3()
                        .py_2()
                        .rounded(px(theme.radii.row))
                        .hover(move |s| s.bg(theme.colors.hover))
                        .active(move |s| s.bg(theme.colors.active))
                        .cursor(CursorStyle::PointingHand)
                        .child(label)
                        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                            (on_select)(this, original_index, event, window, cx);
                        })),
                );
            }
        }

        body.child(list)
    }
}

#[derive(Clone, Debug)]
struct Match {
    index: usize,
    range: Option<Range<usize>>,
    sort_key: (usize, usize, String),
}

fn match_items(items: &[SharedString], query: &str) -> Vec<Match> {
    if query.is_empty() {
        return items
            .iter()
            .enumerate()
            .map(|(index, label)| Match {
                index,
                range: None,
                sort_key: (0, label.len(), label.to_string()),
            })
            .collect();
    }

    let mut out = Vec::new();
    for (index, label) in items.iter().enumerate() {
        let Some(range) = find_ascii_case_insensitive(label, query) else {
            continue;
        };
        out.push(Match {
            index,
            range: Some(range.clone()),
            sort_key: (range.start, label.len(), label.to_string()),
        });
    }

    out.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
    out
}

fn highlighted_label(
    theme: AppTheme,
    label: &str,
    query: &str,
    range: Option<Range<usize>>,
) -> Div {
    let Some(range) = range.filter(|_| !query.is_empty()) else {
        return div().min_w(px(0.0)).text_sm().child(label.to_string());
    };

    let prefix = label.get(..range.start).unwrap_or("");
    let hit = label.get(range.clone()).unwrap_or("");
    let suffix = label.get(range.end..).unwrap_or("");

    div()
        .flex()
        .min_w(px(0.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_sm()
        .child(prefix.to_string())
        .child(
            div()
                .font_weight(FontWeight::BOLD)
                .text_color(theme.colors.accent)
                .child(hit.to_string()),
        )
        .child(suffix.to_string())
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
            if haystack_byte.to_ascii_lowercase() != needle_byte.to_ascii_lowercase() {
                continue 'outer;
            }
        }
        return Some(start..(start + needle_bytes.len()));
    }

    None
}
