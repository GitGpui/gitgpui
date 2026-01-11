use super::*;

impl GitGpuiView {
    pub(super) fn render_branch_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let Loadable::Ready(branches) = &repo.branches else {
            return Vec::new();
        };
        let theme = this.theme;
        range
            .filter_map(|ix| branches.get(ix).map(|b| (ix, b)))
            .map(|(ix, branch)| {
                div()
                    .id(ix)
                    .px_2()
                    .py_1()
                    .rounded(px(theme.radii.row))
                    .hover(move |s| s.bg(theme.colors.hover))
                    .child(branch.name.clone())
                    .into_any_element()
            })
            .collect()
    }

    pub(super) fn render_remote_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let rows = Self::remote_rows(repo);
        let theme = this.theme;
        range
            .filter_map(|ix| rows.get(ix).cloned().map(|r| (ix, r)))
            .map(|(ix, row)| match row {
                RemoteRow::Header(name) => div()
                    .id(("remote_hdr", ix))
                    .px_2()
                    .py_1()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.colors.text)
                    .child(name)
                    .into_any_element(),
                RemoteRow::Branch { remote: _, name } => div()
                    .id(("remote_branch", ix))
                    .px_2()
                    .py_1()
                    .pl_4()
                    .rounded(px(theme.radii.row))
                    .hover(move |s| s.bg(theme.colors.hover))
                    .text_sm()
                    .text_color(theme.colors.text_muted)
                    .child(name)
                    .into_any_element(),
            })
            .collect()
    }

    pub(super) fn render_diagnostic_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let theme = this.theme;

        range
            .filter_map(|ix| repo.diagnostics.get(ix).map(|d| (ix, d)))
            .map(|(ix, d)| {
                let (label, color) = match d.kind {
                    DiagnosticKind::Info => ("Info", theme.colors.accent),
                    DiagnosticKind::Warning => ("Warning", theme.colors.warning),
                    DiagnosticKind::Error => ("Error", theme.colors.danger),
                };

                div()
                    .id(("diag", ix))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded(px(theme.radii.row))
                    .hover(move |s| s.bg(theme.colors.hover))
                    .child(components::pill(theme, label, color))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.colors.text_muted)
                            .line_clamp(2)
                            .child(d.message.clone()),
                    )
                    .into_any_element()
            })
            .collect()
    }

    pub(super) fn render_commit_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let Loadable::Ready(page) = &repo.log else {
            return Vec::new();
        };
        let theme = this.theme;
        range
            .filter_map(|ix| page.commits.get(ix).map(|c| (ix, c)))
            .map(|(ix, commit)| commit_row(theme, ix, commit))
            .collect()
    }

    pub(super) fn render_unstaged_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let Loadable::Ready(RepoStatus { unstaged, .. }) = &repo.status else {
            return Vec::new();
        };
        let theme = this.theme;
        range
            .filter_map(|ix| unstaged.get(ix).map(|e| (ix, e)))
            .map(|(ix, entry)| status_row(theme, ix, entry, DiffArea::Unstaged, repo.id, cx))
            .collect()
    }

    pub(super) fn render_staged_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(repo) = this.active_repo() else {
            return Vec::new();
        };
        let Loadable::Ready(RepoStatus { staged, .. }) = &repo.status else {
            return Vec::new();
        };
        let theme = this.theme;
        range
            .filter_map(|ix| staged.get(ix).map(|e| (ix, e)))
            .map(|(ix, entry)| status_row(theme, ix, entry, DiffArea::Staged, repo.id, cx))
            .collect()
    }

    pub(super) fn render_diff_rows(
        this: &mut Self,
        range: Range<usize>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let theme = this.theme;
        range
            .filter_map(|ix| this.diff_cache.get(ix).map(|l| (ix, l)))
            .map(|(ix, line)| diff_row(theme, ix, this.diff_view, line))
            .collect()
    }
}

fn commit_row(theme: AppTheme, ix: usize, commit: &Commit) -> AnyElement {
    let id: &str = <CommitId as AsRef<str>>::as_ref(&commit.id);
    let short = id.get(0..8).unwrap_or(id);
    div()
        .id(ix)
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .px_2()
        .py_1()
        .rounded(px(theme.radii.row))
        .hover(move |s| s.bg(theme.colors.hover))
        .child(div().text_sm().line_clamp(1).child(commit.summary.clone()))
        .child(div().text_xs().text_color(theme.colors.text_muted).child(short.to_string()))
        .into_any_element()
}

fn status_row(
    theme: AppTheme,
    ix: usize,
    entry: &FileStatus,
    area: DiffArea,
    repo_id: RepoId,
    cx: &mut gpui::Context<GitGpuiView>,
) -> AnyElement {
    let (label, color) = match entry.kind {
        FileStatusKind::Untracked => ("Untracked", theme.colors.warning),
        FileStatusKind::Modified => ("Modified", theme.colors.accent),
        FileStatusKind::Added => ("Added", theme.colors.success),
        FileStatusKind::Deleted => ("Deleted", theme.colors.danger),
        FileStatusKind::Renamed => ("Renamed", theme.colors.accent),
        FileStatusKind::Conflicted => ("Conflicted", theme.colors.danger),
    };

    let path = entry.path.clone();
    let path_for_stage = path.clone();
    let path_for_row = path.clone();
    let stage_label = match area {
        DiffArea::Unstaged => "Stage",
        DiffArea::Staged => "Unstage",
    };

    div()
        .id(ix)
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .px_2()
        .py_1()
        .rounded(px(theme.radii.row))
        .hover(move |s| s.bg(theme.colors.hover))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(components::pill(theme, label, color))
                .child(
                    div()
                        .text_sm()
                        .line_clamp(1)
                        .child(path.display().to_string()),
                ),
        )
        .child(
            kit::Button::new(format!("stage_btn_{ix}"), stage_label)
                .style(kit::ButtonStyle::Secondary)
                .on_click(theme, cx, move |this, _e, _w, cx| {
                    this.store.dispatch(Msg::SelectDiff {
                        repo_id,
                        target: DiffTarget {
                            path: path_for_stage.clone(),
                            area,
                        },
                    });
                    match area {
                        DiffArea::Unstaged => this.store.dispatch(Msg::StagePath {
                            repo_id,
                            path: path_for_stage.clone(),
                        }),
                        DiffArea::Staged => this.store.dispatch(Msg::UnstagePath {
                            repo_id,
                            path: path_for_stage.clone(),
                        }),
                    }
                    cx.notify();
                }),
        )
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.store.dispatch(Msg::SelectDiff {
                repo_id,
                target: DiffTarget {
                    path: path_for_row.clone(),
                    area,
                },
            });
            this.rebuild_diff_cache();
            cx.notify();
        }))
        .into_any_element()
}

fn diff_row(theme: AppTheme, ix: usize, mode: DiffViewMode, line: &AnnotatedDiffLine) -> AnyElement {
    let (bg, fg, gutter_fg) = diff_line_colors(theme, line.kind);

    let text = match line.kind {
        gitgpui_core::domain::DiffLineKind::Add => line.text.strip_prefix('+').unwrap_or(&line.text),
        gitgpui_core::domain::DiffLineKind::Remove => {
            line.text.strip_prefix('-').unwrap_or(&line.text)
        }
        gitgpui_core::domain::DiffLineKind::Context => {
            line.text.strip_prefix(' ').unwrap_or(&line.text)
        }
        _ => &line.text,
    };

    let old = line.old_line.map(|n| n.to_string()).unwrap_or_default();
    let new = line.new_line.map(|n| n.to_string()).unwrap_or_default();

    let (left_text, right_text) = match (mode, line.kind) {
        (DiffViewMode::Split, gitgpui_core::domain::DiffLineKind::Remove) => {
            (text.to_string(), String::new())
        }
        (DiffViewMode::Split, gitgpui_core::domain::DiffLineKind::Add) => {
            (String::new(), text.to_string())
        }
        (DiffViewMode::Split, gitgpui_core::domain::DiffLineKind::Context) => {
            (text.to_string(), text.to_string())
        }
        (DiffViewMode::Split, _) => (text.to_string(), String::new()),
        (DiffViewMode::Inline, _) => (text.to_string(), String::new()),
    };

    let row = div()
        .id(ix)
        .h(px(20.0))
        .flex()
        .items_center()
        .bg(bg)
        .font_family("monospace")
        .text_xs()
        .child(
            div()
                .w(px(44.0))
                .px_2()
                .text_color(gutter_fg)
                .whitespace_nowrap()
                .child(old),
        )
        .child(
            div()
                .w(px(44.0))
                .px_2()
                .text_color(gutter_fg)
                .whitespace_nowrap()
                .child(new),
        );

    match mode {
        DiffViewMode::Inline => row
            .child(
                div()
                    .flex_1()
                    .px_2()
                    .text_color(fg)
                    .whitespace_nowrap()
                    .child(left_text),
            )
            .into_any_element(),
        DiffViewMode::Split => row
            .child(
                div()
                    .flex_1()
                    .px_2()
                    .text_color(fg)
                    .whitespace_nowrap()
                    .child(left_text),
            )
            .child(
                div()
                    .flex_1()
                    .px_2()
                    .text_color(fg)
                    .whitespace_nowrap()
                    .child(right_text),
            )
            .into_any_element(),
    }
}

fn diff_line_colors(
    theme: AppTheme,
    kind: gitgpui_core::domain::DiffLineKind,
) -> (gpui::Rgba, gpui::Rgba, gpui::Rgba) {
    use gitgpui_core::domain::DiffLineKind::*;

    match (theme.is_dark, kind) {
        (_, Header) => (
            theme.colors.surface_bg,
            theme.colors.text_muted,
            theme.colors.text_muted,
        ),
        (_, Hunk) => (
            theme.colors.surface_bg_elevated,
            theme.colors.accent,
            theme.colors.text_muted,
        ),
        (true, Add) => (gpui::rgb(0x0B2E1C), gpui::rgb(0xBBF7D0), gpui::rgb(0x86EFAC)),
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
            theme.colors.surface_bg_elevated,
            theme.colors.text,
            theme.colors.text_muted,
        ),
    }
}
