//! Rebase previews: a clean replay, a stop at the second of three commits, up to date and
//! fast-forward, a branch with the target merged in replayed rather than up to date, patch-equivalent commits left out, merges left out (and refused with
//! rebase.rebaseMerges, set or bare), an unrelated history replayed from its root, a same-named tag that is
//! never taken for the branch, and a rebase onto a commit.

use super::tests::{commit, diverged, is_kept, is_rewritten, message, oid, row_oid, run, run_with};
use super::*;
use crate::testutil::{git, scratch_repo, set_config};

fn onto(dir: &Path, name: &str) -> PreviewOp {
    PreviewOp::Rebase { onto: Some(BranchRef::Local(name.into())), oid: oid(dir, &format!("refs/heads/{name}")) }
}

#[test]
fn a_clean_rebase_rewrites_each_commit() {
    let (dir, [base, m1, f1, f2]) = diverged("opp-rebase-clean");
    let p = run(&dir, onto(&dir, "main")).unwrap();
    assert_eq!(p.outcome, Outcome::Done);
    assert_eq!(p.moved.iter().map(|m| m.oid).collect::<Vec<_>>(), [f1, f2], "oldest first");
    assert!(is_rewritten(&p.after[0], f2) && is_rewritten(&p.after[1], f1), "{:?}", p.after);
    assert!(matches!(&p.after[2], AfterRow::Kept { line, arrives: true } if line.oid == m1));
    assert!(is_kept(&p.after[3], base));
    assert_eq!(p.arriving, Some(Count { n: 1, more: false }));
    assert!(p.gone.is_empty(), "{:?}", p.gone);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_conflicting_commit_stops_the_rebase() {
    let dir = scratch_repo("opp-rebase-stop");
    commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["branch", "feature"]);
    commit(&dir, "a.txt", "main\n", "main edit");
    git(&dir, &["checkout", "-q", "feature"]);
    let c1 = commit(&dir, "b.txt", "b\n", "c1");
    let c2 = commit(&dir, "a.txt", "feature\n", "c2");
    let c3 = commit(&dir, "c.txt", "c\n", "c3");
    let p = run(&dir, onto(&dir, "main")).unwrap();
    let Outcome::Stopped(stop) = &p.outcome else { panic!("{:?}", p.outcome) };
    assert_eq!(stop.why, StopWhy::Conflicts(vec!["a.txt".into()]));
    assert_eq!(stop.applied, 1);
    assert_eq!(stop.waiting.iter().map(|w| w.oid).collect::<Vec<_>>(), [c3]);
    assert_eq!(row_oid(&p.after[0]), Some(c2));
    assert!(matches!(p.after[0], AfterRow::Stopped { .. }));
    assert!(is_rewritten(&p.after[1], c1), "{:?}", p.after);
    assert!(p.gone.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn up_to_date_and_fast_forward() {
    let (dir, [base, ..]) = diverged("opp-rebase-ff");
    git(&dir, &["branch", "old", &base.to_string()]);
    assert_eq!(run(&dir, onto(&dir, "old")).unwrap().outcome, Outcome::Nothing);
    git(&dir, &["checkout", "-q", "old"]);
    let p = run(&dir, onto(&dir, "main")).unwrap();
    assert_eq!(p.outcome, Outcome::FastForward);
    assert_eq!(p.arriving, Some(Count { n: 1, more: false }));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_branch_with_the_target_merged_in_is_replayed_without_the_merge() {
    let (dir, [_, m1, f1, f2]) = diverged("opp-rebase-merged-in");
    git(&dir, &["merge", "-q", "--no-ff", "-m", "merge main", "main"]);
    let merge = oid(&dir, "HEAD");
    let p = run(&dir, onto(&dir, "main")).unwrap();
    assert_eq!(p.outcome, Outcome::Done, "a merge between HEAD and the target is not up to date");
    assert_eq!(p.moved.iter().map(|m| m.oid).collect::<Vec<_>>(), [f1, f2]);
    assert_eq!(p.gone.iter().map(|g| (g.line.oid, g.why)).collect::<Vec<_>>(), [(merge, GoneWhy::MergeLeftOut)]);
    assert!(is_rewritten(&p.after[0], f2) && is_rewritten(&p.after[1], f1), "{:?}", p.after);
    assert!(matches!(&p.after[2], AfterRow::Kept { line, .. } if line.oid == m1));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_commit_already_on_the_target_is_left_out() {
    let (dir, [_, _, f1, f2]) = diverged("opp-rebase-applied");
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["cherry-pick", &f1.to_string()]);
    git(&dir, &["checkout", "-q", "feature"]);
    let p = run(&dir, onto(&dir, "main")).unwrap();
    assert_eq!(p.outcome, Outcome::Done);
    assert_eq!(p.moved.iter().map(|m| m.oid).collect::<Vec<_>>(), [f2]);
    assert_eq!(p.gone.iter().map(|g| (g.line.oid, g.why)).collect::<Vec<_>>(), [(f1, GoneWhy::AlreadyApplied)]);
    let lim = Limits { patch: 0, ..Limits::DEFAULT };
    let p = run_with(&dir, onto(&dir, "main"), &lim).unwrap();
    assert!(p.patch_check_skipped);
    // Replayed anyway, it becomes empty and is left out all the same.
    assert_eq!(p.gone.iter().map(|g| (g.line.oid, g.why)).collect::<Vec<_>>(), [(f1, GoneWhy::AlreadyApplied)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn merges_are_left_out_and_their_side_rewritten() {
    let (dir, [_, _, f1, f2]) = diverged("opp-rebase-merges");
    git(&dir, &["checkout", "-q", "-b", "side", &f1.to_string()]);
    let s1 = commit(&dir, "s.txt", "s\n", "s1");
    git(&dir, &["checkout", "-q", "feature"]);
    git(&dir, &["merge", "-q", "--no-ff", "-m", "merge side", "side"]);
    let merge = oid(&dir, "HEAD");
    git(&dir, &["branch", "-D", "side"]);
    let p = run(&dir, onto(&dir, "main")).unwrap();
    assert_eq!(p.outcome, Outcome::Done);
    assert_eq!(p.gone.iter().map(|g| (g.line.oid, g.why)).collect::<Vec<_>>(), [(merge, GoneWhy::MergeLeftOut)]);
    let mut moved: Vec<Oid> = p.moved.iter().map(|m| m.oid).collect();
    assert_eq!(moved.remove(0), f1, "topological order: f1 first");
    moved.sort();
    let mut rest = vec![f2, s1];
    rest.sort();
    assert_eq!(moved, rest);
    set_config(&dir, &[("rebase.rebaseMerges", "true")]);
    assert_eq!(message(run(&dir, onto(&dir, "main")).unwrap_err()), REBASE_MERGES);
    // A bare key (no `=`) is true too, as git reads it.
    git(&dir, &["config", "--unset-all", "rebase.rebaseMerges"]);
    crate::testutil::add_bare_key(&dir, "rebase", "rebaseMerges");
    assert_eq!(message(run(&dir, onto(&dir, "main")).unwrap_err()), REBASE_MERGES);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unrelated_history_is_replayed_from_its_root() {
    let dir = scratch_repo("opp-rebase-orphan");
    let main = commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["checkout", "-q", "--orphan", "other"]);
    git(&dir, &["rm", "-q", "-rf", "."]);
    let o1 = commit(&dir, "o.txt", "o\n", "o1");
    let o2 = commit(&dir, "p.txt", "p\n", "o2");
    let p = run(&dir, onto(&dir, "main")).unwrap();
    assert_eq!(p.outcome, Outcome::Done);
    assert_eq!(p.moved.iter().map(|m| m.oid).collect::<Vec<_>>(), [o1, o2]);
    assert!(is_rewritten(&p.after[0], o2) && is_rewritten(&p.after[1], o1));
    assert!(matches!(&p.after[2], AfterRow::Kept { line, .. } if line.oid == main));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_same_named_tag_is_not_the_branch() {
    let (dir, [base, m1, f1, f2]) = diverged("opp-rebase-tag");
    git(&dir, &["tag", "main", &base.to_string()]);
    let p = run(&dir, onto(&dir, "main")).unwrap();
    assert_eq!(p.outcome, Outcome::Done, "onto the branch, not the tag (which feature contains)");
    assert!(is_rewritten(&p.after[0], f2) && is_rewritten(&p.after[1], f1), "{:?}", p.after);
    assert!(is_kept(&p.after[2], m1), "replayed onto main's tip: {:?}", p.after);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_rebase_onto_a_commit_needs_the_commit() {
    let (dir, [_, m1, f1, f2]) = diverged("opp-rebase-commit");
    let p = run(&dir, PreviewOp::Rebase { onto: None, oid: m1 }).unwrap();
    assert_eq!(p.outcome, Outcome::Done);
    assert!(is_rewritten(&p.after[0], f2) && is_rewritten(&p.after[1], f1), "{:?}", p.after);
    let missing = Oid::from_str("1234567890123456789012345678901234567890").unwrap();
    let e = run(&dir, PreviewOp::Rebase { onto: None, oid: missing }).unwrap_err();
    assert!(matches!(e, Error::NotFound(_)), "{e:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
