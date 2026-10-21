//! Renaming a stash list entry. Only the reflog message changes, so the commit (what the details
//! panel shows when a stash is selected) keeps its old message, and the entries above the
//! renamed one get the time of the rename as their reflog date; neither is checked here.

use super::*;
use crate::gitcli::{args, run};
use crate::testutil::{git, scratch_repo, write};
use std::path::{Path, PathBuf};

fn info(index: usize, message: &str, oid: Oid) -> StashInfo {
    StashInfo { index, message: message.into(), oid }
}

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

#[test]
fn split_message_cases() {
    assert_eq!(split_message("On main: login"), (Some("main"), "login"));
    assert_eq!(split_message("WIP on feature/a: 1a2b3c4 Add x"), (Some("feature/a"), "1a2b3c4 Add x"));
    assert_eq!(split_message("On (no branch): x"), (Some("(no branch)"), "x"));
    assert_eq!(split_message("Glowfly: discarded changes"), (None, "Glowfly: discarded changes"));
    assert_eq!(split_message("On main:x"), (None, "On main:x"), "no space after the colon");
    assert_eq!(split_message("On : x"), (None, "On : x"), "no branch");
}

#[test]
fn edited_message_cases() {
    assert_eq!(edited_message("On main: a", "b").as_deref(), Some("On main: b"));
    assert_eq!(edited_message("WIP on main: abc s", "new").as_deref(), Some("On main: new"));
    assert_eq!(edited_message("Glowfly: discarded changes", "kept").as_deref(), Some("kept"));
    assert_eq!(edited_message("plain", "  a\n b\t c \r\n").as_deref(), Some("a b c"));
    assert_eq!(edited_message("On main: a", "").as_deref(), None);
    assert_eq!(edited_message("On main: a", " \n\t ").as_deref(), None);
}

#[test]
fn rename_commands_drop_then_store_from_the_deepest() {
    let entries = [info(0, "On main: zero", oid(1)), info(1, "On main: one", oid(2)), info(2, "On main: two", oid(3))];
    let s = |o: Oid| o.to_string();
    assert_eq!(
        rename_commands(&entries, 1, "On main: new"),
        vec![
            args(&["stash", "drop", "stash@{0}"]),
            args(&["stash", "drop", "stash@{0}"]),
            args(&["stash", "store", "-m", "On main: new", &s(oid(2))]),
            args(&["stash", "store", "-m", "On main: zero", &s(oid(1))]),
        ]
    );
    assert_eq!(
        rename_commands(&entries, 0, "On main: new"),
        vec![args(&["stash", "drop", "stash@{0}"]), args(&["stash", "store", "-m", "On main: new", &s(oid(1))])]
    );
}

/// A repository on `main` with the stashes [0] third, [1] second, [2] first, each holding one
/// untracked file.
fn three_stashes(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    for m in ["first", "second", "third"] {
        write(&dir, &format!("{m}.txt"), m);
        git(&dir, &["stash", "push", "-u", "-m", m]);
    }
    dir
}

/// `(sha, message)` of each `git stash list` entry, newest first.
fn listed(dir: &Path) -> Vec<(String, String)> {
    let (ok, out) = run(dir, &args(&["stash", "list", "--format=%H %gs"]));
    assert!(ok, "{out}");
    out.lines().map(|l| l.split_once(' ').map(|(a, b)| (a.to_owned(), b.to_owned())).unwrap()).collect()
}

fn rev(dir: &Path, r: &str) -> String {
    let (ok, out) = run(dir, &args(&["rev-parse", r]));
    assert!(ok, "{out}");
    out.trim().to_owned()
}

#[test]
fn renames_the_middle_entry_in_place() {
    let dir = three_stashes("stash-rename-middle");
    let before = listed(&dir);
    assert_eq!(before.iter().map(|(_, m)| m.as_str()).collect::<Vec<_>>(), ["On main: third", "On main: second", "On main: first"]);
    let second = list(&dir).unwrap()[1].oid;
    let r = rename(&dir, 1, second, "On main: renamed");
    assert!(r.ok, "{}", r.output);
    assert!(r.output.contains("$ git stash drop stash@{0}") && r.output.contains("Dropped"), "{}", r.output);
    let after = listed(&dir);
    let shas = |l: &[(String, String)]| l.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>();
    assert_eq!(shas(&after), shas(&before), "same commits, same order");
    assert_eq!(after[0].1, before[0].1);
    assert_eq!(after[1].1, "On main: renamed");
    assert_eq!(after[2].1, before[2].1);
    assert_eq!(rev(&dir, "stash@{1}"), second.to_string());
    git(&dir, &["stash", "apply", "stash@{1}"]);
    assert_eq!(std::fs::read_to_string(dir.join("second.txt")).unwrap(), "second");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn renames_the_top_and_the_bottom_entries() {
    let dir = three_stashes("stash-rename-ends");
    let before = list(&dir).unwrap();
    let r = rename(&dir, 0, before[0].oid, "On main: top");
    assert!(r.ok, "{}", r.output);
    let r = rename(&dir, 2, before[2].oid, "bottom");
    assert!(r.ok, "{}", r.output);
    let after = list(&dir).unwrap();
    assert_eq!(after.iter().map(|s| s.oid).collect::<Vec<_>>(), before.iter().map(|s| s.oid).collect::<Vec<_>>());
    let msgs: Vec<&str> = after.iter().map(|s| s.message.as_str()).collect();
    assert_eq!(msgs, ["On main: top", "On main: second", "bottom"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn renames_the_only_stash() {
    let dir = scratch_repo("stash-rename-only");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    write(&dir, "b.txt", "b");
    git(&dir, &["stash", "push", "-u", "-m", "only"]);
    let only = list(&dir).unwrap()[0].oid;
    // The drop deletes refs/stash and the store creates it again.
    let r = rename(&dir, 0, only, "On main: still here");
    assert!(r.ok, "{}", r.output);
    let after = list(&dir).unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].oid, only);
    assert_eq!(after[0].message, "On main: still here");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refuses_a_different_or_missing_entry() {
    let dir = three_stashes("stash-rename-refused");
    let before = listed(&dir);
    let entries = list(&dir).unwrap();
    let r = rename(&dir, 1, entries[0].oid, "On main: renamed");
    assert!(!r.ok && r.output.contains("nothing was changed"), "{}", r.output);
    let r = rename(&dir, 3, entries[0].oid, "On main: renamed");
    assert!(!r.ok && r.output.contains("nothing was changed"), "{}", r.output);
    assert_eq!(listed(&dir), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_same_message_runs_nothing() {
    let dir = three_stashes("stash-rename-same");
    let e = list(&dir).unwrap();
    let mut count = 0;
    let r = rename_with(&dir, 1, e[1].oid, &e[1].message, &mut |_| {
        count += 1;
        (true, String::new())
    });
    assert!(r.ok && r.output.contains("already has this message"), "{}", r.output);
    assert_eq!(count, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Renames stash@{1} to "On main: renamed", failing the `fail_at`th git command (from 1).
fn rename_failing(dir: &Path, fail_at: usize) -> RenameReport {
    let second = list(dir).unwrap()[1].oid;
    let mut n = 0;
    rename_with(dir, 1, second, "On main: renamed", &mut |a| {
        n += 1;
        if n == fail_at { (false, "error: injected failure".into()) } else { run(dir, a) }
    })
}

#[test]
fn a_failed_drop_puts_back_what_was_dropped() {
    let dir = three_stashes("stash-rename-drop-fails");
    let before = listed(&dir);
    let r = rename_failing(&dir, 1);
    assert!(!r.ok && r.output.contains("injected failure"), "{}", r.output);
    assert_eq!(listed(&dir), before, "nothing was dropped");

    let r = rename_failing(&dir, 2);
    assert!(!r.ok, "{}", r.output);
    assert!(!r.output.contains("Not back in the stash list"), "{}", r.output);
    assert_eq!(listed(&dir), before, "the first dropped entry is back with its own message");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_store_keeps_the_others_and_says_how_to_restore() {
    let dir = three_stashes("stash-rename-store-fails");
    let entries = list(&dir).unwrap();
    // Two drops, then the store of the renamed entry fails.
    let r = rename_failing(&dir, 3);
    assert!(!r.ok, "{}", r.output);
    let want = format!("git stash store -m \"On main: renamed\" {}", entries[1].oid);
    assert!(r.output.contains("Not back in the stash list:") && r.output.contains(&want), "{}", r.output);
    let after = list(&dir).unwrap();
    let oids: Vec<Oid> = after.iter().map(|s| s.oid).collect();
    assert_eq!(oids, [entries[0].oid, entries[2].oid], "the others are still listed");
    assert_eq!(after[0].message, entries[0].message);
    let _ = std::fs::remove_dir_all(&dir);
}
