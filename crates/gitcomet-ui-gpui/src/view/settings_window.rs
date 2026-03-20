use super::*;
use gitcomet_core::process::configure_background_command;
use gpui::{Stateful, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions};

const SETTINGS_WINDOW_MIN_WIDTH_PX: f32 = 620.0;
const SETTINGS_WINDOW_MIN_HEIGHT_PX: f32 = 460.0;
const SETTINGS_WINDOW_DEFAULT_WIDTH_PX: f32 = 720.0;
const SETTINGS_WINDOW_DEFAULT_HEIGHT_PX: f32 = 620.0;
const SETTINGS_TRAFFIC_LIGHTS_SAFE_INSET: Pixels = px(78.0);
const MIN_GIT_MAJOR: u32 = 2;
const MIN_GIT_MINOR: u32 = 50;
const GITHUB_URL: &str = "https://github.com/Auto-Explore/GitComet";
const LICENSE_URL: &str = "https://github.com/Auto-Explore/GitComet/blob/main/LICENSE-AGPL-3.0";
const LICENSE_NAME: &str = "AGPL-3.0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SettingsSection {
    Theme,
    DateFormat,
    Timezone,
}

#[derive(Clone, Debug)]
struct SettingsRuntimeInfo {
    git: GitRuntimeInfo,
    app_version_display: SharedString,
    operating_system: SharedString,
}

#[derive(Clone, Debug)]
struct GitRuntimeInfo {
    version_display: SharedString,
    compatibility: GitCompatibility,
    detail: Option<SharedString>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GitCompatibility {
    Supported,
    TooOld,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GitVersion {
    major: u32,
    minor: u32,
}

pub(crate) struct SettingsWindowView {
    theme_mode: ThemeMode,
    theme: AppTheme,
    date_time_format: DateTimeFormat,
    timezone: Timezone,
    show_timezone: bool,
    runtime_info: SettingsRuntimeInfo,
    expanded_section: Option<SettingsSection>,
    _appearance_subscription: gpui::Subscription,
}

pub(crate) fn open_settings_window(cx: &mut App) {
    if let Some(window) = cx
        .windows()
        .into_iter()
        .find_map(|window| window.downcast::<SettingsWindowView>())
    {
        let _ = window.update(cx, |_view, window, _cx| {
            window.activate_window();
        });
        return;
    }

    let bounds = Bounds::centered(
        None,
        size(
            px(SETTINGS_WINDOW_DEFAULT_WIDTH_PX),
            px(SETTINGS_WINDOW_DEFAULT_HEIGHT_PX),
        ),
        cx,
    );
    let window_decorations = if cfg!(target_os = "macos") {
        WindowDecorations::Client
    } else {
        WindowDecorations::Server
    };

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(
                px(SETTINGS_WINDOW_MIN_WIDTH_PX),
                px(SETTINGS_WINDOW_MIN_HEIGHT_PX),
            )),
            titlebar: Some(TitlebarOptions {
                title: Some("Settings GitComet".into()),
                appears_transparent: cfg!(target_os = "macos"),
                traffic_light_position: cfg!(target_os = "macos")
                    .then_some(point(px(9.0), px(9.0))),
            }),
            app_id: Some("gitcomet-settings".into()),
            window_decorations: Some(window_decorations),
            is_movable: true,
            is_resizable: true,
            ..Default::default()
        },
        |window, cx| cx.new(|cx| SettingsWindowView::new(window, cx)),
    )
    .expect("failed to open settings window");

    cx.activate(true);
}

impl SettingsWindowView {
    fn new(window: &mut Window, cx: &mut gpui::Context<Self>) -> Self {
        let ui_session = session::load();
        let theme_mode = ui_session
            .theme_mode
            .as_deref()
            .and_then(ThemeMode::from_key)
            .unwrap_or_default();
        let date_time_format = ui_session
            .date_time_format
            .as_deref()
            .and_then(DateTimeFormat::from_key)
            .unwrap_or(DateTimeFormat::YmdHm);
        let timezone = ui_session
            .timezone
            .as_deref()
            .and_then(Timezone::from_key)
            .unwrap_or_default();
        let show_timezone = ui_session.show_timezone.unwrap_or(true);
        let theme = theme_mode.resolve_theme(window.appearance());

        let appearance_subscription = {
            let view = cx.weak_entity();
            let mut first = true;
            window.observe_window_appearance(move |window, app| {
                if first {
                    first = false;
                    return;
                }

                let _ = view.update(app, |this, cx| {
                    if this.theme_mode != ThemeMode::Automatic {
                        return;
                    }
                    this.theme = this.theme_mode.resolve_theme(window.appearance());
                    cx.notify();
                });
            })
        };

        Self {
            theme_mode,
            theme,
            date_time_format,
            timezone,
            show_timezone,
            runtime_info: SettingsRuntimeInfo::detect(),
            expanded_section: None,
            _appearance_subscription: appearance_subscription,
        }
    }

    fn toggle_section(&mut self, section: SettingsSection, cx: &mut gpui::Context<Self>) {
        self.expanded_section = if self.expanded_section == Some(section) {
            None
        } else {
            Some(section)
        };
        cx.notify();
    }

    fn persist_preferences(&self, cx: &mut gpui::Context<Self>) {
        let settings = session::UiSettings {
            window_width: None,
            window_height: None,
            sidebar_width: None,
            details_width: None,
            theme_mode: Some(self.theme_mode.key().to_string()),
            date_time_format: Some(self.date_time_format.key().to_string()),
            timezone: Some(self.timezone.key()),
            show_timezone: Some(self.show_timezone),
            history_show_author: None,
            history_show_date: None,
            history_show_sha: None,
        };

        cx.background_spawn(async move {
            let _ = session::persist_ui_settings(settings);
        })
        .detach();
    }

    fn update_main_windows(
        &self,
        cx: &mut gpui::Context<Self>,
        f: impl FnMut(&mut GitCometView, &mut Window, &mut gpui::Context<GitCometView>) + 'static,
    ) {
        let handles: Vec<_> = cx
            .windows()
            .into_iter()
            .filter_map(|window| window.downcast::<GitCometView>())
            .collect();
        cx.spawn(
            async move |_view: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let _ = cx.update(move |cx| {
                    let mut f = f;
                    for handle in handles {
                        let _ = handle.update(cx, |view, window, cx| f(view, window, cx));
                    }
                });
            },
        )
        .detach();
    }

    fn set_theme_mode(
        &mut self,
        mode: ThemeMode,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.theme_mode == mode {
            return;
        }

        self.theme_mode = mode;
        self.theme = mode.resolve_theme(window.appearance());
        self.expanded_section = None;
        self.persist_preferences(cx);
        self.update_main_windows(cx, move |view, root_window, cx| {
            view.popover_host.update(cx, |host, cx| {
                host.set_theme_mode(mode, root_window.appearance(), cx);
            });
        });
        cx.notify();
    }

    fn set_date_time_format(&mut self, format: DateTimeFormat, cx: &mut gpui::Context<Self>) {
        if self.date_time_format == format {
            return;
        }

        self.date_time_format = format;
        self.expanded_section = None;
        self.persist_preferences(cx);
        self.update_main_windows(cx, move |view, _window, cx| {
            view.popover_host.update(cx, |host, cx| {
                host.set_date_time_format(format, cx);
            });
        });
        cx.notify();
    }

    fn set_timezone(&mut self, timezone: Timezone, cx: &mut gpui::Context<Self>) {
        if self.timezone == timezone {
            return;
        }

        self.timezone = timezone;
        self.expanded_section = None;
        self.persist_preferences(cx);
        self.update_main_windows(cx, move |view, _window, cx| {
            view.popover_host.update(cx, |host, cx| {
                host.set_timezone(timezone, cx);
            });
        });
        cx.notify();
    }

    fn set_show_timezone(&mut self, enabled: bool, cx: &mut gpui::Context<Self>) {
        if self.show_timezone == enabled {
            return;
        }

        self.show_timezone = enabled;
        self.persist_preferences(cx);
        self.update_main_windows(cx, move |view, _window, cx| {
            view.popover_host.update(cx, |host, cx| {
                host.set_show_timezone(enabled, cx);
            });
        });
        cx.notify();
    }

    fn option_row(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        detail: Option<SharedString>,
        selected: bool,
        theme: AppTheme,
    ) -> Stateful<gpui::Div> {
        let id: SharedString = id.into();
        let text_color = if selected {
            theme.colors.text
        } else {
            theme.colors.text_muted
        };
        let selected_bg = with_alpha(theme.colors.accent, if theme.is_dark { 0.16 } else { 0.10 });
        let hover_bg = with_alpha(theme.colors.text, if theme.is_dark { 0.08 } else { 0.05 });
        let active_bg = with_alpha(theme.colors.text, if theme.is_dark { 0.12 } else { 0.08 });

        div()
            .id(id)
            .w_full()
            .px_2()
            .py_1()
            .flex()
            .items_start()
            .gap_2()
            .rounded(px(theme.radii.row))
            .cursor(CursorStyle::PointingHand)
            .bg(if selected {
                selected_bg
            } else {
                gpui::rgba(0x00000000)
            })
            .hover(move |s| {
                if selected {
                    s.bg(selected_bg)
                } else {
                    s.bg(hover_bg)
                }
            })
            .active(move |s| {
                if selected {
                    s.bg(selected_bg)
                } else {
                    s.bg(active_bg)
                }
            })
            .child(
                div()
                    .w(px(16.0))
                    .text_sm()
                    .font_family(UI_MONOSPACE_FONT_FAMILY)
                    .text_color(if selected {
                        theme.colors.accent
                    } else {
                        theme.colors.text_muted
                    })
                    .child(if selected { ">" } else { " " }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(div().text_sm().text_color(text_color).child(label.into()))
                    .when_some(detail, |this, detail| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(theme.colors.text_muted)
                                .child(detail),
                        )
                    }),
            )
    }

    fn summary_row(
        &self,
        id: &'static str,
        label: &'static str,
        value: SharedString,
        expanded: bool,
        theme: AppTheme,
    ) -> Stateful<gpui::Div> {
        div()
            .id(id)
            .w_full()
            .px_2()
            .py_1()
            .flex()
            .items_center()
            .justify_between()
            .rounded(px(theme.radii.row))
            .cursor(CursorStyle::PointingHand)
            .hover(move |s| s.bg(theme.colors.hover))
            .active(move |s| s.bg(theme.colors.active))
            .child(div().text_sm().child(label))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .text_color(theme.colors.text_muted)
                    .child(value)
                    .child(
                        div()
                            .font_family(UI_MONOSPACE_FONT_FAMILY)
                            .child(if expanded { "^" } else { "v" }),
                    ),
            )
    }

    fn toggle_row(
        &self,
        id: &'static str,
        label: &'static str,
        enabled: bool,
        theme: AppTheme,
    ) -> Stateful<gpui::Div> {
        div()
            .id(id)
            .w_full()
            .px_2()
            .py_1()
            .flex()
            .items_center()
            .justify_between()
            .rounded(px(theme.radii.row))
            .cursor(CursorStyle::PointingHand)
            .hover(move |s| s.bg(theme.colors.hover))
            .active(move |s| s.bg(theme.colors.active))
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
    }

    fn info_row(
        &self,
        id: &'static str,
        label: &'static str,
        value: SharedString,
        theme: AppTheme,
    ) -> Stateful<gpui::Div> {
        div()
            .id(id)
            .w_full()
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
                    .font_family(UI_MONOSPACE_FONT_FAMILY)
                    .text_color(theme.colors.text_muted)
                    .child(value),
            )
    }

    fn link_row(
        &self,
        id: &'static str,
        label: &'static str,
        value: SharedString,
        theme: AppTheme,
    ) -> Stateful<gpui::Div> {
        div()
            .id(id)
            .w_full()
            .px_2()
            .py_1()
            .flex()
            .items_center()
            .justify_between()
            .rounded(px(theme.radii.row))
            .cursor(CursorStyle::PointingHand)
            .hover(move |s| s.bg(theme.colors.hover))
            .active(move |s| s.bg(theme.colors.active))
            .child(div().text_sm().child(label))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .text_color(theme.colors.accent)
                    .child(value)
                    .child(div().font_family(UI_MONOSPACE_FONT_FAMILY).child("->")),
            )
    }

    fn card(&self, id: &'static str, title: &'static str, theme: AppTheme) -> Stateful<gpui::Div> {
        div()
            .id(id)
            .w_full()
            .flex()
            .flex_col()
            .rounded(px(theme.radii.panel))
            .border_1()
            .border_color(theme.colors.border)
            .bg(theme.colors.surface_bg_elevated)
            .p_2()
            .gap_1()
            .child(
                div()
                    .px_2()
                    .pb_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.colors.text_muted)
                    .child(title),
            )
    }
}

impl Render for SettingsWindowView {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let header_bg = if window.is_window_active() {
            with_alpha(
                theme.colors.surface_bg,
                if theme.is_dark { 0.98 } else { 0.94 },
            )
        } else {
            theme.colors.surface_bg
        };

        let header = div()
            .id("settings_window_header")
            .h(chrome::TITLE_BAR_HEIGHT)
            .w_full()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(theme.colors.border)
            .bg(header_bg)
            .window_control_area(WindowControlArea::Drag)
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .px_3()
                    .when(cfg!(target_os = "macos"), |this| {
                        this.pl(SETTINGS_TRAFFIC_LIGHTS_SAFE_INSET)
                    })
                    .child(
                        div()
                            .overflow_hidden()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .whitespace_nowrap()
                            .child("Settings GitComet"),
                    ),
            );

        let theme_row = self
            .summary_row(
                "settings_window_theme",
                "Theme",
                self.theme_mode.label().into(),
                self.expanded_section == Some(SettingsSection::Theme),
                theme,
            )
            .on_click(cx.listener(|this, _e: &ClickEvent, _window, cx| {
                this.toggle_section(SettingsSection::Theme, cx);
            }));

        let date_format_row = self
            .summary_row(
                "settings_window_date_format",
                "Date format",
                self.date_time_format.label().into(),
                self.expanded_section == Some(SettingsSection::DateFormat),
                theme,
            )
            .on_click(cx.listener(|this, _e: &ClickEvent, _window, cx| {
                this.toggle_section(SettingsSection::DateFormat, cx);
            }));

        let timezone_row = self
            .summary_row(
                "settings_window_timezone",
                "Date timezone",
                self.timezone.label().into(),
                self.expanded_section == Some(SettingsSection::Timezone),
                theme,
            )
            .on_click(cx.listener(|this, _e: &ClickEvent, _window, cx| {
                this.toggle_section(SettingsSection::Timezone, cx);
            }));

        let show_timezone_row = self
            .toggle_row(
                "settings_window_show_timezone",
                "Show timezone",
                self.show_timezone,
                theme,
            )
            .on_click(cx.listener(|this, _e: &ClickEvent, _window, cx| {
                this.set_show_timezone(!this.show_timezone, cx);
            }));

        let mut general_card = self
            .card("settings_window_general", "General", theme)
            .child(theme_row);

        if self.expanded_section == Some(SettingsSection::Theme) {
            for option in [ThemeMode::Automatic, ThemeMode::Light, ThemeMode::Dark] {
                general_card = general_card.child(
                    self.option_row(
                        match option {
                            ThemeMode::Automatic => "settings_window_theme_auto",
                            ThemeMode::Light => "settings_window_theme_light",
                            ThemeMode::Dark => "settings_window_theme_dark",
                        },
                        option.label(),
                        None,
                        self.theme_mode == option,
                        theme,
                    )
                    .on_click(cx.listener(
                        move |this, _e: &ClickEvent, window, cx| {
                            this.set_theme_mode(option, window, cx);
                        },
                    )),
                );
            }
        }

        general_card = general_card.child(date_format_row);
        if self.expanded_section == Some(SettingsSection::DateFormat) {
            for format in DateTimeFormat::all().iter().copied() {
                general_card = general_card.child(
                    self.option_row(
                        match format {
                            DateTimeFormat::YmdHm => "settings_window_date_format_ymd_hm",
                            DateTimeFormat::YmdHms => "settings_window_date_format_ymd_hms",
                            DateTimeFormat::DmyHm => "settings_window_date_format_dmy_hm",
                            DateTimeFormat::MdyHm => "settings_window_date_format_mdy_hm",
                        },
                        format.label(),
                        None,
                        self.date_time_format == format,
                        theme,
                    )
                    .on_click(cx.listener(
                        move |this, _e: &ClickEvent, _window, cx| {
                            this.set_date_time_format(format, cx);
                        },
                    )),
                );
            }
        }

        general_card = general_card.child(timezone_row);
        if self.expanded_section == Some(SettingsSection::Timezone) {
            for timezone in Timezone::all().iter().copied() {
                general_card = general_card.child(
                    self.option_row(
                        format!("settings_window_timezone_{}", timezone.key()),
                        timezone.label(),
                        Some(timezone.cities().into()),
                        self.timezone == timezone,
                        theme,
                    )
                    .on_click(cx.listener(
                        move |this, _e: &ClickEvent, _window, cx| {
                            this.set_timezone(timezone, cx);
                        },
                    )),
                );
            }
        }

        general_card = general_card.child(show_timezone_row);

        let min_git_version = format!("{MIN_GIT_MAJOR}.{MIN_GIT_MINOR}");
        let (git_icon_path, git_icon_color, git_status_text): (
            &'static str,
            gpui::Rgba,
            SharedString,
        ) = match self.runtime_info.git.compatibility {
            GitCompatibility::Supported => (
                "icons/check.svg",
                theme.colors.success,
                format!("Git >= {min_git_version}").into(),
            ),
            GitCompatibility::TooOld => (
                "icons/warning.svg",
                theme.colors.warning,
                format!("Git < {min_git_version}").into(),
            ),
            GitCompatibility::Unknown => (
                "icons/warning.svg",
                theme.colors.warning,
                "Git version unknown".into(),
            ),
        };

        let mut environment_card = self
            .card("settings_window_environment", "Environment", theme)
            .child(
                div()
                    .id("settings_window_git")
                    .w_full()
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
                            .child(svg_icon(git_icon_path, git_icon_color, px(14.0)))
                            .child(
                                div()
                                    .text_sm()
                                    .font_family(UI_MONOSPACE_FONT_FAMILY)
                                    .text_color(theme.colors.text_muted)
                                    .child(self.runtime_info.git.version_display.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(git_icon_color)
                                    .child(git_status_text),
                            ),
                    ),
            )
            .child(self.info_row(
                "settings_window_build",
                "Build",
                self.runtime_info.app_version_display.clone(),
                theme,
            ))
            .child(self.info_row(
                "settings_window_os",
                "Operating system",
                self.runtime_info.operating_system.clone(),
                theme,
            ));

        if let Some(detail) = self.runtime_info.git.detail.clone() {
            environment_card = environment_card.child(
                div()
                    .px_2()
                    .pt_1()
                    .text_xs()
                    .text_color(theme.colors.text_muted)
                    .child(detail),
            );
        }

        let links_card = self
            .card("settings_window_links", "Links", theme)
            .child(
                self.link_row("settings_window_github", "GitHub", GITHUB_URL.into(), theme)
                    .on_click(|_, _, cx| {
                        cx.open_url(GITHUB_URL);
                    }),
            )
            .child(
                self.link_row(
                    "settings_window_license",
                    "License",
                    LICENSE_NAME.into(),
                    theme,
                )
                .on_click(|_, _, cx| {
                    cx.open_url(LICENSE_URL);
                }),
            );

        let content = div()
            .id("settings_window_content")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.colors.window_bg)
            .text_color(theme.colors.text)
            .child(header)
            .child(
                div()
                    .id("settings_window_scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_3()
                    .child(general_card)
                    .child(environment_card)
                    .child(links_card),
            );

        if cfg!(target_os = "macos") {
            window_frame(
                theme,
                window.window_decorations(),
                content.into_any_element(),
            )
        } else {
            content.into_any_element()
        }
    }
}

impl SettingsRuntimeInfo {
    fn detect() -> Self {
        Self {
            git: detect_git_runtime_info(),
            app_version_display: format!("GitComet v{}", env!("CARGO_PKG_VERSION")).into(),
            operating_system: format!(
                "{} ({}, {})",
                std::env::consts::OS,
                std::env::consts::FAMILY,
                std::env::consts::ARCH
            )
            .into(),
        }
    }
}

fn detect_git_runtime_info() -> GitRuntimeInfo {
    let compatibility_message =
        format!("GitComet has been tested only with Git {MIN_GIT_MAJOR}.{MIN_GIT_MINOR} or newer.");

    let mut command = std::process::Command::new("git");
    configure_background_command(&mut command);
    match command.arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version_output = if !output.stdout.is_empty() {
                bytes_to_text_preserving_utf8(&output.stdout)
                    .trim()
                    .to_string()
            } else {
                bytes_to_text_preserving_utf8(&output.stderr)
                    .trim()
                    .to_string()
            };

            if version_output.is_empty() {
                return GitRuntimeInfo {
                    version_display: "Unavailable".into(),
                    compatibility: GitCompatibility::Unknown,
                    detail: Some(compatibility_message.into()),
                };
            }

            let compatibility = match parse_git_version(&version_output) {
                Some(version) if is_supported_git_version(version) => GitCompatibility::Supported,
                Some(_) => GitCompatibility::TooOld,
                None => GitCompatibility::Unknown,
            };

            GitRuntimeInfo {
                version_display: version_output.into(),
                compatibility,
                detail: match compatibility {
                    GitCompatibility::Supported => None,
                    GitCompatibility::TooOld | GitCompatibility::Unknown => {
                        Some(compatibility_message.into())
                    }
                },
            }
        }
        Ok(output) => {
            let stderr = bytes_to_text_preserving_utf8(&output.stderr)
                .trim()
                .to_string();
            let display = if stderr.is_empty() {
                format!("Unavailable (exit code: {})", output.status)
            } else {
                format!("Unavailable ({stderr})")
            };
            GitRuntimeInfo {
                version_display: display.into(),
                compatibility: GitCompatibility::Unknown,
                detail: Some(compatibility_message.into()),
            }
        }
        Err(err) => GitRuntimeInfo {
            version_display: format!("Unavailable ({err})").into(),
            compatibility: GitCompatibility::Unknown,
            detail: Some(compatibility_message.into()),
        },
    }
}

fn bytes_to_text_preserving_utf8(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut out = String::with_capacity(bytes.len());
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        match std::str::from_utf8(&bytes[cursor..]) {
            Ok(valid) => {
                out.push_str(valid);
                break;
            }
            Err(err) => {
                let valid_len = err.valid_up_to();
                if valid_len > 0 {
                    let valid = &bytes[cursor..cursor + valid_len];
                    out.push_str(
                        std::str::from_utf8(valid)
                            .expect("slice identified by valid_up_to must be valid UTF-8"),
                    );
                    cursor += valid_len;
                }

                let invalid_len = err.error_len().unwrap_or(1);
                let invalid_end = cursor.saturating_add(invalid_len).min(bytes.len());
                for byte in &bytes[cursor..invalid_end] {
                    let _ = write!(out, "\\x{byte:02x}");
                }
                cursor = invalid_end;
            }
        }
    }

    out
}

fn parse_git_version(raw: &str) -> Option<GitVersion> {
    raw.split_whitespace().find_map(parse_git_version_token)
}

fn parse_git_version_token(token: &str) -> Option<GitVersion> {
    let mut parts = token.split('.');
    let major = parse_u32_prefix(parts.next()?)?;
    let minor = parse_u32_prefix(parts.next()?)?;
    Some(GitVersion { major, minor })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::lock_visual_test;
    use gitcomet_core::error::{Error, ErrorKind};
    use gitcomet_core::services::{GitBackend, GitRepository, Result};
    use std::path::Path;

    struct TestBackend;

    impl GitBackend for TestBackend {
        fn open(&self, _workdir: &Path) -> Result<std::sync::Arc<dyn GitRepository>> {
            Err(Error::new(ErrorKind::Unsupported(
                "Test backend does not open repositories",
            )))
        }
    }

    #[test]
    fn bytes_to_text_preserving_utf8_escapes_invalid_bytes() {
        assert_eq!(
            bytes_to_text_preserving_utf8(b"ok\xff\xfeend"),
            "ok\\xff\\xfeend"
        );
    }

    #[test]
    fn parse_git_version_extracts_first_version_token() {
        assert_eq!(
            parse_git_version("git version 2.50.7"),
            Some(GitVersion {
                major: 2,
                minor: 50
            })
        );
    }

    #[test]
    fn parse_git_version_token_accepts_numeric_prefixes_and_rejects_non_numeric_prefixes() {
        assert_eq!(
            parse_git_version_token("2.45.1.windows.1"),
            Some(GitVersion {
                major: 2,
                minor: 45
            })
        );
        assert_eq!(parse_git_version_token("v2.45.1"), None);
        assert_eq!(parse_u32_prefix("53rc1"), Some(53));
        assert_eq!(parse_u32_prefix("rc53"), None);
    }

    #[test]
    fn supported_version_requires_minimum_2_50() {
        assert!(is_supported_git_version(GitVersion {
            major: MIN_GIT_MAJOR,
            minor: MIN_GIT_MINOR,
        }));
        assert!(is_supported_git_version(GitVersion {
            major: MIN_GIT_MAJOR,
            minor: MIN_GIT_MINOR + 1,
        }));
        assert!(!is_supported_git_version(GitVersion {
            major: MIN_GIT_MAJOR,
            minor: MIN_GIT_MINOR - 1,
        }));
        assert!(is_supported_git_version(GitVersion {
            major: MIN_GIT_MAJOR + 1,
            minor: 0,
        }));
    }

    #[gpui::test]
    fn show_timezone_toggle_defers_main_window_update(cx: &mut gpui::TestAppContext) {
        let _visual_guard = lock_visual_test();
        let (store, events) = AppStore::new(std::sync::Arc::new(TestBackend));
        let (main_view, cx) =
            cx.add_window_view(|window, cx| GitCometView::new(store, events, None, window, cx));

        cx.update(|window, app| {
            let _ = window.draw(app);
            open_settings_window(app);
        });
        cx.run_until_parked();

        let settings_window = cx.update(|_window, app| {
            app.windows()
                .into_iter()
                .find_map(|window| window.downcast::<SettingsWindowView>())
                .expect("settings window should be open")
        });

        let next_show_timezone = cx.update(|_window, app| {
            !settings_window
                .read_with(app, |settings, _cx| settings.show_timezone)
                .expect("settings window should be readable")
        });

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cx.update(|_window, app| {
                main_view.update(app, |_view, cx| {
                    let _ = settings_window.update(cx, |settings, _window, cx| {
                        settings.set_show_timezone(next_show_timezone, cx);
                    });
                });
            });
        }));
        assert!(
            result.is_ok(),
            "settings window toggle should not re-enter GitCometView updates"
        );

        cx.run_until_parked();

        cx.update(|_window, app| {
            assert_eq!(
                main_view.read(app).show_timezone_for_test(),
                next_show_timezone
            );
            assert_eq!(
                settings_window
                    .read_with(app, |settings, _cx| settings.show_timezone)
                    .expect("settings window should remain readable"),
                next_show_timezone
            );
        });
    }
}
