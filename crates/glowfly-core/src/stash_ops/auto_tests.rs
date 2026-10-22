//! The auto-stash: which commands it guards, its messages, and putting it back.

use super::*;
use crate::gitcli::args;
use crate::testutil::{git, scratch_repo, write};
use std::path::{Path, PathBuf};

fn of(a: &[&str]) -> Option<AutoStashFor> {
    AutoStashFor::of_command(&args(a), Some("main"))
}

fn s(v: &str) -> String {
    v.to_owned()
}

const SHA: &str = "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b";

#[test]
fn classifies_guarded_commands() {
    use AutoStashFor::*;
    assert_eq!(of(&["checkout", "feature", "--"]), Some(Checkout { target: s("feature") }));
    assert_eq!(of(&["checkout", "--track", "origin/x", "--"]), Some(Checkout { target: s("origin/x") }));
    assert_eq!(of(&["checkout", "refs/tags/v1"]), Some(Checkout { target: s("v1") }));
    assert_eq!(of(&["checkout", SHA]), Some(Checkout { target: s("1a2b3c4") }));
    assert_eq!(of(&["checkout", "-b", "n"]), None, "no start point changes no files");
    assert_eq!(of(&["checkout", "-b", "n", "start"]), Some(Checkout { target: s("n") }));
    assert_eq!(of(&["checkout", "--", "a.txt"]), None, "restoring files is never guarded");
    assert_eq!(of(&["checkout", "--theirs", "--", "a.txt"]), None);
    assert_eq!(of(&["pull", "--ff"]), Some(Pull { branch: s("main") }));
    assert_eq!(of(&["merge", "refs/remotes/origin/x"]), Some(Merge { from: s("origin/x"), into: s("main") }));
    assert_eq!(of(&["merge", "--ff-only", "topic"]), Some(Merge { from: s("topic"), into: s("main") }));
    assert_eq!(of(&["merge", "--squash", "topic"]), Some(Merge { from: s("topic"), into: s("main") }));
    assert_eq!(of(&["merge", "--abort"]), None);
    assert_eq!(of(&["rebase", "x"]), Some(Rebase { branch: s("main"), onto: s("x") }));
    assert_eq!(of(&["rebase", "--continue"]), None);
    assert_eq!(of(&["rebase", "-i", "x"]), None);
    assert_eq!(of(&["cherry-pick", "a", "b"]), Some(CherryPick { commits: s("2 commits"), onto: s("main") }));
    assert_eq!(of(&["cherry-pick", "-m", "1", SHA]), Some(CherryPick { commits: s("1a2b3c4"), onto: s("main") }));
    assert_eq!(of(&["cherry-pick", "--skip"]), None);
    assert_eq!(of(&["revert", "--no-edit", SHA]), Some(Revert { commits: s("1a2b3c4"), on: s("main") }));
    assert_eq!(of(&["revert", "--abort"]), None);
    assert_eq!(of(&["reset", "--hard", SHA]), Some(HardReset { branch: s("main"), to: s("1a2b3c4") }));
    assert_eq!(of(&["reset", "--soft", SHA]), None);
    assert_eq!(of(&["reset", "--mixed", SHA]), None);
    assert_eq!(of(&["stash", "push"]), None);
    assert_eq!(AutoStashFor::of_command(&args(&["pull"]), None), Some(Pull { branch: s("HEAD") }));
}

#[test]
fn messages_are_fixed_english_and_recognised() {
    use AutoStashFor::*;
    let cases = [
        (Checkout { target: s("feature") }, "Auto-stash before checking out feature"),
        (Pull { branch: s("main") }, "Auto-stash before pulling into main"),
        (Merge { from: s("origin/x"), into: s("main") }, "Auto-stash before merging origin/x into main"),
        (Rebase { branch: s("main"), onto: s("rebase/stepwise") }, "Auto-stash before rebasing main onto rebase/stepwise"),
        (CherryPick { commits: s("1a2b3c4"), onto: s("main") }, "Auto-stash before cherry-picking 1a2b3c4 onto main"),
        (CherryPick { commits: s("3 commits"), onto: s("main") }, "Auto-stash before cherry-picking 3 commits onto main"),
        (Revert { commits: s("1a2b3c4"), on: s("main") }, "Auto-stash before reverting 1a2b3c4 on main"),
        (HardReset { branch: s("main"), to: s("1a2b3c4") }, "Auto-stash before resetting main to 1a2b3c4"),
    ];
    for (op, want) in cases {
        assert_eq!(op.message(), want);
        assert!(is_auto_stash(&format!("On main: {want}")), "{want}");
        assert_eq!(op.stash_command(), args(&["stash", "push", "-m", want]));
    }
    assert!(is_auto_stash("On main: Auto-stash before merging x into main"));
    assert!(is_auto_stash("autostash"));
    assert!(!is_auto_stash("WIP on main: 1a2b3c4 x"));
    assert!(!is_auto_stash("On main: auto-stash before x"));
    assert!(!is_auto_stash("Auto-stash"));
    assert!(HardReset { branch: s("main"), to: s("x") }.is_hard_reset());
    assert!(!HardReset { branch: s("main"), to: s("x") }.can_stop());
    assert!(!Checkout { target: s("x") }.can_stop());
    assert!(Merge { from: s("x"), into: s("main") }.can_stop());
}

#[test]
fn a_git_flow_finish_stash_is_asked_for_by_its_dialog() {
    let op = AutoStashFor::GitFlowFinish { branch: s("feature/login") };
    let want = "Auto-stash before finishing feature/login";
    assert_eq!(op.message(), want);
    assert_eq!(op.stash_command(), args(&["stash", "push", "-m", want]));
    assert!(op.can_stop(), "a merge step can stop on conflicts");
    assert!(is_auto_stash(&format!("On feature/login: {want}")));
    // The finish's own commands are never classified as a finish.
    for cmd in [
        &["checkout", "develop", "--"][..],
        &["merge", "--no-ff", "--no-edit", "refs/heads/feature/login"],
        &["tag", "-a", "-m", "v1", "v1", "refs/heads/release/1"],
        &["branch", "-d", "feature/login"],
    ] {
        assert!(!matches!(of(cmd), Some(AutoStashFor::GitFlowFinish { .. })), "{cmd:?}");
    }
}

/// A repository on `main` with a.txt committed ("one\n") and then edited and stashed with
/// `message`. Returns the folder and the stash's commit.
fn stashed(name: &str, message: &str) -> (PathBuf, Oid) {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    write(&dir, "a.txt", "mine\n");
    git(&dir, &["stash", "push", "-m", message]);
    let oid = list(&dir).unwrap()[0].oid;
    (dir, oid)
}

fn read(dir: &Path, file: &str) -> String {
    std::fs::read_to_string(dir.join(file)).unwrap()
}

#[test]
fn put_back_restores_changes() {
    let (dir, oid) = stashed("auto-stash-back", "Auto-stash before checking out feature");
    assert_eq!(prepare_put_back(&dir, oid), Ok(0));
    let (outcome, output) = pop_put_back(&dir, 0, oid);
    assert_eq!(outcome, PutBack::Restored, "{output}");
    assert!(output.starts_with("$ git stash pop stash@{0}"), "{output}");
    assert_eq!(read(&dir, "a.txt"), "mine\n");
    assert!(list(&dir).unwrap().is_empty());
}

#[test]
fn conflicting_put_back_keeps_the_stash() {
    let (dir, oid) = stashed("auto-stash-conflict", "Auto-stash before pulling into main");
    write(&dir, "a.txt", "theirs\n");
    git(&dir, &["commit", "-q", "-am", "change a"]);
    let (outcome, output) = pop_put_back(&dir, 0, oid);
    assert_eq!(outcome, PutBack::Conflicted { paths: vec![s("a.txt")] }, "{output}");
    let left = list(&dir).unwrap();
    assert_eq!((left.len(), left[0].index, left[0].oid), (1, 0, oid), "the stash stays");
    assert!(read(&dir, "a.txt").contains("<<<<<<<"));
    assert!(git2::Repository::open(&dir).unwrap().index().unwrap().has_conflicts());
}

#[test]
fn put_back_waits_for_an_operation() {
    let (dir, oid) = stashed("auto-stash-wait", "Auto-stash before merging topic into main");
    git(&dir, &["checkout", "-q", "-b", "topic"]);
    write(&dir, "a.txt", "topic\n");
    git(&dir, &["commit", "-q", "-am", "topic"]);
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, "a.txt", "main\n");
    git(&dir, &["commit", "-q", "-am", "main"]);
    let (ok, _) = crate::gitcli::run(&dir, &args(&["merge", "topic"]));
    assert!(!ok, "the merge stops on conflicts");
    assert!(matches!(prepare_put_back(&dir, oid), Err((PutBack::NotReady, _))));
    assert_eq!(list(&dir).unwrap()[0].oid, oid, "the stash is untouched");
    git(&dir, &["merge", "--abort"]);
    assert_eq!(prepare_put_back(&dir, oid), Ok(0));
    assert!(matches!(prepare_put_back(&dir, Oid::from_bytes(&[7; 20]).unwrap()), Err((PutBack::Gone, _))));
}
