//! Tests of what the probes tell from readings on disk.

use super::super::super::fixture::{Fixture, commit_files, git, git_try, write};
use super::super::action::Action;
use super::{read, took_effect};
use std::path::PathBuf;

fn repo(name: &str) -> (Fixture, PathBuf) {
    let fx = Fixture::new(&format!("probe-{name}"));
    let dir = fx.work().join("r");
    fx.init(&dir, &[("a.txt", "base\n"), ("sub/b.txt", "b\n")]);
    (fx, dir)
}

#[test]
fn commit_probe() {
    let (_fx, dir) = repo("commit");
    let before = read(&dir);
    commit_files(&dir, &[("a.txt", "base\nmore\n")], "typed text");
    let after = read(&dir);
    assert_eq!(took_effect(&Action::Commit("typed text".into()), &before, &after), Some(true));
    assert_eq!(took_effect(&Action::Commit("other".into()), &before, &after), Some(false));
    assert_eq!(took_effect(&Action::Commit("typed text".into()), &after, &after), Some(false));
}

#[test]
fn amend_probe() {
    let (_fx, dir) = repo("amend");
    let before = read(&dir);
    git(&dir, &["commit", "-q", "--amend", "-m", "reworded"]);
    let after = read(&dir);
    assert_eq!(took_effect(&Action::Amend, &before, &after), Some(true));
    assert_eq!(took_effect(&Action::Amend, &after, &after), Some(false));
}

#[test]
fn stash_probe() {
    let (_fx, dir) = repo("stash");
    write(&dir, "a.txt", "changed\n");
    let before = read(&dir);
    git(&dir, &["stash", "push", "-q"]);
    let after = read(&dir);
    assert_eq!(took_effect(&Action::Stash, &before, &after), Some(true));
    assert_eq!(took_effect(&Action::DropStash, &after, &before), Some(true));
    assert_eq!(took_effect(&Action::Stash, &after, &after), Some(false));
}

#[test]
fn checkout_probe() {
    let (_fx, dir) = repo("checkout");
    git(&dir, &["branch", "side"]);
    let before = read(&dir);
    assert_eq!(took_effect(&Action::CreateBranch("side".into()), &before, &before), Some(true));
    assert_eq!(took_effect(&Action::Checkout("side".into()), &before, &before), Some(false));
    git(&dir, &["checkout", "-q", "side"]);
    let after = read(&dir);
    assert_eq!(took_effect(&Action::Checkout("side".into()), &before, &after), Some(true));
    assert_eq!(took_effect(&Action::DeleteBranch("side".into()), &before, &after), Some(false));
}

#[test]
fn discard_probe() {
    let (_fx, dir) = repo("discard");
    write(&dir, "sub/b.txt", "edit\n");
    write(&dir, "new.txt", "new\n");
    let before = read(&dir);
    assert!(before.changed.contains("sub/b.txt") && before.changed.contains("new.txt"));
    git(&dir, &["checkout", "-q", "--", "sub/b.txt"]);
    let after = read(&dir);
    assert_eq!(took_effect(&Action::Discard("sub/b.txt".into()), &before, &after), Some(true));
    assert_eq!(took_effect(&Action::Discard("new.txt".into()), &before, &after), Some(false));
}

#[test]
fn stage_probes() {
    let (_fx, dir) = repo("stage");
    write(&dir, "a.txt", "edit\n");
    write(&dir, "new.txt", "new\n");
    let before = read(&dir);
    git(&dir, &["add", "a.txt"]);
    let one = read(&dir);
    assert_eq!(took_effect(&Action::Stage("a.txt".into()), &before, &one), Some(true));
    assert_eq!(took_effect(&Action::StageAll, &before, &one), Some(false));
    git(&dir, &["add", "-A"]);
    let all = read(&dir);
    assert_eq!(took_effect(&Action::StageAll, &one, &all), Some(true));
    assert_eq!(took_effect(&Action::Unstage("a.txt".into()), &all, &all), Some(false));
}

#[test]
fn merge_conflict_counts_as_effect() {
    let (_fx, dir) = repo("merge");
    git(&dir, &["branch", "other"]);
    commit_files(&dir, &[("a.txt", "mine\n")], "mine");
    git(&dir, &["checkout", "-q", "other"]);
    commit_files(&dir, &[("a.txt", "theirs\n")], "theirs");
    git(&dir, &["checkout", "-q", "-"]);
    let before = read(&dir);
    assert!(!git_try(&dir, &["merge", "other"]));
    let after = read(&dir);
    assert_eq!(took_effect(&Action::Merge("other".into()), &before, &after), Some(true));
    assert!(after.conflicted.contains("a.txt"));
    git(&dir, &["checkout", "--ours", "a.txt"]);
    git(&dir, &["add", "a.txt"]);
    let resolved = read(&dir);
    assert_eq!(took_effect(&Action::ResolveOurs("a.txt".into()), &after, &resolved), Some(true));
    assert_eq!(took_effect(&Action::ResolveTheirs("a.txt".into()), &after, &after), Some(false));
    git(&dir, &["merge", "--abort"]);
    assert_eq!(took_effect(&Action::Abort, &after, &read(&dir)), Some(true));
    assert_eq!(took_effect(&Action::Abort, &before, &after), Some(false));
}

#[test]
fn nothing_to_probe() {
    let (_fx, dir) = repo("none");
    let r = read(&dir);
    for a in [Action::SelectWip, Action::Refresh, Action::SwitchTab("x".into()), Action::Confirm("Cancel".into())] {
        assert_eq!(took_effect(&a, &r, &r), None);
    }
}
