mod app;
mod assets;
pub mod focused_diff;
pub mod focused_merge;
mod kit;
mod theme;
mod view;
mod zed_port;

pub use app::run;
pub use focused_diff::{FocusedDiffConfig, run_focused_diff};
pub use focused_merge::{FocusedMergeConfig, run_focused_merge};

#[doc(hidden)]
pub mod benchmarks {
    pub use crate::view::rows::benchmarks::*;
}

#[cfg(test)]
mod smoke_tests;
