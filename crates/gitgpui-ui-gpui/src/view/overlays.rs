use super::*;

impl GitGpuiView {
    pub(super) fn popover_layer(&mut self, cx: &mut gpui::Context<Self>) -> AnyElement {
        let close = cx.listener(|this, _e: &MouseDownEvent, _w, cx| this.close_popover(cx));

        let scrim = div()
            .id("popover_scrim")
            .debug_selector(|| "repo_popover_close".to_string())
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(gpui::rgba(0x00000000))
            .occlude()
            .on_any_mouse_down(close);

        let popover = self
            .popover
            .clone()
            .map(|kind| self.popover_view(kind, cx).into_any_element())
            .unwrap_or_else(|| div().into_any_element());

        div()
            .id("popover_layer")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(scrim)
            .child(popover)
            .into_any_element()
    }

    pub(super) fn toast_layer(&self, cx: &gpui::Context<Self>) -> AnyElement {
        if self.toasts.is_empty() {
            return div().into_any_element();
        }
        let theme = self.theme;

        let progress_id = self.clone_progress_toast_id;
        let max_other = if progress_id.is_some() { 2 } else { 3 };
        let mut displayed = self
            .toasts
            .iter()
            .rev()
            .filter(|t| Some(t.id) != progress_id)
            .take(max_other)
            .cloned()
            .collect::<Vec<_>>();
        if let Some(id) = progress_id
            && let Some(progress) = self.toasts.iter().find(|t| t.id == id).cloned()
        {
            displayed.push(progress);
        }

        let fade_in = toast_fade_in_duration();
        let fade_out = toast_fade_out_duration();
        let children = displayed.into_iter().map(move |t| {
            let animations = match t.ttl {
                Some(ttl) => vec![
                    Animation::new(fade_in).with_easing(gpui::quadratic),
                    Animation::new(ttl),
                    Animation::new(fade_out).with_easing(gpui::quadratic),
                ],
                None => vec![Animation::new(fade_in).with_easing(gpui::quadratic)],
            };

            let close = zed::Button::new(format!("toast_close_{}", t.id), "✕")
                .style(zed::ButtonStyle::Transparent)
                .on_click(theme, cx, move |this, _e, _w, cx| {
                    this.remove_toast(t.id, cx);
                })
                .on_hover(cx.listener(|this, hovering: &bool, _w, cx| {
                    let text: SharedString = "Dismiss notification".into();
                    let mut changed = false;
                    if *hovering {
                        changed |= this.set_tooltip_text_if_changed(Some(text));
                    } else if this.tooltip_text.as_ref() == Some(&text) {
                        changed |= this.set_tooltip_text_if_changed(None);
                    }
                    if changed {
                        cx.notify();
                    }
                }));

            div()
                .relative()
                .child(zed::toast(theme, t.kind, t.input.clone()))
                .child(div().absolute().top(px(8.0)).right(px(8.0)).child(close))
                .with_animations(
                    ("toast", t.id),
                    animations,
                    move |toast, animation_ix, delta| {
                        let opacity = match animation_ix {
                            0 => delta,
                            1 => 1.0,
                            2 => 1.0 - delta,
                            _ => 1.0,
                        };
                        let slide_x = match animation_ix {
                            0 => (1.0 - delta) * TOAST_SLIDE_PX,
                            2 => delta * TOAST_SLIDE_PX,
                            _ => 0.0,
                        };
                        toast.opacity(opacity).relative().left(px(slide_x))
                    },
                )
        });

        div()
            .id("toast_layer")
            .absolute()
            .right_0()
            .bottom_0()
            .p(px(16.0))
            .flex()
            .flex_col()
            .items_end()
            .gap(px(12.0))
            .children(children)
            .into_any_element()
    }
}
