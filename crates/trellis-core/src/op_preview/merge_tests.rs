//! Merge previews: a clean merge and one with conflicts (merge-tree, git 2.38 or newer),
//! fast-forwards and up-to-date branches from the graph, merge.ff and the branch's
//! mergeOptions, and a ref that moved.

use super::tests::{commit, diverged, is_kept, message, oid, run};
use super::*;
use crate::testutil::{git, scratch_repo, set_config};

/// Whether this git has merge-tree --write-tree; says so and skips otherwise.
fn new_git(dir: &Path, test: &str) -> bool {
    let ok = crate::gitcli::version(dir).is_none_or(|v| (v.0, v.1) >= crate::conflict_check::MIN_GIT);
    if !ok {
        eprintln!("{test}: git is older than 2.38, skipping");
    }
    ok
}

fn merge_op(dir: &Path, name: &str) -> PreviewOp {
    PreviewOp::Merge { theirs: name.into(), oid: oid(dir, name) }
}

#[test]
fn a_clean_merge_makes_a_merge_commit() {
    let (dir, [_, m1, _, f2]) = diverged("opp-merge-clean");
    git(&dir, &["checkout", "-q", "main"]);
    if !new_git(&dir, "a_clean_merge_makes_a_merge_commit") {
        return;
    }
    let p = run(&dir, merge_op(&dir, "feature")).unwrap();
    assert_eq!(p.outcome, Outcome::Done);
    assert_eq!(p.after[0], AfterRow::NewMerge { theirs: "feature".into() });
    assert!(is_kept(&p.after[1], m1), "{:?}", p.after);
    assert_eq!(p.arriving, Some(Count { n: 2, more: false }));
    assert!(p.gone.is_empty() && p.moved.is_empty());
    assert_eq!(p.subject.as_deref(), Some("main"));
    let _ = f2;
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn both_sides_editing_a_file_stops_with_conflicts() {
    let dir = scratch_repo("opp-merge-conflict");
    commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["branch", "feature"]);
    commit(&dir, "a.txt", "main\n", "main edit");
    git(&dir, &["checkout", "-q", "feature"]);
    commit(&dir, "a.txt", "feature\n", "feature edit");
    git(&dir, &["checkout", "-q", "main"]);
    if !new_git(&dir, "both_sides_editing_a_file_stops_with_conflicts") {
        return;
    }
    let p = run(&dir, merge_op(&dir, "feature")).unwrap();
    let Outcome::Stopped(stop) = &p.outcome else { panic!("{:?}", p.outcome) };
    assert_eq!(stop.why, StopWhy::Conflicts(vec!["a.txt".into()]));
    assert_eq!(stop.applied, 0);
    assert!(stop.waiting.is_empty());
    assert_eq!(p.after[0], AfterRow::Stopped { at: None });
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fast_forwards_and_up_to_date_come_from_the_graph() {
    let dir = scratch_repo("opp-merge-ff");
    let base = commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["branch", "old"]);
    let c1 = commit(&dir, "b.txt", "b\n", "c1");
    let c2 = commit(&dir, "c.txt", "c\n", "c2");
    git(&dir, &["checkout", "-q", "old"]);
    let p = run(&dir, merge_op(&dir, "main")).unwrap();
    assert_eq!(p.outcome, Outcome::FastForward);
    assert_eq!(p.arriving, Some(Count { n: 2, more: false }));
    assert!(matches!(&p.after[0], AfterRow::Kept { line, arrives: true } if line.oid == c2));
    assert!(matches!(&p.after[1], AfterRow::Kept { line, arrives: true } if line.oid == c1));
    assert!(matches!(&p.after[2], AfterRow::Kept { line, arrives: false } if line.oid == base));
    git(&dir, &["checkout", "-q", "main"]);
    let p = run(&dir, merge_op(&dir, "old")).unwrap();
    assert_eq!(p.outcome, Outcome::Nothing);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn merge_ff_and_the_branch_merge_options_are_honoured() {
    let dir = scratch_repo("opp-merge-ffmode");
    commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["branch", "old"]);
    commit(&dir, "b.txt", "b\n", "c1");
    git(&dir, &["checkout", "-q", "old"]);
    set_config(&dir, &[("merge.ff", "false")]);
    let p = run(&dir, merge_op(&dir, "main")).unwrap();
    assert_eq!(p.outcome, Outcome::Done, "a merge commit even though it could fast-forward");
    assert_eq!(p.after[0], AfterRow::NewMerge { theirs: "main".into() });
    set_config(&dir, &[("merge.ff", "true"), ("branch.old.mergeoptions", "--ff-only --no-ff")]);
    let p = run(&dir, merge_op(&dir, "main")).unwrap();
    assert_eq!(p.outcome, Outcome::Done, "the branch's last option wins over merge.ff");
    // Diverged with fast-forward only: git refuses.
    commit(&dir, "o.txt", "o\n", "o1");
    set_config(&dir, &[("merge.ff", "only"), ("branch.old.mergeoptions", "")]);
    let e = run(&dir, merge_op(&dir, "main")).unwrap_err();
    assert!(message(e).contains("fast-forward"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_ref_that_moved_is_refused() {
    let (dir, [base, ..]) = diverged("opp-merge-moved");
    git(&dir, &["checkout", "-q", "main"]);
    let e = run(&dir, PreviewOp::Merge { theirs: "feature".into(), oid: base }).unwrap_err();
    assert!(message(e).contains("moved since"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unrelated_histories_are_refused() {
    let dir = scratch_repo("opp-merge-unrelated");
    commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["checkout", "-q", "--orphan", "other"]);
    git(&dir, &["rm", "-q", "-rf", "."]);
    commit(&dir, "o.txt", "o\n", "orphan");
    git(&dir, &["checkout", "-q", "main"]);
    let e = run(&dir, merge_op(&dir, "other")).unwrap_err();
    assert_eq!(message(e), UNRELATED);
    let _ = std::fs::remove_dir_all(&dir);
}
