//! The whole app for headless tests (`src/journey/`): built from a context and a `TestStart`
//! instead of eframe's creation context, with a runner that keeps everything inside the test's
//! folders, and accessors that tell a test when the app has gone idle.

use super::GlowflyApp;
use crate::msg::{Msg, RepoMsg};
use crate::runner::Runner;
use crate::state::AppState;
use crate::state::window_chrome::Chrome;
use eframe::egui;
use glowfly_core::settings::{RepoTrust, Settings, ThemeCatalog};
use std::path::PathBuf;

/// How a test app starts.
pub(crate) struct TestStart {
    pub settings: Settings,
    /// The per-test config folder: settings.json, recent.txt and tabs.txt are written here.
    pub files: PathBuf,
    /// Repositories recorded as trusted before the app starts, so the trust prompt never shows.
    pub trusted: Vec<PathBuf>,
    /// The repository to open, like a path on the command line.
    pub open: Option<PathBuf>,
    /// The window frame to draw from the first frame on, as `Chrome::current()` picks it.
    pub chrome: Chrome,
}

/// What the app is doing, for a test waiting until it is idle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Activity {
    /// Effects the runner has started so far.
    pub started: u64,
    /// Effects still running or queued.
    pub in_flight: usize,
    /// Results taken from the runner's channel so far.
    pub drained: u64,
    /// A job is running or a repository is loading.
    pub busy: bool,
}

impl GlowflyApp {
    /// The app as `GlowflyApp::new` builds it, minus everything that reaches outside the test:
    /// no settings, recent, tabs or theme files are read, the saved git isn't set, and the
    /// askpass server, the single-instance listener, the log worker and the update check never
    /// start. The runner is sandboxed (see `runner/sandbox.rs`).
    pub(crate) fn for_test(ctx: &egui::Context, start: TestStart) -> Self {
        let TestStart { mut settings, files, trusted, open, chrome } = start;
        for path in &trusted {
            crate::state::trust::record(&mut settings, path, RepoTrust::Trusted);
        }
        let mut app = Self::assemble(ctx, settings, Vec::new(), |tx, waker| Runner::sandboxed(tx, waker, files));
        app.st.view.chrome = chrome;
        app.start(ctx, ThemeCatalog::default(), None);
        if let Some(path) = open {
            app.dispatch(ctx, Msg::Repo(RepoMsg::Open(path)));
        }
        app
    }

    pub(crate) fn state(&self) -> &AppState {
        &self.st
    }

    pub(crate) fn activity(&self) -> Activity {
        Activity {
            started: self.runner.started(),
            in_flight: self.runner.in_flight_names().len(),
            drained: self.drained,
            busy: self.st.job.is_some() || self.st.loading,
        }
    }

    /// The names of the effects still running or queued.
    pub(crate) fn in_flight_names(&self) -> Vec<&'static str> {
        self.runner.in_flight_names()
    }

    /// The names of the effects the sandbox dropped (network, OS shell, global config, …).
    pub(crate) fn dropped_effects(&self) -> Vec<&'static str> {
        self.runner.dropped()
    }
}
