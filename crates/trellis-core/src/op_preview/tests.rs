//! Shared fixture helpers, and what every preview has in common: nothing on disk changes, a
//! stale HEAD, an operation in progress and an unborn HEAD are refused, and the history is cut
//! at the row limit.

use super::*;
use crate::testutil::{git, scratch_repo, write};
use std::path::{Path, PathBuf};

/// Commits `file` with `text` on the checked-out branch; returns the commit.
pub(super) fn commit(dir: &Path, file: &str, text: &str, msg: &str) -> Oid {
    write(dir, file, text);
    git(dir, &["add", "--", file]);
    git(dir, &["commit", "-q", "-m", msg]);
    oid(dir, "HEAD")
}

pub(super) fn oid(dir: &Path, rev: &str) -> Oid {
    Repository::open(dir).unwrap().revparse_single(rev).unwrap().peel_to_commit().unwrap().id()
}

/// The preview of `op` against HEAD now.
pub(super) fn run(dir: &Path, op: PreviewOp) -> Result<OpPreview> {
    run_with(dir, op, &Limits::DEFAULT)
}

pub(super) fn run_with(dir: &Path, op: PreviewOp, lim: &Limits) -> Result<OpPreview> {
    preview_with(dir, &Request { op, head: oid(dir, "HEAD") }, lim)
}

/// The commit of a Kept row, the original of a Rewritten one.
pub(super) fn row_oid(r: &AfterRow) -> Option<Oid> {
    match r {
        AfterRow::Kept { line, .. } => Some(line.oid),
        AfterRow::Rewritten { from, .. } => Some(from.oid),
        AfterRow::Stopped { at } => at.as_ref().map(|l| l.oid),
        AfterRow::NewMerge { .. } => None,
    }
}

pub(super) fn is_rewritten(r: &AfterRow, of: Oid) -> bool {
    matches!(r, AfterRow::Rewritten { from, .. } if from.oid == of)
}

pub(super) fn is_kept(r: &AfterRow, of: Oid) -> bool {
    matches!(r, AfterRow::Kept { line, .. } if line.oid == of)
}

pub(super) fn message(e: Error) -> String {
    match e {
        Error::Unsupported(t) => t,
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

/// main: base (a.txt) - m1 (m.txt); feature from base: f1 (f.txt) - f2 (g.txt); `feature`
/// checked out.
pub(super) fn diverged(name: &str) -> (PathBuf, [Oid; 4]) {
    let dir = scratch_repo(name);
    let base = commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["branch", "feature"]);
    let m1 = commit(&dir, "m.txt", "m\n", "m1");
    git(&dir, &["checkout", "-q", "feature"]);
    let f1 = commit(&dir, "f.txt", "f\n", "f1");
    let f2 = commit(&dir, "g.txt", "g\n", "f2");
    (dir, [base, m1, f1, f2])
}

/// Every file under `.git/objects`, counted.
fn object_files(dir: &Path) -> usize {
    fn count(p: &Path) -> usize {
        std::fs::read_dir(p).map_or(0, |rd| rd.flatten().map(|e| if e.path().is_dir() { count(&e.path()) } else { 1 }).sum())
    }
    count(&dir.join(".git").join("objects"))
}

/// HEAD, every ref and its target, the index's bytes and git's status.
fn state_of(dir: &Path) -> (String, Vec<(String, Oid)>, Vec<u8>, String) {
    let repo = Repository::open(dir).unwrap();
    let head = repo.head().map(|h| format!("{:?} {:?}", h.name(), h.target())).unwrap_or_default();
    let mut refs: Vec<(String, Oid)> =
        repo.references().unwrap().flatten().filter_map(|r| Some((r.name()?.to_owned(), r.target()?))).collect();
    refs.sort();
    let index = std::fs::read(dir.join(".git").join("index")).unwrap_or_default();
    let (ok, status) = crate::gitcli::run(dir, &crate::gitcli::args(&["status", "--porcelain"]));
    assert!(ok);
    (head, refs, index, status)
}

#[test]
fn previews_leave_the_repository_as_it_was() {
    let (dir, [base, m1, f1, f2]) = diverged("opp-untouched");
    // f3 conflicts with nothing; f4 edits a.txt, which main's m2 edits too.
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "a.txt", "main\n", "m2");
    git(&dir, &["checkout", "-q", "feature"]);
    commit(&dir, "a.txt", "feature\n", "f4");
    write(&dir, "untracked.txt", "u\n");
    let before = (state_of(&dir), object_files(&dir));
    let ops = [
        PreviewOp::Rebase { onto: "main".into(), oid: oid(&dir, "main") },
        PreviewOp::CherryPick(vec![m1]),
        PreviewOp::Drop(vec![f1]),
        PreviewOp::Reset { to: base },
        PreviewOp::Reset { to: f2 },
    ];
    for op in ops {
        let p = run(&dir, op.clone()).unwrap_or_else(|e| panic!("{op:?}: {e}"));
        assert_eq!(p.op, op);
        assert_eq!((state_of(&dir), object_files(&dir)), before, "{op:?} changed something on disk");
    }
    let p = run(&dir, PreviewOp::Rebase { onto: "main".into(), oid: oid(&dir, "main") }).unwrap();
    assert!(matches!(p.outcome, Outcome::Stopped(_)), "{:?}", p.outcome);
    // A merge runs merge-tree, which may write unreferenced objects; refs, index and status stay.
    if crate::gitcli::version(&dir).is_some_and(|v| (v.0, v.1) >= crate::conflict_check::MIN_GIT) {
        let p = run(&dir, PreviewOp::Merge { theirs: "main".into(), oid: oid(&dir, "main") }).unwrap();
        assert!(matches!(p.outcome, Outcome::Stopped(_)), "{:?}", p.outcome);
        assert_eq!(state_of(&dir), before.0);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stale_head_is_refused() {
    let (dir, [base, ..]) = diverged("opp-stale");
    let req = Request { op: PreviewOp::Reset { to: base }, head: base };
    let e = preview(&dir, &req).unwrap_err();
    assert_eq!(message(e), STALE);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_operation_in_progress_is_refused() {
    let dir = scratch_repo("opp-in-progress");
    commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["branch", "side"]);
    commit(&dir, "a.txt", "main\n", "main edit");
    git(&dir, &["checkout", "-q", "side"]);
    commit(&dir, "a.txt", "side\n", "side edit");
    let (ok, _) = crate::gitcli::run(&dir, &crate::gitcli::args(&["merge", "main"]));
    assert!(!ok, "the merge stops with conflicts");
    let e = run(&dir, PreviewOp::Reset { to: oid(&dir, "main") }).unwrap_err();
    let text = message(e);
    assert!(text.contains("merge in progress"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unborn_head_is_refused() {
    let dir = scratch_repo("opp-unborn");
    let req = Request { op: PreviewOp::Reset { to: Oid::zero() }, head: Oid::zero() };
    assert_eq!(message(preview(&dir, &req).unwrap_err()), UNBORN);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn history_is_cut_at_the_row_limit() {
    let dir = scratch_repo("opp-rows");
    let first = commit(&dir, "a.txt", "0\n", "c0");
    for i in 1..6 {
        commit(&dir, "a.txt", &format!("{i}\n"), &format!("c{i}"));
    }
    let lim = Limits { rows: 3, ..Limits::DEFAULT };
    let p = run_with(&dir, PreviewOp::Reset { to: oid(&dir, "HEAD~1") }, &lim).unwrap();
    assert_eq!(p.after.len(), 3);
    assert!(p.after_more);
    let all = run(&dir, PreviewOp::Reset { to: oid(&dir, "HEAD~1") }).unwrap();
    assert_eq!(all.after.len(), 5);
    assert!(!all.after_more);
    assert_eq!(row_oid(all.after.last().unwrap()), Some(first));
    let _ = std::fs::remove_dir_all(&dir);
}
