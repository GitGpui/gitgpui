use crate::theme::AppTheme;
use gpui::prelude::*;
use gpui::{
    Bounds, CursorStyle, DispatchPhase, ElementId, Hitbox, HitboxBehavior, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, ScrollHandle, canvas, div, fill, point,
    px, size,
};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollbarMarkerKind {
    Add,
    Remove,
    Modify,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollbarMarker {
    /// Start of the marker as a fraction of total content height in `[0, 1]`.
    pub start: f32,
    /// End of the marker as a fraction of total content height in `[0, 1]`.
    pub end: f32,
    pub kind: ScrollbarMarkerKind,
}

#[derive(Clone)]
pub struct Scrollbar {
    id: ElementId,
    handle: ScrollHandle,
    markers: Vec<ScrollbarMarker>,
    #[cfg(test)]
    debug_selector: Option<&'static str>,
}

struct ScrollbarInteractionState {
    drag_offset_y: Option<Pixels>,
    showing: bool,
    hide_task: Option<gpui::Task<()>>,
    last_scroll_y: Pixels,
    /// Some GPUI scroll surfaces report positive offsets while others report negative offsets.
    /// Track the observed sign so the thumb moves/drag-scrolls in the correct direction.
    offset_sign_y: i8,
}

impl Default for ScrollbarInteractionState {
    fn default() -> Self {
        Self {
            drag_offset_y: None,
            showing: false,
            hide_task: None,
            last_scroll_y: px(0.0),
            offset_sign_y: -1,
        }
    }
}

#[derive(Clone, Debug)]
struct ScrollbarPrepaintState {
    track_bounds: Bounds<Pixels>,
    thumb_bounds: Bounds<Pixels>,
    cursor_hitbox: Hitbox,
}

impl Scrollbar {
    pub fn new(id: impl Into<ElementId>, handle: ScrollHandle) -> Self {
        Self {
            id: id.into(),
            handle,
            markers: Vec::new(),
            #[cfg(test)]
            debug_selector: None,
        }
    }

    pub fn markers(mut self, markers: Vec<ScrollbarMarker>) -> Self {
        self.markers = markers;
        self
    }

    #[cfg(test)]
    pub fn debug_selector(mut self, selector: &'static str) -> Self {
        self.debug_selector = Some(selector);
        self
    }

    pub fn render(self, theme: AppTheme) -> impl IntoElement {
        let handle = self.handle.clone();
        let markers = self.markers;
        let id = self.id.clone();

        let prepaint_handle = handle.clone();
        let paint = canvas(
            move |bounds, window, _cx| {
                let viewport_h = bounds.size.height;
                let max_offset = prepaint_handle.max_offset().height.max(px(0.0));
                let raw_offset_y = prepaint_handle.offset().y;
                let scroll_y = if raw_offset_y < px(0.0) {
                    (-raw_offset_y).max(px(0.0)).min(max_offset)
                } else {
                    raw_offset_y.max(px(0.0)).min(max_offset)
                };

                let metrics = vertical_thumb_metrics(viewport_h, max_offset, scroll_y)?;

                let margin = px(4.0);
                let track_h = (viewport_h - margin * 2.0).max(px(0.0));
                let track_bounds = Bounds::new(
                    point(bounds.left(), bounds.top() + margin),
                    size(bounds.size.width, track_h),
                );

                let thumb_x = bounds.right() - margin - metrics.width;
                let thumb_bounds = Bounds::new(
                    point(thumb_x, bounds.top() + metrics.top),
                    size(metrics.width, metrics.height),
                );

                let cursor_hitbox =
                    window.insert_hitbox(track_bounds, HitboxBehavior::BlockMouseExceptScroll);

                Some(ScrollbarPrepaintState {
                    track_bounds,
                    thumb_bounds,
                    cursor_hitbox,
                })
            },
            move |bounds, prepaint, window, cx| {
                let Some(prepaint) = prepaint else {
                    return;
                };

                let interaction = window.use_keyed_state(
                    (id.clone(), "scrollbar_interaction"),
                    cx,
                    |_window, _cx| ScrollbarInteractionState::default(),
                );

                let capture_phase = if interaction.read(cx).drag_offset_y.is_some() {
                    DispatchPhase::Capture
                } else {
                    DispatchPhase::Bubble
                };

                let margin = px(4.0);
                let track_h = prepaint.track_bounds.size.height.max(px(0.0));

                let thumb_x = prepaint.thumb_bounds.origin.x;
                let marker_w = px(4.0);
                let marker_x = (thumb_x - margin - marker_w).max(bounds.left());

                for marker in &markers {
                    let start = marker.start.clamp(0.0, 1.0);
                    let end = marker.end.clamp(0.0, 1.0);
                    if end <= start {
                        continue;
                    }

                    let y0 = prepaint.track_bounds.top() + track_h * start;
                    let y1 = prepaint.track_bounds.top() + track_h * end;
                    let min_h = px(2.0);
                    let h = (y1 - y0).max(min_h);

                    let (left, right) = marker_colors(theme, marker.kind);
                    if let Some(left) = left {
                        window.paint_quad(fill(
                            gpui::Bounds::new(point(marker_x, y0), size(marker_w / 2.0, h)),
                            left,
                        ));
                    }
                    if let Some(right) = right {
                        window.paint_quad(fill(
                            gpui::Bounds::new(
                                point(marker_x + marker_w / 2.0, y0),
                                size(marker_w / 2.0, h),
                            ),
                            right,
                        ));
                    }
                }

                let hovered = prepaint.cursor_hitbox.is_hovered(window);
                let is_dragging = interaction.read(cx).drag_offset_y.is_some();

                let max_offset = handle.max_offset().height.max(px(0.0));
                let raw_offset_y = handle.offset().y;
                let observed_sign = if raw_offset_y < px(0.0) {
                    -1
                } else if raw_offset_y > px(0.0) {
                    1
                } else {
                    interaction.read(cx).offset_sign_y
                };
                if observed_sign != interaction.read(cx).offset_sign_y && raw_offset_y != px(0.0) {
                    interaction.update(cx, |state, _cx| state.offset_sign_y = observed_sign);
                }
                let scroll_y = if observed_sign < 0 {
                    (-raw_offset_y).max(px(0.0)).min(max_offset)
                } else {
                    raw_offset_y.max(px(0.0)).min(max_offset)
                };
                let scrolled = interaction.read(cx).last_scroll_y != scroll_y;
                if scrolled {
                    interaction.update(cx, |state, _cx| {
                        state.last_scroll_y = scroll_y;
                        state.showing = true;
                        state.hide_task.take();
                    });
                }

                // Zed-style autohide: show on hover/drag, then hide after a delay.
                let state = interaction.read(cx);
                let show = hovered || is_dragging || state.showing;
                let should_schedule_hide =
                    !hovered && !is_dragging && state.showing && state.hide_task.is_none();
                let _ = state;

                if hovered || is_dragging {
                    interaction.update(cx, |state, _cx| {
                        state.showing = true;
                        state.hide_task.take();
                    });
                } else if should_schedule_hide {
                    interaction.update(cx, |state, cx| {
                        state.hide_task.take();
                        let task = cx.spawn(
                            async move |state: gpui::WeakEntity<ScrollbarInteractionState>,
                                        cx: &mut gpui::AsyncApp| {
                                gpui::Timer::after(Duration::from_millis(1000)).await;
                                let _ = state.update(cx, |s, cx| {
                                    if s.drag_offset_y.is_none() {
                                        s.showing = false;
                                        cx.notify();
                                    }
                                    s.hide_task = None;
                                });
                            },
                        );
                        state.hide_task = Some(task);
                    });
                }
                let thumb_color = if is_dragging {
                    theme.colors.scrollbar_thumb_active
                } else if hovered {
                    theme.colors.scrollbar_thumb_hover
                } else {
                    theme.colors.scrollbar_thumb
                };

                if show {
                    window.paint_quad(fill(prepaint.thumb_bounds, thumb_color));
                }

                if interaction.read(cx).drag_offset_y.is_some() {
                    window.set_window_cursor_style(CursorStyle::Arrow);
                } else {
                    window.set_cursor_style(CursorStyle::Arrow, &prepaint.cursor_hitbox);
                }

                let track_bounds = prepaint.track_bounds;
                let thumb_bounds = prepaint.thumb_bounds;
                let thumb_h = thumb_bounds.size.height;

                window.on_mouse_event({
                    let interaction = interaction.clone();
                    let handle = handle.clone();
                    move |event: &MouseDownEvent, phase, window, cx| {
                        if phase != capture_phase || event.button != MouseButton::Left {
                            return;
                        }
                        if !track_bounds.contains(&event.position) {
                            return;
                        }

                        let max_offset = handle.max_offset().height.max(px(0.0));
                        if max_offset <= px(0.0) {
                            return;
                        }

                        if thumb_bounds.contains(&event.position) {
                            let grab = event.position.y - thumb_bounds.origin.y;
                            interaction.update(cx, |state, _cx| {
                                state.drag_offset_y = Some(grab);
                                state.showing = true;
                                state.hide_task.take();
                            });
                        } else {
                            interaction.update(cx, |state, _cx| {
                                state.drag_offset_y = None;
                                state.showing = true;
                                state.hide_task.take();
                            });
                            let sign = interaction.read(cx).offset_sign_y;
                            let offset_y = compute_vertical_click_offset(
                                event.position.y,
                                track_bounds,
                                thumb_h,
                                thumb_h / 2.0,
                                max_offset,
                                sign,
                            );
                            let x = handle.offset().x;
                            handle.set_offset(point(x, offset_y));
                        }

                        window.refresh();
                        cx.stop_propagation();
                    }
                });

                window.on_mouse_event({
                    let interaction = interaction.clone();
                    let handle = handle.clone();
                    move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase != capture_phase || !event.dragging() {
                            return;
                        }

                        let Some(grab) = interaction.read(cx).drag_offset_y else {
                            return;
                        };

                        let max_offset = handle.max_offset().height.max(px(0.0));
                        if max_offset <= px(0.0) {
                            return;
                        }

                        let sign = interaction.read(cx).offset_sign_y;
                        let offset_y = compute_vertical_click_offset(
                            event.position.y,
                            track_bounds,
                            thumb_h,
                            grab,
                            max_offset,
                            sign,
                        );
                        let x = handle.offset().x;
                        handle.set_offset(point(x, offset_y));
                        interaction.update(cx, |state, _cx| state.showing = true);
                        window.refresh();
                        cx.stop_propagation();
                    }
                });

                window.on_mouse_event({
                    let interaction = interaction.clone();
                    move |event: &MouseUpEvent, phase, window, cx| {
                        if phase != capture_phase || event.button != MouseButton::Left {
                            return;
                        }
                        if interaction.read(cx).drag_offset_y.is_none() {
                            return;
                        }
                        interaction.update(cx, |state, _cx| state.drag_offset_y = None);
                        window.refresh();
                        cx.stop_propagation();
                    }
                });
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();

        let base = div()
            .id(self.id)
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .w(px(16.0))
            .child(paint);

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
        let viewport_h = viewport_h_fallback;
        let max_offset = handle.max_offset().height.max(px(0.0));
        let raw_offset_y = handle.offset().y;
        let scroll_y = if raw_offset_y < px(0.0) {
            (-raw_offset_y).max(px(0.0)).min(max_offset)
        } else {
            raw_offset_y.max(px(0.0)).min(max_offset)
        };
        vertical_thumb_metrics(viewport_h, max_offset, scroll_y).is_some()
    }
}

#[derive(Clone, Copy, Debug)]
struct ThumbMetrics {
    top: Pixels,
    height: Pixels,
    width: Pixels,
}

fn marker_colors(
    theme: AppTheme,
    kind: ScrollbarMarkerKind,
) -> (Option<gpui::Rgba>, Option<gpui::Rgba>) {
    let mut add = theme.colors.success;
    let mut rem = theme.colors.danger;
    let alpha = if theme.is_dark { 0.70 } else { 0.55 };
    add.a = alpha;
    rem.a = alpha;

    match kind {
        ScrollbarMarkerKind::Add => (Some(add), Some(add)),
        ScrollbarMarkerKind::Remove => (Some(rem), Some(rem)),
        ScrollbarMarkerKind::Modify => (Some(rem), Some(add)),
    }
}

fn compute_vertical_click_offset(
    event_y: Pixels,
    track_bounds: Bounds<Pixels>,
    thumb_size: Pixels,
    thumb_offset: Pixels,
    max_offset: Pixels,
    sign_y: i8,
) -> Pixels {
    let viewport_size = track_bounds.size.height.max(px(0.0));
    if viewport_size <= px(0.0) || max_offset <= px(0.0) {
        return px(0.0);
    }

    let max_thumb_start = (viewport_size - thumb_size).max(px(0.0));
    let thumb_start = (event_y - track_bounds.origin.y - thumb_offset)
        .max(px(0.0))
        .min(max_thumb_start);

    let pct = if max_thumb_start > px(0.0) {
        thumb_start / max_thumb_start
    } else {
        0.0
    };

    let scroll_y = (max_offset * pct).max(px(0.0)).min(max_offset);
    let sign = if sign_y < 0 { -1.0 } else { 1.0 };
    scroll_y * sign
}

fn vertical_thumb_metrics(
    viewport_h: Pixels,
    max_offset: Pixels,
    scroll_y: Pixels,
) -> Option<ThumbMetrics> {
    if viewport_h <= px(0.0) || max_offset <= px(0.0) {
        return None;
    }
    let content_h = viewport_h + max_offset;
    let margin = px(4.0);
    let track_h = (viewport_h - margin * 2.0).max(px(0.0));

    let thumb_h = ((viewport_h * (viewport_h / content_h)).max(px(24.0))).min(track_h);
    let available = (track_h - thumb_h).max(px(0.0));

    let pct = if max_offset <= px(0.0) {
        0.0
    } else {
        scroll_y / max_offset
    };

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
    fn scrollbar_thumb_alpha_in_range() {
        for theme in [AppTheme::zed_ayu_dark(), AppTheme::zed_one_light()] {
            for c in [
                theme.colors.scrollbar_thumb,
                theme.colors.scrollbar_thumb_hover,
                theme.colors.scrollbar_thumb_active,
            ] {
                assert!(c.a >= 0.0 && c.a <= 1.0);
            }
        }
    }
}
