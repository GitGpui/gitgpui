use super::*;
use gpui::{Bounds, ContentMask, fill, point, px, size};

pub(super) fn history_commit_row_canvas(
    theme: AppTheme,
    row_id: usize,
    col_branch: Pixels,
    col_graph: Pixels,
    col_date: Pixels,
    col_sha: Pixels,
    show_date: bool,
    show_sha: bool,
    show_graph_color_marker: bool,
    is_stash_node: bool,
    graph_row: history_graph::GraphRow,
    refs: SharedString,
    summary: SharedString,
    when: SharedString,
    short_sha: SharedString,
) -> AnyElement {
    super::canvas::keyed_canvas(
        ("history_commit_row_canvas", row_id),
        move |bounds, window, _cx| {
            let pad = window.rem_size() * 0.5;
            let inner = Bounds::new(
                point(bounds.left() + pad, bounds.top()),
                size((bounds.size.width - pad * 2.0).max(px(0.0)), bounds.size.height),
            );
            (inner, pad)
        },
        move |bounds, (inner, _pad), window, cx| {
            let base_style = window.text_style();
            let sm_font = base_style.font_size.to_pixels(window.rem_size());
            let sm_line_height = base_style
                .line_height
                .to_pixels(sm_font.into(), window.rem_size());
            let xs_font = sm_font * 0.86;
            let xs_line_height = base_style
                .line_height
                .to_pixels(xs_font.into(), window.rem_size());

            let center_y = |line_height: Pixels| {
                let extra = (bounds.size.height - line_height).max(px(0.0));
                bounds.top() + extra * 0.5
            };

            let mut x = inner.left();
            let branch_bounds = Bounds::new(
                point(x, bounds.top()),
                size(col_branch.max(px(0.0)), bounds.size.height),
            );
            x += col_branch;
            let graph_bounds = Bounds::new(
                point(x, bounds.top()),
                size(col_graph.max(px(0.0)), bounds.size.height),
            );
            x += col_graph;

            let right_total = (if show_date { col_date } else { px(0.0) })
                + (if show_sha { col_sha } else { px(0.0) });
            let summary_right = (inner.right() - right_total).max(x);
            let summary_bounds = Bounds::new(
                point(x, bounds.top()),
                size((summary_right - x).max(px(0.0)), bounds.size.height),
            );

            let sha_bounds = if show_sha {
                Bounds::new(
                    point(inner.right() - col_sha, bounds.top()),
                    size(col_sha.max(px(0.0)), bounds.size.height),
                )
            } else {
                Bounds::new(point(inner.right(), bounds.top()), size(px(0.0), bounds.size.height))
            };
            let date_bounds = if show_date {
                let right = if show_sha { sha_bounds.left() } else { inner.right() };
                Bounds::new(
                    point(right - col_date, bounds.top()),
                    size(col_date.max(px(0.0)), bounds.size.height),
                )
            } else {
                Bounds::new(point(sha_bounds.left(), bounds.top()), size(px(0.0), bounds.size.height))
            };

            window.paint_layer(graph_bounds, |window| {
                super::history_graph_paint::paint_history_graph(
                    theme,
                    &graph_row,
                    is_stash_node,
                    graph_bounds,
                    window,
                );
            });

            if !refs.as_ref().trim().is_empty() {
                let mut style = base_style.clone();
                style.color = theme.colors.text_muted.into();
                let mut runs = vec![style.to_run(refs.len())];
                let mut wrapper = window.text_system().line_wrapper(style.font(), xs_font);
                let truncated = wrapper.truncate_line(
                    refs.clone(),
                    branch_bounds.size.width.max(px(0.0)),
                    "…",
                    &mut runs,
                );
                let shaped = window
                    .text_system()
                    .shape_line(truncated, xs_font, &runs, None);
                window.with_content_mask(
                    Some(ContentMask {
                        bounds: branch_bounds,
                    }),
                    |window| {
                        let _ = shaped.paint(
                            point(branch_bounds.left(), center_y(xs_line_height)),
                            xs_line_height,
                            window,
                            cx,
                        );
                    },
                );
            }

            let node_color = graph_row
                .lanes_now
                .get(graph_row.node_col)
                .map(|l| l.color)
                .unwrap_or(theme.colors.text_muted);

            let summary_left_offset = if show_graph_color_marker {
                let marker_w = px(2.0);
                let marker_h = px(12.0);
                let y = bounds.top() + (bounds.size.height - marker_h) * 0.5;
                window.paint_quad(
                    fill(
                        Bounds::new(point(summary_bounds.left(), y), size(marker_w, marker_h)),
                        node_color,
                    )
                    .corner_radii(px(999.0)),
                );
                marker_w + window.rem_size() * 0.5
            } else {
                px(0.0)
            };

            let summary_text_bounds = Bounds::new(
                point(summary_bounds.left() + summary_left_offset, bounds.top()),
                size((summary_bounds.size.width - summary_left_offset).max(px(0.0)), bounds.size.height),
            );
            if !summary.as_ref().is_empty() {
                let mut style = base_style.clone();
                style.color = theme.colors.text.into();
                let mut runs = vec![style.to_run(summary.len())];
                let mut wrapper = window.text_system().line_wrapper(style.font(), sm_font);
                let truncated = wrapper.truncate_line(
                    summary.clone(),
                    summary_text_bounds.size.width.max(px(0.0)),
                    "…",
                    &mut runs,
                );
                let shaped = window
                    .text_system()
                    .shape_line(truncated, sm_font, &runs, None);
                window.with_content_mask(
                    Some(ContentMask {
                        bounds: summary_text_bounds,
                    }),
                    |window| {
                        let _ = shaped.paint(
                            point(summary_text_bounds.left(), center_y(sm_line_height)),
                            sm_line_height,
                            window,
                            cx,
                        );
                    },
                );
            }

            if show_date && !when.as_ref().is_empty() {
                let mut style = base_style.clone();
                style.color = theme.colors.text_muted.into();
                let mut runs = vec![style.to_run(when.len())];
                let mut wrapper = window.text_system().line_wrapper(style.font(), xs_font);
                let truncated = wrapper.truncate_line(
                    when.clone(),
                    date_bounds.size.width.max(px(0.0)),
                    "…",
                    &mut runs,
                );
                let shaped = window.text_system().shape_line(truncated, xs_font, &runs, None);
                let origin_x = (date_bounds.right() - shaped.width).max(date_bounds.left());
                window.with_content_mask(
                    Some(ContentMask { bounds: date_bounds }),
                    |window| {
                        let _ = shaped.paint(
                            point(origin_x, center_y(xs_line_height)),
                            xs_line_height,
                            window,
                            cx,
                        );
                    },
                );
            }

            if show_sha && !short_sha.as_ref().is_empty() {
                let mut style = base_style.clone();
                style.color = theme.colors.text_muted.into();
                let mut runs = vec![style.to_run(short_sha.len())];
                let mut wrapper = window.text_system().line_wrapper(style.font(), xs_font);
                let truncated = wrapper.truncate_line(
                    short_sha.clone(),
                    sha_bounds.size.width.max(px(0.0)),
                    "…",
                    &mut runs,
                );
                let shaped = window.text_system().shape_line(truncated, xs_font, &runs, None);
                let origin_x = (sha_bounds.right() - shaped.width).max(sha_bounds.left());
                window.with_content_mask(
                    Some(ContentMask { bounds: sha_bounds }),
                    |window| {
                        let _ = shaped.paint(
                            point(origin_x, center_y(xs_line_height)),
                            xs_line_height,
                            window,
                            cx,
                        );
                    },
                );
            }
        },
    )
    .h(px(24.0))
    .w_full()
    .into_any_element()
}
