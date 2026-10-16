//! The app's tracing setup, run once by `main` after the helper modes have exited: warnings and
//! errors from every thread go to `errors.log` in the logs folder (rotated by size, opened on
//! its first write), and `RUST_LOG` controls what goes to stderr. Panics on any thread are
//! logged with their thread, message, location and backtrace before the default hook runs.
//! Extended logging raises the error log's level at runtime through `LogSetup::file_detail`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::Level;
use tracing_subscriber::filter::{EnvFilter, Targets};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, reload};
use trellis_core::logs::{ERROR_FILE, ERROR_LOG_BYTES, KEEP_ROTATED, RotatingFile};

/// Switches the error log between its normal and extended levels.
pub type FileDetail = Arc<dyn Fn(bool) + Send + Sync>;

/// What `init` set up, for the runner's log worker.
pub struct LogSetup {
    /// The logs folder; None when this system has no place for it (logs stay in memory).
    pub dir: Option<PathBuf>,
    /// Sets the error log's level for extended logging (true) or not; None without a folder.
    pub file_detail: Option<FileDetail>,
}

/// Installs the tracing subscriber and the panic hook.
pub fn init() -> LogSetup {
    let dir = trellis_core::settings::log_dir();
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
    // RUST_LOG=trellis=debug shows background effects as they run.
    let stderr =
        fmt::layer().with_writer(std::io::stderr).with_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "trellis=info".into()));
    tracing_subscriber::registry().with(file_layer).with(stderr).init();
    install_panic_hook();
    LogSetup { dir, file_detail }
}

/// What the error log takes: warnings and errors, or with extended logging Trellis's debug
/// messages and everything else's informational ones.
pub fn file_targets(extended: bool) -> Targets {
    if extended {
        Targets::new().with_target("trellis", Level::DEBUG).with_target("trellis_core", Level::DEBUG).with_default(Level::INFO)
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
        tracing::error!(target: "trellis::panic", "thread '{name}' {info}\nbacktrace:\n{backtrace}");
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_targets_levels() {
        let normal = file_targets(false);
        assert!(normal.would_enable("trellis", &Level::WARN));
        assert!(normal.would_enable("trellis::runner", &Level::ERROR));
        assert!(!normal.would_enable("trellis", &Level::INFO));
        let extended = file_targets(true);
        assert!(extended.would_enable("trellis", &Level::DEBUG));
        assert!(extended.would_enable("trellis_core::gitcli", &Level::DEBUG));
        assert!(!extended.would_enable("trellis", &Level::TRACE));
        assert!(extended.would_enable("wgpu_core", &Level::INFO));
        assert!(!extended.would_enable("wgpu_core", &Level::DEBUG));
    }
}
