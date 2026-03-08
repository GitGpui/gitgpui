use super::*;

const MIN_GIT_MAJOR: u32 = 2;
const MIN_GIT_MINOR: u32 = 53;
const GITHUB_URL: &str = "https://github.com/Auto-Explore/GitComet";
const LICENSE_URL: &str = "https://github.com/Auto-Explore/GitComet/blob/main/LICENSE-AGPL-3.0";
const LICENSE_NAME: &str = "AGPL-3.0";

#[derive(Clone, Debug)]
pub(super) struct SettingsRuntimeInfo {
    pub(super) git: GitRuntimeInfo,
    pub(super) operating_system: SharedString,
    pub(super) github_url: SharedString,
    pub(super) license_url: SharedString,
}

#[derive(Clone, Debug)]
pub(super) struct GitRuntimeInfo {
    pub(super) version_display: SharedString,
    pub(super) compatibility: GitCompatibility,
    pub(super) detail: Option<SharedString>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GitCompatibility {
    Supported,
    TooOld,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GitVersion {
    major: u32,
    minor: u32,
    patch: Option<u32>,
}

impl SettingsRuntimeInfo {
    pub(super) fn detect() -> Self {
        Self {
            git: detect_git_runtime_info(),
            operating_system: format!(
                "{} ({}, {})",
                std::env::consts::OS,
                std::env::consts::FAMILY,
                std::env::consts::ARCH
            )
            .into(),
            github_url: GITHUB_URL.into(),
            license_url: LICENSE_URL.into(),
        }
    }
}

fn detect_git_runtime_info() -> GitRuntimeInfo {
    let tested_only_message = format!(
        "GitComet has been tested only with Git {MIN_GIT_MAJOR}.{MIN_GIT_MINOR}. \
         Please use Git {MIN_GIT_MAJOR}.{MIN_GIT_MINOR} or newer."
    );

    match std::process::Command::new("git").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version_output = if !output.stdout.is_empty() {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            } else {
                String::from_utf8_lossy(&output.stderr).trim().to_string()
            };

            if version_output.is_empty() {
                return GitRuntimeInfo {
                    version_display: "Unavailable".into(),
                    compatibility: GitCompatibility::Unknown,
                    detail: Some(tested_only_message.into()),
                };
            }

            let compatibility = match parse_git_version(&version_output) {
                Some(version) if is_supported_git_version(version) => GitCompatibility::Supported,
                Some(_) => GitCompatibility::TooOld,
                None => GitCompatibility::Unknown,
            };

            let detail = match compatibility {
                GitCompatibility::Supported => None,
                GitCompatibility::TooOld | GitCompatibility::Unknown => {
                    Some(tested_only_message.into())
                }
            };

            GitRuntimeInfo {
                version_display: version_output.into(),
                compatibility,
                detail,
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let display = if stderr.is_empty() {
                format!("Unavailable (exit code: {})", output.status)
            } else {
                format!("Unavailable ({stderr})")
            };
            GitRuntimeInfo {
                version_display: display.into(),
                compatibility: GitCompatibility::Unknown,
                detail: Some(tested_only_message.into()),
            }
        }
        Err(err) => GitRuntimeInfo {
            version_display: format!("Unavailable ({err})").into(),
            compatibility: GitCompatibility::Unknown,
            detail: Some(tested_only_message.into()),
        },
    }
}

fn parse_git_version(raw: &str) -> Option<GitVersion> {
    raw.split_whitespace().find_map(parse_git_version_token)
}

fn parse_git_version_token(token: &str) -> Option<GitVersion> {
    let mut parts = token.split('.');
    let major = parse_u32_prefix(parts.next()?)?;
    let minor = parse_u32_prefix(parts.next()?)?;
    let patch = parts.next().and_then(parse_u32_prefix);
    Some(GitVersion {
        major,
        minor,
        patch,
    })
}

fn parse_u32_prefix(part: &str) -> Option<u32> {
    let end = part
        .char_indices()
        .find_map(|(ix, ch)| (!ch.is_ascii_digit()).then_some(ix))
        .unwrap_or(part.len());
    if end == 0 {
        return None;
    }
    part[..end].parse::<u32>().ok()
}

fn is_supported_git_version(version: GitVersion) -> bool {
    version.major > MIN_GIT_MAJOR
        || (version.major == MIN_GIT_MAJOR && version.minor >= MIN_GIT_MINOR)
}

pub(super) fn panel(this: &mut PopoverHost, cx: &mut gpui::Context<PopoverHost>) -> gpui::Div {
    let theme = this.theme;
    let current_format = this.date_time_format;
    let current_timezone = this.timezone;
    let show_timezone = this.show_timezone;
    let runtime = &this.settings_runtime_info;
    let (
        conflict_enable_whitespace_autosolve,
        conflict_enable_regex_autosolve,
        conflict_enable_history_autosolve,
    ) = this
        .main_pane
        .read(cx)
        .conflict_advanced_autosolve_settings();
    let preview_now = std::time::SystemTime::now();

    let row = |id: &'static str, label: &'static str, value: SharedString, open: bool| {
        div()
            .id(id)
            .px_2()
            .py_1()
            .flex()
            .items_center()
            .justify_between()
            .rounded(px(theme.radii.row))
            .hover(move |s| s.bg(theme.colors.hover))
            .active(move |s| s.bg(theme.colors.active))
            .cursor(CursorStyle::PointingHand)
            .child(div().text_sm().child(label))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_sm()
                    .text_color(theme.colors.text_muted)
                    .child(value)
                    .child(
                        div()
                            .font_family("monospace")
                            .child(if open { "▴" } else { "▾" }),
                    ),
            )
    };

    let toggle_row = |id: &'static str, label: &'static str, enabled: bool| {
        div()
            .id(id)
            .px_2()
            .py_1()
            .flex()
            .items_center()
            .justify_between()
            .rounded(px(theme.radii.row))
            .hover(move |s| s.bg(theme.colors.hover))
            .active(move |s| s.bg(theme.colors.active))
            .cursor(CursorStyle::PointingHand)
            .child(div().text_sm().child(label))
            .child(
                div()
                    .text_sm()
                    .text_color(if enabled {
                        theme.colors.success
                    } else {
                        theme.colors.text_muted
                    })
                    .child(if enabled { "On" } else { "Off" }),
            )
    };

    let info_row = |id: &'static str, label: &'static str, value: SharedString| {
        div()
            .id(id)
            .px_2()
            .py_1()
            .flex()
            .items_center()
            .justify_between()
            .rounded(px(theme.radii.row))
            .child(div().text_sm().child(label))
            .child(
                div()
                    .text_sm()
                    .font_family("monospace")
                    .text_color(theme.colors.text_muted)
                    .child(value),
            )
    };

    // --- Date format dropdown ---
    let mut date_dropdown = div().flex().flex_col().gap_1().px_2().pb_2();

    if this.settings_date_format_open {
        for fmt in DateTimeFormat::all() {
            let selected = *fmt == current_format;
            let fmt_val = *fmt;
            let preview: SharedString =
                format_datetime(preview_now, fmt_val, current_timezone, show_timezone).into();
            date_dropdown = date_dropdown.child(
                div()
                    .id(("settings_date_format_item", *fmt as usize))
                    .px_2()
                    .py_1()
                    .rounded(px(theme.radii.row))
                    .when(!selected, |d| {
                        d.hover(move |s| s.bg(theme.colors.hover))
                            .active(move |s| s.bg(theme.colors.active))
                    })
                    .when(selected, |d| d.bg(with_alpha(theme.colors.accent, 0.15)))
                    .cursor(CursorStyle::PointingHand)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(div().text_sm().child(fmt.label()))
                            .child(
                                div()
                                    .font_family("monospace")
                                    .text_xs()
                                    .text_color(theme.colors.text_muted)
                                    .child(preview),
                            ),
                    )
                    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                        this.settings_date_format_open = false;
                        this.set_date_time_format(fmt_val, cx);
                        cx.notify();
                    })),
            );
        }
    }

    // --- Timezone dropdown ---
    let mut tz_dropdown = div().flex().flex_col().gap_1().px_2().pb_2();

    if this.settings_timezone_open {
        for tz in Timezone::all() {
            let selected = *tz == current_timezone;
            let tz_val = *tz;
            let preview: SharedString =
                format_datetime(preview_now, current_format, tz_val, show_timezone).into();
            tz_dropdown = tz_dropdown.child(
                div()
                    .id(SharedString::from(format!(
                        "settings_tz_item_{}",
                        tz.offset_seconds()
                    )))
                    .px_2()
                    .py_1()
                    .rounded(px(theme.radii.row))
                    .when(!selected, |d| {
                        d.hover(move |s| s.bg(theme.colors.hover))
                            .active(move |s| s.bg(theme.colors.active))
                    })
                    .when(selected, |d| d.bg(with_alpha(theme.colors.accent, 0.15)))
                    .cursor(CursorStyle::PointingHand)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(div().text_sm().child(tz.label()))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.colors.text_muted)
                                            .child(tz.cities()),
                                    ),
                            )
                            .child(
                                div()
                                    .font_family("monospace")
                                    .text_xs()
                                    .text_color(theme.colors.text_muted)
                                    .child(preview),
                            ),
                    )
                    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                        this.settings_timezone_open = false;
                        this.set_timezone(tz_val, cx);
                        cx.notify();
                    })),
            );
        }
    }

    let header = div()
        .px_2()
        .py_1()
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .child("Settings");

    let section_label = div()
        .px_2()
        .pt(px(6.0))
        .pb(px(4.0))
        .text_xs()
        .text_color(theme.colors.text_muted)
        .child("General");

    let date_row = row(
        "settings_date_format",
        "Date format",
        current_format.label().into(),
        this.settings_date_format_open,
    )
    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
        this.settings_date_format_open = !this.settings_date_format_open;
        this.settings_timezone_open = false;
        cx.notify();
    }));

    let tz_row = row(
        "settings_timezone",
        "Date timezone",
        current_timezone.label().into(),
        this.settings_timezone_open,
    )
    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
        this.settings_timezone_open = !this.settings_timezone_open;
        this.settings_date_format_open = false;
        cx.notify();
    }));

    let show_timezone_row = toggle_row("settings_show_timezone", "Show timezone", show_timezone)
        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
            this.set_show_timezone(!this.show_timezone, cx);
            cx.notify();
        }));

    let mut content = div()
        .flex()
        .flex_col()
        .min_w(px(560.0))
        .max_w(px(720.0))
        .child(header)
        .child(div().border_t_1().border_color(theme.colors.border))
        .child(section_label)
        .child(
            div()
                .px_2()
                .pb_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(date_row)
                .child(tz_row)
                .child(show_timezone_row),
        );

    let conflict_section_label = div()
        .px_2()
        .pt(px(6.0))
        .pb(px(4.0))
        .text_xs()
        .text_color(theme.colors.text_muted)
        .child("Conflict resolver");

    let whitespace_row = toggle_row(
        "settings_conflict_whitespace_autosolve",
        "Auto-resolve whitespace-only",
        conflict_enable_whitespace_autosolve,
    )
    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
        let (enabled, _, _) = this
            .main_pane
            .read(cx)
            .conflict_advanced_autosolve_settings();
        this.set_conflict_enable_whitespace_autosolve(!enabled, cx);
        cx.notify();
    }));

    let regex_row = toggle_row(
        "settings_conflict_regex_autosolve",
        "Enable regex auto-resolve",
        conflict_enable_regex_autosolve,
    )
    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
        let (_, enabled, _) = this
            .main_pane
            .read(cx)
            .conflict_advanced_autosolve_settings();
        this.set_conflict_enable_regex_autosolve(!enabled, cx);
        cx.notify();
    }));

    let history_row = toggle_row(
        "settings_conflict_history_autosolve",
        "Enable history auto-resolve",
        conflict_enable_history_autosolve,
    )
    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
        let (_, _, enabled) = this
            .main_pane
            .read(cx)
            .conflict_advanced_autosolve_settings();
        this.set_conflict_enable_history_autosolve(!enabled, cx);
        cx.notify();
    }));

    content = content.child(conflict_section_label).child(
        div()
            .px_2()
            .pb_1()
            .flex()
            .flex_col()
            .gap_1()
            .child(whitespace_row)
            .child(regex_row)
            .child(history_row),
    );

    let environment_section_label = div()
        .px_2()
        .pt(px(6.0))
        .pb(px(4.0))
        .text_xs()
        .text_color(theme.colors.text_muted)
        .child("Environment");

    let (git_icon, git_icon_color, git_status_text): (&'static str, gpui::Rgba, SharedString) =
        match runtime.git.compatibility {
            GitCompatibility::Supported => ("✓", theme.colors.success, "Git >= 2.53".into()),
            GitCompatibility::TooOld => ("⚠", theme.colors.warning, "Git < 2.53".into()),
            GitCompatibility::Unknown => ("⚠", theme.colors.warning, "Git version unknown".into()),
        };

    let git_row = div()
        .id("settings_git_version")
        .px_2()
        .py_1()
        .flex()
        .items_center()
        .justify_between()
        .rounded(px(theme.radii.row))
        .child(div().text_sm().child("Git"))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .font_family("monospace")
                        .text_sm()
                        .text_color(git_icon_color)
                        .child(git_icon),
                )
                .child(
                    div()
                        .font_family("monospace")
                        .text_sm()
                        .text_color(theme.colors.text_muted)
                        .child(runtime.git.version_display.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(git_icon_color)
                        .child(git_status_text),
                ),
        );

    let os_row = info_row(
        "settings_os_info",
        "Operating system",
        runtime.operating_system.clone(),
    );
    let github_row = div()
        .id("settings_github_link")
        .px_2()
        .py_1()
        .flex()
        .items_center()
        .justify_between()
        .rounded(px(theme.radii.row))
        .hover(move |s| s.bg(theme.colors.hover))
        .active(move |s| s.bg(theme.colors.active))
        .cursor(CursorStyle::PointingHand)
        .child(div().text_sm().child("GitHub"))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_sm()
                .text_color(theme.colors.accent)
                .child(runtime.github_url.clone())
                .child(div().font_family("monospace").child("↗")),
        )
        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
            let url = this.settings_runtime_info.github_url.clone().to_string();
            match this.open_external_url(&url) {
                Ok(()) => this.push_toast(
                    components::ToastKind::Success,
                    "Opened GitHub repository in your browser.".to_string(),
                    cx,
                ),
                Err(err) => this.push_toast(
                    components::ToastKind::Error,
                    format!("Failed to open browser: {err}"),
                    cx,
                ),
            }
            cx.notify();
        }));

    let license_row = div()
        .id("settings_license_link")
        .px_2()
        .py_1()
        .flex()
        .items_center()
        .justify_between()
        .rounded(px(theme.radii.row))
        .hover(move |s| s.bg(theme.colors.hover))
        .active(move |s| s.bg(theme.colors.active))
        .cursor(CursorStyle::PointingHand)
        .child(div().text_sm().child("License"))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_sm()
                .text_color(theme.colors.accent)
                .child(LICENSE_NAME)
                .child(div().font_family("monospace").child("↗")),
        )
        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
            let url = this.settings_runtime_info.license_url.clone().to_string();
            match this.open_external_url(&url) {
                Ok(()) => this.push_toast(
                    components::ToastKind::Success,
                    "Opened license in your browser.".to_string(),
                    cx,
                ),
                Err(err) => this.push_toast(
                    components::ToastKind::Error,
                    format!("Failed to open browser: {err}"),
                    cx,
                ),
            }
            cx.notify();
        }));

    let open_source_licenses_row = div()
        .id("settings_open_source_licenses")
        .px_2()
        .py_1()
        .flex()
        .items_center()
        .justify_between()
        .rounded(px(theme.radii.row))
        .hover(move |s| s.bg(theme.colors.hover))
        .active(move |s| s.bg(theme.colors.active))
        .cursor(CursorStyle::PointingHand)
        .child(div().text_sm().child("Open source licenses"))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_sm()
                .text_color(theme.colors.accent)
                .child("Show")
                .child(div().font_family("monospace").child("↗")),
        )
        .on_click(cx.listener(|this, _e: &ClickEvent, window, cx| {
            this.settings_date_format_open = false;
            this.settings_timezone_open = false;
            this.open_popover_at(
                PopoverKind::OpenSourceLicenses,
                crate::view::chrome::window_top_left_corner(window),
                window,
                cx,
            );
        }));

    content = content.child(environment_section_label).child(
        div()
            .px_2()
            .pb_1()
            .flex()
            .flex_col()
            .gap_1()
            .child(git_row)
            .child(os_row)
            .child(github_row)
            .child(license_row)
            .child(open_source_licenses_row),
    );

    if let Some(detail) = runtime.git.detail.clone() {
        content = content.child(
            div()
                .px_2()
                .pb_1()
                .text_xs()
                .text_color(theme.colors.warning)
                .child(detail),
        );
    }

    if this.settings_date_format_open {
        content = content
            .child(
                div()
                    .px_2()
                    .pb_1()
                    .text_xs()
                    .text_color(theme.colors.text_muted)
                    .child("Choose a format:"),
            )
            .child(date_dropdown);
    }

    if this.settings_timezone_open {
        content = content
            .child(
                div()
                    .px_2()
                    .pb_1()
                    .text_xs()
                    .text_color(theme.colors.text_muted)
                    .child("Choose a timezone:"),
            )
            .child(tz_dropdown);
    }

    components::context_menu(theme, content)
}

#[cfg(test)]
mod tests {
    use super::{
        GitVersion, MIN_GIT_MAJOR, MIN_GIT_MINOR, is_supported_git_version, parse_git_version,
    };

    #[test]
    fn parse_git_version_extracts_semver_from_standard_output() {
        let parsed = parse_git_version("git version 2.53.1").expect("parsed");
        assert_eq!(
            parsed,
            GitVersion {
                major: 2,
                minor: 53,
                patch: Some(1)
            }
        );
    }

    #[test]
    fn parse_git_version_handles_windows_suffix_output() {
        let parsed = parse_git_version("git version 2.45.1.windows.1").expect("parsed");
        assert_eq!(
            parsed,
            GitVersion {
                major: 2,
                minor: 45,
                patch: Some(1)
            }
        );
    }

    #[test]
    fn supported_version_requires_minimum_2_53() {
        assert!(is_supported_git_version(GitVersion {
            major: MIN_GIT_MAJOR,
            minor: MIN_GIT_MINOR,
            patch: Some(0)
        }));
        assert!(!is_supported_git_version(GitVersion {
            major: MIN_GIT_MAJOR,
            minor: MIN_GIT_MINOR - 1,
            patch: Some(9)
        }));
        assert!(is_supported_git_version(GitVersion {
            major: MIN_GIT_MAJOR + 1,
            minor: 0,
            patch: Some(0)
        }));
    }
}
