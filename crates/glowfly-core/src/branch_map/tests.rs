//! Measuring and forecasting on fixture repositories: diverged, merged and identical branches,
//! conflicting and merely sharing pairs, renames, unrelated histories, the cap, remembered
//! forecasts, stopping, odd paths, and that the repository is left exactly as it was.

use super::*;
use crate::repo::{LoadOptions, load};
use crate::testkit::git_env;
use crate::testutil::{git, scratch_repo, write};
use git2::Repository;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};

/// Committer times of the fixtures' commits: each commit is a minute after the one before.
static CLOCK: AtomicI64 = AtomicI64::new(1_700_000_000);

/// Commits `file` with `text` on the checked-out branch; returns the commit.
pub(super) fn commit(dir: &Path, file: &str, text: &str, msg: &str) -> Oid {
    let when = format!("{} +0000", CLOCK.fetch_add(60, Ordering::SeqCst));
    write(dir, file, text);
    git(dir, &["add", "--", file]);
    git_env(dir, &["commit", "-q", "-m", msg], &[("GIT_COMMITTER_DATE", &when), ("GIT_AUTHOR_DATE", &when)]);
    oid(dir, "HEAD")
}

fn oid(dir: &Path, rev: &str) -> Oid {
    Repository::open(dir).unwrap().revparse_single(rev).unwrap().peel_to_commit().unwrap().id()
}

fn plan(dir: &Path) -> Request {
    request(&load(dir, &LoadOptions::all(100)).unwrap()).unwrap()
}

fn map(dir: &Path) -> BranchMap {
    compute(dir, &plan(dir), &Forecasts::new(), &|| false).unwrap().unwrap()
}

fn row<'a>(m: &'a BranchMap, name: &str) -> (usize, &'a Row) {
    let i = m.row_of(&BranchRef::Local(name.into())).unwrap_or_else(|| panic!("no row for {name}"));
    (i, &m.rows[i])
}

fn pair<'a>(m: &'a BranchMap, a: &str, b: &str) -> Option<&'a Pair> {
    let (x, y) = (row(m, a).0, row(m, b).0);
    m.pairs.iter().find(|p| (p.a, p.b) == (x.min(y), x.max(y)))
}

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

/// A long file, so edits far apart in it merge without touching.
fn lines(edits: &[(usize, &str)]) -> String {
    (0..20).map(|i| edits.iter().find(|(n, _)| *n == i).map_or(format!("line {i}"), |(_, t)| (*t).to_owned()) + "\n").collect()
}

#[test]
fn diverged_merged_and_identical_branches() {
    let dir = scratch_repo("bm-diverged");
    commit(&dir, "base.txt", "base\n", "base 0");
    let old = commit(&dir, "m0.txt", "m0\n", "main 1");
    let fork = commit(&dir, "m1.txt", "m1\n", "main 2");
    git(&dir, &["branch", "merged", &old.to_string()]);
    git(&dir, &["checkout", "-q", "-b", "diverged"]);
    commit(&dir, "a.txt", "a\n", "d1");
    commit(&dir, "b.txt", "b\n", "d2");
    git(&dir, &["checkout", "-q", "main"]);
    for i in 0..3 {
        commit(&dir, &format!("main{i}.txt"), "m\n", "more main");
    }
    git(&dir, &["branch", "identical"]);
    let m = map(&dir);
    assert_eq!(m.rows[0].branch, BranchRef::Local("main".into()));
    let (_, d) = row(&m, "diverged");
    assert_eq!((d.ahead, d.behind, d.base), (2, 3, Some(fork)));
    assert_eq!((d.files.clone(), d.files_total), (names(&["a.txt", "b.txt"]), 2));
    let (_, mg) = row(&m, "merged");
    assert_eq!((mg.ahead, mg.behind, mg.base), (0, 4, Some(old)));
    assert!(mg.files.is_empty());
    let (_, id) = row(&m, "identical");
    assert_eq!((id.ahead, id.behind), (0, 0));
    assert!(id.files.is_empty());
    let times: Vec<i64> = m.rows[1..].iter().map(|r| r.time).collect();
    assert!(times.windows(2).all(|w| w[0] >= w[1]), "newest first: {times:?}");
    assert_eq!(m.total, 3);
    assert_eq!(m.current, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_conflicting_pair_and_a_sharing_pair_without_conflict() {
    let dir = scratch_repo("bm-pairs");
    commit(&dir, "f.txt", &lines(&[]), "base");
    commit(&dir, "other.txt", "x\n", "other base");
    for (name, edit) in [("one", (5, "one")), ("two", (5, "two")), ("three", (19, "three"))] {
        git(&dir, &["checkout", "-q", "-b", name, "main"]);
        commit(&dir, "f.txt", &lines(&[edit]), name);
    }
    git(&dir, &["checkout", "-q", "-b", "four", "main"]);
    commit(&dir, "other.txt", "four\n", "four");
    let m = map(&dir);
    let p = pair(&m, "one", "two").unwrap();
    assert_eq!(p.shared, names(&["f.txt"]));
    assert_eq!(p.forecast, Forecast::Conflicts(names(&["f.txt"])));
    let p = pair(&m, "one", "three").unwrap();
    assert_eq!((p.shared.clone(), p.forecast.clone()), (names(&["f.txt"]), Forecast::Clean));
    assert!(pair(&m, "four", "one").is_none() && pair(&m, "four", "two").is_none());
    let (one, _) = row(&m, "one");
    let blocks = m.shared_with(one);
    let order: Vec<&str> = blocks.iter().map(|b| m.rows[b.other].branch.shown()).collect();
    assert_eq!(order, ["two", "three"]);
    assert_eq!(blocks[0].files, vec![("f.txt", true)]);
    assert_eq!(blocks[1].files, vec![("f.txt", false)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn conflict_with_the_main_branch() {
    let dir = scratch_repo("bm-main-conflict");
    commit(&dir, "f.txt", &lines(&[]), "base");
    git(&dir, &["checkout", "-q", "-b", "topic"]);
    commit(&dir, "f.txt", &lines(&[(3, "topic")]), "topic");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "f.txt", &lines(&[(3, "main")]), "main");
    let m = map(&dir);
    let (i, _) = row(&m, "topic");
    assert!(matches!(&pair(&m, "main", "topic").unwrap().forecast, Forecast::Conflicts(_)));
    assert_eq!(m.conflicts_with_main(i), Some(&names(&["f.txt"])[..]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_branch_that_is_only_ahead_has_no_pair_with_main() {
    let dir = scratch_repo("bm-only-ahead");
    commit(&dir, "f.txt", &lines(&[]), "base");
    git(&dir, &["checkout", "-q", "-b", "topic"]);
    commit(&dir, "f.txt", &lines(&[(3, "topic")]), "topic");
    git(&dir, &["checkout", "-q", "main"]);
    let m = map(&dir);
    let (i, r) = row(&m, "topic");
    assert_eq!((r.ahead, r.behind), (1, 0));
    assert!(m.pairs.is_empty() && m.conflicts_with_main(i).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_rename_counts_as_both_names() {
    let dir = scratch_repo("bm-rename");
    commit(&dir, "old.txt", &lines(&[]), "base");
    git(&dir, &["checkout", "-q", "-b", "mover"]);
    git(&dir, &["mv", "old.txt", "new.txt"]);
    git(&dir, &["commit", "-q", "-m", "rename"]);
    git(&dir, &["checkout", "-q", "main"]);
    let m = map(&dir);
    assert_eq!(row(&m, "mover").1.files, names(&["new.txt", "old.txt"]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unrelated_history_has_no_base_or_pairs() {
    let dir = scratch_repo("bm-unrelated");
    commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["checkout", "-q", "--orphan", "island"]);
    commit(&dir, "a.txt", "island\n", "i1");
    commit(&dir, "a.txt", "island two\n", "i2");
    git(&dir, &["checkout", "-q", "main"]);
    let m = map(&dir);
    let (_, r) = row(&m, "island");
    assert_eq!((r.base, r.files.len(), r.files_total, r.ahead), (None, 0, 0, 2));
    assert!(m.pairs.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_forty_most_recent_branches_and_the_current_one() {
    let dir = scratch_repo("bm-cap");
    commit(&dir, "a.txt", "a\n", "base");
    git(&dir, &["checkout", "-q", "-b", "oldest"]);
    commit(&dir, "o.txt", "o\n", "oldest");
    for i in 0..42 {
        git(&dir, &["checkout", "-q", "-b", &format!("b{i:02}"), "main"]);
        commit(&dir, &format!("f{i}.txt"), "x\n", "tip");
    }
    git(&dir, &["checkout", "-q", "oldest"]);
    let m = map(&dir);
    assert_eq!((m.rows.len(), m.total), (41, 43));
    let shown: Vec<&str> = m.rows.iter().map(|r| r.branch.shown()).collect();
    assert!(shown.contains(&"oldest") && !shown.contains(&"b00") && !shown.contains(&"b01"), "{shown:?}");
    assert_eq!(m.current.map(|i| m.rows[i].branch.shown()), Some("oldest"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// main with f.txt and three branches: `one` and `two` conflict, `three` merges with both.
fn conflicting(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    commit(&dir, "f.txt", &lines(&[]), "base");
    for (b, edit) in [("one", (5, "one")), ("two", (5, "two")), ("three", (19, "three"))] {
        git(&dir, &["checkout", "-q", "-b", b, "main"]);
        commit(&dir, "f.txt", &lines(&[edit]), b);
    }
    dir
}

fn remember(m: &BranchMap) -> Forecasts {
    m.pairs.iter().map(|p| (pair_key(m.rows[p.a].tip, m.rows[p.b].tip), p.forecast.clone())).collect()
}

#[test]
fn known_forecasts_are_not_merged_again() {
    let dir = conflicting("bm-known");
    let first = compute(&dir, &plan(&dir), &Forecasts::new(), &|| false).unwrap().unwrap();
    assert!(first.merged >= 1);
    let known = remember(&first);
    let second = compute(&dir, &plan(&dir), &known, &|| false).unwrap().unwrap();
    assert_eq!((&second.rows, &second.pairs, second.merged), (&first.rows, &first.pairs, 0));
    // One branch moves: only the pairs with its new tip are merged again.
    git(&dir, &["checkout", "-q", "three"]);
    commit(&dir, "f.txt", &lines(&[(19, "three"), (18, "more")]), "three again");
    let third = compute(&dir, &plan(&dir), &known, &|| false).unwrap().unwrap();
    assert_eq!(third.merged, 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stop_returns_none() {
    let dir = conflicting("bm-stop");
    assert_eq!(compute(&dir, &plan(&dir), &Forecasts::new(), &|| true).unwrap(), None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every file under `.git/objects`, counted.
fn object_files(dir: &Path) -> usize {
    fn count(p: &Path) -> usize {
        std::fs::read_dir(p).map_or(0, |rd| rd.flatten().map(|e| if e.path().is_dir() { count(&e.path()) } else { 1 }).sum())
    }
    count(&dir.join(".git").join("objects"))
}

type State = (String, Vec<(String, Oid)>, Vec<u8>, String, Vec<Vec<u8>>);

/// HEAD, every ref, the index, git's status and the working files, as bytes.
fn state_of(dir: &Path) -> State {
    let repo = Repository::open(dir).unwrap();
    let head = std::fs::read_to_string(dir.join(".git").join("HEAD")).unwrap();
    let mut refs: Vec<(String, Oid)> =
        repo.references().unwrap().flatten().filter_map(|r| Some((r.name()?.to_owned(), r.target()?))).collect();
    refs.sort();
    let index = std::fs::read(dir.join(".git").join("index")).unwrap_or_default();
    let (ok, status) = crate::gitcli::run(dir, &crate::gitcli::args(&["status", "--porcelain=v1"]));
    assert!(ok);
    let files = ["f.txt", "staged.txt", "untracked.txt"].iter().map(|f| std::fs::read(dir.join(f)).unwrap_or_default()).collect();
    (head, refs, index, status, files)
}

#[test]
fn the_forecast_changes_nothing() {
    let dir = conflicting("bm-nothing");
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, "f.txt", &lines(&[(1, "dirty")]));
    write(&dir, "staged.txt", "staged\n");
    git(&dir, &["add", "staged.txt"]);
    write(&dir, "untracked.txt", "u\n");
    let (before, objects) = (state_of(&dir), object_files(&dir));
    let m = compute(&dir, &plan(&dir), &Forecasts::new(), &|| false).unwrap().unwrap();
    assert!(m.pairs.iter().any(|p| matches!(p.forecast, Forecast::Conflicts(_))));
    assert_eq!(state_of(&dir), before);
    assert_eq!(object_files(&dir), objects, "the trial merges wrote object files");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn odd_paths_come_back_exactly() {
    let dir = scratch_repo("bm-odd-paths");
    let path = "dir/b c \u{fc}.txt";
    std::fs::create_dir_all(dir.join("dir")).unwrap();
    commit(&dir, path, &lines(&[]), "base");
    for b in ["one", "two"] {
        git(&dir, &["checkout", "-q", "-b", b, "main"]);
        commit(&dir, path, &lines(&[(5, b)]), b);
    }
    let m = map(&dir);
    let p = pair(&m, "one", "two").unwrap();
    assert_eq!(p.shared, names(&[path]));
    assert_eq!(p.forecast, Forecast::Conflicts(names(&[path])));
    assert_eq!(row(&m, "one").1.files, names(&[path]));
    let _ = std::fs::remove_dir_all(&dir);
}
