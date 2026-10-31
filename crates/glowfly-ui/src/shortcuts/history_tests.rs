//! The history keys: Reword, Squash and Drop only while the graph is the central view and
//! commits are selected, Continue and Skip only while a rebase is in progress, and none of
//! them while a text field has the keyboard.

use crate::msg::{Msg, RebaseMsg};
use crate::state::rebase::RebasePanel;
use crate::state::selection::MultiSelection;
use crate::state::{AppState, Selection};
use crate::ui::snapshot_tests::{remove_fixture, with_repo};
use eframe::egui::{self, Event, Key, Modifiers};
use egui_kittest::Harness;
use git2::{Oid, RepositoryState};
use glowfly_core::rebase::{QuickEdit, RebaseBase, Resume};
use glowfly_core::settings::ThemeChoice;

struct Keys {
    st: AppState,
    msgs: Vec<Msg>,
    /// Draw a text field that takes the focus on the next frame.
    field: Option<(String, bool)>,
}

fn harness(st: AppState, field: bool) -> Harness<'static, Keys> {
    let keys = Keys { st, msgs: Vec::new(), field: field.then(|| (String::new(), true)) };
    let mut h = crate::ui::snapshot_check::builder().build_state(
        |ctx, k: &mut Keys| {
            super::input(ctx, &mut k.st, &mut k.msgs);
            if let Some((text, focus)) = k.field.as_mut() {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let r = ui.text_edit_singleline(text);
                    if std::mem::take(focus) {
                        r.request_focus();
                    }
                });
            }
        },
        keys,
    );
    h.run();
    h
}

/// Presses `key` with `m` for one frame and returns the rebase messages it produced.
fn press(h: &mut Harness<'static, Keys>, m: Modifiers, key: Key) -> Vec<String> {
    h.state_mut().msgs.clear();
    let input = h.input_mut();
    input.modifiers = m;
    input.events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: m });
    h.step();
    h.input_mut().modifiers = Modifiers::NONE;
    std::mem::take(&mut h.state_mut().msgs)
        .into_iter()
        .map(|msg| match msg {
            Msg::Rebase(RebaseMsg::Edit(e)) => format!("{e:?}"),
            Msg::Rebase(RebaseMsg::Resume(r)) => format!("{r:?}"),
            _ => "other".into(),
        })
        .collect()
}

/// The fixture with its newest two commits; `selection` picks which are selected.
fn fixture(name: &str) -> (AppState, std::path::PathBuf, Oid, Oid) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    let snap = st.snap.as_ref().unwrap();
    let (a, b) = (snap.commits[0].oid, snap.commits[1].oid);
    st.session.selection = Selection::Commit(a);
    (st, dir, a, b)
}

#[test]
fn graph_edits_follow_the_selection() {
    let (st, dir, a, b) = fixture("keys-history-graph");
    assert!(st.graph_shown());
    let mut h = harness(st, false);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::F2), [format!("{:?}", QuickEdit::Reword(a))]);
    assert_eq!(press(&mut h, Modifiers::ALT, Key::S), [format!("{:?}", QuickEdit::SquashIntoParent(a))]);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::Delete), [format!("{:?}", QuickEdit::Drop(vec![a]))]);
    assert_eq!(press(&mut h, Modifiers::COMMAND, Key::Backspace), [format!("{:?}", QuickEdit::Drop(vec![a]))]);

    let snap = h.state().st.snap.as_ref().unwrap();
    let both = MultiSelection::build([a, b], a, snap);
    let Selection::Commits(m) = &both else { panic!("two commits") };
    let oids = m.oids.clone();
    h.state_mut().st.session.selection = both;
    assert_eq!(press(&mut h, Modifiers::ALT, Key::S), [format!("{:?}", QuickEdit::Squash(oids.clone()))]);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::Delete), [format!("{:?}", QuickEdit::Drop(oids))]);
    assert!(press(&mut h, Modifiers::NONE, Key::F2).is_empty(), "no reword of several commits");

    h.state_mut().st.session.selection = Selection::Wip;
    assert!(press(&mut h, Modifiers::NONE, Key::F2).is_empty());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_edits_need_the_graph() {
    let (mut st, dir, a, _) = fixture("keys-history-panel");
    st.session.rebase.view = Some(RebasePanel::loading(RebaseBase::FromCommit(a)));
    assert!(!st.graph_shown());
    let mut h = harness(st, false);
    assert!(press(&mut h, Modifiers::NONE, Key::F2).is_empty(), "the rebase panel covers the graph");
    assert!(press(&mut h, Modifiers::NONE, Key::Delete).is_empty());
    drop(h);

    let (st, _, _, _) = fixture("keys-history-panel");
    let mut h = harness(st, true);
    assert!(press(&mut h, Modifiers::NONE, Key::F2).is_empty(), "typing in a field");
    assert!(press(&mut h, Modifiers::NONE, Key::Delete).is_empty());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn continue_and_skip_need_a_rebase() {
    let (st, dir, _, _) = fixture("keys-history-resume");
    let mut h = harness(st, false);
    assert!(press(&mut h, Modifiers::ALT, Key::C).is_empty(), "no rebase in progress");
    assert!(press(&mut h, Modifiers::ALT, Key::K).is_empty());
    h.state_mut().st.snap.as_mut().unwrap().state = RepositoryState::RebaseInteractive;
    assert_eq!(press(&mut h, Modifiers::ALT, Key::C), [format!("{:?}", Resume::Continue)]);
    assert_eq!(press(&mut h, Modifiers::ALT, Key::K), [format!("{:?}", Resume::Skip)]);
    drop(h);

    let (mut st, _, _, _) = fixture("keys-history-resume");
    st.snap.as_mut().unwrap().state = RepositoryState::RebaseInteractive;
    let mut h = harness(st, true);
    assert!(press(&mut h, Modifiers::ALT, Key::C).is_empty(), "typing in a field");
    drop(h);
    remove_fixture(&dir);
}
