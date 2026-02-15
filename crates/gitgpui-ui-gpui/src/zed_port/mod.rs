//! Minimal Zed UI ports.
//!
//! These components are adapted from Zed's GPL-licensed UI implementation
//! and trimmed to fit GitGpui's smaller codebase.

mod button;
mod components;
mod context_menu;
mod diff_stat;
mod picker_prompt;
mod split_button;
mod tab;
mod tab_bar;
mod toast;
mod tokens;

pub use button::{Button, ButtonStyle};
pub use components::{empty_state, key_value_monospace_value, split_columns_header};
#[cfg(test)]
pub use components::{panel, pill};
pub use context_menu::{
    context_menu, context_menu_entry, context_menu_header, context_menu_label,
    context_menu_separator,
};
pub use diff_stat::diff_stat;
pub use picker_prompt::PickerPrompt;
pub use split_button::{SplitButton, SplitButtonStyle};
pub use tab::{Tab, TabPosition};
pub use tab_bar::TabBar;
pub use toast::{ToastKind, toast};
pub use tokens::*;

// Re-exports for "Zed surface area" consistency within this repo.
pub use crate::kit::{
    Scrollbar, ScrollbarMarker, ScrollbarMarkerKind, TextInput, TextInputOptions,
};
