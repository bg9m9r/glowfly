//! Stash push, pop and apply: undoing a push pops it back with its index; undoing a pop or apply
//! onto a clean tree puts the stash back and clears what it applied.

use super::tests::{cmds, commit, refused, run_plan};
use super::*;
use crate::gitcli::run;
use crate::repo::status_at;
use crate::testutil::*;
use git2::Repository;
use std::path::{Path, PathBuf};

/// Status as "<code><path>" lists: (unstaged, staged).
fn split(dir: &Path) -> (Vec<String>, Vec<String>) {
    let (unstaged, staged, _) = status_at(dir, true).unwrap();
    let names = |v: Vec<crate::repo::FileEntry>| v.into_iter().map(|f| format!("{}{}", f.status, f.path)).collect::<Vec<_>>();
    (names(unstaged), names(staged))
}

fn clean(dir: &Path) -> bool {
    split(dir) == (vec![], vec![])
}

/// (index, oid, message) of every stash, newest first.
fn stashes(dir: &Path) -> Vec<(usize, Oid, String)> {
    let mut repo = Repository::open(dir).unwrap();
    let mut out = Vec::new();
    repo.stash_foreach(|i, m, id| {
        out.push((i, *id, m.to_owned()));
        true
    })
    .unwrap();
    out
}

/// A repository with a.txt and b.txt committed.
fn base(name: &str) -> (PathBuf, Oid) {
    let dir = scratch_repo(name);
    write(&dir, "b.txt", "b\n");
    let head = commit(&dir, "a.txt", "a\n", "base");
    (dir, head)
}

/// Runs `git <a>` as Trellis would and returns what it recorded, checking `irreversible` first.
fn record_kind(dir: &Path, a: &[&str]) -> (bool, Option<Action>) {
    let kind = UndoKind::classify(&args(a)).expect("an undoable command");
    let b = before(dir, &kind).unwrap();
    git(dir, a);
    let lost = irreversible(&b);
    (lost, after(dir, kind, b).unwrap())
}

#[test]
fn stash_push_undo_and_redo() {
    let (dir, head) = base("undo-stash-push");
    write(&dir, "a.txt", "a unstaged\n");
    write(&dir, "b.txt", "b staged\n");
    git(&dir, &["add", "b.txt"]);
    write(&dir, "c.txt", "untracked\n");
    let dirty = split(&dir);
    let (_, action) = record_kind(&dir, &["stash", "push", "-u"]);
    let action = action.expect("recorded");
    let Action::StashPush { stash, head: h, replay: Some(_), .. } = action.clone() else { panic!("{action:?}") };
    assert_eq!(h, head);
    assert!(clean(&dir));
    assert!(action.describe().starts_with("stash 'WIP on main"), "{}", action.describe());

    let undo = plan(&dir, &action, false).unwrap();
    assert_eq!(undo, cmds(&[&["stash", "pop", "--index", "stash@{0}"]]));
    run_plan(&dir, undo);
    assert_eq!(split(&dir), dirty, "unstaged, staged and untracked come back exactly");
    assert!(stashes(&dir).is_empty());

    let redo = plan(&dir, &action, true).unwrap();
    assert_eq!(redo, Plan::Run(vec![args(&["stash", "push", "-u"])]));
    run_plan(&dir, redo);
    assert!(clean(&dir));
    let Action::StashPush { stash: again, .. } = redone(&dir, action.clone()).unwrap() else { panic!() };
    assert_eq!(stashes(&dir)[0].1, again);
    let _ = stash;

    // The redone entry undoes like the first.
    let action = redone(&dir, action).unwrap();
    commit(&dir, "d.txt", "d\n", "moved on");
    assert!(refused(plan(&dir, &action, false).unwrap()).is_stale(), "HEAD has moved");
    git(&dir, &["reset", "-q", "--hard", "HEAD~1"]);
    git(&dir, &["stash", "drop", "-q"]);
    let r = refused(plan(&dir, &action, false).unwrap());
    assert!(r.is_stale() && r.message().contains("dropped"), "{r:?}");
    let Action::StashPush { stash, head, message, .. } = action else { panic!() };
    let no_replay = Action::StashPush { stash, head, message, replay: None };
    assert!(refused(plan(&dir, &no_replay, true).unwrap()).is_stale());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn partial_stash_undo_ignores_other_changes() {
    let (dir, _) = base("undo-stash-files");
    write(&dir, "a.txt", "a edited\n");
    write(&dir, "b.txt", "b edited\n");
    let (_, action) = record_kind(&dir, &["--literal-pathspecs", "stash", "push", "-u", "--", "a.txt"]);
    let action = action.expect("recorded");
    assert_eq!(split(&dir).0, vec!["Mb.txt".to_owned()]);

    write(&dir, "a.txt", "a again\n");
    assert!(matches!(refused(plan(&dir, &action, false).unwrap()), Refusal::Dirty(_)), "a.txt has changed");
    git(&dir, &["checkout", "--", "a.txt"]);
    run_plan(&dir, plan(&dir, &action, false).unwrap());
    assert_eq!(split(&dir).0, vec!["Ma.txt".to_owned(), "Mb.txt".to_owned()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pop_undo_and_redo() {
    let (dir, head) = base("undo-stash-pop");
    write(&dir, "a.txt", "a edited\n");
    write(&dir, "n.txt", "new\n");
    git(&dir, &["add", "n.txt"]);
    write(&dir, "u.txt", "untracked\n");
    git(&dir, &["stash", "push", "-q", "-u", "-m", "wip"]);
    let [(_, oid, message)] = stashes(&dir).try_into().unwrap();
    assert_eq!(message, "On main: wip");

    let (lost, action) = record_kind(&dir, &["stash", "pop"]);
    assert!(!lost);
    let action = action.expect("recorded");
    let Action::StashApply { pop: true, stash, head: h, message: m, .. } = &action else { panic!("{action:?}") };
    assert_eq!((*stash, *h, m.as_str()), (oid, head, "On main: wip"));
    assert_eq!(action.describe(), "pop of stash 'On main: wip'");

    let Plan::Run(undo) = plan(&dir, &action, false).unwrap() else { panic!() };
    assert_eq!(undo[0], args(&["stash", "store", "-m", "On main: wip", &oid.to_string()]));
    assert_eq!(undo[1], args(&["reset", "--hard", "-q"]));
    assert_eq!(undo.last().unwrap(), &args(&["--literal-pathspecs", "clean", "-f", "-q", "--", "u.txt"]));
    run_plan(&dir, Plan::Run(undo));
    assert!(clean(&dir), "{:?}", split(&dir));
    assert_eq!(stashes(&dir), vec![(0, oid, "On main: wip".to_owned())]);

    let redo = plan(&dir, &action, true).unwrap();
    assert_eq!(redo, cmds(&[&["stash", "pop", "stash@{0}"]]));
    run_plan(&dir, redo);
    assert!(stashes(&dir).is_empty() && !clean(&dir));
    let action = redone(&dir, action).unwrap();

    // Edited since: the pop's result is no longer what undo would clear.
    write(&dir, "a.txt", "edited after the pop\n");
    let r = refused(plan(&dir, &action, false).unwrap());
    assert!(r.is_stale() && r.message().starts_with("The working tree has changed since the pop of stash"), "{r:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn apply_undo_keeps_the_list_and_restores_a_dropped_stash() {
    let (dir, _) = base("undo-stash-apply");
    write(&dir, "a.txt", "a edited\n");
    git(&dir, &["stash", "push", "-q", "-m", "wip"]);
    let (_, action) = record_kind(&dir, &["stash", "apply", "stash@{0}"]);
    let action = action.expect("recorded");
    assert_eq!(action.describe(), "apply of stash 'On main: wip'");
    let undo = plan(&dir, &action, false).unwrap();
    assert_eq!(undo, cmds(&[&["reset", "--hard", "-q"]]), "the stash is still listed");
    run_plan(&dir, undo);
    assert!(clean(&dir));
    assert_eq!(stashes(&dir).len(), 1, "no duplicate");
    run_plan(&dir, plan(&dir, &action, true).unwrap());
    assert!(!clean(&dir));

    // Dropped after the apply: undo puts it back first.
    let action = redone(&dir, action).unwrap();
    git(&dir, &["stash", "drop", "-q"]);
    let Plan::Run(undo) = plan(&dir, &action, false).unwrap() else { panic!() };
    assert_eq!(undo[0][..3], args(&["stash", "store", "-m"]));
    run_plan(&dir, Plan::Run(undo));
    assert!(clean(&dir));
    assert_eq!(stashes(&dir).len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pop_onto_a_dirty_tree_is_irreversible() {
    let (dir, _) = base("undo-stash-dirty");
    write(&dir, "a.txt", "a edited\n");
    git(&dir, &["stash", "push", "-q"]);
    write(&dir, "b.txt", "b edited\n");
    let (lost, action) = record_kind(&dir, &["stash", "pop"]);
    assert!(lost);
    assert_eq!(action, None);
    assert!(!run(&dir, &args(&["stash", "pop"])).0, "nothing left to pop");
    let _ = std::fs::remove_dir_all(&dir);
}
