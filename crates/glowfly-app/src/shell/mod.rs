//! The shell, which stays in the binary: `GlowflyApp` owns the state, the UI-thread caches and
//! the runner, feeds the views' messages and the runner's results through `update`, and does what
//! needs the window (`app.rs` and its parts); it alone talks to the window's OS glue
//! (`crate::platform`). The views it draws are `crate::ui`.

mod app;
mod app_menu;
mod app_native;
pub use app::GlowflyApp;
#[cfg(test)]
pub(crate) use app::TestStart;
