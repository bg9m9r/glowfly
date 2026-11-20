//! An update that stops walking where the rest is the previous index's tail (`splice.rs`): what
//! each kind of ref change reads, walks and appends, always equal to a fresh build; the cap, stop
//! and progress while the tail is appended.

use super::build::{Counts, Pace, build_counted};
use super::build_tests::{branchy, commit_at, dated, index, oids, rows, snapshot};
use super::*;
use crate::commit_query::CommitQuery;
use crate::mailmap::Mailmap;
use crate::repo::LoadOptions;
use crate::testutil::*;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The update of `base` to the history of `snap`, with what it did.
fn update(dir: &Path, snap: &Snapshot, base: &CommitIndex) -> (CommitIndex, Counts) {
    build_counted(dir, &IndexKey::of(snap), Some(base), &mut |_| {}, &|| false, &Pace::capped(MAX_INDEXED)).unwrap().expect("not stopped")
}

type Full = (Vec<(Oid, String, String, (String, String))>, Vec<(i64, i64)>, Vec<Vec<Oid>>);

/// Everything an index holds per position, after checking its positions are consistent.
fn full(ix: &CommitIndex) -> Full {
    assert_eq!(ix.by_oid.len(), ix.len());
    let mut times = Vec::new();
    let mut parents = Vec::new();
    for p in 0..ix.len() {
        let oid = ix.oid_at(p).unwrap();
        assert_eq!(ix.position(oid), Some(p as u32));
        times.push(ix.times_at(p).unwrap());
        parents.push(ix.walked(oid).unwrap().2.to_vec());
    }
    (rows(ix), times, parents)
}

fn linear(name: &str, n: usize) -> PathBuf {
    let dir = scratch_repo(name);
    for i in 1..=n {
        commit_at(&dir, &format!("c{i}"), 100 * i as i64);
    }
    dir
}

fn rev(dir: &Path, name: &str) -> Oid {
    Oid::from_str(crate::testkit::git_out(dir, &["rev-parse", name]).trim()).unwrap()
}

fn same_search(a: &CommitIndex, b: &CommitIndex) {
    let last = b.oid_at(b.len() - 1).unwrap().to_string();
    for q in ["c1", "body of", "author:test", "new", "b1", &last[..7]] {
        let q = CommitQuery::parse(q);
        assert_eq!(a.matching(&q, &Mailmap::none(), &|| false), b.matching(&q, &Mailmap::none(), &|| false));
    }
}

/// Updates `base` to the history now in `dir`, checks it against a fresh build and returns what
/// the update did.
fn checked(dir: &Path, base: &CommitIndex) -> (CommitIndex, Counts) {
    let after = snapshot(dir, LoadOptions::all(1000));
    let (updated, counts) = update(dir, &after, base);
    let fresh = index(dir, &after, None);
    assert_eq!(full(&updated), full(&fresh));
    assert_eq!(updated.complete(), fresh.complete());
    same_search(&updated, &fresh);
    (updated, counts)
}

fn base_of(dir: &Path) -> CommitIndex {
    index(dir, &snapshot(dir, LoadOptions::all(1000)), None)
}

#[test]
fn a_fast_forward_reads_only_the_new_commits() {
    let dir = branchy("splice-ff");
    let base = base_of(&dir);
    commit_at(&dir, "new1", 700);
    commit_at(&dir, "new2", 710);
    let (updated, c) = checked(&dir, &base);
    assert_eq!((c.read, c.walked, c.splice_at, c.spliced), (2, 0, Some(0), base.len()));
    assert!(Arc::ptr_eq(&updated.texts[0], &base.texts[0]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_branch_with_an_old_tip_fast_forwards() {
    let dir = branchy("splice-old-tip");
    let base = base_of(&dir);
    git(&dir, &["checkout", "-q", "b3"]);
    commit_at(&dir, "b3b", 800);
    let (_, c) = checked(&dir, &base);
    assert_eq!((c.read, c.walked, c.splice_at), (1, 0, Some(0)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unchanged_refs_copy_the_whole_base() {
    let dir = branchy("splice-same");
    let snap = snapshot(&dir, LoadOptions::all(1000));
    let base = index(&dir, &snap, None);
    let (updated, c) = update(&dir, &snap, &base);
    assert_eq!((c.read, c.walked, c.splice_at, c.spliced), (0, 0, Some(0), base.len()));
    assert_eq!(full(&updated), full(&base));
    assert!(updated.complete());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_new_commit_older_than_a_tip_splices_after_the_commits_it_passes() {
    let dir = linear("splice-skew", 5);
    git(&dir, &["branch", "side"]);
    git(&dir, &["checkout", "-q", "side"]);
    commit_at(&dir, "s1", 600);
    git(&dir, &["checkout", "-q", "main"]);
    let base = base_of(&dir);
    commit_at(&dir, "n", 350);
    let (_, c) = checked(&dir, &base);
    assert_eq!((c.read, c.walked, c.splice_at, c.spliced), (1, 3, Some(3), base.len() - 3));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_new_branch_at_an_old_commit_splices() {
    let dir = branchy("splice-new-branch");
    let base = base_of(&dir);
    git(&dir, &["branch", "old", &rev(&dir, "main~3").to_string()]);
    let (_, c) = checked(&dir, &base);
    assert_eq!(c.read, 0);
    assert!(c.splice_at.is_some(), "{c:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn merging_an_old_side_branch_splices() {
    let dir = branchy("splice-merge-side");
    git(&dir, &["checkout", "-q", "-b", "side", &rev(&dir, "main~3").to_string()]);
    commit_at(&dir, "s1", 250);
    git(&dir, &["checkout", "-q", "main"]);
    let base = base_of(&dir);
    dated(&dir, &["merge", "-q", "--no-ff", "-m", "merge side", "side"], 800);
    git(&dir, &["branch", "-q", "-D", "side"]);
    let (_, c) = checked(&dir, &base);
    assert_eq!((c.read, c.splice_at), (1, Some(0)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_deleted_merged_branch_splices() {
    let dir = linear("splice-del-merged", 4);
    git(&dir, &["checkout", "-q", "-b", "feat"]);
    commit_at(&dir, "f1", 500);
    git(&dir, &["checkout", "-q", "main"]);
    dated(&dir, &["merge", "-q", "--no-ff", "-m", "merge feat", "feat"], 600);
    let base = base_of(&dir);
    git(&dir, &["branch", "-q", "-D", "feat"]);
    let (_, c) = checked(&dir, &base);
    assert_eq!((c.read, c.walked, c.splice_at), (0, 1, Some(1)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_deleted_branch_that_reorders_ties_still_equals_a_fresh_build() {
    let dir = branchy("splice-ties");
    let base = base_of(&dir);
    git(&dir, &["branch", "-q", "-D", "b1"]);
    let (_, c) = checked(&dir, &base);
    assert_eq!(c.read, 0);
    assert_ne!(c.splice_at, Some(0), "b1b now queues behind b2a, the same time: no splice at the start");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_force_pushed_branch_equals_a_fresh_build() {
    let dir = branchy("splice-force");
    git(&dir, &["checkout", "-q", "-b", "feat"]);
    commit_at(&dir, "f1", 650);
    commit_at(&dir, "f2", 660);
    let (f1, f2) = (rev(&dir, "HEAD~1"), rev(&dir, "HEAD"));
    let base = base_of(&dir);
    git(&dir, &["reset", "-q", "--hard", "HEAD~2"]);
    commit_at(&dir, "f2x", 670);
    let (updated, c) = checked(&dir, &base);
    assert_eq!((updated.position(f1), updated.position(f2)), (None, None));
    assert_eq!((c.splice_at, c.read), (None, 1));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_deleted_branch_with_unique_commits_equals_a_fresh_build() {
    let dir = branchy("splice-del-unique");
    let skewed = rev(&dir, "skew");
    let base = base_of(&dir);
    git(&dir, &["branch", "-q", "-D", "skew"]);
    let (updated, c) = checked(&dir, &base);
    assert_eq!(updated.position(skewed), None);
    assert_eq!((c.splice_at, c.read), (None, 0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_capped_update_splices_up_to_the_cap() {
    let dir = linear("splice-cap", 5);
    let base = base_of(&dir);
    commit_at(&dir, "c6", 600);
    let after = snapshot(&dir, LoadOptions::all(1000));
    let key = IndexKey::of(&after);
    let fresh = index(&dir, &after, None);
    let capped = |max: usize| build_counted(&dir, &key, Some(&base), &mut |_| {}, &|| false, &Pace::capped(max)).unwrap().unwrap();

    let (four, c) = capped(4);
    assert_eq!((four.len(), four.complete()), (4, false));
    assert_eq!(oids(&four), oids(&fresh)[..4]);
    assert_eq!((c.splice_at, c.spliced), (Some(0), 3));
    let fresh4 = build_counted(&dir, &key, None, &mut |_| {}, &|| false, &Pace::capped(4)).unwrap().unwrap().0;
    assert_eq!(full(&four), full(&fresh4));

    let (six, c) = capped(6);
    assert_eq!((six.len(), six.complete(), c.spliced), (6, true, 5));
    assert_eq!(full(&six), full(&fresh));
    let _ = std::fs::remove_dir_all(&dir);
}

fn every_commit() -> Pace {
    Pace { max: MAX_INDEXED, check_every: 1, report_every: Duration::ZERO }
}

#[test]
fn stopping_during_the_bulk_copy_returns_none() {
    let dir = linear("splice-stop", 5);
    let base = base_of(&dir);
    commit_at(&dir, "c6", 600);
    let key = IndexKey::of(&snapshot(&dir, LoadOptions::all(1000)));
    let asked = std::cell::Cell::new(0);
    let stop = || {
        asked.set(asked.get() + 1);
        asked.get() >= 3
    };
    let r = build_counted(&dir, &key, Some(&base), &mut |_| {}, &stop, &every_commit()).unwrap();
    assert!(r.is_none());
    assert_eq!(asked.get(), 3, "one look in the walk, then the bulk chunks");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn progress_grows_during_the_bulk_copy() {
    let dir = linear("splice-progress", 5);
    let base = base_of(&dir);
    commit_at(&dir, "c6", 600);
    let key = IndexKey::of(&snapshot(&dir, LoadOptions::all(1000)));
    let mut seen = Vec::new();
    let (updated, _) = build_counted(&dir, &key, Some(&base), &mut |p| seen.push(p), &|| false, &every_commit()).unwrap().unwrap();
    assert!(seen.len() > 2, "{seen:?}");
    assert!(seen.windows(2).all(|w| w[1].indexed > w[0].indexed), "{seen:?}");
    assert!(seen.iter().all(|p| p.estimate.is_none()));
    assert!(seen.last().unwrap().indexed <= updated.len());
    let _ = std::fs::remove_dir_all(&dir);
}
