//! Undoing and redoing conflict resolutions saved in the conflict view against real merges:
//! the conflict's stages and the exact working bytes come back, redo resolves again, and a
//! repository that changed since refuses with nothing written.

use super::*;
use crate::conflict_pick::{ChunkPick, render_picks};
use crate::gitcli::run;
use crate::repo::{self, Choice, ConflictIds, load_conflict, render_output, write_partial_resolution, write_resolution};
use crate::testutil::*;
use git2::Repository;
use std::path::{Path, PathBuf};

/// `file` mid-merge of `other`, its sides from `content("base" | "ours" | "theirs")`, with
/// `config` set before any file is written.
fn merging(name: &str, file: &str, config: &[(&str, &str)], content: impl Fn(&str) -> String) -> PathBuf {
    let dir = scratch_repo(name);
    set_config(&dir, config);
    write(&dir, file, &content("base"));
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    write(&dir, file, &content("theirs"));
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, file, &content("ours"));
    git(&dir, &["commit", "-q", "-am", "ours"]);
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok, "merge should conflict");
    dir
}

/// a.txt with one conflicting line.
fn one_chunk(name: &str) -> PathBuf {
    merging(name, "a.txt", &[], |side| format!("one\n{side}\n"))
}

/// f.txt with lines 2 and 10 changed differently on each side: two chunks.
fn two_chunks(name: &str) -> PathBuf {
    let base: String = (1..=12).map(|i| format!("line {i}\n")).collect();
    merging(name, "f.txt", &[], move |s| {
        if s == "base" {
            base.clone()
        } else {
            base.replace("line 2\n", &format!("line 2 {s}\n")).replace("line 10\n", &format!("line 10 {s}\n"))
        }
    })
}

fn bytes(dir: &Path, f: &str) -> Vec<u8> {
    std::fs::read(dir.join(f)).unwrap()
}

fn conflict(dir: &Path, f: &str) -> Option<ConflictIds> {
    repo::conflict_ids(&Repository::open(dir).unwrap(), f)
}

fn stage0(dir: &Path, f: &str) -> Option<git2::Oid> {
    Repository::open(dir).unwrap().index().unwrap().get_path(Path::new(f), 0).map(|e| e.id)
}

fn resolved(dir: &Path) -> Vec<(String, bool)> {
    repo::resolved_paths(dir).unwrap().into_iter().map(|r| (r.entry.path, r.both_sides)).collect()
}

/// Saves `f` resolved with `choices`, as Save in the conflict view does.
fn save(dir: &Path, f: &str, choices: &[Choice]) -> Action {
    let file = load_conflict(dir, f).unwrap();
    let out = render_output(&file, choices).unwrap();
    resolve_saving(dir, f, &file.ids, || write_resolution(dir, f, &out, &file.ids)).unwrap().expect("undoable")
}

/// Saves `f` with `picks`, markers left where a chunk has none, as Save with markers does.
fn save_markers(dir: &Path, f: &str, picks: &[ChunkPick]) -> Option<Action> {
    let file = load_conflict(dir, f).unwrap();
    let out = render_picks(&file, picks, true, false, None).unwrap();
    resolve_saving(dir, f, &file.ids, || write_partial_resolution(dir, f, &out, &file.ids)).unwrap()
}

/// Plans and carries out an undo (or redo) of `a`; panics on a refusal.
fn apply(dir: &Path, a: &Action, redo: bool) {
    match plan(dir, a, redo).unwrap() {
        Plan::RunThenWrite { cmds, path, content } => {
            for cmd in &cmds {
                let (ok, out) = run(dir, cmd);
                assert!(ok, "{cmd:?} failed: {out}");
            }
            write_saved(dir, &path, content).unwrap();
        }
        Plan::Write { path, content } => write_saved(dir, &path, content).unwrap(),
        other => panic!("expected a plan to carry out, got {other:?}"),
    }
}

fn stale_message(dir: &Path, a: &Action, redo: bool) -> String {
    match plan(dir, a, redo).unwrap() {
        Plan::Refuse(Refusal::Stale(m)) => {
            assert!(m.ends_with("; removed from the undo history"), "{m}");
            m
        }
        other => panic!("expected a stale refusal, got {other:?}"),
    }
}

#[test]
fn resolution_undo_and_redo_round_trip() {
    let dir = one_chunk("rsv-round-trip");
    let markers = bytes(&dir, "a.txt");
    let ids = load_conflict(&dir, "a.txt").unwrap().ids;
    let a = save(&dir, "a.txt", &[Choice::Theirs]);
    let Action::Resolve { staged: Some(staged), after, .. } = &a else { panic!("a full resolution: {a:?}") };
    assert_eq!(staged.mode, 0o100644);
    assert_eq!(Some(staged.blob), stage0(&dir, "a.txt"));
    assert_eq!(a.describe(), "resolution of a.txt");
    let (staged, after) = (*staged, after.unwrap());

    apply(&dir, &a, false);
    assert_eq!(conflict(&dir, "a.txt"), Some(ids.clone()));
    assert_eq!(bytes(&dir, "a.txt"), markers);
    assert!(resolved(&dir).is_empty(), "the record went back into the index");

    apply(&dir, &a, true);
    assert_eq!(conflict(&dir, "a.txt"), None);
    assert_eq!(stage0(&dir, "a.txt"), Some(staged.blob));
    assert_eq!(git2::Oid::hash_object(git2::ObjectType::Blob, &bytes(&dir, "a.txt")).unwrap(), after.blob);
    assert_eq!(bytes(&dir, "a.txt"), b"one\ntheirs\n");
    assert_eq!(resolved(&dir), [("a.txt".to_owned(), true)], "update-index records resolve-undo");

    apply(&dir, &a, false);
    assert_eq!(conflict(&dir, "a.txt"), Some(ids));
    assert_eq!(bytes(&dir, "a.txt"), markers);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_brings_back_picks_saved_with_markers() {
    let dir = two_chunks("rsv-picks");
    let markers = bytes(&dir, "f.txt");
    let ids = load_conflict(&dir, "f.txt").unwrap().ids;
    let partial = save_markers(&dir, "f.txt", &[Choice::Ours.into(), ChunkPick::default()]).expect("undoable");
    assert!(matches!(partial, Action::Resolve { staged: None, .. }));
    assert_eq!(partial.describe(), "save of f.txt with markers");
    let partial_bytes = bytes(&dir, "f.txt");
    let full = save(&dir, "f.txt", &[Choice::Ours, Choice::Theirs]);

    apply(&dir, &full, false);
    assert_eq!(bytes(&dir, "f.txt"), partial_bytes);
    assert_eq!(conflict(&dir, "f.txt"), Some(ids.clone()));
    let picks = repo::saved_picks(&dir, &load_conflict(&dir, "f.txt").unwrap()).expect("the pick comes back");
    assert_eq!(picks.picks, [Choice::Ours.into(), ChunkPick::default()]);

    apply(&dir, &partial, false);
    assert_eq!(bytes(&dir, "f.txt"), markers);
    assert_eq!(conflict(&dir, "f.txt"), Some(ids.clone()));

    apply(&dir, &partial, true);
    assert_eq!(bytes(&dir, "f.txt"), partial_bytes);
    assert_eq!(conflict(&dir, "f.txt"), Some(ids));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stale_resolution_is_refused() {
    // (a) The working file was edited after the save.
    let dir = one_chunk("rsv-stale-edit");
    let a = save(&dir, "a.txt", &[Choice::Theirs]);
    let Action::Resolve { before: Some(before), .. } = &a else { panic!("{a:?}") };
    write(&dir, "a.txt", "edited\n");
    let m = stale_message(&dir, &a, false);
    assert!(m.contains("a.txt has changed since") && m.contains(&before.blob.to_string()), "{m}");
    assert_eq!(bytes(&dir, "a.txt"), b"edited\n");
    let _ = std::fs::remove_dir_all(&dir);

    // (b) Edited and staged.
    let dir = one_chunk("rsv-stale-index");
    let a = save(&dir, "a.txt", &[Choice::Theirs]);
    write(&dir, "a.txt", "edited\n");
    git(&dir, &["add", "a.txt"]);
    assert!(stale_message(&dir, &a, false).contains("a.txt has changed in the index since"));
    assert_eq!(bytes(&dir, "a.txt"), b"edited\n");
    let _ = std::fs::remove_dir_all(&dir);

    // (c) The merge was committed.
    let dir = one_chunk("rsv-stale-head");
    let a = save(&dir, "a.txt", &[Choice::Theirs]);
    git(&dir, &["commit", "-q", "-m", "merge"]);
    assert!(stale_message(&dir, &a, false).contains("HEAD has moved since the resolution of a.txt"));
    assert_eq!(bytes(&dir, "a.txt"), b"one\ntheirs\n");
    let _ = std::fs::remove_dir_all(&dir);

    // (d) git's resolve-undo record was cleared.
    let dir = one_chunk("rsv-stale-reuc");
    let a = save(&dir, "a.txt", &[Choice::Theirs]);
    if run(&dir, &args(&["update-index", "--clear-resolve-undo"])).0 {
        assert!(stale_message(&dir, &a, false).contains("git no longer keeps the conflict of a.txt, so it can't be brought back"));
        assert_eq!(bytes(&dir, "a.txt"), b"one\ntheirs\n");
    } else {
        eprintln!("skipped: this git has no update-index --clear-resolve-undo");
    }
    let _ = std::fs::remove_dir_all(&dir);

    // (e) Redo after the conflict was resolved outside.
    let dir = one_chunk("rsv-stale-redo");
    let a = save(&dir, "a.txt", &[Choice::Theirs]);
    apply(&dir, &a, false);
    let markers = bytes(&dir, "a.txt");
    git(&dir, &["add", "a.txt"]);
    assert!(stale_message(&dir, &a, true).contains("The conflict in a.txt changed since it was saved"));
    assert_eq!(bytes(&dir, "a.txt"), markers);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn crlf_bytes_come_back_exactly() {
    let dir = merging("rsv-crlf", "a.txt", &[("core.autocrlf", "true")], |side| format!("one\n{side}\n"));
    let markers = bytes(&dir, "a.txt");
    assert!(markers.windows(2).any(|w| w == b"\r\n"), "checked out with CR LF");
    let a = save(&dir, "a.txt", &[Choice::Ours]);
    apply(&dir, &a, false);
    assert_eq!(bytes(&dir, "a.txt"), markers);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_entry_when_markers_save_changes_nothing() {
    let dir = two_chunks("rsv-nothing");
    let picks = [Choice::Ours.into(), ChunkPick::default()];
    assert!(save_markers(&dir, "f.txt", &picks).is_some());
    let saved = bytes(&dir, "f.txt");
    assert_eq!(save_markers(&dir, "f.txt", &picks), None, "the same bytes again");
    assert_eq!(bytes(&dir, "f.txt"), saved);
    let _ = std::fs::remove_dir_all(&dir);
}
