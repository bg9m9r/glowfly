//! The rebase panel's keys and its row menu: what each key sends, and that nothing is taken
//! while an editor is open or a button has the keyboard focus.

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_rebase::with_panel;
use crate::msg::{Msg, RebaseMsg};
use crate::state::AppState;
use crate::state::rebase::Editing;
use crate::state::rebase_steps::NewStep;
use eframe::egui::{Event, Key, Modifiers, PointerButton, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use trellis_core::rebase::RebaseAction::{self, *};
use trellis_core::settings::ThemeChoice;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// Presses `key` with `m` and returns the rebase messages sent.
fn press(h: &mut H, m: Modifiers, key: Key) -> Vec<String> {
    h.state_mut().1.clear();
    h.input_mut().modifiers = m;
    h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: m });
    h.step();
    h.input_mut().modifiers = Modifiers::NONE;
    h.run();
    rebase_msgs(h)
}

fn rebase_msgs(h: &H) -> Vec<String> {
    h.state()
        .1
        .iter()
        .filter_map(|m| match m {
            Msg::Rebase(RebaseMsg::RebaseSelect(i)) => Some(format!("select {i}")),
            Msg::Rebase(RebaseMsg::RebaseSetAction { index, action }) => Some(format!("set {index} {}", action.keyword())),
            Msg::Rebase(RebaseMsg::RebaseMove { from, to }) => Some(format!("move {from} {to}")),
            Msg::Rebase(RebaseMsg::RebaseAddStep { after, step }) => Some(format!("add {after:?} {step:?}")),
            Msg::Rebase(RebaseMsg::RebaseRemoveStep(i)) => Some(format!("remove {i}")),
            Msg::Rebase(RebaseMsg::RebaseEditMessage(i)) => Some(format!("edit {i}")),
            _ => None,
        })
        .collect()
}

fn select(h: &mut H, i: Option<usize>) {
    h.state_mut().0.rebase.as_mut().unwrap().selected = i;
    h.run();
}

const PICKS: [RebaseAction; 6] = [Pick, Pick, Pick, Pick, Pick, Pick];

#[test]
fn keys_send_panel_messages() {
    let (st, dir) = with_panel(ThemeChoice::Dark, "rebase-keys", &PICKS);
    let mut h = harness(st, true);
    h.run();
    assert_eq!(press(&mut h, Modifiers::NONE, Key::ArrowDown), ["select 0"], "Down from no selection");
    assert_eq!(press(&mut h, Modifiers::NONE, Key::ArrowUp), ["select 5"], "Up from no selection");
    select(&mut h, Some(0));
    assert_eq!(press(&mut h, Modifiers::NONE, Key::ArrowDown), ["select 1"]);
    assert!(press(&mut h, Modifiers::NONE, Key::ArrowUp).is_empty(), "already at the top");
    assert_eq!(press(&mut h, Modifiers::NONE, Key::S), ["set 0 squash"]);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::R), ["set 0 reword"]);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::D), ["set 0 drop"]);
    assert_eq!(press(&mut h, Modifiers::ALT, Key::ArrowDown), ["move 0 1"]);
    assert!(press(&mut h, Modifiers::ALT, Key::ArrowUp).is_empty(), "the top row can't move up");
    assert!(press(&mut h, Modifiers::SHIFT, Key::S).is_empty(), "Shift is not tolerated");
    assert_eq!(press(&mut h, Modifiers::NONE, Key::X), [format!("add Some(0) {:?}", NewStep::Exec)]);
    assert_eq!(press(&mut h, Modifiers::NONE, Key::B), [format!("add Some(0) {:?}", NewStep::Break)]);
    assert!(press(&mut h, Modifiers::NONE, Key::Delete).is_empty(), "a commit row can't be removed");
    assert_eq!(press(&mut h, Modifiers::NONE, Key::Enter), ["edit 0"]);

    // An open editor, then a focused button, keep the keys.
    h.state_mut().0.rebase.as_mut().unwrap().editing = Some(Editing::Message(0));
    h.run();
    assert!(press(&mut h, Modifiers::NONE, Key::S).is_empty(), "the editor is open");
    h.state_mut().0.rebase.as_mut().unwrap().editing = None;
    h.run();
    h.get_by_label("Cancel").focus();
    h.run();
    assert!(press(&mut h, Modifiers::NONE, Key::S).is_empty(), "the Cancel button has the focus");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn step_rows_and_the_row_menu() {
    let (mut st, dir) = with_panel(ThemeChoice::Dark, "rebase-keys-steps", &PICKS);
    let p = st.rebase.as_mut().unwrap();
    p.add_step(Some(0), NewStep::Break);
    p.selected = Some(1);
    let mut h = harness(st, true);
    h.run();
    assert_eq!(press(&mut h, Modifiers::NONE, Key::Backspace), ["remove 1"]);
    assert!(press(&mut h, Modifiers::NONE, Key::S).is_empty(), "no action on a step");

    // A right-click on a row offers to add a step after it.
    let b = h.get_by_label("Add app skeleton").bounding_box().expect("the row has bounds");
    let at = pos2(b.x0 as f32 + 200.0, ((b.y0 + b.y1) / 2.0) as f32);
    for pressed in [true, false] {
        h.input_mut().events.push(Event::PointerMoved(at));
        h.input_mut().events.push(Event::PointerButton { pos: at, button: PointerButton::Secondary, pressed, modifiers: Modifiers::NONE });
        h.step();
    }
    h.run();
    h.get_by_label("Add exec step after").click();
    h.run();
    let row = h.state().0.rebase.as_ref().unwrap().items.iter().position(|s| s.commit().is_some_and(|c| c.summary == "Add app skeleton"));
    assert_eq!(rebase_msgs(&h), [format!("add {row:?} {:?}", NewStep::Exec)]);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn keys_still_work_after_clicking_a_row() {
    let (st, dir) = with_panel(ThemeChoice::Dark, "rebase-keys-click", &PICKS);
    let mut h = harness(st, true);
    h.run();
    let b = h.get_by_label("Add login page").bounding_box().expect("the row has bounds");
    let at = pos2(b.x0 as f32 + 300.0, ((b.y0 + b.y1) / 2.0) as f32);
    for pressed in [true, false] {
        h.input_mut().events.push(Event::PointerMoved(at));
        h.input_mut().events.push(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.step();
    }
    h.run();
    assert_eq!(rebase_msgs(&h), ["select 2"], "a click selects the row");
    select(&mut h, Some(2));
    assert_eq!(press(&mut h, Modifiers::NONE, Key::F), ["set 2 fixup"], "the row doesn't keep the keyboard");
    drop(h);
    remove_fixture(&dir);
}
