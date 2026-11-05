//! Headless tests for the views' edits (`Msg::Edit`, update/edit.rs): `edits_first`'s order,
//! each edit set back on the state it was drawn from (and dropped once that state has gone),
//! a key handled in the same frame seeing the edit, toasts expiring on `Msg::Tick`, and the
//! credential dialog's fields, which stay in `ViewState` and are wiped by `PromptFields::clear`.

use super::tests::state;
use super::update;
use crate::effect::{Effect, SaveEffect};
use crate::msg::{CommitMsg, EditMsg, Msg, edits_first};
use crate::onboarding::{OnboardingEffect, OnboardingMsg as O};
use crate::state::AppState;
use crate::state::askpass::PromptFields;
use crate::state::blame::BlameView;
use crate::state::commit::CommitFields;
use crate::state::history::FileHistoryView;
use crate::state::op_fixture::{rows, snapshot};
use crate::state::terminal::{Selection, SessionStatus, TabRename, TermSession};
use glowfly_core::identity::Identity;
use glowfly_core::settings::ToastTime;
use glowfly_core::terminal::{TermPoint, TermScreen};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn edit(st: &mut AppState, e: EditMsg) -> Vec<Effect> {
    update(st, Msg::Edit(e), Instant::now())
}

/// A shell session `id` in a scratch workdir, as the terminal panel holds it.
fn session(id: u64) -> TermSession {
    TermSession {
        id,
        number: 1,
        workdir: PathBuf::from("demo"),
        program: "pwsh".into(),
        shell: Default::default(),
        name: None,
        screen: TermScreen::new(4, 20),
        status: SessionStatus::Running,
        selection: None,
    }
}

#[test]
fn edits_go_first_keeping_their_order() {
    let summary = |s: &str| EditMsg::Commit(CommitFields { summary: s.into(), ..Default::default() });
    let mut msgs = vec![Msg::ClearLog, Msg::Edit(summary("first")), Msg::Tick(Instant::now()), Msg::Edit(summary("second"))];
    assert!(edits_first(&mut msgs), "there were edits");
    let typed: Vec<_> = msgs.iter().map(|m| if let Msg::Edit(EditMsg::Commit(f)) = m { f.summary.as_str() } else { "" }).collect();
    assert_eq!(typed, ["first", "second", "", ""]);
    assert!(matches!(msgs[2..], [Msg::ClearLog, Msg::Tick(_)]), "the rest in their order");

    let mut none = vec![Msg::ClearLog, Msg::Tick(Instant::now())];
    assert!(!edits_first(&mut none), "no edits: no extra frame");
    assert!(matches!(none[..], [Msg::ClearLog, Msg::Tick(_)]));
}

#[test]
fn a_commit_key_in_the_same_frame_sees_the_typed_summary() {
    let mut st = state();
    let mut snap = snapshot("edit-commit");
    snap.staged = rows(&[("a.txt", 'M')]);
    st.snap = Some(snap);
    let mut f = st.session.draft.fields();
    f.summary = "Typed this frame".into();
    // The key's message was pushed first; edits_first runs the edit ahead of it.
    let mut msgs = vec![Msg::Commit(CommitMsg::Commit), Msg::Edit(EditMsg::Commit(f))];
    edits_first(&mut msgs);
    let fx: Vec<Effect> = msgs.into_iter().flat_map(|m| update(&mut st, m, Instant::now())).collect();
    assert_eq!(st.session.draft.summary, "Typed this frame");
    assert!(!fx.is_empty() && st.toasts.is_empty(), "it commits rather than asking for a summary");
}

#[test]
fn the_welcome_takes_its_fields_only_while_open() {
    let mut st = state();
    update(&mut st, Msg::Onboarding(O::Startup), Instant::now());
    update(&mut st, Msg::Onboarding(O::IdentityRead(Ok(Identity::default()))), Instant::now());
    let mut f = st.onboarding.welcome.as_ref().expect("open").fields();
    (f.name, f.email, f.avatars) = ("Ada".into(), "ada@example.com".into(), true);
    edit(&mut st, EditMsg::Welcome(f.clone()));
    let w = st.onboarding.welcome.as_ref().unwrap();
    assert_eq!((w.identity.name.as_str(), w.avatars), ("Ada", true));
    let fx = update(&mut st, Msg::Onboarding(O::Next), Instant::now());
    let [Effect::Onboarding(OnboardingEffect::WriteIdentity { identity, .. })] = fx.as_slice() else { panic!("one write: {fx:?}") };
    assert_eq!(identity.name, "Ada", "Next writes what was typed");

    st.onboarding.welcome = None;
    assert!(edit(&mut st, EditMsg::Welcome(f)).is_empty());
    assert!(st.onboarding.welcome.is_none(), "a closed welcome isn't brought back");
}

#[test]
fn a_rename_edit_applies_only_to_the_open_rename() {
    let mut st = state();
    let open = TabRename { id: 1, text: "pwsh".into(), shown: "pwsh".into(), focus: true };
    st.terminal.renaming = Some(open.clone());
    edit(&mut st, EditMsg::TerminalRename(TabRename { text: "build".into(), focus: false, ..open.clone() }));
    let r = st.terminal.renaming.as_ref().unwrap();
    assert_eq!((r.text.as_str(), r.focus), ("build", false));
    edit(&mut st, EditMsg::TerminalRename(TabRename { id: 2, text: "other".into(), ..open }));
    assert_eq!(st.terminal.renaming.as_ref().unwrap().text, "build", "another tab's edit is dropped");
    st.terminal.renaming = None;
    edit(&mut st, EditMsg::TerminalRename(TabRename { id: 1, text: "late".into(), shown: String::new(), focus: false }));
    assert!(st.terminal.renaming.is_none(), "a finished rename isn't reopened");
}

#[test]
fn a_selection_edit_finds_its_session() {
    let mut st = state();
    st.terminal.sessions = vec![session(1), session(2)];
    let at = |col| TermPoint { line: 0, col };
    let sel = Selection { anchor: at(1), head: at(5), epoch: 0 };
    edit(&mut st, EditMsg::TerminalSelection { id: 2, selection: Some(sel) });
    assert!(st.terminal.sessions[0].selection.is_none());
    assert_eq!(st.terminal.sessions[1].selection, Some(sel));
    edit(&mut st, EditMsg::TerminalSelection { id: 2, selection: None });
    assert!(st.terminal.sessions[1].selection.is_none(), "a click clears it");
    assert!(edit(&mut st, EditMsg::TerminalSelection { id: 9, selection: Some(sel) }).is_empty(), "a closed session's is dropped");
}

#[test]
fn a_blame_edit_keeps_the_matches_and_needs_the_same_blame() {
    let mut st = state();
    let mut v = BlameView::loading("a.txt".into(), None);
    v.find.hits = vec![2, 4];
    st.session.blame.view = Some(v);
    let mut f = st.session.blame.view.as_ref().unwrap().fields();
    f.find.query = "foo".into();
    f.find.hits.clear();
    (f.goto, f.top_line, f.focus_find) = ("12".into(), 3, false);
    edit(&mut st, EditMsg::Blame(Box::new(f.clone())));
    let v = st.session.blame.view.as_ref().unwrap();
    assert_eq!((v.find.query.as_str(), v.goto.as_str(), v.top_line), ("foo", "12", 3));
    assert_eq!(v.find.hits, [2, 4], "the matches stay update's");

    f.path = "b.txt".into();
    f.goto = "99".into();
    edit(&mut st, EditMsg::Blame(Box::new(f)));
    assert_eq!(st.session.blame.view.as_ref().unwrap().goto, "12", "another file's blame is dropped");
}

#[test]
fn a_file_view_edit_keeps_the_matches() {
    let mut st = state();
    let mut v = FileHistoryView::loading("a.txt".into(), None);
    (v.nav.find.hits, v.nav.find.current) = (vec![1, 3], Some(1));
    st.session.history.view = Some(v);
    let mut nav = st.session.history.view.as_ref().unwrap().nav.clone();
    (nav.find.query, nav.goto, nav.top, nav.cursor) = ("beta".into(), "7".into(), 5, Some(6));
    (nav.find.hits, nav.find.current) = (Vec::new(), None);
    edit(&mut st, EditMsg::FileNav(Box::new(nav)));
    let n = &st.session.history.view.as_ref().unwrap().nav;
    assert_eq!((n.find.query.as_str(), n.goto.as_str(), n.top, n.cursor), ("beta", "7", 5, Some(6)));
    assert_eq!((n.find.hits.as_slice(), n.find.current), ([1, 3].as_slice(), Some(1)));
}

#[test]
fn a_settings_edit_saves_once_and_an_unchanged_one_does_nothing() {
    let mut st = state();
    let mut s = st.settings.clone();
    s.a11y.toast_time = ToastTime::Longer;
    let fx = edit(&mut st, EditMsg::Settings(Box::new(s)));
    assert_eq!(st.settings.a11y.toast_time, ToastTime::Longer);
    assert!(fx.iter().any(|e| matches!(e, Effect::Save(SaveEffect::Settings(_)))), "saved: {fx:?}");
    let same = st.settings.clone();
    let fx = edit(&mut st, EditMsg::Settings(Box::new(same)));
    assert!(!fx.iter().any(|e| matches!(e, Effect::Save(_))), "nothing changed, nothing saved");
}

#[test]
fn toasts_expire_on_tick() {
    let (mut st, t0) = (state(), Instant::now());
    st.settings.a11y.toast_time = ToastTime::Standard;
    st.toast(true, "Fetched", t0);
    update(&mut st, Msg::Tick(t0 + Duration::from_secs(1)), t0 + Duration::from_secs(1));
    assert_eq!(st.toasts.len(), 1, "still up");
    update(&mut st, Msg::Tick(t0 + Duration::from_secs(60)), t0 + Duration::from_secs(60));
    assert!(st.toasts.is_empty(), "gone without a frame drawing it");
}

#[test]
fn prompt_fields_clear_wipes_the_text_and_asks_for_focus() {
    let mut f = PromptFields { user: "ada".into(), secret: "hunter2".into(), reveal: true, focus_pending: false, remember: true };
    f.clear();
    assert!(f.user.is_empty() && f.secret.is_empty());
    assert!(!f.reveal && f.focus_pending);
    assert!(f.remember, "the Remember box follows the setting, not the prompt");
}
