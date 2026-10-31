//! The app's tracing setup, run once by `main` after the helper modes have exited: warnings and
//! errors from every thread go to `errors.log` in the logs folder (rotated by size, opened on
//! its first write), and `RUST_LOG` controls what goes to stderr. Panics on any thread are
//! logged with their thread, message, location and backtrace before the default hook runs.
//! Extended logging raises the error log's level at runtime through `LogSetup::file_detail`.

use crate::runner::{FileDetail, LogSetup};
use glowfly_core::logs::{ERROR_FILE, ERROR_LOG_BYTES, KEEP_ROTATED, RotatingFile};
use std::sync::{Arc, Mutex};
use tracing::Level;
use tracing_subscriber::filter::{EnvFilter, Targets};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, reload};

/// Installs the tracing subscriber and the panic hook.
pub fn init() -> LogSetup {
    let dir = glowfly_core::settings::log_dir();
    let mut file_detail: Option<FileDetail> = None;
    // First, so its reloadable filter's subscriber is the registry itself.
    let file_layer = dir.as_ref().map(|dir| {
        let (filter, handle) = reload::Layer::new(file_targets(false));
        file_detail = Some(Arc::new(move |on| {
            let _ = handle.reload(file_targets(on));
        }));
        let writer = Mutex::new(RotatingFile::new(dir.join(ERROR_FILE), ERROR_LOG_BYTES, KEEP_ROTATED));
        fmt::layer()
            .with_ansi(false)
            .with_thread_names(true)
            .with_timer(fmt::time::ChronoLocal::new("%Y-%m-%d %H:%M:%S%.3f %:z".into()))
            .with_writer(writer)
            .with_filter(filter)
    });
    // RUST_LOG=glowfly=debug shows background effects as they run, and how long each effect and
    // each phase of a repository load or an index build took (a line as its span closes);
    // glowfly=trace adds every frame, its phases and each message's handling.
    let stderr = fmt::layer()
        .with_writer(std::io::stderr)
        .with_span_events(fmt::format::FmtSpan::CLOSE)
        .with_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "glowfly=info".into()));
    tracing_subscriber::registry().with(file_layer).with(stderr).init();
    install_panic_hook();
    LogSetup { dir, file_detail }
}

/// What the error log takes: warnings and errors, or with extended logging Glowfly's debug
/// messages and everything else's informational ones.
pub fn file_targets(extended: bool) -> Targets {
    if extended {
        Targets::new().with_target("glowfly", Level::DEBUG).with_target("glowfly_core", Level::DEBUG).with_default(Level::INFO)
    } else {
        Targets::new().with_default(Level::WARN)
    }
}

/// Logs every panic to the error log, then runs the hook that was there before (which prints it
/// to stderr).
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("unnamed");
        let backtrace = std::backtrace::Backtrace::force_capture();
        tracing::error!(target: "glowfly::panic", "thread '{name}' {info}\nbacktrace:\n{backtrace}");
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_targets_levels() {
        let normal = file_targets(false);
        assert!(normal.would_enable("glowfly", &Level::WARN));
        assert!(normal.would_enable("glowfly::runner", &Level::ERROR));
        assert!(!normal.would_enable("glowfly", &Level::INFO));
        let extended = file_targets(true);
        assert!(extended.would_enable("glowfly", &Level::DEBUG));
        assert!(extended.would_enable("glowfly_core::gitcli", &Level::DEBUG));
        assert!(!extended.would_enable("glowfly", &Level::TRACE));
        assert!(extended.would_enable("wgpu_core", &Level::INFO));
        assert!(!extended.would_enable("wgpu_core", &Level::DEBUG));
    }
}
