//! A stash whose reflog message isn't UTF-8 (Latin-1 "caf\xe9", as `git stash push -m` writes it
//! under a legacy locale): the repository loads, the list shows it lossily, a shallower rename
//! leaves its bytes alone, and popping, dropping and undoing both act on the right entry.

use super::stash_tests::{base, clean, record_kind, stashes};
use super::tests::{cmds, run_plan};
use super::*;
use crate::testutil::*;
use std::path::Path;

/// The stash `rev` names in `dir`.
fn rev(dir: &Path, rev: &str) -> Oid {
    git2::Repository::open(dir).unwrap().revparse_single(rev).unwrap().id()
}

#[test]
fn latin1_stash_loads_lists_renames_pops_drops_and_undoes() {
    let (dir, _) = base("undo-stash-latin1");
    write(&dir, "a.txt", "first\n");
    git(&dir, &["stash", "push", "-q", "-m", "latin1-here"]);
    rewrite_reflog(&dir, "refs/stash", b"latin1-here", b"caf\xe9");
    write(&dir, "a.txt", "second\n");
    git(&dir, &["stash", "push", "-q", "-m", "second"]);
    let lossy = format!("On main: caf{}", char::REPLACEMENT_CHARACTER);
    let (oid0, oid1) = (rev(&dir, "stash@{0}"), rev(&dir, "stash@{1}"));

    // The repository loads, with the entry decoded lossily at its git index.
    let snap = crate::repo::load(&dir, &crate::repo::LoadOptions::all(10)).unwrap();
    let loaded: Vec<_> = snap.stashes.iter().map(|s| (s.index, s.oid, s.message.clone())).collect();
    assert_eq!(loaded, vec![(0, oid0, "On main: second".to_owned()), (1, oid1, lossy.clone())]);
    assert_eq!(stashes(&dir), loaded);
    assert_eq!(stash_first::undo_stash_made(&dir, "no such run").unwrap(), None);

    // Renaming the entry above leaves the deeper one's bytes alone.
    let report = crate::stash_ops::rename(&dir, 0, oid0, "On main: renamed");
    assert!(report.ok, "{}", report.output);
    assert_eq!(stashes(&dir), vec![(0, oid0, "On main: renamed".to_owned()), (1, oid1, lossy.clone())]);
    let log = dir.join(".git").join("logs").join("refs").join("stash");
    assert!(std::fs::read(&log).unwrap().contains(&0xE9), "the Latin-1 byte is still in the reflog");

    // Popping it by index records the lossy text, and undoing stores it back at the top.
    let (lost, action) = record_kind(&dir, &["stash", "pop", "stash@{1}"]);
    assert!(!lost);
    let action = action.expect("recorded");
    let Action::StashApply { pop: true, stash, message, .. } = &action else { panic!("{action:?}") };
    assert_eq!((*stash, message.as_str()), (oid1, lossy.as_str()));
    assert_eq!(action.describe(), format!("unstash of '{lossy}'"));
    assert_eq!(stashes(&dir), vec![(0, oid0, "On main: renamed".to_owned())]);
    let undo = plan(&dir, &action, false).unwrap();
    let Plan::Run(steps) = &undo else { panic!("{undo:?}") };
    assert_eq!(steps[0], args(&["stash", "store", "-m", &lossy, &oid1.to_string()]));
    run_plan(&dir, undo);
    assert!(clean(&dir));
    assert_eq!(stashes(&dir), vec![(0, oid1, lossy.clone()), (1, oid0, "On main: renamed".to_owned())]);

    // Back to raw Latin-1, then dropped and put back.
    rewrite_reflog(&dir, "refs/stash", lossy.as_bytes(), b"On main: caf\xe9");
    let (_, action) = record_kind(&dir, &["stash", "drop", "stash@{0}"]);
    let action = action.expect("recorded");
    assert_eq!(action, Action::StashDrop { stash: oid1, message: lossy.clone() });
    assert_eq!(stashes(&dir), vec![(0, oid0, "On main: renamed".to_owned())]);
    let undo = plan(&dir, &action, false).unwrap();
    assert_eq!(undo, cmds(&[&["stash", "store", "-m", &lossy, &oid1.to_string()]]));
    run_plan(&dir, undo);
    assert_eq!(stashes(&dir)[0].1, oid1);
    let _ = std::fs::remove_dir_all(&dir);
}
