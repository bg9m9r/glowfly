//! Drives the whole app headlessly: the real `TrellisApp` in an egui_kittest harness, clicked
//! and typed into through widgets found by their accessible names. After every action it waits
//! until the app is idle, and every toast the app showed since is kept for the checks.

use super::find::{self, Pick, Region};
use super::fixture::Fixture;
use crate::ui::{TestStart, TrellisApp};
use eframe::egui;
use egui_kittest::kittest::Queryable;
use egui_kittest::{Harness, TestRenderer};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use trellis_core::settings::{Settings, ThemeChoice};

/// How long one step may take before the journey fails with a diagnosis.
const TIMEOUT: Duration = Duration::from_secs(120);

pub(super) struct Shell {
    app: Option<TrellisApp>,
    start: Option<TestStart>,
}

impl Shell {
    fn app(&self) -> &TrellisApp {
        self.app.as_ref().expect("the app is built on the first frame")
    }
}

/// The journeys never render: textures are ignored.
struct NoRender;

impl TestRenderer for NoRender {
    fn handle_delta(&mut self, _delta: &egui::TexturesDelta) {}

    fn render(&mut self, _ctx: &egui::Context, _output: &egui::FullOutput) -> Result<image::RgbaImage, String> {
        Err("journeys don't render".into())
    }
}

/// A toast the app showed: whether it reported success, its text, and whether it was seen on
/// screen.
struct Seen {
    ok: bool,
    text: String,
    shown: bool,
}

pub(super) struct Journey {
    // Dropped before the fixture, so the app's threads let go of its folders first.
    h: Harness<'static, Shell>,
    pub fx: Fixture,
    /// The toasts shown since the last action.
    toasts: Vec<Seen>,
    /// Every toast id recorded so far, so one still fading isn't recorded again.
    known: Vec<u64>,
}

/// The settings a journey starts with: English, fixed formats, nothing on a timer, nothing
/// that reaches outside the test.
fn settings() -> Settings {
    Settings {
        theme: ThemeChoice::Dark,
        language: "en".into(),
        date_locale: "en_US".into(),
        use_initials: true,
        auto_fetch_minutes: 0,
        watch_files: false,
        conflict_check: false,
        desktop_notifications: false,
        behind_alerts: false,
        spellcheck: false,
        show_menu_bar: false,
        toolbar_labels: true,
        welcome_done: true,
        reopen_last_repo: false,
        show_left_panel: true,
        show_details_panel: true,
        show_toolbar: true,
        ..Settings::default()
    }
}

impl Journey {
    /// Starts the app on `fx` (opening `open`, with `trusted` recorded as trusted) and waits
    /// until it is idle.
    pub fn start(fx: Fixture, open: Option<PathBuf>, trusted: Vec<PathBuf>) -> Self {
        let start = TestStart { settings: settings(), files: fx.config(), trusted, open };
        let h = Harness::builder().with_size(egui::vec2(1440.0, 900.0)).renderer(NoRender).build_state(
            |ctx, shell: &mut Shell| {
                if shell.app.is_none() {
                    let start = shell.start.take().expect("one start");
                    shell.app = Some(TrellisApp::for_test(ctx, start));
                }
                if let Some(app) = shell.app.as_mut() {
                    app.frame(ctx);
                }
            },
            Shell { app: None, start: Some(start) },
        );
        let mut j = Self { h, fx, toasts: Vec::new(), known: Vec::new() };
        j.settle();
        j
    }

    pub fn app(&self) -> &TrellisApp {
        self.h.state().app()
    }

    /// The app's state, for diagnostics.
    pub fn state(&self) -> &crate::state::AppState {
        self.app().state()
    }

    /// The names of the effects the sandbox dropped.
    pub fn dropped(&self) -> Vec<&'static str> {
        self.app().dropped_effects()
    }

    /// One frame, then records the toasts that appeared and checks they are on screen.
    fn step(&mut self) {
        self.h.step();
        let st = self.h.state().app().state();
        for t in &st.toasts {
            if !self.known.contains(&t.id) {
                self.known.push(t.id);
                self.toasts.push(Seen { ok: t.ok, text: t.text.clone(), shown: false });
            }
        }
        for s in self.toasts.iter_mut().filter(|s| !s.shown) {
            s.shown = self.h.query_all_by_label_contains(&s.text).next().is_some();
        }
    }

    /// Steps until the app is idle: no job, nothing loading, no effect in flight, and no
    /// effect started or result taken for two frames in a row. Panics with a diagnosis after
    /// `TIMEOUT`.
    pub fn settle(&mut self) {
        let began = Instant::now();
        let mut last = self.app().activity();
        let mut quiet = 0;
        while quiet < 2 {
            self.step();
            std::thread::sleep(Duration::from_millis(5));
            let now = self.app().activity();
            quiet =
                if now.in_flight == 0 && !now.busy && now.started == last.started && now.drained == last.drained { quiet + 1 } else { 0 };
            last = now;
            if began.elapsed() > TIMEOUT {
                panic!("the app did not settle within {TIMEOUT:?}\n{}", self.diagnosis());
            }
        }
        // A toast drawn after the last result shows on the next frame.
        if self.toasts.iter().any(|s| !s.shown) {
            self.step();
        }
        let unseen: Vec<&str> = self.toasts.iter().filter(|s| !s.shown).map(|s| s.text.as_str()).collect();
        assert!(unseen.is_empty(), "toasts never drawn: {unseen:?}\n{}", self.diagnosis());
    }

    /// What the app was doing, for a failure message.
    fn diagnosis(&self) -> String {
        let st = self.state();
        let log: Vec<&str> = st.log.iter().rev().take(10).rev().map(|e| e.text.as_str()).collect();
        format!(
            "job: {:?}\nloading: {}\nin flight: {:?}\nlast activity-log lines:\n  {}\nlabels on screen: {:?}",
            st.job.as_ref().map(|j| j.label.as_str()),
            st.loading,
            self.app().in_flight_names(),
            log.join("\n  "),
            find::labels_in(&self.h, Region::Anywhere),
        )
    }

    /// The failure message of a lookup that found nothing.
    fn missing(&self, what: &str, label: &str, region: Region, pick: Pick<'_>) -> String {
        format!("no {what} {label:?} ({pick:?}) in {region:?}; labels there: {:?}\n{}", find::labels_in(&self.h, region), self.diagnosis())
    }

    /// Clicks the enabled widget labelled `label` in `region` and waits until the app is idle.
    #[track_caller]
    pub fn click(&mut self, label: &str, region: Region, pick: Pick<'_>) {
        self.click_matching(label, true, region, pick);
    }

    /// `click` on the widget whose name starts with or contains `label`.
    #[track_caller]
    pub fn click_containing(&mut self, label: &str, region: Region, pick: Pick<'_>) {
        self.click_matching(label, false, region, pick);
    }

    #[track_caller]
    fn click_matching(&mut self, label: &str, exact: bool, region: Region, pick: Pick<'_>) {
        self.toasts.clear();
        {
            let Some(node) = find::find(&self.h, label, exact, region, pick) else {
                panic!("{}", self.missing("widget", label, region, pick))
            };
            assert!(!node.is_disabled(), "{label:?} is disabled\n{}", self.diagnosis());
            node.click();
        }
        self.settle();
    }

    /// Right-clicks the widget labelled `label` in `region` (opening its menu).
    #[track_caller]
    pub fn right_click(&mut self, label: &str, region: Region, pick: Pick<'_>) {
        self.toasts.clear();
        let pos = {
            let Some(node) = find::find(&self.h, label, true, region, pick) else {
                panic!("{}", self.missing("widget", label, region, pick))
            };
            node.hover();
            find::centre(&node).expect("a widget on screen has bounds")
        };
        self.step();
        for pressed in [true, false] {
            let button = egui::PointerButton::Secondary;
            self.h.input_mut().events.push(egui::Event::PointerButton { pos, button, pressed, modifiers: egui::Modifiers::NONE });
            self.step();
        }
        self.settle();
    }

    /// Clicks `label` in the open context menu.
    #[track_caller]
    pub fn menu(&mut self, label: &str) {
        self.click(label, Region::Anywhere, Pick::Only);
    }

    /// Types `text` into the text field named `name`.
    #[track_caller]
    pub fn type_into(&mut self, name: &str, text: &str) {
        self.toasts.clear();
        {
            let Some(node) = find::field(&self.h, name) else {
                panic!("{}", self.missing("text field", name, Region::Anywhere, Pick::Only))
            };
            node.type_text(text);
        }
        self.settle();
    }

    /// Asserts a widget labelled `label` is shown in `region`.
    #[track_caller]
    pub fn see(&self, label: &str, region: Region) {
        assert!(find::count(&self.h, label, true, region) > 0, "{}", self.missing("label", label, region, Pick::First));
    }

    /// Asserts a widget whose name contains `label` is shown in `region`.
    #[track_caller]
    pub fn see_contains(&self, label: &str, region: Region) {
        assert!(find::count(&self.h, label, false, region) > 0, "{}", self.missing("label containing", label, region, Pick::First));
    }

    /// How many widgets labelled `label` are shown in `region`.
    pub fn count(&self, label: &str, region: Region) -> usize {
        find::count(&self.h, label, true, region)
    }

    /// How many widgets whose name contains `label` are shown in `region`.
    pub fn count_containing(&self, label: &str, region: Region) -> usize {
        find::count(&self.h, label, false, region)
    }

    /// Asserts nothing labelled `label` is shown in `region`.
    #[track_caller]
    pub fn absent(&self, label: &str, region: Region) {
        assert_eq!(find::count(&self.h, label, true, region), 0, "{label:?} is still shown in {region:?}\n{}", self.diagnosis());
    }

    /// Asserts the widget labelled `label` in `region` is shown and disabled.
    #[track_caller]
    pub fn disabled(&self, label: &str, region: Region) {
        let Some(node) = find::find(&self.h, label, true, region, Pick::First) else {
            panic!("{}", self.missing("widget", label, region, Pick::First))
        };
        assert!(node.is_disabled(), "{label:?} is enabled\n{}", self.diagnosis());
    }

    /// Asserts the widget labelled `label` in `region` is shown and enabled.
    #[track_caller]
    pub fn enabled(&self, label: &str, region: Region) {
        let Some(node) = find::find(&self.h, label, true, region, Pick::First) else {
            panic!("{}", self.missing("widget", label, region, Pick::First))
        };
        assert!(!node.is_disabled(), "{label:?} is disabled\n{}", self.diagnosis());
    }

    /// Asserts that since the last action the app toasted a message starting with `prefix`,
    /// as a success (`ok`) or a failure.
    #[track_caller]
    pub fn toast(&self, ok: bool, prefix: &str) {
        let all: Vec<(bool, &str)> = self.toasts.iter().map(|s| (s.ok, s.text.as_str())).collect();
        assert!(
            all.iter().any(|(o, t)| *o == ok && t.starts_with(prefix)),
            "no {} toast starting {prefix:?}; toasts: {all:?}\n{}",
            if ok { "success" } else { "failure" },
            self.diagnosis()
        );
    }

    /// Whether the activity log mentions `text`.
    pub fn logged(&self, text: &str) -> bool {
        self.state().log.iter().any(|e| e.text.contains(text))
    }

    // ---- what a user does often

    /// Runs a command from the palette: opens it, types `query` and clicks the row `row`.
    #[track_caller]
    pub fn run_command(&mut self, query: &str, row: &str) {
        self.open_palette(query);
        self.click(row, Region::Palette, Pick::Only);
    }

    /// Presses the palette's key and types `query` into its field.
    #[track_caller]
    pub fn open_palette(&mut self, query: &str) {
        self.toasts.clear();
        let chord = self.state().keymap.chords(crate::keymap::BindingId::Palette)[0];
        let key = egui::Key::from_name(chord.key.config()).expect("the palette's key is an egui key");
        let (m, mac) = (chord.mods, cfg!(target_os = "macos"));
        let command = egui::Modifiers { ctrl: !mac, mac_cmd: mac, command: true, ..egui::Modifiers::NONE };
        let mut modifiers = if m.command { command } else { egui::Modifiers::NONE };
        modifiers.ctrl |= m.ctrl;
        (modifiers.shift, modifiers.alt) = (m.shift, m.alt);
        self.h.press_key_modifiers(modifiers, key);
        self.settle();
        {
            let Some(node) = find::field_in(&self.h, Region::Palette) else {
                panic!("{}", self.missing("text field", "palette search", Region::Palette, Pick::First))
            };
            node.type_text(query);
        }
        self.settle();
    }

    /// Refresh in the toolbar, as after editing files outside the app.
    pub fn refresh(&mut self) {
        self.click("Refresh the repository", Region::Toolbar, Pick::Only);
    }

    /// Selects the working tree's row in the graph.
    pub fn select_wip(&mut self) {
        self.click("Uncommitted changes", Region::Central, Pick::Only);
    }

    /// Stages everything and commits it as `summary` from the details panel.
    pub fn commit_all(&mut self, summary: &str) {
        self.refresh();
        self.select_wip();
        self.click("Stage all", Region::Details, Pick::Only);
        self.type_into("Commit summary", summary);
        self.click_containing("Commit changes to", Region::Details, Pick::Only);
        self.toast(true, "Commit succeeded");
    }

    /// The sidebar row of local branch `name` (checked out or not); local rows come before the
    /// remotes' rows of the same name.
    pub fn local_branch(&self, name: &str) -> String {
        let head = format!("✔  {name}");
        if find::count(&self.h, &head, true, Region::Sidebar) > 0 { head } else { name.to_owned() }
    }

    /// Right-clicks local branch `name` in the sidebar and picks `item` from its menu.
    pub fn branch_menu(&mut self, name: &str, item: &str) {
        let row = self.local_branch(name);
        self.right_click(&row, Region::Sidebar, Pick::First);
        self.menu(item);
    }

    pub fn config_file(&self, name: &str) -> PathBuf {
        self.fx.config().join(name)
    }

    /// Whether `a` and `b` name the same folder.
    pub fn same(a: &Path, b: &Path) -> bool {
        trellis_core::paths::same_folder(a, b)
    }
}
