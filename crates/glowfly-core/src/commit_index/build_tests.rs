//! Building an index against real repositories: the same order as the graph's walk, starts from
//! filtered refs, incremental updates equal to fresh builds, shallow boundaries, stopping,
//! progress and the cap.

use super::build::{Pace, build_capped, build_paced};
use super::*;
use crate::repo::{LoadOptions, Snapshot, load};
use crate::testutil::*;
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

fn dated(dir: &Path, a: &[&str], time: i64) {
    let date = format!("{} +0000", 1_600_000_000 + time);
    let env = [("GIT_COMMITTER_DATE", date.as_str()), ("GIT_AUTHOR_DATE", date.as_str())];
    let (ok, out) = crate::gitcli::run_with_input(dir, &crate::gitcli::args(a), "", &env);
    assert!(ok, "git {a:?}: {out}");
}

/// A commit writing `file` (named after `msg`) with committer and author time `time`.
fn commit_at(dir: &Path, msg: &str, time: i64) {
    write(dir, &format!("{msg}.txt"), msg);
    git(dir, &["add", "."]);
    dated(dir, &["commit", "-q", "-m", msg, "-m", &format!("The body of {msg}.")], time);
}

/// Two branches merged, an octopus merge, equal timestamps and a child older than its parent.
fn branchy(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    commit_at(&dir, "base", 100);
    commit_at(&dir, "m1", 200);
    for b in ["b1", "b2", "b3"] {
        git(&dir, &["branch", b]);
    }
    git(&dir, &["checkout", "-q", "b1"]);
    commit_at(&dir, "b1a", 300);
    commit_at(&dir, "b1b", 300);
    git(&dir, &["checkout", "-q", "b2"]);
    commit_at(&dir, "b2a", 300);
    git(&dir, &["checkout", "-q", "b3"]);
    // The clock went back.
    commit_at(&dir, "b3a", 50);
    git(&dir, &["checkout", "-q", "main"]);
    commit_at(&dir, "m2", 400);
    dated(&dir, &["merge", "-q", "--no-ff", "-m", "merge b1", "b1"], 500);
    dated(&dir, &["merge", "-q", "--no-ff", "-m", "octopus", "b2", "b3"], 600);
    git(&dir, &["checkout", "-q", "-b", "skew"]);
    commit_at(&dir, "skewed", 10);
    git(&dir, &["checkout", "-q", "main"]);
    dir
}

fn snapshot(dir: &Path, opts: LoadOptions) -> Snapshot {
    load(dir, &opts).unwrap()
}

fn index(dir: &Path, snap: &Snapshot, base: Option<&CommitIndex>) -> CommitIndex {
    build(dir, &IndexKey::of(snap), base, &mut |_| {}, &|| false).unwrap().expect("not stopped")
}

fn oids(ix: &CommitIndex) -> Vec<Oid> {
    (0..ix.len()).map(|p| ix.oid_at(p).unwrap()).collect()
}

/// Every position's oid, texts and author, for comparing two indexes.
fn rows(ix: &CommitIndex) -> Vec<(Oid, String, String, (String, String))> {
    (0..ix.len())
        .map(|p| {
            let (n, m) = ix.person_at(p).unwrap();
            (ix.oid_at(p).unwrap(), ix.summary_at(p).unwrap().to_owned(), ix.body_at(p).unwrap().to_owned(), (n.to_owned(), m.to_owned()))
        })
        .collect()
}

#[test]
fn walk_positions_match_the_graph_load() {
    let dir = branchy("index-walk");
    let all = snapshot(&dir, LoadOptions::all(1000));
    assert!(!all.truncated);
    assert_eq!(all.starts.first(), all.head_oid.as_ref(), "the walk starts at HEAD");
    for b in ["b1", "b2", "b3", "skew"] {
        let tip = git2::Repository::open(&dir).unwrap().revparse_single(b).unwrap().id();
        assert!(all.starts.contains(&tip), "{b} is a start");
    }
    let ix = index(&dir, &all, None);
    assert_eq!(ix.len(), all.commits.len());
    assert!(ix.complete());
    let order = oids(&ix);
    for k in 1..=ix.len() {
        let snap = snapshot(&dir, LoadOptions::all(k));
        let loaded: HashSet<Oid> = snap.commits.iter().map(|c| c.oid).collect();
        let first: HashSet<Oid> = order[..k].iter().copied().collect();
        assert_eq!(first, loaded, "the first {k} positions are what a load of {k} shows");
    }
    // The texts are the graph's.
    for c in &all.commits {
        let p = ix.position(c.oid).unwrap() as usize;
        assert_eq!((ix.summary_at(p).unwrap(), ix.body_at(p).unwrap()), (c.summary.as_str(), c.body.as_str()));
        assert_eq!(ix.person_at(p).unwrap(), (c.author.as_str(), c.email.as_str()));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn index_with_smart_and_solo_starts_covers_only_their_history() {
    let dir = branchy("index-solo");
    let opts = |f: fn(&mut LoadOptions)| {
        let mut o = LoadOptions::all(1000);
        f(&mut o);
        o
    };
    for o in
        [opts(|o| o.solo = vec!["refs/heads/b3".into()]), opts(|o| o.hidden = vec!["refs/heads/skew".into()]), opts(|o| o.smart = true)]
    {
        let snap = snapshot(&dir, o);
        let ix = index(&dir, &snap, None);
        let loaded: HashSet<Oid> = snap.commits.iter().map(|c| c.oid).collect();
        assert_eq!(oids(&ix).into_iter().collect::<HashSet<_>>(), loaded);
        assert_eq!(ix.len(), snap.commits.len());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn incremental_update_equals_a_fresh_build() {
    let dir = branchy("index-update");
    let before = snapshot(&dir, LoadOptions::all(1000));
    let base = index(&dir, &before, None);

    git(&dir, &["checkout", "-q", "-b", "new"]);
    commit_at(&dir, "new1", 700);
    commit_at(&dir, "new2", 710);
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["branch", "-q", "-D", "skew"]);
    git(&dir, &["checkout", "-q", "b2"]);
    git(&dir, &["reset", "-q", "--hard", "HEAD~1"]);
    commit_at(&dir, "b2-again", 720);
    git(&dir, &["checkout", "-q", "main"]);
    commit_at(&dir, "m3", 730);

    let after = snapshot(&dir, LoadOptions::all(1000));
    assert!(!IndexKey::of(&after).covers(&before) && IndexKey::of(&after).covers(&after));
    let updated = index(&dir, &after, Some(&base));
    let fresh = index(&dir, &after, None);
    assert_eq!(rows(&updated), rows(&fresh));
    assert_eq!(updated.len(), after.commits.len());
    assert!(updated.position(before.head_oid.unwrap()).is_some(), "the old main is still reached");
    assert!(Arc::ptr_eq(&updated.texts[0], &base.texts[0]), "the text blocks are shared");
    assert!(!Arc::ptr_eq(&fresh.texts[0], &base.texts[0]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_shallow_change_rebuilds_from_scratch() {
    let src = scratch_repo("index-shallow-src");
    for (i, m) in ["one", "two", "three", "four", "five", "six"].into_iter().enumerate() {
        commit_at(&src, m, 100 * i as i64);
    }
    let dir = src.with_file_name(format!("{}-clone", src.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&dir);
    git(src.parent().unwrap(), &["clone", "-q", "--depth", "2", &file_url(&src), &dir.to_string_lossy()]);
    let shallow = snapshot(&dir, LoadOptions::all(1000));
    assert!(shallow.is_shallow());
    let old = index(&dir, &shallow, None);
    assert_eq!(old.len(), 2);

    git(&dir, &["fetch", "-q", "--deepen", "3"]);
    let deeper = snapshot(&dir, LoadOptions::all(1000));
    assert_ne!(IndexKey::of(&deeper).shallow, old.key().shallow);
    let updated = index(&dir, &deeper, Some(&old));
    assert_eq!(rows(&updated), rows(&index(&dir, &deeper, None)));
    assert_eq!(updated.len(), 5, "the deepened commits are in");
    assert!(!Arc::ptr_eq(&updated.texts[0], &old.texts[0]), "nothing reused");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&src);
}

fn every_commit(max: usize) -> Pace {
    Pace { max, check_every: 1, report_every: Duration::ZERO }
}

#[test]
fn stopping_returns_none() {
    let dir = branchy("index-stop");
    let key = IndexKey::of(&snapshot(&dir, LoadOptions::all(1000)));
    let asked = std::cell::Cell::new(0);
    let stop = || {
        asked.set(asked.get() + 1);
        asked.get() > 1
    };
    assert!(build_paced(&dir, &key, None, &mut |_| {}, &stop, &every_commit(MAX_INDEXED)).unwrap().is_none());
    assert_eq!(asked.get(), 2, "stopped at the second look");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn progress_reports_grow() {
    let dir = branchy("index-progress");
    git(&dir, &["commit-graph", "write", "--reachable"]);
    let snap = snapshot(&dir, LoadOptions::all(1000));
    let mut seen = Vec::new();
    let ix = build_paced(&dir, &IndexKey::of(&snap), None, &mut |p| seen.push(p), &|| false, &every_commit(MAX_INDEXED)).unwrap().unwrap();
    assert!(seen.len() > 2, "{seen:?}");
    assert!(seen.windows(2).all(|w| w[1].indexed > w[0].indexed), "{seen:?}");
    assert!(seen.iter().all(|p| p.estimate == Some(ix.len())), "the commit-graph's count: {seen:?}");
    // An update reads no estimate: it has its base's count.
    let mut seen = Vec::new();
    build_paced(&dir, &IndexKey::of(&snap), Some(&ix), &mut |p| seen.push(p), &|| false, &every_commit(MAX_INDEXED)).unwrap().unwrap();
    assert!(seen.iter().all(|p| p.estimate.is_none()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_capped_build_is_incomplete() {
    let dir = scratch_repo("index-capped");
    for (i, m) in ["one", "two", "three", "four", "five"].into_iter().enumerate() {
        commit_at(&dir, m, 100 * i as i64);
    }
    let snap = snapshot(&dir, LoadOptions::all(1000));
    let key = IndexKey::of(&snap);
    let capped = build_capped(&dir, &key, None, &mut |_| {}, &|| false, 3).unwrap().unwrap();
    assert_eq!((capped.len(), capped.complete()), (3, false));
    let fresh = index(&dir, &snap, None);
    assert_eq!(oids(&capped), oids(&fresh)[..3]);
    // A capped base isn't reused.
    let again = index(&dir, &snap, Some(&capped));
    assert_eq!(rows(&again), rows(&fresh));
    assert!(again.complete());
    assert!(!Arc::ptr_eq(&again.texts[0], &capped.texts[0]));
    let _ = std::fs::remove_dir_all(&dir);
}
