//! Conflict lists and helpers: the WIP panel's Conflicted Files and Resolved Files, the
//! conflict view with automatic picks, single line picks and the output edited by hand. The
//! warning before marking files with markers left resolved runs the marker check through the
//! runner, so it is the app crate's (`runner_snapshots`).

use super::snapshot_tests::{render, with_conflict};
use crate::msg::{ConflictMsg, Msg};
pub(crate) use crate::snapshot_fixtures::scenes::{fill_lists, merging};
use crate::state::AppState;
use glowfly_core::conflict_pick::ChunkPick;
use glowfly_core::repo::Side;
use std::path::PathBuf;
use std::time::Instant;

#[test]
fn wip_conflicted_and_resolved() {
    let (mut st, dir) = merging("conflict-lists", true);
    fill_lists(&mut st, &dir);
    assert_eq!(st.session.conflict_files.count_note(glowfly_core::i18n::Lang::En, "alpha.txt"), Some(("2 conflicts".into(), false)));
    assert_eq!(st.session.conflict_files.count_note(glowfly_core::i18n::Lang::En, "beta.txt"), Some(("no markers left".into(), true)));
    assert_eq!(st.session.conflict_files.resolved_rows(st.snap.as_ref().unwrap()).len(), 1);
    render("wip_conflicted_and_resolved", st, true);
    super::snapshot_tests::remove_fixture(&dir);
}

#[test]
fn conflict_auto_picked() {
    let settings = |side: &str| {
        let mut lines: Vec<String> = (1..=12).map(|i| format!("setting_{i} = {i}")).collect();
        match side {
            "ours" => {
                lines[2] = "    setting_3 = 3".into();
                lines[8] = "font_size = 13".into();
            }
            "theirs" => {
                lines[2] = "setting_3  =  3".into();
                lines[8] = "font_size = 14".into();
            }
            _ => {}
        }
        (lines.join("\n") + "\n").into_bytes()
    };
    let (st, dir) = with_conflict("conflict-auto", "settings.txt", settings);
    let view = st.session.conflict.view.as_ref().unwrap();
    assert_eq!((view.chunk_count(), view.auto_count(), view.current), (2, 1, 1), "starts on the real conflict");
    render("conflict_auto_picked", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// settings.txt with two conflicts: ours sets one theme line where theirs sets a theme and an
/// accent, then each side sets its own font size.
fn theme_settings(side: &str) -> Vec<u8> {
    let mut lines: Vec<String> = (1..=12).map(|i| format!("setting_{i} = {i}")).collect();
    match side {
        "ours" => {
            lines[2] = "theme = \"dark\"".into();
            lines[8] = "font_size = 13".into();
        }
        "theirs" => {
            lines[2] = "theme = \"solarized\"\naccent = \"teal\"".into();
            lines[8] = "font_size = 14".into();
        }
        _ => {}
    }
    (lines.join("\n") + "\n").into_bytes()
}

fn send(st: &mut AppState, m: ConflictMsg) {
    crate::update::update(st, Msg::Conflict(m), Instant::now());
}

#[test]
fn conflict_line_picks() {
    let (mut st, dir) = with_conflict("conflict-line-picks", "settings.txt", theme_settings);
    send(&mut st, ConflictMsg::ConflictToggleLine { chunk: 0, side: Side::Theirs, line: 1 });
    send(&mut st, ConflictMsg::ConflictToggleLine { chunk: 0, side: Side::Ours, line: 0 });
    let view = st.session.conflict.view.as_ref().unwrap();
    assert!(matches!(view.picks[0], ChunkPick::Lines(_)) && view.picks[1].is_open());
    render("conflict_line_picks", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// The conflict view with the output in the editor and the second conflict typed by hand.
fn edited(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_conflict(name, "settings.txt", theme_settings);
    send(&mut st, ConflictMsg::ConflictEditOutput(true));
    let e = st.session.conflict.view.as_mut().unwrap().edit.as_mut().unwrap();
    let r = e.rendered.chunks[1].clone();
    e.buffer.replace_range(r, "font_size = 15\n");
    send(&mut st, ConflictMsg::ConflictOutputEdited);
    let view = st.session.conflict.view.as_ref().unwrap();
    assert!(view.picks[1].is_edited() && view.can_save_partial());
    (st, dir)
}

#[test]
fn conflict_output_editing() {
    let (st, dir) = edited("conflict-output-editing");
    render("conflict_output_editing", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn conflict_output_edited() {
    let (mut st, dir) = edited("conflict-output-edited");
    send(&mut st, ConflictMsg::ConflictEditOutput(false));
    render("conflict_output_edited", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}
