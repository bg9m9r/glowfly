//! The whole app for headless tests (`src/journey/`): built from a context and a `TestStart`
//! instead of eframe's creation context, with a runner that keeps everything inside the test's
//! folders, and accessors that tell a test when the app has gone idle.

use super::GlowflyApp;
use eframe::egui;
use glowfly_core::settings::{LoadProblem, RepoTrust, Settings, ThemeCatalog};
use glowfly_model::msg::{Msg, RepoMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::window_chrome::Chrome;
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
    /// Repositories opened as tabs before `open`, so `open` ends up the active tab.
    pub also_open: Vec<PathBuf>,
    /// The window frame to draw from the first frame on, as `Chrome::current()` picks it.
    pub chrome: Chrome,
    /// What the settings load reported, as if the file couldn't be read.
    pub problem: Option<LoadProblem>,
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
        let TestStart { mut settings, files, trusted, open, also_open, chrome, problem } = start;
        for path in &trusted {
            glowfly_model::state::trust::record(&mut settings, path, RepoTrust::Trusted);
        }
        let mut app = Self::assemble(ctx, settings, Vec::new(), |tx, waker| glowfly_runner::testing::sandboxed(tx, waker, files));
        app.st.cues.chrome = chrome;
        app.start(ctx, ThemeCatalog::default(), None, problem);
        for path in also_open {
            app.dispatch(ctx, Msg::Repo(RepoMsg::Open(path)));
        }
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

    /// The names of the effects whose worker panicked, in order.
    #[expect(dead_code, reason = "the exploration tests read it from task 8 on")]
    pub(crate) fn panicked_effects(&self) -> Vec<&'static str> {
        self.runner.panicked()
    }

    /// The names of the effects the sandbox dropped (network, OS shell, global config, …).
    pub(crate) fn dropped_effects(&self) -> Vec<&'static str> {
        self.runner.dropped()
    }
}
