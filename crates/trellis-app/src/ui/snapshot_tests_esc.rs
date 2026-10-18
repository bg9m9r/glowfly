//! Esc against the keyboard focus: in a text field it only leaves the field, and a terminal
//! hidden under Preferences doesn't keep it from closing them. Runs the shell's Esc handling
//! before drawing, in the app's order.

use super::snapshot_tests::{remove_fixture, with_repo};
use super::{UiCache, avatars::Avatars, draw, escape_key, theme::color};
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::msg::RepoMsg;
use crate::state::terminal::{SessionStatus, TermSession};
use crate::state::{AppState, PrefPage, PrefsState};
use eframe::egui::{self, Key};
use egui_kittest::Harness;
use trellis_core::repo::{DiffTarget, FileDiff};
use trellis_core::settings::ThemeChoice;
use trellis_core::spell::Speller;
use trellis_core::terminal::TermScreen;

pub(super) fn app_harness(st: AppState) -> Harness<'static, (AppState, Vec<Msg>)> {
    let mut cache: Option<UiCache> = None;
    Harness::builder().with_size(egui::vec2(1400.0, 820.0)).build_state(
        move |ctx, (st, msgs): &mut (AppState, Vec<Msg>)| {
            color::set_dark(true);
            escape_key(ctx, st, msgs);
            let cache = cache.get_or_insert_with(|| UiCache {
                avatars: Avatars::default(),
                speller: Speller::new(),
                diff_rows: Default::default(),
                history_text: Default::default(),
            });
            draw(ctx, st, cache, msgs);
        },
        (st, Vec::new()),
    )
}

pub(super) fn esc(h: &mut Harness<'_, (AppState, Vec<Msg>)>) -> Vec<Msg> {
    h.state_mut().1.clear();
    h.press_key(Key::Escape);
    h.run();
    std::mem::take(&mut h.state_mut().1)
}

#[test]
fn esc_in_a_text_field_leaves_it_before_closing_the_diff() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "esc-typing");
    let diff = FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
    st.diff = Some((DiffTarget::Unstaged("README.md".into()), diff.into()));
    st.view.focus_message = true;
    let mut h = app_harness(st);
    h.run();
    assert!(h.state().0.view.was_typing, "the commit summary has the keyboard");

    let msgs = esc(&mut h);
    assert!(!msgs.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::CloseDiff))), "the first Esc only leaves the field");
    assert!(!h.state().0.view.was_typing);
    assert!(esc(&mut h).iter().any(|m| matches!(m, Msg::Repo(RepoMsg::CloseDiff))), "the next closes the diff");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn esc_closes_preferences_opened_while_the_terminal_had_focus() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "esc-terminal-prefs");
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    let screen = TermScreen::new(24, 80);
    st.terminal.sessions.push(TermSession {
        id: 1,
        number: 1,
        selection: None,
        workdir,
        program: "pwsh".into(),
        screen,
        status: SessionStatus::Running,
    });
    st.terminal.next_id = 2;
    st.terminal.open = true;
    st.terminal.focus_requested = true;
    let mut h = app_harness(st);
    h.run();
    assert!(h.state().0.terminal.focused);

    // As Ctrl+comma does from the terminal.
    h.state_mut().0.prefs = Some(PrefsState::new(PrefPage::General));
    h.run();
    assert!(esc(&mut h).iter().any(|m| matches!(m, Msg::Prefs(PrefsMsg::ClosePrefs))), "Esc closes Preferences");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn esc_closes_the_git_flow_panel() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "esc-gitflow-panel");
    st.gitflow_panel = true;
    let mut h = app_harness(st);
    h.run();
    let msgs = esc(&mut h);
    assert!(msgs.iter().any(|m| matches!(m, Msg::GitFlow(crate::gitflow::GitFlowMsg::ClosePanel))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn esc_closes_blame_before_history() {
    use crate::msg::{BlameMsg, HistoryMsg};
    use crate::state::blame::BlameView;
    use crate::state::history::FileHistoryView;
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "esc-blame");
    st.history = Some(FileHistoryView::loading("README.md".into(), None));
    st.blame = Some(BlameView::loading("README.md".into(), None));
    let mut h = app_harness(st);
    h.run();
    let msgs = esc(&mut h);
    assert!(msgs.iter().any(|m| matches!(m, Msg::Blame(BlameMsg::Close))), "Esc closes the blame view");
    assert!(!msgs.iter().any(|m| matches!(m, Msg::History(HistoryMsg::CloseFileHistory))), "and leaves the history open");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn esc_closes_the_tour_and_leaves_the_welcome_to_itself() {
    use crate::onboarding::OnboardingMsg;
    use crate::state::onboarding_tour::TourStep;
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "esc-tour");
    st.onboarding.tour = Some(TourStep::Graph);
    st.diff = Some((DiffTarget::Unstaged("README.md".into()), empty_diff().into()));
    let mut h = app_harness(st);
    h.run();
    let msgs = esc(&mut h);
    assert!(msgs.iter().any(|m| matches!(m, Msg::Onboarding(OnboardingMsg::CloseTour))), "the tour closes first");
    assert!(!msgs.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::CloseDiff))));

    h.state_mut().0.onboarding.tour = None;
    let mut w = crate::state::onboarding::Welcome::new();
    w.identity.reading = false;
    h.state_mut().0.onboarding.welcome = Some(w);
    h.run();
    let msgs = esc(&mut h);
    assert!(!msgs.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::CloseDiff))), "the central chain leaves Esc to the welcome");
    assert!(msgs.iter().any(|m| matches!(m, Msg::Onboarding(OnboardingMsg::CloseWelcome))), "which closes itself");
    drop(h);
    remove_fixture(&dir);
}

fn empty_diff() -> FileDiff {
    FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None }
}

#[test]
fn esc_cancels_a_tab_drag() {
    use super::tabs_tests::{pointer, tab_centre};
    use crate::msg::TabsMsg;
    use crate::ui::snapshot_check::CheckSnapshot;
    use eframe::egui::PointerButton;
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "esc-tab-drag");
    st.diff = Some((DiffTarget::Unstaged("README.md".into()), empty_diff().into()));
    let mut h = app_harness(st);
    h.run();
    let start = tab_centre(0);
    pointer(&mut h, start, Some((PointerButton::Primary, true)));
    h.step();
    pointer(&mut h, start + egui::vec2(12.0, 0.0), None);
    h.step();
    let over = start + egui::vec2(272.0, 0.0);
    pointer(&mut h, over, None);
    h.step();
    h.run();
    assert!(super::tabs::dragging(&h.ctx), "the tab floats");
    let msgs = esc(&mut h);
    assert!(!msgs.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::CloseDiff))), "that Esc only cancels the drag");
    assert!(!super::tabs::dragging(&h.ctx));
    h.check_snapshot("tabs_drag_cancelled");

    pointer(&mut h, over, Some((PointerButton::Primary, false)));
    h.run();
    let msgs = &h.state().1;
    assert!(!msgs.iter().any(|m| matches!(m, Msg::Tabs(TabsMsg::MoveTab { .. }))), "the release moves nothing");
    assert!(!msgs.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::CloseDiff))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn esc_in_file_history_clears_a_line_selection_first() {
    use crate::msg::{DiffMsg, HistoryMsg};
    use crate::state::history::FileHistoryView;
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "esc-history-lines");
    st.history = Some(FileHistoryView::loading("README.md".into(), None));
    st.view.diff_lines.target = Some(DiffTarget::Commit(git2::Oid::zero(), "README.md".into()));
    st.view.diff_lines.lines.insert(1);
    let mut h = app_harness(st);
    h.run();
    let msgs = esc(&mut h);
    assert!(msgs.iter().any(|m| matches!(m, Msg::Diff(DiffMsg::ClearLineSelection))), "Esc drops the selection");
    assert!(!msgs.iter().any(|m| matches!(m, Msg::History(HistoryMsg::CloseFileHistory))), "and leaves the history open");
    h.state_mut().0.view.diff_lines.clear();
    let msgs = esc(&mut h);
    assert!(msgs.iter().any(|m| matches!(m, Msg::History(HistoryMsg::CloseFileHistory))), "the next Esc closes the history");
    drop(h);
    remove_fixture(&dir);
}
