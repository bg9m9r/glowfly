//! Drop previews: a middle commit, one a later commit depends on, the newest commit, and a
//! commit below a merge (the prepared list recreates the merge).

use super::tests::{commit, is_kept, is_rewritten, oid, run};
use super::*;
use crate::testutil::{git, scratch_repo};
use std::path::PathBuf;

/// a, b, c on main: each adds its own file. Returns (dir, [a, b, c]).
fn three(name: &str) -> (PathBuf, [Oid; 3]) {
    let dir = scratch_repo(name);
    let a = commit(&dir, "a.txt", "a\n", "Add a");
    let b = commit(&dir, "b.txt", "b\n", "Add b");
    let c = commit(&dir, "c.txt", "c\n", "Add c");
    (dir, [a, b, c])
}

fn gone_of(p: &OpPreview) -> Vec<(Oid, GoneWhy)> {
    p.gone.iter().map(|g| (g.line.oid, g.why)).collect()
}

#[test]
fn dropping_a_middle_commit_rewrites_the_later_one() {
    let (dir, [a, b, c]) = three("opp-drop-middle");
    let p = run(&dir, PreviewOp::Drop(vec![b])).unwrap();
    assert_eq!(p.outcome, Outcome::Done);
    assert_eq!(gone_of(&p), [(b, GoneWhy::Dropped)]);
    assert_eq!(p.moved.iter().map(|m| m.oid).collect::<Vec<_>>(), [c]);
    assert!(is_rewritten(&p.after[0], c) && is_kept(&p.after[1], a), "{:?}", p.after);
    assert_eq!(p.after.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_later_commit_on_the_same_lines_stops_it() {
    let dir = scratch_repo("opp-drop-stop");
    commit(&dir, "a.txt", "a\n", "Add a");
    let b = commit(&dir, "b.txt", "one\n", "Add b");
    let c = commit(&dir, "b.txt", "two\n", "Edit b");
    let p = run(&dir, PreviewOp::Drop(vec![b])).unwrap();
    let Outcome::Stopped(stop) = &p.outcome else { panic!("{:?}", p.outcome) };
    assert!(matches!(stop.why, StopWhy::Conflicts(ref paths) if paths == &["b.txt".to_owned()]), "{:?}", stop.why);
    assert_eq!(stop.applied, 0);
    assert!(matches!(&p.after[0], AfterRow::Stopped { at: Some(l) } if l.oid == c));
    assert!(p.gone.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dropping_the_newest_commit_rewrites_nothing() {
    let (dir, [_, b, c]) = three("opp-drop-newest");
    let p = run(&dir, PreviewOp::Drop(vec![c])).unwrap();
    assert_eq!(p.outcome, Outcome::Done);
    assert_eq!(gone_of(&p), [(c, GoneWhy::Dropped)]);
    assert!(p.moved.is_empty());
    assert!(is_kept(&p.after[0], b), "{:?}", p.after);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dropping_below_a_merge_recreates_it() {
    let (dir, [a, b, c]) = three("opp-drop-merge");
    git(&dir, &["checkout", "-q", "-b", "side", &a.to_string()]);
    let s = commit(&dir, "s.txt", "s\n", "Add s");
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["merge", "-q", "--no-ff", "-m", "Merge side", "side"]);
    let merge = oid(&dir, "HEAD");
    let p = run(&dir, PreviewOp::Drop(vec![b])).unwrap();
    assert_eq!(p.outcome, Outcome::Done);
    assert!(is_rewritten(&p.after[0], merge), "{:?}", p.after);
    assert!(is_rewritten(&p.after[1], c));
    assert!(is_kept(&p.after[2], a));
    assert_eq!(gone_of(&p), [(b, GoneWhy::Dropped)], "the side branch keeps s");
    assert!(!p.moved.iter().any(|m| m.oid == s), "s is the merge's second parent, kept as it is");
    let _ = std::fs::remove_dir_all(&dir);
}
