use super::*;

pub(super) const CLIENT_SIDE_DECORATION_INSET: Pixels = px(10.0);

fn titlebar_control_button(theme: AppTheme, hover_bg: gpui::Rgba) -> gpui::Div {
    div()
        .h_full()
        .px_3()
        .flex()
        .items_center()
        .cursor(CursorStyle::PointingHand)
        .hover(move |s| s.bg(hover_bg))
        .text_color(theme.colors.text)
}

pub(super) fn cursor_style_for_resize_edge(edge: ResizeEdge) -> CursorStyle {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
        ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
    }
}

pub(super) fn resize_edge(
    pos: Point<Pixels>,
    inset: Pixels,
    window_size: Size<Pixels>,
    tiling: Tiling,
) -> Option<ResizeEdge> {
    let bounds = Bounds::new(Point::default(), window_size).inset(inset * 1.5);
    if bounds.contains(&pos) {
        return None;
    }

    let corner_size = size(inset * 1.5, inset * 1.5);
    let top_left_bounds = Bounds::new(Point::new(px(0.0), px(0.0)), corner_size);
    if !tiling.top && top_left_bounds.contains(&pos) {
        return Some(ResizeEdge::TopLeft);
    }

    let top_right_bounds = Bounds::new(
        Point::new(window_size.width - corner_size.width, px(0.0)),
        corner_size,
    );
    if !tiling.top && top_right_bounds.contains(&pos) {
        return Some(ResizeEdge::TopRight);
    }

    let bottom_left_bounds = Bounds::new(
        Point::new(px(0.0), window_size.height - corner_size.height),
        corner_size,
    );
    if !tiling.bottom && bottom_left_bounds.contains(&pos) {
        return Some(ResizeEdge::BottomLeft);
    }

    let bottom_right_bounds = Bounds::new(
        Point::new(
            window_size.width - corner_size.width,
            window_size.height - corner_size.height,
        ),
        corner_size,
    );
    if !tiling.bottom && bottom_right_bounds.contains(&pos) {
        return Some(ResizeEdge::BottomRight);
    }

    if !tiling.top && pos.y < inset {
        Some(ResizeEdge::Top)
    } else if !tiling.bottom && pos.y > window_size.height - inset {
        Some(ResizeEdge::Bottom)
    } else if !tiling.left && pos.x < inset {
        Some(ResizeEdge::Left)
    } else if !tiling.right && pos.x > window_size.width - inset {
        Some(ResizeEdge::Right)
    } else {
        None
    }
}

impl GitGpuiView {
    pub(super) fn title_bar(
        &mut self,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let bar_bg = if window.is_window_active() {
            theme.colors.surface_bg
        } else {
            with_alpha(theme.colors.surface_bg, 0.92)
        };
        let bar_border = if window.is_window_active() {
            theme.colors.border
        } else {
            with_alpha(theme.colors.border, 0.7)
        };

        let hamburger = div()
            .id("app_menu")
            .debug_selector(|| "app_menu".to_string())
            .h_full()
            .px_2()
            .flex()
            .items_center()
            .cursor(CursorStyle::PointingHand)
            .hover(move |s| s.bg(theme.colors.hover))
            .child("≡")
            .on_click(cx.listener(|this, e: &ClickEvent, _w, cx| {
                this.popover = Some(PopoverKind::AppMenu);
                this.popover_anchor = Some(e.position());
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|_this, e: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    window.show_window_menu(e.position);
                }),
            );

        let drag_region = div()
            .id("title_drag")
            .flex_1()
            .h_full()
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _e, _w, cx| {
                    this.title_should_move = true;
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _e, _w, cx| {
                    this.title_should_move = false;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _e, _w, cx| {
                    this.title_should_move = false;
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, _e, window, _cx| {
                if this.title_should_move {
                    this.title_should_move = false;
                    window.start_window_move();
                }
            }))
            .child(
                div()
                    .text_sm()
                    .text_color(theme.colors.text_muted)
                    .child("GitGpui"),
            );

        let min = titlebar_control_button(theme, theme.colors.hover)
            .id("win_min")
            .window_control_area(WindowControlArea::Min)
            .child("—")
            .on_click(cx.listener(|_this, _e: &ClickEvent, window, cx| {
                cx.stop_propagation();
                window.minimize_window();
            }));

        let max = titlebar_control_button(theme, theme.colors.hover)
            .id("win_max")
            .window_control_area(WindowControlArea::Max)
            .child(if window.is_maximized() { "❐" } else { "□" })
            .on_click(cx.listener(|_this, _e: &ClickEvent, window, cx| {
                cx.stop_propagation();
                window.zoom_window();
            }));

        let close = titlebar_control_button(theme, with_alpha(theme.colors.danger, 0.25))
            .id("win_close")
            .window_control_area(WindowControlArea::Close)
            .child("×")
            .on_click(cx.listener(|_this, _e: &ClickEvent, _window, cx| {
                cx.stop_propagation();
                cx.quit();
            }));

        div()
            .id("title_bar")
            .flex()
            .items_center()
            .h(px(34.0))
            .w_full()
            .bg(bar_bg)
            .border_b_1()
            .border_color(bar_border)
            .child(hamburger)
            .child(drag_region)
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(min)
                    .child(max)
                    .child(close),
            )
            .into_any_element()
    }
}

pub(crate) fn window_frame(
    theme: AppTheme,
    decorations: Decorations,
    content: AnyElement,
) -> AnyElement {
    let mut outer = div()
        .id("window_frame")
        .size_full()
        .bg(gpui::rgba(0x00000000));

    if let Decorations::Client { tiling } = decorations {
        outer = outer
            .when(!tiling.top, |d| d.pt(CLIENT_SIDE_DECORATION_INSET))
            .when(!tiling.bottom, |d| d.pb(CLIENT_SIDE_DECORATION_INSET))
            .when(!tiling.left, |d| d.pl(CLIENT_SIDE_DECORATION_INSET))
            .when(!tiling.right, |d| d.pr(CLIENT_SIDE_DECORATION_INSET));
    }

    let inner = div()
        .id("window_surface")
        .size_full()
        .bg(theme.colors.window_bg)
        .border_1()
        .border_color(with_alpha(theme.colors.border, 0.9))
        .rounded(px(theme.radii.panel))
        .shadow_lg()
        .overflow_hidden()
        .child(content);

    outer.child(inner).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titlebar_buttons_do_not_double_set_hover_style() {
        let theme = AppTheme::zed_ayu_dark();
        assert!(
            std::panic::catch_unwind(|| {
                let _ = titlebar_control_button(theme, theme.colors.hover);
            })
            .is_ok()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _ = titlebar_control_button(theme, with_alpha(theme.colors.danger, 0.25));
            })
            .is_ok()
        );
    }
}
