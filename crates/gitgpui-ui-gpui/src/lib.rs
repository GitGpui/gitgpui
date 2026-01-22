mod app;
mod assets;
mod kit;
mod theme;
mod view;
mod zed_port;

pub use app::run;

#[cfg(feature = "bench")]
pub mod benchmarks {
    pub use crate::view::rows::benchmarks::*;
}

#[cfg(test)]
mod smoke_tests;
