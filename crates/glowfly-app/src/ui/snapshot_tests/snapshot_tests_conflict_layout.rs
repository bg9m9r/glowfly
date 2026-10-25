//! The conflict view's layout: long unchanged runs folded and opened, long lines wrapped, clean
//! lines edited in the output, and picks brought back from a file saved with markers. Shares
//! the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{render, with_conflict};
use crate::msg::{ConflictMsg, Msg};
use crate::state::AppState;
use glowfly_core::conflict_pick::ChunkPick;
use glowfly_core::repo::{self, Choice, Segment};
use std::time::Instant;

fn send(st: &mut AppState, m: ConflictMsg) {
    crate::update::update(st, Msg::Conflict(m), Instant::now());
}

/// A 60-line settings file whose sides set lines 5 and 50 differently.
fn long_settings(side: &str) -> Vec<u8> {
    let mut lines: Vec<String> = (1..=60).map(|i| format!("setting_{i} = {i}")).collect();
    match side {
        "ours" => {
            lines[4] = "theme = \"dark\"".into();
            lines[49] = "font_size = 13".into();
        }
        "theirs" => {
            lines[4] = "theme = \"solarized\"".into();
            lines[49] = "font_size = 14".into();
        }
        _ => {}
    }
    (lines.join("\n") + "\n").into_bytes()
}

#[test]
fn conflict_folded() {
    let (mut st, dir) = with_conflict("conflict-folded", "settings.txt", long_settings);
    let view = st.conflict.as_mut().unwrap();
    assert_eq!(view.chunk_count(), 2);
    view.picks[0] = Choice::Ours.into();
    render("conflict_folded", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn conflict_fold_opened() {
    let (mut st, dir) = with_conflict("conflict-fold-opened", "settings.txt", long_settings);
    let segs = st.conflict.as_ref().unwrap().segments().unwrap().to_vec();
    assert!(matches!(&segs[2], Segment::Clean(lines) if lines.len() == 44), "the run between the conflicts");
    send(&mut st, ConflictMsg::ConflictFold { seg: 2, open: true });
    render("conflict_fold_opened", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// A 200-character line.
fn long_line(what: &str) -> String {
    let mut s = format!("{what} = \"");
    while s.len() < 198 {
        s.push_str("a long value that keeps going ");
    }
    s.truncate(198);
    s + "\""
}

#[test]
fn conflict_wrapped() {
    let settings = |side: &str| {
        let mut lines: Vec<String> = (1..=12).map(|i| format!("setting_{i} = {i}")).collect();
        lines[1] = long_line("description");
        match side {
            "ours" => lines[5] = "theme = \"dark\"".into(),
            "theirs" => lines[5] = long_line("theme"),
            _ => {}
        }
        (lines.join("\n") + "\n").into_bytes()
    };
    let (mut st, dir) = with_conflict("conflict-wrapped", "settings.txt", settings);
    st.settings.diff_word_wrap = true;
    st.conflict.as_mut().unwrap().picks[0] = Choice::OursThenTheirs.into();
    render("conflict_wrapped", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// settings.txt with conflicts at lines 3 and 9.
fn short_settings(side: &str) -> Vec<u8> {
    let mut lines: Vec<String> = (1..=12).map(|i| format!("setting_{i} = {i}")).collect();
    match side {
        "ours" => {
            lines[2] = "theme = \"dark\"".into();
            lines[8] = "font_size = 13".into();
        }
        "theirs" => {
            lines[2] = "theme = \"solarized\"".into();
            lines[8] = "font_size = 14".into();
        }
        _ => {}
    }
    (lines.join("\n") + "\n").into_bytes()
}

#[test]
fn conflict_output_clean_edit() {
    let (mut st, dir) = with_conflict("conflict-clean-edit", "settings.txt", short_settings);
    send(&mut st, ConflictMsg::ConflictEditOutput(true));
    let e = st.conflict.as_mut().unwrap().edit.as_mut().unwrap();
    let at = e.buffer.find("setting_6 = 6\n").unwrap();
    e.buffer.replace_range(at..at + "setting_6 = 6\n".len(), "setting_6 = 60\nsetting_6b = 61\n");
    send(&mut st, ConflictMsg::ConflictOutputEdited);
    send(&mut st, ConflictMsg::ConflictEditOutput(false));
    let view = st.conflict.as_ref().unwrap();
    assert_eq!(view.cleans.len(), 1);
    render("conflict_output_clean_edit", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn conflict_restored() {
    let (mut st, dir) = with_conflict("conflict-restored", "settings.txt", short_settings);
    let view = st.conflict.as_mut().unwrap();
    view.picks[0] = Choice::Theirs.into();
    let f = view.file.clone().unwrap();
    repo::write_partial_resolution(&dir, "settings.txt", &view.partial_output().unwrap(), &f.ids).unwrap();
    let f = repo::load_conflict(&dir, "settings.txt").unwrap();
    let saved = repo::saved_picks(&dir, &f);
    assert_eq!(saved.as_ref().map(|s| s.picks.clone()), Some(vec![Choice::Theirs.into(), ChunkPick::default()]));
    let view = st.conflict.as_mut().unwrap();
    view.set_file_saved(f, saved);
    assert_eq!((view.restored, view.current, view.user_choices()), (1, 1, 0));
    render("conflict_restored", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}
