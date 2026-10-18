//! Select every commit (Cmd/Ctrl+A): it fires only with the graph in view and nothing typing,
//! no diff over the graph and no dialog open.

use crate::msg::{Msg, SelectionMsg};
use crate::state::{AppState, Modal};
use crate::ui::snapshot_tests::{remove_fixture, with_repo};
use eframe::egui::{self, Event, Key, Modifiers};
use egui_kittest::Harness;
use trellis_core::repo::{DiffTarget, FileDiff};
use trellis_core::settings::ThemeChoice;

struct Keys {
    st: AppState,
    msgs: Vec<Msg>,
    /// Draw a focused text field.
    field: Option<String>,
}

fn harness(st: AppState, field: bool) -> Harness<'static, Keys> {
    let keys = Keys { st, msgs: Vec::new(), field: field.then(String::new) };
    let mut h = Harness::new_state(
        |ctx, k: &mut Keys| {
            super::input(ctx, &mut k.st, &mut k.msgs);
            if let Some(text) = k.field.as_mut() {
                egui::CentralPanel::default().show(ctx, |ui| ui.text_edit_singleline(text).request_focus());
            }
        },
        keys,
    );
    h.run();
    h
}

/// Whether Cmd/Ctrl+A asked to select every commit.
fn selects_all(h: &mut Harness<'static, Keys>) -> bool {
    h.state_mut().msgs.clear();
    let input = h.input_mut();
    input.modifiers = Modifiers::COMMAND;
    input.events.push(Event::Key { key: Key::A, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND });
    h.step();
    h.input_mut().modifiers = Modifiers::NONE;
    std::mem::take(&mut h.state_mut().msgs).iter().any(|m| matches!(m, Msg::Selection(SelectionMsg::SelectAll)))
}

#[test]
fn select_all_needs_the_graph() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "select-all-keys");
    assert!(st.graph_shown());
    let mut h = harness(st, false);
    assert!(selects_all(&mut h), "the graph is in view");

    let empty = FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
    h.state_mut().st.diff = Some((DiffTarget::Unstaged("f.txt".into()), empty.into()));
    assert!(!selects_all(&mut h), "a diff covers the graph");
    h.state_mut().st.diff = None;
    h.state_mut().st.modal = Some(Modal::About);
    assert!(!selects_all(&mut h), "a dialog is open");
    drop(h);

    let (st, typing_dir) = with_repo(ThemeChoice::Dark, "select-all-keys-typing");
    let mut h = harness(st, true);
    assert!(!selects_all(&mut h), "a text field keeps select all");
    drop(h);
    remove_fixture(&dir);
    remove_fixture(&typing_dir);
}
