//! Cherry-pick previews: copies on the branch, a conflicting second pick, a pick whose change
//! is already there, merges and the replay limit.

use super::tests::{commit, diverged, is_kept, message, oid, run, run_with};
use super::*;
use crate::testutil::{git, scratch_repo};

#[test]
fn picks_become_copies_on_the_branch() {
    let (dir, [_, m1, f1, f2]) = diverged("opp-pick-clean");
    git(&dir, &["checkout", "-q", "main"]);
    let p = run(&dir, PreviewOp::CherryPick(vec![f1, f2])).unwrap();
    assert_eq!(p.outcome, Outcome::Done);
    assert!(matches!(&p.after[0], AfterRow::Rewritten { from, copy: true } if from.oid == f2), "{:?}", p.after);
    assert!(matches!(&p.after[1], AfterRow::Rewritten { from, copy: true } if from.oid == f1));
    assert!(is_kept(&p.after[2], m1));
    assert!(p.moved.is_empty() && p.gone.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_conflict_or_an_empty_pick_stops_it() {
    let dir = scratch_repo("opp-pick-stop");
    commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["branch", "side"]);
    commit(&dir, "a.txt", "main\n", "main edit");
    let done = commit(&dir, "d.txt", "d\n", "d on main");
    git(&dir, &["checkout", "-q", "side"]);
    let s1 = commit(&dir, "b.txt", "b\n", "s1");
    let s2 = commit(&dir, "a.txt", "side\n", "s2");
    let s3 = commit(&dir, "d.txt", "d\n", "s3 same as main's");
    git(&dir, &["checkout", "-q", "main"]);
    let p = run(&dir, PreviewOp::CherryPick(vec![s1, s2, s3])).unwrap();
    let Outcome::Stopped(stop) = &p.outcome else { panic!("{:?}", p.outcome) };
    assert_eq!(stop.why, StopWhy::Conflicts(vec!["a.txt".into()]));
    assert_eq!(stop.applied, 1);
    assert_eq!(stop.waiting.iter().map(|w| w.oid).collect::<Vec<_>>(), [s3]);
    assert!(matches!(&p.after[0], AfterRow::Stopped { at: Some(l) } if l.oid == s2));
    assert!(matches!(&p.after[1], AfterRow::Rewritten { from, copy: true } if from.oid == s1));
    let p = run(&dir, PreviewOp::CherryPick(vec![s3])).unwrap();
    let Outcome::Stopped(stop) = &p.outcome else { panic!("{:?}", p.outcome) };
    assert_eq!((stop.why.clone(), stop.applied), (StopWhy::Empty, 0));
    assert!(is_kept(&p.after[1], done));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn merges_and_long_lists_are_refused() {
    let (dir, [_, _, f1, f2]) = diverged("opp-pick-refused");
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["merge", "-q", "--no-ff", "-m", "merge feature", "feature"]);
    let merge = oid(&dir, "HEAD");
    git(&dir, &["checkout", "-q", "-b", "other", "HEAD~1"]);
    assert_eq!(message(run(&dir, PreviewOp::CherryPick(vec![merge])).unwrap_err()), PICK_MERGE);
    let lim = Limits { replay: 1, ..Limits::DEFAULT };
    let e = run_with(&dir, PreviewOp::CherryPick(vec![f1, f2]), &lim).unwrap_err();
    assert_eq!(message(e), too_many(2));
    let _ = std::fs::remove_dir_all(&dir);
}
