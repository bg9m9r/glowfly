//! Tests for reading a gitlink's move from the submodule's repository: forward, back, sideways
//! with and without a common commit, the same commit with changes inside, a commit the
//! submodule lacks, one not checked out or on a refused place, the list's cap, additions,
//! removals and type changes.

use super::*;
use crate::repo::{LoadDiffOptions, load_diff_for_view};
use crate::testutil::{git, link_to, net_place, scratch_repo, with_submodule, write};
use std::path::PathBuf;

const PATH: &str = "libs/lib";

fn head_of(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

/// Commits a change to lib.txt in `dir` with the message `msg`; returns the commit.
fn commit_in(dir: &Path, msg: &str) -> Oid {
    write(dir, "lib.txt", &format!("{msg}\n"));
    git(dir, &["add", "lib.txt"]);
    git(dir, &["commit", "-q", "-m", msg]);
    head_of(dir)
}

fn gitlink(dir: &Path, target: DiffTarget) -> GitlinkMove {
    *load_diff_for_view(dir, &target, LoadDiffOptions::default()).unwrap().gitlink.expect("a gitlink change")
}

fn subjects(run: &CommitRun) -> Vec<&str> {
    run.commits.iter().map(|(_, s)| s.as_str()).collect()
}

/// A parent whose recorded commit moved from the first one (c0) over c1 and c2, committed.
/// Returns (parent, sub, c0).
fn moved_forward(name: &str) -> (PathBuf, PathBuf, Oid) {
    let (parent, sub) = with_submodule(name);
    let c0 = head_of(&sub);
    commit_in(&sub, "c1");
    commit_in(&sub, "c2");
    git(&parent, &["add", PATH]);
    git(&parent, &["commit", "-q", "-m", "move lib"]);
    (parent, sub, c0)
}

#[test]
fn a_commit_moving_forward_lists_the_commits_oldest_first() {
    let (parent, _, c0) = moved_forward("gitlink-forward");
    let m = gitlink(&parent, DiffTarget::Commit(head_of(&parent), PATH.into()));
    assert_eq!(m.kind, MoveKind::Forward);
    assert_eq!(m.old, Some(c0));
    assert_eq!(subjects(&m.ahead), ["c1", "c2"]);
    assert_eq!((m.ahead.total, m.ahead.capped), (2, false));
    assert_eq!(m.behind, CommitRun::default());
    assert!(!m.dirty && !m.type_change);
}

#[test]
fn checking_out_an_older_commit_moves_back() {
    let (parent, sub, c0) = moved_forward("gitlink-backward");
    git(&sub, &["checkout", "-q", "--detach", &c0.to_string()]);
    let m = gitlink(&parent, DiffTarget::Unstaged(PATH.into()));
    assert_eq!(m.kind, MoveKind::Backward);
    assert_eq!(subjects(&m.behind), ["c1", "c2"]);
    assert!(m.ahead.commits.is_empty());
    assert!(!m.dirty);
}

#[test]
fn an_unrelated_history_moves_sideways_without_a_base() {
    let (parent, sub) = with_submodule("gitlink-unrelated");
    git(&sub, &["checkout", "-q", "--orphan", "other"]);
    let orphan = commit_in(&sub, "orphan");
    let m = gitlink(&parent, DiffTarget::Unstaged(PATH.into()));
    assert_eq!(m.kind, MoveKind::Sideways { base: None });
    assert_eq!(m.new, Some(orphan));
    assert_eq!(subjects(&m.ahead), ["orphan"]);
    assert_eq!(subjects(&m.behind), ["lib"]);
}

#[test]
fn a_branch_from_an_older_commit_moves_sideways_from_their_base() {
    let (parent, sub, c0) = moved_forward("gitlink-sideways");
    let c2 = head_of(&sub);
    git(&sub, &["checkout", "-q", "-b", "side", &c0.to_string()]);
    let d1 = commit_in(&sub, "d1");
    let m = between(&parent, PATH, Some(c2), Some(d1));
    assert_eq!(m.kind, MoveKind::Sideways { base: Some((c0, "lib".into())) });
    assert_eq!(subjects(&m.ahead), ["d1"]);
    assert_eq!(subjects(&m.behind), ["c1", "c2"]);
}

#[test]
fn changes_inside_show_only_where_the_working_tree_is_a_side() {
    let (parent, sub) = with_submodule("gitlink-dirty");
    commit_in(&sub, "c1");
    git(&parent, &["add", PATH]);
    write(&sub, "lib.txt", "edited\n");
    let unstaged = gitlink(&parent, DiffTarget::Unstaged(PATH.into()));
    assert_eq!(unstaged.kind, MoveKind::Same);
    assert!(unstaged.dirty);
    let staged = gitlink(&parent, DiffTarget::Staged(PATH.into()));
    assert_eq!(staged.kind, MoveKind::Forward);
    assert!(!staged.dirty, "neither side of a staged diff is the working tree");
}

#[test]
fn a_commit_the_submodule_lacks_is_unknown() {
    let (parent, _) = with_submodule("gitlink-missing");
    let unknown = "0123456789abcdef0123456789abcdef01234567";
    git(&parent, &["update-index", "--cacheinfo", &format!("160000,{unknown},{PATH}")]);
    let staged = gitlink(&parent, DiffTarget::Staged(PATH.into()));
    assert_eq!(staged.kind, MoveKind::Unknown(Unknown::Missing { old: false, new: true }));
    let unstaged = gitlink(&parent, DiffTarget::Unstaged(PATH.into()));
    assert_eq!(unstaged.kind, MoveKind::Unknown(Unknown::Missing { old: true, new: false }));
}

#[test]
fn a_submodule_not_checked_out_is_unknown_and_the_diff_still_loads() {
    let (parent, _, _) = moved_forward("gitlink-deinit");
    git(&parent, &["submodule", "deinit", "-q", "-f", PATH]);
    let v = load_diff_for_view(&parent, &DiffTarget::Commit(head_of(&parent), PATH.into()), LoadDiffOptions::default()).unwrap();
    assert_eq!(v.gitlink.expect("a gitlink change").kind, MoveKind::Unknown(Unknown::NotCheckedOut));
    assert!(!v.diff.hunks.is_empty(), "the Subproject commit rows still load");
}

#[test]
fn a_submodule_naming_a_network_place_is_never_opened() {
    let parent = scratch_repo("gitlink-refused");
    let folder = parent.join("libs").join("lib");
    std::fs::create_dir_all(&folder).unwrap();
    let (a, b) = (Oid::from_str(&"a".repeat(40)).unwrap(), Oid::from_str(&"b".repeat(40)).unwrap());
    // A .git file naming the network place.
    let named = crate::testutil::git_spelling(&net_place());
    std::fs::write(folder.join(".git"), format!("gitdir: {named}\n")).unwrap();
    assert_eq!(between(&parent, PATH, Some(a), Some(b)).kind, MoveKind::Unknown(Unknown::Refused));
    // A .git that is a link there.
    std::fs::remove_file(folder.join(".git")).unwrap();
    if link_to(&net_place(), &folder.join(".git"), true) {
        assert_eq!(between(&parent, PATH, Some(a), Some(b)).kind, MoveKind::Unknown(Unknown::Refused));
    }
}

#[test]
fn a_long_move_lists_the_newest_commits_oldest_first() {
    let (parent, sub) = with_submodule("gitlink-cap");
    let repo = Repository::open(&sub).unwrap();
    let sig = git2::Signature::now("Test", "test@example.com").unwrap();
    let start = head_of(&sub);
    let mut tip = repo.find_commit(start).unwrap();
    for i in 1..=60 {
        let id = repo.commit(None, &sig, &sig, &format!("c{i}"), &tip.tree().unwrap(), &[&tip]).unwrap();
        tip = repo.find_commit(id).unwrap();
    }
    let m = between(&parent, PATH, Some(start), Some(tip.id()));
    assert_eq!(m.kind, MoveKind::Forward);
    assert_eq!((m.ahead.total, m.ahead.capped, m.ahead.commits.len()), (60, false, GITLINK_LIST_MAX));
    assert_eq!(m.ahead.commits.first().map(|(_, s)| s.as_str()), Some("c11"));
    assert_eq!(m.ahead.commits.last().map(|(_, s)| s.as_str()), Some("c60"));
}

#[test]
fn adding_and_removing_a_submodule() {
    let (parent, _) = with_submodule("gitlink-add-remove");
    let added = gitlink(&parent, DiffTarget::Commit(head_of(&parent), PATH.into()));
    assert_eq!(added.kind, MoveKind::Added);
    assert_eq!(added.old, None);
    git(&parent, &["rm", "-q", PATH]);
    git(&parent, &["commit", "-q", "-m", "remove lib"]);
    let removed = gitlink(&parent, DiffTarget::Commit(head_of(&parent), PATH.into()));
    assert_eq!(removed.kind, MoveKind::Removed);
    assert_eq!(removed.new, None);
    assert!(!removed.type_change);
}

#[test]
fn a_file_replaced_by_a_gitlink_is_a_type_change() {
    let (parent, sub) = with_submodule("gitlink-type-change");
    let c0 = head_of(&sub);
    write(&parent, "libs/x", "a file\n");
    git(&parent, &["add", "libs/x"]);
    git(&parent, &["commit", "-q", "-m", "file"]);
    git(&parent, &["rm", "-q", "--cached", "libs/x"]);
    git(&parent, &["update-index", "--add", "--cacheinfo", &format!("160000,{c0},libs/x")]);
    git(&parent, &["commit", "-q", "-m", "gitlink"]);
    let m = gitlink(&parent, DiffTarget::Commit(head_of(&parent), "libs/x".into()));
    assert!(m.type_change);
    assert_eq!(m.new, Some(c0));
}
