//! Headless tests for resizing graph columns: a drag's width is stored clamped and saved once,
//! and a double-click restores the default.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::{GraphMsg, Msg, PrefsMsg};
use crate::state::AppState;
use std::time::Instant;
use trellis_core::settings::ResizableColumn::{self, *};

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// A state with the settings as saved, so only the messages below count.
fn synced() -> AppState {
    let mut st = state();
    run(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited));
    st
}

fn resize(st: &mut AppState, col: ResizableColumn, width: f32) -> Vec<Effect> {
    run(st, Msg::Graph(GraphMsg::ResizeColumn { col, width }))
}

fn reset(st: &mut AppState, col: ResizableColumn) -> Vec<Effect> {
    run(st, Msg::Graph(GraphMsg::ResetColumnWidth(col)))
}

#[test]
fn resize_column_saves_once() {
    let mut st = synced();
    let fx = resize(&mut st, Author, 220.0);
    let [Effect::SaveSettings(s)] = fx.as_slice() else { panic!("expected one save, got {fx:?}") };
    assert_eq!(s.column_widths.get(Author), 220.0);
    for col in [Refs, Sha, Changes, Date] {
        assert_eq!(s.column_widths.get(col), col.default_width(), "{col:?}");
    }
    assert!(resize(&mut st, Author, 220.0).is_empty(), "the same width saves nothing");
    assert!(run(&mut st, Msg::Prefs(PrefsMsg::SettingsEdited)).is_empty(), "no second save");
}

#[test]
fn resize_column_clamps() {
    let mut st = synced();
    let fx = resize(&mut st, Refs, 5000.0);
    assert!(matches!(fx.as_slice(), [Effect::SaveSettings(_)]), "{fx:?}");
    assert_eq!(st.settings.column_widths.get(Refs), 480.0);
    assert!(resize(&mut st, Sha, f32::NAN).is_empty());
    assert_eq!(st.settings.column_widths.get(Sha), 74.0);
}

#[test]
fn reset_column_width() {
    let mut st = synced();
    resize(&mut st, Author, 220.0);
    let fx = reset(&mut st, Author);
    let [Effect::SaveSettings(s)] = fx.as_slice() else { panic!("expected one save, got {fx:?}") };
    assert_eq!(s.column_widths.get(Author), 148.0);
    assert!(reset(&mut st, Author).is_empty());
    assert!(reset(&mut st, Date).is_empty(), "an untouched column is already at its default");
}
