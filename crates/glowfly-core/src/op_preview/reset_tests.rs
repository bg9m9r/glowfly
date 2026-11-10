//! Reset previews: commits left behind, unless another branch or tag keeps them; a reset to a
//! commit of another branch brings its commits in; a reset to HEAD changes nothing; another
//! worktree's HEAD keeps its commits.

use super::tests::{commit, diverged, is_kept, run};
use super::*;
use crate::testutil::{git, scratch_repo};

#[test]
fn commits_only_the_old_tip_reached_are_gone() {
    let dir = scratch_repo("opp-reset-back");
    let a = commit(&dir, "a.txt", "a\n", "Add a");
    let b = commit(&dir, "b.txt", "b\n", "Add b");
    let c = commit(&dir, "c.txt", "c\n", "Add c");
    let p = run(&dir, PreviewOp::Reset { to: a }).unwrap();
    assert_eq!(p.outcome, Outcome::Done);
    let gone: Vec<(Oid, GoneWhy)> = p.gone.iter().map(|g| (g.line.oid, g.why)).collect();
    assert_eq!(gone, [(c, GoneWhy::LeftBehind), (b, GoneWhy::LeftBehind)]);
    assert_eq!(p.gone_count, Count { n: 2, more: false });
    assert!(is_kept(&p.after[0], a));
    // A tag on c and a branch at b keep them.
    git(&dir, &["tag", "keep", &c.to_string()]);
    let p = run(&dir, PreviewOp::Reset { to: a }).unwrap();
    assert!(p.gone.is_empty(), "{:?}", p.gone);
    git(&dir, &["tag", "-d", "keep"]);
    git(&dir, &["branch", "at-b", &b.to_string()]);
    let p = run(&dir, PreviewOp::Reset { to: a }).unwrap();
    assert_eq!(p.gone.iter().map(|g| g.line.oid).collect::<Vec<_>>(), [c]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_reset_to_another_branch_brings_its_commits() {
    let (dir, [base, m1, f1, f2]) = diverged("opp-reset-other");
    let p = run(&dir, PreviewOp::Reset { to: m1 }).unwrap();
    assert_eq!(p.outcome, Outcome::Done);
    assert_eq!(p.arriving, Some(Count { n: 1, more: false }));
    assert!(matches!(&p.after[0], AfterRow::Kept { line, arrives: true } if line.oid == m1));
    assert!(matches!(&p.after[1], AfterRow::Kept { line, arrives: false } if line.oid == base));
    assert_eq!(p.gone.iter().map(|g| g.line.oid).collect::<Vec<_>>(), [f2, f1]);
    assert_eq!(run(&dir, PreviewOp::Reset { to: f2 }).unwrap().outcome, Outcome::Nothing);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn another_worktree_s_head_keeps_its_commits_but_this_one_s_own_does_not() {
    let dir = scratch_repo("opp-reset-worktrees");
    let a = commit(&dir, "a.txt", "a\n", "Add a");
    let b = commit(&dir, "b.txt", "b\n", "Add b");
    let wt = dir.with_file_name(format!("{}-wt", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&dir, &["worktree", "add", "-q", "--detach", &wt.display().to_string()]);
    // The linked worktree's detached HEAD (read from its admin dir) keeps b.
    assert!(run(&dir, PreviewOp::Reset { to: a }).unwrap().gone.is_empty());
    // From the linked worktree, on its own branch: its own HEAD keeps nothing.
    git(&wt, &["checkout", "-q", "-b", "side"]);
    let c = commit(&wt, "c.txt", "c\n", "Add c");
    let p = run(&wt, PreviewOp::Reset { to: b }).unwrap();
    assert_eq!(p.gone.iter().map(|g| g.line.oid).collect::<Vec<_>>(), [c]);
    let _ = std::fs::remove_dir_all(&wt);
    let _ = std::fs::remove_dir_all(&dir);
}
