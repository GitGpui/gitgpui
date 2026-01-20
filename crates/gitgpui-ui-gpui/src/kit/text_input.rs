use crate::theme::AppTheme;
use gpui::prelude::*;
use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, Rgba, ShapedLine,
    SharedString, Style, TextRun, UTF16Selection, Window, actions, div, fill, hsla,
    point, px, relative, size,
};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation as _;

actions!(
    text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
        ShowCharacterPalette,
    ]
);

#[derive(Clone, Copy, Debug)]
struct TextInputStyle {
    is_dark: bool,
    background: Rgba,
    border: Rgba,
    hover_border: Rgba,
    focus_border: Rgba,
    radius: f32,
    cursor: Rgba,
    selection: Rgba,
}

impl TextInputStyle {
    fn from_theme(theme: AppTheme) -> Self {
        let hover_border = with_alpha(theme.colors.border, if theme.is_dark { 0.95 } else { 1.0 });
        Self {
            is_dark: theme.is_dark,
            background: theme.colors.surface_bg_elevated,
            border: theme.colors.border,
            hover_border,
            focus_border: theme.colors.focus_ring,
            radius: theme.radii.row,
            cursor: theme.colors.accent,
            selection: with_alpha(theme.colors.accent, if theme.is_dark { 0.28 } else { 0.18 }),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TextInputOptions {
    pub placeholder: SharedString,
    pub multiline: bool,
    pub read_only: bool,
    pub chromeless: bool,
}

pub struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    multiline: bool,
    read_only: bool,
    chromeless: bool,
    style: TextInputStyle,

    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,

    last_layout: Option<Vec<ShapedLine>>,
    last_line_starts: Option<Vec<usize>>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
}

impl TextInput {
    pub fn new(options: TextInputOptions, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle().tab_index(0).tab_stop(true);
        let _ = window;
        Self {
            focus_handle,
            content: "".into(),
            placeholder: options.placeholder,
            multiline: options.multiline,
            read_only: options.read_only,
            chromeless: options.chromeless,
            style: TextInputStyle::from_theme(AppTheme::zed_ayu_dark()),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_line_starts: None,
            last_bounds: None,
            is_selecting: false,
        }
    }

    pub fn new_inert(options: TextInputOptions, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle().tab_index(0).tab_stop(true);
        Self {
            focus_handle,
            content: "".into(),
            placeholder: options.placeholder,
            multiline: options.multiline,
            read_only: options.read_only,
            chromeless: options.chromeless,
            style: TextInputStyle::from_theme(AppTheme::zed_ayu_dark()),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_line_starts: None,
            last_bounds: None,
            is_selecting: false,
        }
    }

    pub fn text(&self) -> &str {
        self.content.as_ref()
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    pub fn set_theme(&mut self, theme: AppTheme, cx: &mut Context<Self>) {
        self.style = TextInputStyle::from_theme(theme);
        cx.notify();
    }

    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = text.into();
        self.selected_range = self.content.len()..self.content.len();
        cx.notify();
    }

    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        self.read_only = read_only;
        cx.notify();
    }

    fn sanitize_insert_text(&self, text: &str) -> Option<String> {
        if self.multiline {
            return Some(text.to_string());
        }

        if text == "\n" || text == "\r" || text == "\r\n" {
            return None;
        }

        Some(
            text.replace("\r\n", "\n")
                .replace('\r', "\n")
                .replace('\n', " "),
        )
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text, window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            if !self.read_only {
                self.replace_text_in_range(None, "", window, cx)
            }
        }
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn set_cursor(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        cx.notify();
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify();
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        window.focus(&self.focus_handle);
        if self.read_only && event.button == MouseButton::Left && event.click_count >= 2 {
            self.move_to(0, cx);
            self.select_to(self.content.len(), cx);
            self.is_selecting = false;
            return;
        }

        self.is_selecting = true;

        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }

        let (Some(bounds), Some(lines), Some(starts)) = (
            self.last_bounds.as_ref(),
            self.last_layout.as_ref(),
            self.last_line_starts.as_ref(),
        )
        else {
            return 0;
        };

        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }

        let total_height = bounds.bottom() - bounds.top();
        let line_height = if lines.is_empty() {
            px(16.0)
        } else {
            total_height * (1.0 / lines.len() as f32)
        };

        let mut line_ix = ((position.y - bounds.top()) / line_height).floor() as isize;
        line_ix = line_ix.clamp(0, lines.len().saturating_sub(1) as isize);
        let line_ix = line_ix as usize;

        let local_ix = lines[line_ix].closest_index_for_x(position.x - bounds.left());
        let doc_ix = starts.get(line_ix).copied().unwrap_or(0) + local_ix;
        doc_ix.min(self.content.len())
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;

        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }

        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;

        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let Some(new_text) = self.sanitize_insert_text(new_text) else {
            return;
        };

        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + &new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let Some(new_text) = self.sanitize_insert_text(new_text) else {
            return;
        };

        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + &new_text + &self.content[range.end..])
                .into();
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layouts = self.last_layout.as_ref()?;
        let starts = self.last_line_starts.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let offset = range.start.min(self.content.len());
        let (line_ix, local_ix) = line_for_offset(starts, layouts, offset);
        let line = layouts.get(line_ix)?;
        let x = line.x_for_index(local_ix);
        let top = bounds.top() + window.line_height() * line_ix as f32;
        Some(Bounds::from_corners(
            point(bounds.left() + x, top),
            point(bounds.left() + x + px(2.0), top + px(16.0)),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let local = self.last_bounds?.localize(&point)?;
        let layouts = self.last_layout.as_ref()?;
        let starts = self.last_line_starts.as_ref()?;
        let line_height = window.line_height();
        let mut line_ix = (local.y / line_height).floor() as isize;
        line_ix = line_ix.clamp(0, layouts.len().saturating_sub(1) as isize);
        let line_ix = line_ix as usize;
        let line = layouts.get(line_ix)?;
        let local_x = local.x;
        let idx = line.index_for_x(local_x).unwrap_or(line.len());
        let doc_offset = starts.get(line_ix).copied().unwrap_or(0) + idx;
        Some(self.offset_to_utf16(doc_offset))
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    lines: Option<Vec<ShapedLine>>,
    cursor: Option<PaintQuad>,
    selections: Vec<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let input = self.input.read(cx);
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        if input.multiline {
            let line_count = input.content.as_ref().split('\n').count().max(1) as f32;
            style.size.height = (window.line_height() * line_count).into();
        } else {
            style.size.height = window.line_height().into();
        }
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        let cursor = input.cursor_offset();
        let style_colors = input.style;
        let style = window.text_style();

        let placeholder_color = if style_colors.is_dark {
            hsla(0., 0., 1., 0.35)
        } else {
            hsla(0., 0., 0., 0.2)
        };

        let (display_text, text_color) = if content.is_empty() {
            (input.placeholder.clone(), placeholder_color)
        } else {
            (content, style.color)
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();

        let (line_starts, lines_text): (Vec<usize>, Vec<SharedString>) =
            split_lines_with_starts(&display_text);

        let mut lines = Vec::with_capacity(lines_text.len());
        for line_text in &lines_text {
            let run = TextRun {
                len: line_text.len(),
                font: style.font(),
                color: text_color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let shaped = window
                .text_system()
                .shape_line(line_text.clone(), font_size, &[run], None);
            lines.push(shaped);
        }

        let mut selections = Vec::new();
        let cursor_quad = if selected_range.is_empty() {
            let (line_ix, local_ix) = line_for_offset(&line_starts, &lines, cursor);
            let x = lines[line_ix].x_for_index(local_ix);
            let top = bounds.top() + line_height * line_ix as f32;
            Some(fill(
                Bounds::new(point(bounds.left() + x, top), size(px(2.0), line_height)),
                style_colors.cursor,
            ))
        } else {
            for ix in 0..lines.len() {
                let start = line_starts[ix];
                let next_start = line_starts.get(ix + 1).copied().unwrap_or(display_text.len());
                let line_len = lines[ix].len();
                let line_end = start + line_len;

                let seg_start = selected_range.start.max(start);
                let seg_end = selected_range.end.min(next_start);
                if seg_start >= seg_end {
                    continue;
                }

                let local_start = seg_start.min(line_end) - start;
                let local_end = seg_end.min(line_end) - start;

                let x0 = lines[ix].x_for_index(local_start);
                let x1 = lines[ix].x_for_index(local_end);
                let top = bounds.top() + line_height * ix as f32;
                selections.push(fill(
                    Bounds::from_corners(
                        point(bounds.left() + x0, top),
                        point(bounds.left() + x1, top + line_height),
                    ),
                    style_colors.selection,
                ));
            }
            None
        };

        PrepaintState {
            lines: Some(lines),
            cursor: cursor_quad,
            selections,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );

        if self.input.read(cx).is_selecting {
            let input = self.input.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, _phase, _window, cx| {
                let _ = input.update(cx, |input, cx| {
                    if input.is_selecting {
                        input.select_to(input.index_for_mouse_position(event.position), cx);
                    }
                });
            });

            let input = self.input.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, _phase, _window, cx| {
                if event.button != MouseButton::Left {
                    return;
                }
                let _ = input.update(cx, |input, _cx| {
                    input.is_selecting = false;
                });
            });
        }

        for selection in prepaint.selections.drain(..) {
            window.paint_quad(selection);
        }
        let lines = prepaint.lines.take().unwrap();
        for (ix, line) in lines.iter().enumerate() {
            line.paint(
                point(bounds.origin.x, bounds.origin.y + window.line_height() * ix as f32),
                window.line_height(),
                window,
                cx,
            )
            .unwrap();
        }

        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(lines);
            input.last_line_starts =
                Some(split_lines_with_starts(&input.content).0);
            input.last_bounds = Some(bounds);
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let style = self.style;
        let focus = self.focus_handle.clone();
        let chromeless = self.chromeless;
        let padding = if chromeless { px(0.0) } else { px(8.0) };

        let mut outer = div()
            .flex()
            .track_focus(&focus)
            .key_context("TextInput")
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::show_character_palette))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_down(MouseButton::Right, cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                cx.stop_propagation();
                if !this.selected_range.is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        this.content[this.selected_range.clone()].to_string(),
                    ));
                }
            }))
            .line_height(window.line_height())
            .text_size(px(13.0))
            .when(self.multiline, |d| d.items_start())
            .child(div().p(padding).child(TextElement { input: cx.entity() }));

        if !chromeless {
            outer = outer
                .bg(style.background)
                .border_1()
                .border_color(style.border)
                .hover(move |s| s.border_color(style.hover_border))
                .focus(move |s| s.border_color(style.focus_border))
                .rounded(px(style.radius));
        }

        outer
    }
}

fn with_alpha(mut color: Rgba, alpha: f32) -> Rgba {
    color.a = alpha;
    color
}

fn split_lines_with_starts(text: &SharedString) -> (Vec<usize>, Vec<SharedString>) {
    let s = text.as_ref();
    let mut starts = Vec::new();
    let mut lines = Vec::new();
    starts.push(0);
    let mut start = 0usize;
    for (ix, b) in s.bytes().enumerate() {
        if b == b'\n' {
            lines.push(s[start..ix].to_string().into());
            start = ix + 1;
            starts.push(start);
        }
    }
    lines.push(s[start..].to_string().into());
    (starts, lines)
}

fn line_for_offset(starts: &[usize], lines: &[ShapedLine], offset: usize) -> (usize, usize) {
    let mut ix = starts.partition_point(|&s| s <= offset);
    if ix == 0 {
        ix = 1;
    }
    let line_ix = (ix - 1).min(lines.len().saturating_sub(1));
    let start = starts.get(line_ix).copied().unwrap_or(0);
    let local = offset.saturating_sub(start).min(lines[line_ix].len());
    (line_ix, local)
}
