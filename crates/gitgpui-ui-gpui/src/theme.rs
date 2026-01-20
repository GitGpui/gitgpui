use gpui::Rgba;
use gpui::WindowAppearance;

#[derive(Clone, Copy)]
pub struct AppTheme {
    pub is_dark: bool,
    pub colors: Colors,
    pub radii: Radii,
}

#[derive(Clone, Copy)]
pub struct Colors {
    pub window_bg: Rgba,
    pub surface_bg: Rgba,
    pub surface_bg_elevated: Rgba,
    pub active_section: Rgba,
    pub border: Rgba,
    pub text: Rgba,
    pub text_muted: Rgba,
    pub accent: Rgba,
    pub hover: Rgba,
    pub active: Rgba,
    pub focus_ring: Rgba,
    pub focus_ring_bg: Rgba,
    pub scrollbar_thumb: Rgba,
    pub scrollbar_thumb_hover: Rgba,
    pub scrollbar_thumb_active: Rgba,
    pub danger: Rgba,
    pub warning: Rgba,
    pub success: Rgba,
}

#[derive(Clone, Copy)]
pub struct Radii {
    pub panel: f32,
    pub pill: f32,
    pub row: f32,
}

impl AppTheme {
    pub fn default_for_window_appearance(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::zed_one_light(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::zed_ayu_dark(),
        }
    }

    pub fn dark_default() -> Self {
        let accent = gpui::rgb(0x60A5FA);
        let hover = gpui::rgb(0x172033);
        let text_muted = gpui::rgb(0x94A3B8);
        let scrollbar_thumb = with_alpha(text_muted, 0.30);
        Self {
            is_dark: true,
            colors: Colors {
                window_bg: gpui::rgb(0x0E1116),
                surface_bg: gpui::rgb(0x111827),
                surface_bg_elevated: gpui::rgb(0x0B1220),
                active_section: hover,
                border: gpui::rgb(0x1F2A37),
                text: gpui::rgb(0xE5E7EB),
                text_muted,
                accent,
                hover,
                active: with_alpha(hover, 0.78),
                focus_ring: with_alpha(accent, 0.58),
                focus_ring_bg: with_alpha(accent, 0.16),
                scrollbar_thumb,
                scrollbar_thumb_hover: with_alpha(text_muted, 0.42),
                scrollbar_thumb_active: with_alpha(text_muted, 0.52),
                danger: gpui::rgb(0xF87171),
                warning: gpui::rgb(0xFBBF24),
                success: gpui::rgb(0x34D399),
            },
            radii: Radii {
                panel: 8.0,
                pill: 999.0,
                row: 6.0,
            },
        }
    }

    /// Zed's "One Dark" theme (ported from `zed/assets/themes/one/one.json`).
    pub fn zed_one_dark() -> Self {
        let accent = gpui::rgba(0x74ade8ff);
        let hover = gpui::rgba(0x363c46ff);
        let text_muted = gpui::rgba(0xa9afbcff);
        Self {
            is_dark: true,
            colors: Colors {
                // editor.background
                window_bg: gpui::rgba(0x282c33ff),
                // panel.background / surface.background
                surface_bg: gpui::rgba(0x2f343eff),
                // elevated_surface.background
                surface_bg_elevated: gpui::rgba(0x2f343eff),
                active_section: hover,
                // border.variant
                border: gpui::rgba(0x363c46ff),
                // text
                text: gpui::rgba(0xdce0e5ff),
                // text.muted
                text_muted,
                // text.accent
                accent,
                // element.hover
                hover,
                active: with_alpha(hover, 0.78),
                focus_ring: with_alpha(accent, 0.60),
                focus_ring_bg: with_alpha(accent, 0.16),
                scrollbar_thumb: with_alpha(text_muted, 0.30),
                scrollbar_thumb_hover: with_alpha(text_muted, 0.42),
                scrollbar_thumb_active: with_alpha(text_muted, 0.52),
                // terminal.ansi.red
                danger: gpui::rgba(0xe06c75ff),
                // terminal.ansi.yellow
                warning: gpui::rgba(0xe5c07bff),
                // terminal.ansi.green
                success: gpui::rgba(0x98c379ff),
            },
            radii: Radii {
                panel: 6.0,
                pill: 999.0,
                row: 4.0,
            },
        }
    }

    /// Zed's "Ayu Dark" theme (ported from `zed/assets/themes/ayu/ayu.json`).
    pub fn zed_ayu_dark() -> Self {
        let accent = gpui::rgba(0x5ac1feff);
        let hover = gpui::rgba(0x2d2f34ff);
        let text_muted = gpui::rgba(0x8a8986ff);
        Self {
            is_dark: true,
            colors: Colors {
                // editor.background
                window_bg: gpui::rgba(0x0d1016ff),
                // surface.background
                surface_bg: gpui::rgba(0x1f2127ff),
                // elevated_surface.background
                surface_bg_elevated: gpui::rgba(0x1f2127ff),
                active_section: hover,
                // border.variant
                border: gpui::rgba(0x2d2f34ff),
                // text
                text: gpui::rgba(0xbfbdb6ff),
                // text.muted
                text_muted,
                // text.accent
                accent,
                // element.hover
                hover,
                active: with_alpha(hover, 0.78),
                focus_ring: with_alpha(accent, 0.60),
                focus_ring_bg: with_alpha(accent, 0.16),
                scrollbar_thumb: with_alpha(text_muted, 0.30),
                scrollbar_thumb_hover: with_alpha(text_muted, 0.42),
                scrollbar_thumb_active: with_alpha(text_muted, 0.52),
                // terminal.ansi.red
                danger: gpui::rgba(0xef7177ff),
                // terminal.ansi.yellow
                warning: gpui::rgba(0xfeb454ff),
                // terminal.ansi.green
                success: gpui::rgba(0xaad84cff),
            },
            radii: Radii {
                panel: 6.0,
                pill: 999.0,
                row: 4.0,
            },
        }
    }

    /// Zed's "One Light" theme (ported from `zed/assets/themes/one/one.json`).
    pub fn zed_one_light() -> Self {
        let accent = gpui::rgba(0x5c78e2ff);
        let hover = gpui::rgba(0xdfdfe0ff);
        let text_muted = gpui::rgba(0x58585aff);
        Self {
            is_dark: false,
            colors: Colors {
                // editor.background
                window_bg: gpui::rgba(0xfafafaff),
                // surface.background
                surface_bg: gpui::rgba(0xebebecff),
                // elevated_surface.background
                surface_bg_elevated: gpui::rgba(0xebebecff),
                active_section: gpui::rgba(0xfafafaff),
                // border.variant
                border: gpui::rgba(0xdfdfe0ff),
                // text
                text: gpui::rgba(0x242529ff),
                // text.muted
                text_muted,
                // text.accent
                accent,
                // element.hover
                hover,
                active: with_alpha(hover, 0.88),
                focus_ring: with_alpha(accent, 0.52),
                focus_ring_bg: with_alpha(accent, 0.12),
                scrollbar_thumb: with_alpha(text_muted, 0.26),
                scrollbar_thumb_hover: with_alpha(text_muted, 0.36),
                scrollbar_thumb_active: with_alpha(text_muted, 0.46),
                // terminal.ansi.red
                danger: gpui::rgba(0xde3e35ff),
                // terminal.ansi.yellow
                warning: gpui::rgba(0xd2b67cff),
                // terminal.ansi.green
                success: gpui::rgba(0x3f953aff),
            },
            radii: Radii {
                panel: 6.0,
                pill: 999.0,
                row: 4.0,
            },
        }
    }
}

fn with_alpha(mut color: Rgba, alpha: f32) -> Rgba {
    color.a = alpha;
    color
}
