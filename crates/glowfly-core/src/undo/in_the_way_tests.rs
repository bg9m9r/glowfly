//! An undo or redo that moves HEAD with `reset --keep`, `reset --hard` or a checkout is refused,
//! naming the files, when an untracked or ignored file (or a file where a folder goes) sits at a
//! path the target tracks and HEAD doesn't: git would write over it without keeping a copy.

use super::tests::{commit, record, refused, run_plan};
use super::*;
use crate::testutil::*;
use std::path::Path;

fn read(dir: &Path, f: &str) -> String {
    std::fs::read_to_string(dir.join(f)).unwrap().replace("\r\n", "\n")
}

/// main at c1 with cfg.local tracked; feature at c2 untracks and ignores it.
fn untracked_by_feature(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    commit(&dir, "cfg.local", "old\n", "c1");
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    git(&dir, &["rm", "-q", "--cached", "cfg.local"]);
    write(&dir, ".gitignore", "cfg.local\n");
    git(&dir, &["add", ".gitignore"]);
    git(&dir, &["commit", "-q", "-m", "c2"]);
    git(&dir, &["checkout", "-q", "main"]);
    dir
}

fn assert_names(plan: Plan, file: &str) {
    let r = refused(plan);
    assert!(matches!(r, Refusal::Unsafe(_)), "no stash offer: stashing can't move an ignored file: {r:?}");
    assert!(r.message().starts_with(&format!("Move {file} away first")), "{}", r.message());
}

#[test]
fn a_merge_undo_wont_write_over_an_ignored_file() {
    let dir = untracked_by_feature("undo-in-the-way-merge");
    let merge = record(&dir, &["merge", "-q", "feature"]).expect("recorded");
    assert!(!dir.join("cfg.local").exists(), "the merge removed the tracked file");
    write(&dir, "cfg.local", "mine\n");
    assert_names(plan(&dir, &merge, false).unwrap(), "cfg.local");
    assert_eq!(read(&dir, "cfg.local"), "mine\n");

    std::fs::remove_file(dir.join("cfg.local")).unwrap();
    run_plan(&dir, plan(&dir, &merge, false).unwrap());
    assert_eq!(read(&dir, "cfg.local"), "old\n", "with nothing in the way it goes back");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hard_reset_undo_and_redo_wont_write_over_an_ignored_file() {
    let dir = untracked_by_feature("undo-in-the-way-reset");
    // Undo: main reset to feature, which doesn't track cfg.local; the undo goes back to main.
    let reset = record(&dir, &["reset", "--hard", "feature"]).expect("recorded");
    write(&dir, "cfg.local", "mine\n");
    assert_names(plan(&dir, &reset, false).unwrap(), "cfg.local");
    assert_eq!(read(&dir, "cfg.local"), "mine\n");
    std::fs::remove_file(dir.join("cfg.local")).unwrap();
    run_plan(&dir, plan(&dir, &reset, false).unwrap());
    // Redo: feature reset to main, undone, then redone onto a new cfg.local.
    git(&dir, &["checkout", "-q", "feature"]);
    let reset = record(&dir, &["reset", "--hard", "main"]).expect("recorded");
    run_plan(&dir, plan(&dir, &reset, false).unwrap());
    write(&dir, "cfg.local", "mine\n");
    assert_names(plan(&dir, &reset, true).unwrap(), "cfg.local");
    assert_eq!(read(&dir, "cfg.local"), "mine\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_where_a_tracked_folder_goes_is_in_the_way() {
    let dir = scratch_repo("undo-in-the-way-folder");
    commit(&dir, "a.txt", "one\n", "c1");
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    std::fs::create_dir(dir.join("d")).unwrap();
    commit(&dir, "d/x.txt", "x\n", "c2");
    git(&dir, &["checkout", "-q", "main"]);
    let checkout = record(&dir, &["checkout", "feature"]).expect("recorded");
    run_plan(&dir, plan(&dir, &checkout, false).unwrap());
    write(&dir, "d", "a file, not a folder\n");
    assert_names(plan(&dir, &checkout, true).unwrap(), "d/x.txt");
    let _ = std::fs::remove_dir_all(&dir);
}
