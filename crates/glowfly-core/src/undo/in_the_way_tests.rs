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

#[test]
fn redoing_a_new_branch_checkout_wont_write_over_an_ignored_file() {
    let dir = untracked_by_feature("undo-in-the-way-recreate");
    git(&dir, &["checkout", "-q", "feature"]);
    let checkout = record(&dir, &["checkout", "-b", "again", "main"]).expect("recorded");
    run_plan(&dir, plan_with(&dir, &checkout, false, Options { delete_created: true }).unwrap());
    assert!(!dir.join("cfg.local").exists());
    write(&dir, "cfg.local", "mine\n");
    assert_names(plan(&dir, &checkout, true).unwrap(), "cfg.local");
    assert_eq!(read(&dir, "cfg.local"), "mine\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Ignores cfg.local from now on and writes the user's own there.
fn ignored_mine(dir: &Path) {
    std::fs::write(dir.join(".git/info/exclude"), "cfg.local\n").unwrap();
    write(dir, "cfg.local", "mine\n");
}

#[test]
fn putting_a_stash_back_wont_write_over_an_ignored_file() {
    // A hard reset stashed a new staged cfg.local; undoing it pops that back.
    let dir = scratch_repo("undo-in-the-way-pop");
    commit(&dir, "a.txt", "one\n", "c1");
    write(&dir, "cfg.local", "staged\n");
    git(&dir, &["add", "cfg.local"]);
    let reset = super::reset_tests::hard_reset(&dir, "HEAD").expect("recorded");
    assert!(!dir.join("cfg.local").exists());
    ignored_mine(&dir);
    assert_names(plan(&dir, &reset, false).unwrap(), "cfg.local");
    assert_eq!(read(&dir, "cfg.local"), "mine\n");
    let _ = std::fs::remove_dir_all(&dir);

    // Undoing a stash pops it back the same way.
    let dir = scratch_repo("undo-in-the-way-stash");
    commit(&dir, "a.txt", "one\n", "c1");
    write(&dir, "cfg.local", "staged\n");
    git(&dir, &["add", "cfg.local"]);
    let push = record(&dir, &["stash", "push"]).expect("recorded");
    ignored_mine(&dir);
    assert_names(plan(&dir, &push, false).unwrap(), "cfg.local");
    assert_eq!(read(&dir, "cfg.local"), "mine\n");
    std::fs::remove_file(dir.join("cfg.local")).unwrap();
    assert!(matches!(plan(&dir, &push, false).unwrap(), Plan::Run(_)), "with nothing in the way it goes back");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn what_git_handles_itself_isnt_in_the_way() {
    let dir = scratch_repo("undo-in-the-way-handled");
    let c1 = commit(&dir, "Readme.md", "r\n", "c1");
    let repo = || git2::Repository::open(&dir).unwrap();

    // A submodule's folder: checking out a gitlink writes nothing into it.
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    write(&dir, "sub/inside.txt", "the submodule's own\n");
    git(&dir, &["update-index", "--add", "--cacheinfo", &format!("160000,{c1},sub")]);
    git(&dir, &["commit", "-q", "-m", "add sub"]);
    let with_sub = repo().head().unwrap().target().unwrap();
    git(&dir, &["reset", "-q", "--mixed", &c1.to_string()]);
    assert!(in_the_way::in_the_way(&dir, &repo(), with_sub).unwrap().is_empty());
    std::fs::remove_dir_all(dir.join("sub")).unwrap();
    git(&dir, &["reset", "-q", "--hard", &c1.to_string()]);

    // A case-only rename, either way.
    git(&dir, &["mv", "Readme.md", "README.md"]);
    git(&dir, &["commit", "-q", "-m", "rename"]);
    let renamed = repo().head().unwrap().target().unwrap();
    assert!(in_the_way::in_the_way(&dir, &repo(), c1).unwrap().is_empty());
    git(&dir, &["reset", "-q", "--hard", &c1.to_string()]);
    assert!(in_the_way::in_the_way(&dir, &repo(), renamed).unwrap().is_empty());

    // A folder of tracked files where the target has a file; one untracked file in it blocks.
    std::fs::create_dir_all(dir.join("d")).unwrap();
    let with_folder = commit(&dir, "d/a.txt", "a\n", "folder");
    git(&dir, &["rm", "-q", "-r", "d"]);
    let with_file = commit(&dir, "d", "a file now\n", "file");
    git(&dir, &["reset", "-q", "--hard", &with_folder.to_string()]);
    assert!(in_the_way::in_the_way(&dir, &repo(), with_file).unwrap().is_empty());
    write(&dir, "d/mine.txt", "mine\n");
    assert_eq!(in_the_way::in_the_way(&dir, &repo(), with_file).unwrap(), ["d"]);
    let _ = std::fs::remove_dir_all(&dir);
}
