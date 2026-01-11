//! Minimal Zed UI ports.
//!
//! These components are adapted from Zed's GPL-licensed UI implementation
//! and trimmed to fit GitGpui's smaller codebase.

mod button;
mod split_button;
mod tab;
mod tab_bar;

pub use button::{Button, ButtonStyle};
pub use split_button::{SplitButton, SplitButtonStyle};
#[allow(unused_imports)]
pub use tab::{Tab, TabCloseSide, TabPosition};
pub use tab_bar::TabBar;
