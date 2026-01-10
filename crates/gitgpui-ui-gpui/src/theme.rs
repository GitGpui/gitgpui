use gpui::Rgba;

#[derive(Clone, Copy)]
pub struct AppTheme {
    pub colors: Colors,
    pub radii: Radii,
}

#[derive(Clone, Copy)]
pub struct Colors {
    pub window_bg: Rgba,
    pub surface_bg: Rgba,
    pub surface_bg_elevated: Rgba,
    pub border: Rgba,
    pub text: Rgba,
    pub text_muted: Rgba,
    pub accent: Rgba,
    pub hover: Rgba,
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
    pub fn dark_default() -> Self {
        Self {
            colors: Colors {
                window_bg: gpui::rgb(0x0E1116),
                surface_bg: gpui::rgb(0x111827),
                surface_bg_elevated: gpui::rgb(0x0B1220),
                border: gpui::rgb(0x1F2A37),
                text: gpui::rgb(0xE5E7EB),
                text_muted: gpui::rgb(0x94A3B8),
                accent: gpui::rgb(0x60A5FA),
                hover: gpui::rgb(0x172033),
                danger: gpui::rgb(0xF87171),
                warning: gpui::rgb(0xFBBF24),
                success: gpui::rgb(0x34D399),
            },
            radii: Radii {
                panel: 10.0,
                pill: 999.0,
                row: 8.0,
            },
        }
    }

    /// Zed's "One Dark" theme (ported from `zed/assets/themes/one/one.json`).
    pub fn zed_one_dark() -> Self {
        Self {
            colors: Colors {
                // editor.background
                window_bg: gpui::rgba(0x282c33ff),
                // panel.background / surface.background
                surface_bg: gpui::rgba(0x2f343eff),
                // elevated_surface.background
                surface_bg_elevated: gpui::rgba(0x2f343eff),
                // border.variant
                border: gpui::rgba(0x363c46ff),
                // text
                text: gpui::rgba(0xdce0e5ff),
                // text.muted
                text_muted: gpui::rgba(0xa9afbcff),
                // text.accent
                accent: gpui::rgba(0x74ade8ff),
                // element.hover
                hover: gpui::rgba(0x363c46ff),
                // terminal.ansi.red
                danger: gpui::rgba(0xe06c75ff),
                // terminal.ansi.yellow
                warning: gpui::rgba(0xe5c07bff),
                // terminal.ansi.green
                success: gpui::rgba(0x98c379ff),
            },
            radii: Radii {
                panel: 8.0,
                pill: 999.0,
                row: 6.0,
            },
        }
    }
}
