//! The Open in external editor key (Ctrl+Shift+E, Cmd+Shift+E on macOS): it opens the shown
//! diff's file only while a diff is shown and no text field has the keyboard, and not in the
//! conflict view, blame or under the shortcut sheet.

use super::fold_key_tests::{Keys, harness, with_diff};
use eframe::egui::{Event, Key, Modifiers};
use egui_kittest::Harness;
use glowfly_core::settings::Settings;
use glowfly_model::msg::Msg;
use glowfly_model::state::blame::BlameView;
use glowfly_model::state::conflict::ConflictView;
use glowfly_model::state::{AppState, Modal};
use glowfly_model::tools::ToolsMsg;
use std::time::Instant;

/// Presses E with `m` and returns the paths of the `OpenFile` messages (all with no line).
fn press(h: &mut Harness<'static, Keys>, m: Modifiers) -> Vec<String> {
    h.state_mut().msgs.clear();
    let input = h.input_mut();
    input.modifiers = m;
    input.events.push(Event::Key { key: Key::E, physical_key: None, pressed: true, repeat: false, modifiers: m });
    h.step();
    h.input_mut().modifiers = Modifiers::NONE;
    std::mem::take(&mut h.state_mut().msgs)
        .into_iter()
        .filter_map(|msg| match msg {
            Msg::Tools(ToolsMsg::OpenFile { path, line: None }) => Some(path),
            _ => None,
        })
        .collect()
}

#[test]
fn the_key_opens_the_shown_file() {
    let mut h = harness(with_diff(), false);
    assert_eq!(press(&mut h, Modifiers::COMMAND | Modifiers::SHIFT), ["f.txt"]);
    assert!(press(&mut h, Modifiers::NONE).is_empty(), "E alone folds, it opens nothing");
    assert!(press(&mut h, Modifiers::COMMAND).is_empty(), "without Shift it is another key");
}

#[test]
fn the_key_stays_out_of_other_views() {
    let chord = Modifiers::COMMAND | Modifiers::SHIFT;
    let mut none = harness(AppState::new(Settings::default(), Vec::new(), Instant::now()), false);
    assert!(press(&mut none, chord).is_empty(), "no diff");

    let mut st = with_diff();
    st.session.conflict.view = Some(ConflictView::loading("f.txt".into()));
    assert!(press(&mut harness(st, false), chord).is_empty(), "conflict view");

    let mut st = with_diff();
    st.session.blame.view = Some(BlameView::loading("f.txt".into(), None));
    assert!(press(&mut harness(st, false), chord).is_empty(), "blame");

    let mut typing = harness(with_diff(), true);
    assert!(press(&mut typing, chord).is_empty(), "a text field has the keyboard");

    let mut st = with_diff();
    st.modal = Some(Modal::Shortcuts { filter: String::new() });
    assert!(press(&mut harness(st, false), chord).is_empty(), "the sheet");
}
