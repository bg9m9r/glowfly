//! Branch colours on loaded repositories: which branch is main, every owner coloured, and the
//! colours kept when the work tree turns dirty and the graph is laid out again.

use crate::branch_colour::Swatch;
use crate::repo::*;
use crate::testutil::*;
use git2::Repository;
use std::path::{Path, PathBuf};

fn commit(dir: &Path, file: &str, msg: &str) {
    write(dir, file, msg);
    git(dir, &["add", file]);
    git(dir, &["commit", "-q", "-m", msg]);
}

/// main with one commit, and topic and fix-1 one commit each on top of it, side by side.
fn diverging(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    commit(&dir, "a.txt", "base");
    for b in ["topic", "fix-1"] {
        git(&dir, &["checkout", "-q", "-b", b, "main"]);
        commit(&dir, &format!("{b}.txt"), b);
    }
    git(&dir, &["checkout", "-q", "main"]);
    dir
}

fn open(dir: &Path) -> Snapshot {
    load(dir, &LoadOptions::all(1000)).unwrap()
}

fn tip_swatch(s: &Snapshot, branch: &str) -> Option<Swatch> {
    let oid = Repository::open(&s.workdir).unwrap().refname_to_id(&format!("refs/heads/{branch}")).unwrap();
    s.commit_swatch(oid)
}

#[test]
fn mainline_from_remote_head() {
    let dir = scratch_repo("colour-remote-head");
    commit(&dir, "a.txt", "base");
    git(&dir, &["branch", "trunk"]);
    git(&dir, &["remote", "add", "origin", "https://example.invalid/r.git"]);
    git(&dir, &["update-ref", "refs/remotes/origin/trunk", "HEAD"]);
    git(&dir, &["symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/trunk"]);
    let s = open(&dir);
    assert_eq!(s.mainline().as_deref(), Some("trunk"));
    assert_eq!(s.branch_swatch("trunk"), Swatch::Main);
    assert_eq!(s.ref_swatch("refs/remotes/origin/trunk"), Some(Swatch::Main), "every copy of main");
    assert_ne!(s.branch_swatch("main"), Swatch::Main);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mainline_falls_back_to_main_then_master() {
    let dir = scratch_repo("colour-master");
    commit(&dir, "a.txt", "base");
    git(&dir, &["branch", "master"]);
    assert_eq!(open(&dir).mainline().as_deref(), Some("main"));
    git(&dir, &["checkout", "-q", "master"]);
    git(&dir, &["branch", "-D", "main"]);
    let s = open(&dir);
    assert_eq!(s.mainline().as_deref(), Some("master"));
    assert_eq!(s.head_swatch(), Some(Swatch::Main));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_mainline_without_main_or_master() {
    let dir = scratch_repo("colour-no-main");
    commit(&dir, "a.txt", "base");
    git(&dir, &["branch", "-m", "trunk"]);
    let s = open(&dir);
    assert_eq!(s.mainline(), None);
    assert!(matches!(s.branch_swatch("trunk"), Swatch::Slot(_)));
    assert!(s.graph.swatches.iter().all(|w| *w != Swatch::Main));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_colours_every_owner() {
    let dir = diverging("colour-load");
    let s = open(&dir);
    assert_eq!(s.graph.swatches.len(), s.graph.owners.list.len());
    assert_eq!(s.graph.owners.commit.len(), s.commits.len());
    for b in ["main", "topic", "fix-1"] {
        assert_eq!(Some(s.branch_swatch(b)), tip_swatch(&s, b), "{b}");
    }
    assert_eq!(s.branch_swatch("main"), Swatch::Main);
    assert_ne!(s.branch_swatch("topic"), s.branch_swatch("fix-1"), "side by side, though their names share a slot");
    let topic = Repository::open(&dir).unwrap().refname_to_id("refs/heads/topic").unwrap();
    assert_eq!(s.commit_branch(topic).map(|(k, _)| k), Some("topic"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn relayout_keeps_colours() {
    let dir = diverging("colour-relayout");
    let clean = open(&dir);
    write(&dir, "a.txt", "changed");
    let mut dirty = open(&dir);
    assert!(dirty.graph.wip.is_some());
    dirty.lay_out_graph();
    for b in ["main", "topic", "fix-1"] {
        assert_eq!(clean.branch_swatch(b), dirty.branch_swatch(b), "{b}");
    }
    let wip = dirty.graph.owners.wip.unwrap();
    assert_eq!(dirty.graph.owners.key(wip), Some("main"), "the WIP row takes HEAD's branch");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_pin_moves_its_neighbour_and_survives_a_relayout() {
    use crate::branch_colour::Colouring;
    let dir = diverging("colour-pin");
    let mut s = open(&dir);
    let Swatch::Slot(theirs) = s.branch_swatch("fix-1") else { panic!("fix-1 is not main") };
    let pinned = Colouring { pins: [("topic".to_owned(), theirs)].into_iter().collect(), ..Colouring::default() };
    s.set_colouring(pinned.clone());
    assert_eq!(s.branch_swatch("topic"), Swatch::Slot(theirs), "the pin holds");
    assert_ne!(s.branch_swatch("fix-1"), Swatch::Slot(theirs), "the automatic neighbour moves off it");
    assert_eq!(s.ref_swatch("refs/heads/topic"), Some(Swatch::Slot(theirs)));
    s.lay_out_graph();
    assert_eq!(s.graph.colouring, pinned);
    assert_eq!(s.branch_swatch("topic"), Swatch::Slot(theirs), "kept across relayouts");
    assert_ne!(s.branch_swatch("fix-1"), Swatch::Slot(theirs));
    let _ = std::fs::remove_dir_all(&dir);
}
