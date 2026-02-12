use super::*;

impl GitGpuiView {
    pub(super) fn sync_tooltip_state(&mut self, cx: &mut gpui::Context<Self>) {
        if self.tooltip_text == self.tooltip_candidate_last {
            return;
        }

        self.tooltip_candidate_last = self.tooltip_text.clone();
        self.tooltip_visible_text = None;
        self.tooltip_visible_pos = None;
        self.tooltip_pending_pos = None;
        self.tooltip_delay_seq = self.tooltip_delay_seq.wrapping_add(1);

        let Some(text) = self.tooltip_text.clone() else {
            return;
        };

        let anchor = self.last_mouse_pos;
        self.tooltip_pending_pos = Some(anchor);
        let seq = self.tooltip_delay_seq;

        cx.spawn(
            async move |view: WeakEntity<GitGpuiView>, cx: &mut gpui::AsyncApp| {
                Timer::after(Duration::from_millis(500)).await;
                let _ = view.update(cx, |this, cx| {
                    if this.tooltip_delay_seq != seq {
                        return;
                    }
                    if this.tooltip_text.as_ref() != Some(&text) {
                        return;
                    }
                    let Some(pending_pos) = this.tooltip_pending_pos else {
                        return;
                    };
                    let dx = (this.last_mouse_pos.x - pending_pos.x).abs();
                    let dy = (this.last_mouse_pos.y - pending_pos.y).abs();
                    if dx > px(2.0) || dy > px(2.0) {
                        return;
                    }
                    this.tooltip_visible_text = Some(text.clone());
                    this.tooltip_visible_pos = Some(pending_pos);
                    cx.notify();
                });
            },
        )
        .detach();
    }

    pub(super) fn maybe_restart_tooltip_delay(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(candidate) = self.tooltip_text.clone() else {
            if self.tooltip_visible_text.is_some() {
                self.tooltip_visible_text = None;
                self.tooltip_visible_pos = None;
                cx.notify();
            }
            return;
        };

        if let Some(visible_anchor) = self.tooltip_visible_pos {
            let dx = (self.last_mouse_pos.x - visible_anchor.x).abs();
            let dy = (self.last_mouse_pos.y - visible_anchor.y).abs();
            if dx <= px(6.0) && dy <= px(6.0) {
                return;
            }
        }

        let should_restart = match self.tooltip_pending_pos {
            None => true,
            Some(pending_anchor) => {
                let dx = (self.last_mouse_pos.x - pending_anchor.x).abs();
                let dy = (self.last_mouse_pos.y - pending_anchor.y).abs();
                dx > px(2.0) || dy > px(2.0)
            }
        };

        if !should_restart {
            return;
        }

        self.tooltip_visible_text = None;
        self.tooltip_visible_pos = None;
        self.tooltip_pending_pos = Some(self.last_mouse_pos);
        self.tooltip_delay_seq = self.tooltip_delay_seq.wrapping_add(1);
        let seq = self.tooltip_delay_seq;

        cx.spawn(
            async move |view: WeakEntity<GitGpuiView>, cx: &mut gpui::AsyncApp| {
                Timer::after(Duration::from_millis(500)).await;
                let _ = view.update(cx, |this, cx| {
                    if this.tooltip_delay_seq != seq {
                        return;
                    }
                    if this.tooltip_text.as_ref() != Some(&candidate) {
                        return;
                    }
                    let Some(pending_pos) = this.tooltip_pending_pos else {
                        return;
                    };
                    let dx = (this.last_mouse_pos.x - pending_pos.x).abs();
                    let dy = (this.last_mouse_pos.y - pending_pos.y).abs();
                    if dx > px(2.0) || dy > px(2.0) {
                        return;
                    }
                    this.tooltip_visible_text = Some(candidate.clone());
                    this.tooltip_visible_pos = Some(pending_pos);
                    cx.notify();
                });
            },
        )
        .detach();
    }

    pub(super) fn schedule_ui_settings_persist(&mut self, cx: &mut gpui::Context<Self>) {
        self.ui_settings_persist_seq = self.ui_settings_persist_seq.wrapping_add(1);
        let seq = self.ui_settings_persist_seq;

        cx.spawn(
            async move |view: WeakEntity<GitGpuiView>, cx: &mut gpui::AsyncApp| {
                Timer::after(Duration::from_millis(250)).await;
                let _ = view.update(cx, |this, _cx| {
                    if this.ui_settings_persist_seq != seq {
                        return;
                    }

                    let ww: f32 = this.last_window_size.width.round().into();
                    let wh: f32 = this.last_window_size.height.round().into();
                    let window_width = (ww.is_finite() && ww >= 1.0).then_some(ww as u32);
                    let window_height = (wh.is_finite() && wh >= 1.0).then_some(wh as u32);

                    let sidebar_width: f32 = this.sidebar_width.round().into();
                    let details_width: f32 = this.details_width.round().into();

                    let settings = session::UiSettings {
                        window_width,
                        window_height,
                        sidebar_width: (sidebar_width.is_finite() && sidebar_width >= 1.0)
                            .then_some(sidebar_width as u32),
                        details_width: (details_width.is_finite() && details_width >= 1.0)
                            .then_some(details_width as u32),
                        date_time_format: Some(this.date_time_format.key().to_string()),
                    };

                    let _ = session::persist_ui_settings(settings);
                });
            },
        )
        .detach();
    }

    pub(super) fn clamp_pane_widths_to_window(&mut self) {
        let total_w = self.last_window_size.width;
        if total_w.is_zero() {
            return;
        }

        let handles_w = px(PANE_RESIZE_HANDLE_PX) * 2.0;
        let main_min = px(MAIN_MIN_PX);
        let sidebar_min = px(SIDEBAR_MIN_PX);
        let details_min = px(DETAILS_MIN_PX);

        let max_sidebar = (total_w - self.details_width - main_min - handles_w).max(sidebar_min);
        self.sidebar_width = self.sidebar_width.max(sidebar_min).min(max_sidebar);

        let max_details = (total_w - self.sidebar_width - main_min - handles_w).max(details_min);
        self.details_width = self.details_width.max(details_min).min(max_details);
    }
}
