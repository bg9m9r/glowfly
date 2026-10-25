//! The fold keys: E and Shift+E open and fold back unchanged lines only while a diff is shown
//! and no text field has the keyboard, and not in the conflict view, blame or a dialog.

use crate::msg::{DiffMsg, Msg};
use crate::state::blame::BlameView;
use crate::state::conflict::ConflictView;
use crate::state::{AppState, Modal};
use eframe::egui::{self, Event, Key, Modifiers};
use egui_kittest::Harness;
use glowfly_core::repo::{DiffTarget, FileDiff};
use glowfly_core::settings::Settings;
use std::time::Instant;

struct Keys {
    st: AppState,
    msgs: Vec<Msg>,
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

/// Presses E (with `m`) and returns "open" / "close" for each fold message it produced.
fn press(h: &mut Harness<'static, Keys>, m: Modifiers) -> Vec<&'static str> {
    h.state_mut().msgs.clear();
    let input = h.input_mut();
    input.modifiers = m;
    input.events.push(Event::Key { key: Key::E, physical_key: None, pressed: true, repeat: false, modifiers: m });
    h.step();
    h.input_mut().modifiers = Modifiers::NONE;
    std::mem::take(&mut h.state_mut().msgs)
        .into_iter()
        .map(|msg| match msg {
            Msg::Diff(DiffMsg::OpenFoldAtTop) => "open",
            Msg::Diff(DiffMsg::CloseFoldAtTop) => "close",
            _ => "other",
        })
        .collect()
}

fn with_diff() -> AppState {
    let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
    let empty = FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
    st.diff = Some((DiffTarget::Commit(git2::Oid::zero(), "f.txt".into()), empty.into()));
    st
}

#[test]
fn fold_keys_in_a_diff() {
    let mut h = harness(with_diff(), false);
    assert_eq!(press(&mut h, Modifiers::NONE), ["open"]);
    assert_eq!(press(&mut h, Modifiers::SHIFT), ["close"]);
    assert!(press(&mut h, Modifiers::COMMAND).is_empty());
}

#[test]
fn fold_keys_stay_out_of_other_views() {
    let mut no_diff = harness(AppState::new(Settings::default(), Vec::new(), Instant::now()), false);
    assert!(press(&mut no_diff, Modifiers::NONE).is_empty(), "no diff");

    let mut st = with_diff();
    st.conflict = Some(ConflictView::loading("f.txt".into()));
    assert!(press(&mut harness(st, false), Modifiers::NONE).is_empty(), "conflict view");

    let mut st = with_diff();
    st.blame = Some(BlameView::loading("f.txt".into(), None));
    assert!(press(&mut harness(st, false), Modifiers::SHIFT).is_empty(), "blame");

    let mut typing = harness(with_diff(), true);
    assert!(press(&mut typing, Modifiers::NONE).is_empty(), "a text field takes the letter");

    let mut st = with_diff();
    st.modal = Some(Modal::Shortcuts { filter: String::new() });
    assert!(press(&mut harness(st, false), Modifiers::NONE).is_empty(), "the sheet");
}
