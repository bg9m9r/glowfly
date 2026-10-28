//! The shell: owns the state, the UI-thread caches and the runner, feeds the views' messages and
//! the runner's results through `update`, runs the resulting effects, and handles the few
//! effects that need the window. On the drawn Windows frame it also trades state with the OS
//! glue each frame (`app_native.rs`), and on macOS it runs the system menu bar's picks and
//! hands it the menus (`app_menu.rs`).

use super::{UiCache, app_menu, app_native, avatars, build_status, layout, shortcuts, theme};
use crate::effect::{Effect, UiEffect};
use crate::msg::AppMsg;
use crate::msg::AvatarMsg;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::msg::RecentMsg;
use crate::msg::RepoMsg;
use crate::msg::TabsMsg;
use crate::platform::native_menu::NativeMenu;
use crate::platform::native_window::NativeWindow;
use crate::runner::Runner;
use crate::state::AppState;
use crate::update::update;
use eframe::egui;
use glowfly_core::settings::{Settings, ThemeCatalog};
use glowfly_core::spell::Speller;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Instant;

#[path = "app_frame.rs"]
mod frame_timing;
#[cfg(test)]
#[path = "app_testing.rs"]
mod testing;
#[cfg(test)]
pub(crate) use testing::TestStart;

pub struct GlowflyApp {
    st: AppState,
    cache: UiCache,
    runner: Runner<egui::Context>,
    results: Receiver<Msg>,
    /// The palette for the chosen theme, set at the start of every frame.
    theme: theme::ThemeCache,
    /// The drawn Windows frame's glue with the OS (Snap Layouts, the system menu, Mica); None
    /// elsewhere and in tests.
    native: Option<NativeWindow>,
    /// The macOS system menu bar showing Glowfly's menus; None elsewhere and in tests.
    menu: Option<NativeMenu>,
    /// The installer checks' state file (`smoke_probe.rs`); None unless asked for, and in
    /// tests.
    probe: Option<crate::smoke_probe::ProbeWriter>,
    /// How long recent frames took, and where this one's time goes (`state/frame_times.rs`).
    frames: crate::state::frame_times::FrameTimes,
    /// Results taken from the runner's channel so far (the journeys wait for this to stop
    /// changing).
    #[cfg(test)]
    drained: u64,
}

impl GlowflyApp {
    /// `mica`: the viewport was made transparent for the Mica backdrop. `probe`: the smoke
    /// tests' state file, when `GLOWFLY_SMOKE_PROBE` asked for one.
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        start: Option<crate::StartArg>,
        logs: crate::logging::LogSetup,
        mica: bool,
        probe: Option<crate::smoke_probe::ProbeWriter>,
    ) -> Self {
        let ctx = &cc.egui_ctx;
        // Before the first pass, so the first frame is drawn in the bundled fonts.
        theme::fonts::install(ctx);
        let (settings, problem) = Settings::load();
        // The git chosen in Preferences > Git runs from the first git call on (no disk access here).
        glowfly_core::git_program::set(glowfly_core::git_program::from_setting(settings.git_program.as_deref()).as_deref());
        let recent = crate::platform::load_recent();
        // Restore the last session's tabs (or, before tabs were saved, the last repository).
        let (mut paths, labels, active) =
            if settings.reopen_last_repo { crate::platform::load_tabs() } else { (Vec::new(), Vec::new(), 0) };
        if paths.is_empty() && settings.reopen_last_repo {
            // Not checked here (a network share may not answer): a folder that is gone fails to
            // load like any other.
            paths.extend(recent.first().cloned());
        }
        let mut app = Self::assemble(ctx, settings, recent, Runner::new);
        // Only the real app draws its own frame where the OS calls for it; test apps keep System.
        app.st.view.chrome = crate::state::window_chrome::Chrome::current();
        app.native = NativeWindow::install(cc, app.st.view.chrome, mica, ctx.clone());
        app.menu = NativeMenu::install(app.st.view.chrome, ctx.clone());
        if probe.is_some() {
            // Before the first frame, so the probe sees named widgets from the start.
            super::probe::enable(ctx);
        }
        app.probe = probe;
        // Read now, not through the runner, so the first frame already has the theme's colors;
        // the folder sits next to the settings file just read.
        let themes = glowfly_core::settings::themes::load_all();
        crate::themes::trace_problems(&themes);
        app.start(ctx, themes, Some(logs));
        if let Some(problem) = problem {
            tracing::warn!("settings: {}", problem.message());
            app.dispatch(ctx, Msg::Prefs(PrefsMsg::SettingsUnreadable(problem.message())));
        }
        if !paths.is_empty() {
            app.dispatch(ctx, Msg::Tabs(TabsMsg::RestoreTabs { paths, labels, active }));
        }
        // Folders and links the OS handed over before the window came reach the results
        // channel at `Step::OpenEvents`, so they are opened on the first frame, after this.
        if let Some(start) = start {
            app.dispatch(ctx, start.into_msg());
        }
        app
    }

    /// The app around `settings` and `recent`, with the runner `make_runner` builds from the
    /// results channel and the waker. Nothing is dispatched yet.
    fn assemble(
        ctx: &egui::Context,
        settings: Settings,
        recent: Vec<PathBuf>,
        make_runner: impl FnOnce(Sender<Msg>, egui::Context) -> Runner<egui::Context>,
    ) -> Self {
        let (tx, results) = mpsc::channel();
        Self {
            st: AppState::new(settings, recent, Instant::now()),
            cache: UiCache {
                avatars: avatars::Avatars::default(),
                speller: Speller::new(),
                diff_rows: Default::default(),
                history_text: Default::default(),
                history_preview: Default::default(),
                blame_rows: Default::default(),
                diff_images: Default::default(),
                markdown: Default::default(),
            },
            runner: make_runner(tx, ctx.clone()),
            results,
            theme: theme::ThemeCache::default(),
            native: None,
            menu: None,
            probe: None,
            frames: Default::default(),
            #[cfg(test)]
            drained: 0,
        }
    }

    /// Runs the `STARTUP` steps in order. With `logs` (the real app) every step runs; without
    /// (a test app) the steps that reach outside the process are skipped.
    fn start(&mut self, ctx: &egui::Context, themes: ThemeCatalog, logs: Option<crate::logging::LogSetup>) {
        let outside = logs.is_some();
        let mut logs = logs;
        let mut themes = Some(themes);
        for step in steps(outside) {
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
    }

    fn dispatch(&mut self, ctx: &egui::Context, msg: Msg) {
        let area = msg.area();
        let _span = tracing::trace_span!("update", area).entered();
        let started = Instant::now();
        let effects = update(&mut self.st, msg, started);
        let checks = crate::update::trust_checks(&mut self.st, &effects);
        for (effect, check) in effects.into_iter().zip(checks) {
            match effect {
                Effect::Ui(UiEffect::CopyText(text)) => ctx.copy_text(text),
                Effect::Ui(UiEffect::SetTitle(title)) => ctx.send_viewport_cmd(egui::ViewportCommand::Title(title)),
                Effect::Ui(UiEffect::RequestAttention) => request_attention(ctx),
                Effect::Ui(UiEffect::CloseWindow) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                // In the pass that saw the close request, so eframe keeps the window.
                Effect::Ui(UiEffect::CancelClose) => ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose),
                Effect::Ui(UiEffect::RequestPaste) => ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste),
                Effect::Ui(UiEffect::Window(cmd)) => match super::window_frame::viewport_cmd(cmd) {
                    Some(c) => ctx.send_viewport_cmd(c),
                    // The system menu, the only window action without a viewport command.
                    None => {
                        if let Some(n) = &self.native {
                            n.show_system_menu_at_cursor();
                        }
                    }
                },
                Effect::Ui(UiEffect::FocusWindow) => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    request_attention(ctx);
                }
                other => self.runner.run_checked(other, check),
            }
        }
        self.frames.area(area, started.elapsed());
    }

    /// Keyboard shortcuts and window events, turned into messages or view-state changes.
    fn input(&mut self, ctx: &egui::Context, msgs: &mut Vec<Msg>) {
        let focused = self.st.view.focused;
        if focused && !self.st.view.was_focused {
            msgs.push(Msg::Repo(RepoMsg::WindowFocused));
        }
        self.st.view.was_focused = focused;
        if ctx.input(|i| i.viewport().close_requested()) {
            msgs.push(Msg::App(AppMsg::CloseRequested));
        }
        shortcuts::input(ctx, &mut self.st, msgs);
    }

    /// One frame: the runner's results, then the window drawn from the state and the
    /// messages it pushed.
    pub(crate) fn frame(&mut self, ctx: &egui::Context) {
        let started = Instant::now();
        let span = tracing::trace_span!("frame").entered();
        self.frames.begin();
        self.theme.apply(ctx, &self.st.settings, &self.st.themes);
        layout::apply_zoom(ctx, &self.st);
        self.st.view.focused = ctx.input(|i| i.focused);
        self.st.view.window = ctx.input(|i| crate::state::window_chrome::WindowInfo {
            maximized: i.viewport().maximized.unwrap_or(false),
            fullscreen: i.viewport().fullscreen.unwrap_or(false),
            ..Default::default()
        });
        let mut native_msgs = Vec::new();
        if let Some(native) = &mut self.native {
            app_native::before_draw(native, &mut self.st, self.theme.mica_tint(), &mut native_msgs);
        }
        if let Some(menu) = &mut self.menu {
            app_menu::picked(menu, &mut self.st, &mut native_msgs);
        }
        self.phase("results", |app| {
            while let Ok(msg) = app.results.try_recv() {
                #[cfg(test)]
                {
                    app.drained += 1;
                }
                app.dispatch(ctx, msg);
            }
        });
        for (email, img) in self.st.view.avatars_loaded.drain(..) {
            self.cache.avatars.insert(ctx, email, img);
        }
        self.st.view.hover_tip = self.st.view.hover_tip_next.take();
        self.st.view.now = chrono::Local::now().timestamp();
        self.st.view.utc_offset = glowfly_core::dates::local_offset();

        let now = Instant::now();
        self.st.view.frame_at = Some(now);
        let mut msgs = vec![Msg::Tick(now)];
        msgs.append(&mut native_msgs);
        self.phase("draw", |app| {
            app.input(ctx, &mut msgs);
            super::draw(ctx, &mut app.st, &mut app.cache, &mut msgs);
            app.draw_overlay(ctx);
        });
        if let Some(native) = &self.native {
            app_native::after_draw(native, ctx, &self.st);
        }
        build_status::after_draw(&mut self.st, ctx, now, &mut msgs);
        self.cache.avatars.end_frame();
        let wanted = self.cache.avatars.take_wanted();
        if !wanted.is_empty() {
            msgs.push(Msg::Avatars(AvatarMsg::WantAvatars(wanted)));
        }
        // Only Preferences edits settings in place; persist any change (also on the frame it closes).
        if self.st.prefs.is_some() {
            msgs.push(Msg::Prefs(PrefsMsg::SettingsEdited));
        }

        self.phase("messages", |app| {
            for msg in msgs {
                app.dispatch(ctx, msg);
            }
        });
        if let Some(menu) = &mut self.menu {
            app_menu::sync(menu, &self.st);
        }
        // The smoke tests' opt-in state file: a snapshot of this pass, written by its own
        // thread (`smoke_probe.rs`), never by a view. A changed one asks for one more frame, so
        // the widgets drawn catch up with the messages just handled.
        if let Some(p) = &mut self.probe
            && p.offer(super::probe::snapshot(ctx, &self.st))
        {
            ctx.request_repaint();
        }
        self.schedule_repaints(ctx, now);
        drop(span);
        self.end_frame(started);
    }
}

impl eframe::App for GlowflyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame(ctx);
    }

    /// Transparent where Mica shows through, else the palette's background.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        app_native::clear_rgba(self.st.view.backdrop, theme::color::current().bg)
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

/// Flashes the taskbar entry while the window isn't focused.
fn request_attention(ctx: &egui::Context) {
    if !ctx.input(|i| i.focused) {
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_keeps_the_app_order_and_a_test_app_skips_only_outside_steps() {
        use Step::*;
        let app = [Theme, Layout, Askpass, Logs, IndexStore, Instance, OpenEvents, Lfs, GitEnv, Onboarding, Updates, RecentFolders];
        assert_eq!(steps(true).collect::<Vec<_>>(), app);
        assert_eq!(steps(false).collect::<Vec<_>>(), [Theme, Layout, Lfs, GitEnv, Onboarding, RecentFolders]);
    }
}
