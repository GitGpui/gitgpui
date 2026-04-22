use super::super::text_truncation::{
    TextTruncationProfile, TruncatedLineLayout, shape_truncated_line_cached,
};
use crate::view::tooltip_host::TooltipHost;
use gpui::prelude::*;
use gpui::{
    App, AvailableSpace, Bounds, Context, Element, ElementId, GlobalElementId, HighlightStyle,
    InspectorElementId, IntoElement, LayoutId, Pixels, SharedString, Stateful, TextAlign,
    WeakEntity, Window, div, point, px, size,
};
use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TruncatedTextTooltipMode {
    #[default]
    None,
    FullTextIfTruncated,
}

pub struct TruncatedText {
    text: SharedString,
    profile: TextTruncationProfile,
    highlights: Arc<[(Range<usize>, HighlightStyle)]>,
    focus_range: Option<Range<usize>>,
    tooltip_mode: TruncatedTextTooltipMode,
    tooltip_host: Option<WeakEntity<TooltipHost>>,
}

impl TruncatedText {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            profile: TextTruncationProfile::End,
            highlights: Arc::from([]),
            focus_range: None,
            tooltip_mode: TruncatedTextTooltipMode::None,
            tooltip_host: None,
        }
    }

    pub fn profile(mut self, profile: TextTruncationProfile) -> Self {
        self.profile = profile;
        self
    }

    pub fn highlights(
        mut self,
        highlights: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>,
    ) -> Self {
        self.highlights = Arc::from(highlights.into_iter().collect::<Vec<_>>());
        self
    }

    pub fn focus_range(mut self, focus_range: Option<Range<usize>>) -> Self {
        self.focus_range = focus_range;
        self
    }

    pub fn tooltip_mode(mut self, mode: TruncatedTextTooltipMode) -> Self {
        self.tooltip_mode = mode;
        self
    }

    pub fn tooltip_host(mut self, tooltip_host: WeakEntity<TooltipHost>) -> Self {
        self.tooltip_host = Some(tooltip_host);
        self
    }

    pub fn render<V: 'static>(self, cx: &Context<V>) -> impl IntoElement {
        let tooltip_text = self.text.clone();
        let tooltip_mode = self.tooltip_mode;
        let tooltip_host = self.tooltip_host.clone();
        let truncated = Rc::new(Cell::new(false));
        let element = TruncatedTextElement {
            text: self.text,
            profile: self.profile,
            highlights: self.highlights,
            focus_range: self.focus_range,
            layout: TruncatedTextLayoutState::default(),
            truncated: Rc::clone(&truncated),
        };

        let mut root: Stateful<_> = div()
            .id(("truncated_text", Rc::as_ptr(&truncated) as usize))
            .min_w(px(0.0))
            .overflow_hidden()
            .whitespace_nowrap()
            .child(element);

        if matches!(tooltip_mode, TruncatedTextTooltipMode::FullTextIfTruncated)
            && let Some(tooltip_host) = tooltip_host
        {
            root = root.on_hover(cx.listener(move |_this, hovering: &bool, _window, cx| {
                if *hovering {
                    if truncated.get() {
                        let _ = tooltip_host.update(cx, |host, cx| {
                            host.set_tooltip_text_if_changed(Some(tooltip_text.clone()), cx);
                        });
                    }
                } else {
                    let _ = tooltip_host.update(cx, |host, cx| {
                        host.clear_tooltip_if_matches(&tooltip_text, cx);
                    });
                }
            }));
        }

        root
    }
}

#[derive(Default, Clone)]
struct TruncatedTextLayoutState(Rc<RefCell<Option<TruncatedTextLayoutInner>>>);

struct TruncatedTextLayoutInner {
    line: Arc<TruncatedLineLayout>,
}

struct TruncatedTextElement {
    text: SharedString,
    profile: TextTruncationProfile,
    highlights: Arc<[(Range<usize>, HighlightStyle)]>,
    focus_range: Option<Range<usize>>,
    layout: TruncatedTextLayoutState,
    truncated: Rc<Cell<bool>>,
}

impl Element for TruncatedTextElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_state = self.layout.clone();
        let text = self.text.clone();
        let profile = self.profile;
        let highlights = Arc::clone(&self.highlights);
        let focus_range = self.focus_range.clone();
        let truncated = Rc::clone(&self.truncated);

        let layout_id = window.request_measured_layout(Default::default(), move |known_dimensions, available_space, window, cx| {
            let max_width = known_dimensions.width.or(match available_space.width {
                AvailableSpace::Definite(width) => Some(width),
                _ => None,
            });
            let line = shape_truncated_line_cached(
                window,
                cx,
                &window.text_style(),
                &text,
                max_width,
                profile,
                highlights.as_ref(),
                focus_range.clone(),
            );
            truncated.set(line.truncated);
            let width = max_width
                .map(|width| line.shaped_line.width.min(width.max(px(0.0))))
                .unwrap_or(line.shaped_line.width);
            let size = size(width, line.line_height);
            layout_state.0.replace(Some(TruncatedTextLayoutInner { line }));
            size
        });

        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let binding = self.layout.0.borrow();
        let Some(inner) = binding.as_ref() else {
            return;
        };

        if inner.line.has_background_highlights {
            let _ = inner.line.shaped_line.paint_background(
                point(bounds.left(), bounds.top()),
                inner.line.line_height,
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }

        let _ = inner.line.shaped_line.paint(
            point(bounds.left(), bounds.top()),
            inner.line.line_height,
            TextAlign::Left,
            None,
            window,
            cx,
        );
    }
}

impl IntoElement for TruncatedTextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
