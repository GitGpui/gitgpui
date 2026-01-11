use crate::theme::AppTheme;
use gpui::prelude::*;
use gpui::{ElementId, Pixels, ScrollHandle, canvas, div, fill, point, px, size};

#[derive(Clone)]
pub struct Scrollbar {
    id: ElementId,
    handle: ScrollHandle,
    #[cfg(test)]
    debug_selector: Option<&'static str>,
}

impl Scrollbar {
    pub fn new(id: impl Into<ElementId>, handle: ScrollHandle) -> Self {
        Self {
            id: id.into(),
            handle,
            #[cfg(test)]
            debug_selector: None,
        }
    }

    #[cfg(test)]
    pub fn debug_selector(mut self, selector: &'static str) -> Self {
        self.debug_selector = Some(selector);
        self
    }

    pub fn render(self, theme: AppTheme) -> impl IntoElement {
        let handle = self.handle.clone();
        let paint = canvas(
            |_, _, _| (),
            move |bounds, _, window, _cx| {
                let viewport_h = {
                    let h = handle.bounds().size.height;
                    if h > px(0.0) { h } else { bounds.size.height }
                };
                let max_offset = handle.max_offset().height.max(px(0.0));
                let scroll_y = (-handle.offset().y).max(px(0.0)).min(max_offset);

                let Some(metrics) = vertical_thumb_metrics(viewport_h, max_offset, scroll_y) else {
                    return;
                };

                let x = bounds.right() - px(4.0) - metrics.width;
                let y = bounds.top() + metrics.top;
                window.paint_quad(fill(
                    gpui::Bounds::new(point(x, y), size(metrics.width, metrics.height)),
                    thumb_bg(theme),
                ));
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();

        let base = div().id(self.id).absolute().top_0().left_0().size_full().child(paint);

        #[cfg(test)]
        let base = match self.debug_selector {
            Some(selector) => base.debug_selector(|| selector.to_string()),
            None => base,
        };

        base
    }
}

#[cfg(test)]
impl Scrollbar {
    pub fn thumb_visible_for_test(handle: &ScrollHandle, viewport_h_fallback: Pixels) -> bool {
        let viewport_h = {
            let h = handle.bounds().size.height;
            if h > px(0.0) { h } else { viewport_h_fallback }
        };
        let max_offset = handle.max_offset().height.max(px(0.0));
        let scroll_y = (-handle.offset().y).max(px(0.0)).min(max_offset);
        vertical_thumb_metrics(viewport_h, max_offset, scroll_y).is_some()
    }
}

#[derive(Clone, Copy, Debug)]
struct ThumbMetrics {
    top: Pixels,
    height: Pixels,
    width: Pixels,
}

fn thumb_bg(theme: AppTheme) -> gpui::Rgba {
    let mut color = theme.colors.text_muted;
    color.a = if theme.is_dark { 0.32 } else { 0.28 };
    color
}

fn vertical_thumb_metrics(viewport_h: Pixels, max_offset: Pixels, scroll_y: Pixels) -> Option<ThumbMetrics> {
    if viewport_h <= px(0.0) || max_offset <= px(0.0) {
        return None;
    }
    let content_h = viewport_h + max_offset;
    let margin = px(4.0);
    let track_h = (viewport_h - margin * 2.0).max(px(0.0));

    let thumb_h = ((viewport_h * (viewport_h / content_h)).max(px(24.0))).min(track_h);
    let available = (track_h - thumb_h).max(px(0.0));

    let pct = if max_offset <= px(0.0) { 0.0 } else { scroll_y / max_offset };

    let top = margin + available * pct;

    Some(ThumbMetrics {
        top,
        height: thumb_h,
        width: px(8.0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumb_metrics_none_without_overflow() {
        assert!(vertical_thumb_metrics(px(100.0), px(0.0), px(0.0)).is_none());
    }

    #[test]
    fn thumb_bg_alpha_in_range() {
        for theme in [AppTheme::zed_ayu_dark(), AppTheme::zed_one_light()] {
            let bg = thumb_bg(theme);
            assert!(bg.a >= 0.0 && bg.a <= 1.0);
        }
    }
}
