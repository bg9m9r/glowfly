//! Views drawn from a frame copy send `Msg::Edit` only when something changed: once a view has
//! settled, an idle frame sends none (a widget and update that normalise a value differently
//! would edit every frame), and typing in its field sends one. Covers the commit box, the
//! welcome's identity and start steps, a terminal tab's rename field, the blame and File view
//! with a find query, and the Preferences pages that edit the settings (a terminal drag's edit
//! is in `snapshot_tests_terminal_select`).

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_blame::with_blame;
use super::snapshot_tests_history::with_file_view;
use super::snapshot_tests_onboarding::{ada, on};
use super::snapshot_tests_terminal::with_session;
use crate::snapshot_fixtures::edits_taken;
use eframe::egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use glowfly_core::settings::ThemeChoice;
use glowfly_model::msg::Msg;
use glowfly_model::state::onboarding::WelcomeStep;
use glowfly_model::state::terminal::SessionStatus;
use glowfly_model::state::{AppState, PrefPage, PrefsState};
use glowfly_model::terminal::TerminalMsg;
use std::time::Instant;

pub(super) type H = Harness<'static, (AppState, Vec<Msg>)>;

/// Settles `st`'s window, then draws idle frames and asserts they sent no edit (`what` names
/// the case). Returns the harness for a follow-up.
pub(super) fn idle(st: AppState, what: &str) -> H {
    let mut h = harness(st, true);
    h.run();
    h.run();
    edits_taken();
    h.run();
    h.step();
    h.step();
    assert_eq!(edits_taken(), 0, "{what}: an idle frame sent an edit");
    h
}

/// Types into the text field showing `value` and asserts that sent an edit.
pub(super) fn typing_edits(h: &mut H, value: &str) {
    h.get(By::new().role(Role::TextInput).value(value)).type_text("x");
    h.run();
    assert!(edits_taken() > 0, "typing in {value:?} sends an edit");
}

#[test]
fn the_commit_box_idles_without_edits() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "idle-commit");
    st.session.draft.summary = "Add login form".into();
    st.session.draft.description = "With a remember-me box.".into();
    st.session.draft.skip_hooks = true;
    let mut h = idle(st, "commit box");
    typing_edits(&mut h, "Add login form");
    assert_eq!(h.state().0.session.draft.summary, "Add login formx");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_welcome_idles_without_edits() {
    let mut h = idle(on(ThemeChoice::Dark, WelcomeStep::Identity, Some(ada())), "welcome identity");
    typing_edits(&mut h, "Ada Lovelace");
    idle(on(ThemeChoice::Dark, WelcomeStep::Start, Some(ada())), "welcome start");
}

#[test]
fn a_terminal_rename_idles_without_edits() {
    let (mut st, dir) = with_session(ThemeChoice::Dark, "idle-rename", SessionStatus::Running);
    glowfly_model::update::update(&mut st, Msg::Terminal(TerminalMsg::StartRename(1)), Instant::now());
    let shown = st.terminal.renaming.as_ref().unwrap().text.clone();
    let mut h = idle(st, "terminal rename, its focus request taken while settling");
    typing_edits(&mut h, &shown);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_blame_with_a_query_idles_without_edits() {
    let (mut st, dir, _) = with_blame(ThemeChoice::Dark, "idle-blame");
    let v = st.session.blame.view.as_mut().unwrap();
    v.find.query = "Point".into();
    v.goto = "4".into();
    let mut h = idle(st, "blame");
    typing_edits(&mut h, "Point");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_file_view_with_a_query_idles_without_edits() {
    let (mut st, dir) = with_file_view("idle-file-view", ThemeChoice::Dark);
    let nav = &mut st.session.history.view.as_mut().unwrap().nav;
    nav.find.query = "words".into();
    nav.goto = "3".into();
    let mut h = idle(st, "File view");
    typing_edits(&mut h, "words");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn preferences_pages_idle_without_edits() {
    for page in [PrefPage::General, PrefPage::Ui, PrefPage::Accessibility, PrefPage::Language, PrefPage::DiffViewer] {
        let (mut st, dir) = with_repo(ThemeChoice::Dark, &format!("idle-prefs-{page:?}").to_lowercase());
        st.prefs = Some(PrefsState::new(page));
        drop(idle(st, &format!("{page:?}")));
        remove_fixture(&dir);
    }
}
