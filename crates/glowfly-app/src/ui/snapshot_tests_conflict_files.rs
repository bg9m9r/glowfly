//! Conflict lists and helpers: the WIP panel's Conflicted Files and Resolved Files, the
//! conflict view with automatic picks, single line picks and the output edited by hand, and the
//! warning before marking files with markers left resolved.

use super::snapshot_tests::{git, render, with_conflict, with_repo};
use crate::msg::{ConflictMsg, Msg};
use crate::state::conflict_files::conflicted_paths;
use crate::state::{AppState, Selection};
use glowfly_core::conflict_pick::ChunkPick;
use glowfly_core::gitcli::{args, run_with_input};
use glowfly_core::repo::{self, LoadOptions, Side};
use glowfly_core::settings::ThemeChoice;
use std::path::{Path, PathBuf};
use std::time::Instant;

const DATE: &str = "2026-01-11T10:00:00+00:00";

/// `file`'s text on `side` ("base", "ours" or "theirs"): numbered lines, with `changed` lines
/// rewritten on the two sides.
fn text(file: &str, lines: usize, changed: &[usize], side: &str) -> String {
    (1..=lines)
        .map(|i| if side != "base" && changed.contains(&i) { format!("{file} {i} {side}\n") } else { format!("{file} {i}\n") })
        .collect()
}

/// The fixture mid-merge with alpha.txt (two conflicts), beta.txt (one) and gamma.txt (one,
/// resolved with `git add`). With `fix_beta`, beta.txt's markers are edited away.
pub(super) fn merging(name: &str, fix_beta: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    let files = [("alpha.txt", 10, vec![2, 8]), ("beta.txt", 4, vec![2]), ("gamma.txt", 4, vec![3])];
    let write = |side: &str| {
        for (f, n, changed) in &files {
            std::fs::write(dir.join(f), text(f, *n, changed, side)).unwrap();
        }
    };
    write("base");
    git(&dir, &["add", "alpha.txt", "beta.txt", "gamma.txt"], DATE);
    git(&dir, &["commit", "-q", "-m", "Conflict lists: base"], DATE);
    git(&dir, &["checkout", "-q", "-b", "list-side"], DATE);
    write("theirs");
    git(&dir, &["add", "alpha.txt", "beta.txt", "gamma.txt"], DATE);
    git(&dir, &["commit", "-q", "-m", "Conflict lists: theirs"], DATE);
    git(&dir, &["checkout", "-q", "main"], DATE);
    write("ours");
    git(&dir, &["add", "alpha.txt", "beta.txt", "gamma.txt"], DATE);
    git(&dir, &["commit", "-q", "-m", "Conflict lists: ours"], DATE);
    let (ok, _) = run_with_input(&dir, &args(&["merge", "list-side"]), "", &[]);
    assert!(!ok, "the merge conflicts");
    if fix_beta {
        std::fs::write(dir.join("beta.txt"), "beta.txt 1\nbeta.txt 2 both\nbeta.txt 3\nbeta.txt 4\n").unwrap();
    }
    git(&dir, &["add", "gamma.txt"], DATE);
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    st.selection = Selection::Wip;
    (st, dir)
}

pub(super) fn fill_lists(st: &mut AppState, dir: &Path) {
    let paths = conflicted_paths(st.snap.as_ref().unwrap());
    let summary = repo::conflict_summary(dir, &paths, true).unwrap();
    st.conflict_files.counts = summary.counts.into_iter().collect();
    st.conflict_files.resolved = summary.resolved;
}

#[test]
fn wip_conflicted_and_resolved() {
    let (mut st, dir) = merging("conflict-lists", true);
    fill_lists(&mut st, &dir);
    assert_eq!(st.conflict_files.count_note(glowfly_core::i18n::Lang::En, "alpha.txt"), Some(("2 conflicts".into(), false)));
    assert_eq!(st.conflict_files.count_note(glowfly_core::i18n::Lang::En, "beta.txt"), Some(("no markers left".into(), true)));
    assert_eq!(st.conflict_files.resolved_rows(st.snap.as_ref().unwrap()).len(), 1);
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
    let view = st.conflict.as_ref().unwrap();
    assert_eq!((view.chunk_count(), view.auto_count(), view.current), (2, 1, 1), "starts on the real conflict");
    render("conflict_auto_picked", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn conflict_markers_warning() {
    let (mut st, dir) = merging("conflict-markers-warning", false);
    let paths = vec!["alpha.txt".to_owned(), "beta.txt".to_owned()];
    let fx = crate::update::update(&mut st, Msg::Conflict(ConflictMsg::MarkResolved(paths)), Instant::now());
    for effect in fx {
        if let Some(m) = crate::runner::execute(effect) {
            crate::update::update(&mut st, m, Instant::now());
        }
    }
    assert!(matches!(&st.modal, Some(crate::state::Modal::Confirm { title, .. }) if title == "Conflict markers left"));
    render("conflict_markers_warning", st, true);
    super::snapshot_tests::remove_fixture(&dir);
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
    let view = st.conflict.as_ref().unwrap();
    assert!(matches!(view.picks[0], ChunkPick::Lines(_)) && view.picks[1].is_open());
    render("conflict_line_picks", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// The conflict view with the output in the editor and the second conflict typed by hand.
fn edited(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_conflict(name, "settings.txt", theme_settings);
    send(&mut st, ConflictMsg::ConflictEditOutput(true));
    let e = st.conflict.as_mut().unwrap().edit.as_mut().unwrap();
    let r = e.rendered.chunks[1].clone();
    e.buffer.replace_range(r, "font_size = 15\n");
    send(&mut st, ConflictMsg::ConflictOutputEdited);
    let view = st.conflict.as_ref().unwrap();
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
