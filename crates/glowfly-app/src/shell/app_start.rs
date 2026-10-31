//! Starting the app once the state and the runner exist: the startup steps in order (a test
//! app leaves out those reaching outside the process), with a settings file the load couldn't
//! read held before the first of them, since some save settings.

use super::GlowflyApp;
use crate::msg::{Msg, PrefsMsg, RecentMsg};
use crate::ui::layout;
use eframe::egui;
use glowfly_core::settings::{LoadProblem, ThemeCatalog};

impl GlowflyApp {
    /// Runs the `STARTUP` steps in order: all with `logs` (the real app); a test app skips
    /// those reaching outside the process.
    pub(super) fn start(
        &mut self,
        ctx: &egui::Context,
        themes: ThemeCatalog,
        logs: Option<crate::runner::LogSetup>,
        problem: Option<LoadProblem>,
    ) {
        let outside = logs.is_some();
        self.run_steps(ctx, steps(outside), themes, logs, problem);
    }

    /// Runs `steps` in order. A settings file left unread (`problem`) holds saving back before
    /// the first (some save) and is reported after the last.
    fn run_steps(
        &mut self,
        ctx: &egui::Context,
        steps: impl IntoIterator<Item = Step>,
        themes: ThemeCatalog,
        logs: Option<crate::runner::LogSetup>,
        problem: Option<LoadProblem>,
    ) {
        self.st.settings_file.hold(problem.as_ref());
        let mut logs = logs;
        let mut themes = Some(themes);
        for step in steps {
            match step {
                Step::Theme => {
                    if let Some(themes) = themes.take() {
                        self.dispatch(ctx, Msg::Theme(crate::themes::ThemeMsg::Loaded(themes)));
                    }
                }
                Step::Layout => layout::start(ctx, &self.st),
                // git and ssh ask for missing credentials through a dialog from now on.
                Step::Askpass => self.runner.start_askpass(),
                // The log files, and earlier sessions' activity.
                Step::Logs => {
                    if let Some(logs) = logs.take() {
                        self.runner.attach_logs(logs);
                        self.dispatch(ctx, Msg::Logs(crate::logs::LogsMsg::Start));
                    }
                }
                Step::IndexStore => self.runner.attach_index_store(glowfly_core::commit_index::Store::default_dir()),
                // Links opened while this window runs come here instead of a new window.
                Step::Instance => self.runner.start_instance(),
                Step::OpenEvents => self.runner.start_open_events(),
                Step::Lfs => self.dispatch(ctx, Msg::Lfs(crate::lfs::LfsMsg::Detect)),
                Step::GitEnv => self.dispatch(ctx, Msg::GitEnv(crate::git_env::GitEnvMsg::Detect)),
                Step::Onboarding => self.dispatch(ctx, Msg::Onboarding(crate::onboarding::OnboardingMsg::Startup)),
                Step::Updates => self.dispatch(ctx, Msg::Updates(crate::msg::UpdateMsg::Startup)),
                Step::RecentFolders => self.dispatch(ctx, Msg::Recent(RecentMsg::CheckFolders)),
            }
        }
        if let Some(problem) = problem {
            self.dispatch(ctx, Msg::Prefs(PrefsMsg::SettingsUnreadable(problem)));
        }
    }
}

/// One step of starting the app, after the state and the runner exist.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
    /// The themes read from disk, so the first frame has their colors.
    Theme,
    /// The zoom keys handed to the keymap, and the saved zoom.
    Layout,
    Askpass,
    Logs,
    /// The search index's saved copies in the cache folder.
    IndexStore,
    Instance,
    /// Folders and links the OS hands over (macOS Apple events), including those that came
    /// before the window.
    OpenEvents,
    Lfs,
    GitEnv,
    /// The welcome's first step.
    Onboarding,
    Updates,
    /// Which recent folders are still there.
    RecentFolders,
}

/// Startup, in order; a test app runs the same order minus the `outside` steps.
const STARTUP: [Step; 12] = [
    Step::Theme,
    Step::Layout,
    Step::Askpass,
    Step::Logs,
    Step::IndexStore,
    Step::Instance,
    Step::OpenEvents,
    Step::Lfs,
    Step::GitEnv,
    Step::Onboarding,
    Step::Updates,
    Step::RecentFolders,
];

impl Step {
    /// Whether the step starts a server or a worker, leads to a network call, or touches
    /// process-wide state (the OS events' inbox).
    fn outside(self) -> bool {
        matches!(self, Step::Askpass | Step::Logs | Step::IndexStore | Step::Instance | Step::OpenEvents | Step::Updates)
    }
}

/// The startup steps to run: all of them, or (`outside` false) only those inside the process.
fn steps(outside: bool) -> impl Iterator<Item = Step> {
    STARTUP.into_iter().filter(move |s| outside || !s.outside())
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::settings::Settings;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    /// A test app saving into `files`, run through `steps` with `problem`, then left until its
    /// effects are done.
    fn run(files: &Path, steps: &[Step], problem: Option<LoadProblem>) {
        let ctx = egui::Context::default();
        // A new install's notes aren't recorded yet: the Updates step records them, and saves.
        let settings = Settings { welcome_done: true, ..Settings::default() };
        let folder = files.to_owned();
        let mut app =
            GlowflyApp::assemble(&ctx, settings, Vec::new(), move |tx, waker| crate::runner::testing::sandboxed(tx, waker, folder));
        app.run_steps(&ctx, steps.iter().copied(), ThemeCatalog::default(), None, problem);
        let began = Instant::now();
        while !app.runner.in_flight_names().is_empty() {
            assert!(began.elapsed() < Duration::from_secs(20), "still running: {:?}", app.runner.in_flight_names());
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn folder(name: &str) -> PathBuf {
        let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-start-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn an_unread_settings_file_is_held_before_the_steps_that_save() {
        // The release notes part of Updates saves settings, so it writes the file normally.
        let plain = folder("plain");
        run(&plain, &[Step::Updates], None);
        assert!(plain.join("settings.json").exists(), "the step saves");
        // With the file held from the start, nothing reaches it.
        let held = folder("held");
        let file = held.join("settings.json");
        let problem = LoadProblem { backup: None, reset: None, detail: "access is denied".into(), locked: Some(file.clone()) };
        // Every step a test app runs, and Updates.
        let steps: Vec<Step> = steps(false).chain([Step::Updates]).collect();
        run(&held, &steps, Some(problem));
        assert!(!file.exists(), "no step saved the defaults over it");
        let _ = std::fs::remove_dir_all(&plain);
        let _ = std::fs::remove_dir_all(&held);
    }

    #[test]
    fn startup_keeps_the_app_order_and_a_test_app_skips_only_outside_steps() {
        use Step::*;
        let app = [Theme, Layout, Askpass, Logs, IndexStore, Instance, OpenEvents, Lfs, GitEnv, Onboarding, Updates, RecentFolders];
        assert_eq!(steps(true).collect::<Vec<_>>(), app);
        assert_eq!(steps(false).collect::<Vec<_>>(), [Theme, Layout, Lfs, GitEnv, Onboarding, RecentFolders]);
    }
}
