//! Applying a series as commits with `git am`: the command, a clean series, a series that stops
//! (with and without the three-way fallback), a plain diff, and what a stopped am shows to the
//! rest of Glowfly (the snapshot, the refs fingerprint, the conflict view's labels).

use super::super::{export, gitcli};
use super::*;
use crate::testutil::{git, scratch_repo, write};
use git2::Oid;
use std::path::PathBuf;

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn commit(dir: &Path, msg: &str) -> Oid {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", msg]);
    head(dir)
}

/// What `git <cmd>` printed in `dir`, and whether it worked.
fn run(dir: &Path, cmd: &[String]) -> (bool, String) {
    let out = gitcli::output(dir, cmd, "", &[], None);
    (out.ok, out.combined())
}

/// A base commit and a series "First" (b.txt), "Second" and "Third" (both change a.txt's
/// second line) saved as series.patch next to the repository; the branch is then back at the
/// base with a commit "Mine" that changes that line too, so Second and Third conflict.
fn conflicting(name: &str) -> (PathBuf, PathBuf, Oid) {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "1\n2\n3\n");
    write(&dir, "b.txt", "x\n");
    let base = commit(&dir, "base");
    write(&dir, "b.txt", "x\ny\n");
    let first = commit(&dir, "First");
    write(&dir, "a.txt", "1\ntwo\n3\n");
    let second = commit(&dir, "Second");
    write(&dir, "a.txt", "1\nTWO\n3\n");
    let third = commit(&dir, "Third");
    let patch = dir.with_file_name(format!("{}-series.patch", dir.file_name().unwrap().to_string_lossy()));
    export(&dir, &[first, second, third], &patch).unwrap();
    git(&dir, &["reset", "-q", "--hard", &base.to_string()]);
    write(&dir, "a.txt", "1\ndeux\n3\n");
    let mine = commit(&dir, "Mine");
    (dir, patch, mine)
}

fn cleanup(dir: &Path, patch: &Path) {
    let _ = std::fs::remove_file(patch);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn am_cmd_shape() {
    assert_eq!(am_cmd("/p/x.patch", true), ["am", "--3way", "/p/x.patch"]);
    assert_eq!(am_cmd("/p/x.patch", false), ["am", "--no-3way", "/p/x.patch"]);
    assert_eq!((AM_CONTINUE, AM_SKIP, AM_ABORT), (&["am", "--continue"][..], &["am", "--skip"][..], &["am", "--abort"][..]));
}

#[test]
fn a_clean_series_becomes_commits() {
    let (dir, patch, _) = conflicting("am-clean");
    git(&dir, &["reset", "-q", "--hard", "HEAD~1"]);
    let (ok, out) = run(&dir, &am_cmd(&patch.display().to_string(), true));
    // Second applies on the base; Third on top of it.
    assert!(ok, "{out}");
    let repo = Repository::open(&dir).unwrap();
    let mut walk = repo.revwalk().unwrap();
    walk.push_head().unwrap();
    let subjects: Vec<String> = walk.take(3).map(|o| repo.find_commit(o.unwrap()).unwrap().summary().unwrap().to_owned()).collect();
    assert_eq!(subjects, ["Third", "Second", "First"]);
    assert_eq!(repo.state(), RepositoryState::Clean);
    assert_eq!(progress_of(&repo), None);
    cleanup(&dir, &patch);
}

#[test]
fn a_conflicting_series_stops_with_its_progress() {
    let (dir, patch, mine) = conflicting("am-stop");
    let (ok, out) = run(&dir, &am_cmd(&patch.display().to_string(), true));
    assert!(!ok, "{out}");
    let stop = am_stop(&out);
    assert_eq!(stop.at, Some((2, "Second".to_owned())), "{out}");
    assert!(stop.conflicts && !stop.not_mail, "{out}");
    let repo = Repository::open(&dir).unwrap();
    assert_eq!(repo.state(), RepositoryState::ApplyMailbox);
    assert_eq!(progress_of(&repo), Some(AmProgress { step: 2, total: 3, subject: "Second".into(), conflicted: true }));
    assert_eq!(current_subject(repo.path()).as_deref(), Some("Second"));

    // The snapshot and the conflict view see it.
    let snap = crate::repo::load(&dir, &crate::repo::LoadOptions::all(50)).unwrap();
    assert_eq!(snap.am.as_ref().map(|a| a.step), Some(2));
    let labels = crate::repo::load_conflict(&dir, "a.txt").unwrap().labels;
    assert_eq!((labels.ours.as_str(), labels.theirs.as_str()), ("main", "patch: Second"));

    // Skipping stops at Third without moving HEAD; the refs fingerprint still changes.
    let key = crate::repo::refs_key(&dir).unwrap();
    let before = head(&dir);
    let (ok, out) = run(&dir, &gitcli::args(AM_SKIP));
    assert!(!ok, "Third conflicts too: {out}");
    assert_eq!(head(&dir), before);
    assert_ne!(crate::repo::refs_key(&dir).unwrap(), key);
    let repo = Repository::open(&dir).unwrap();
    assert_eq!(progress_of(&repo).map(|p| (p.step, p.subject)), Some((3, "Third".into())));

    let (ok, out) = run(&dir, &gitcli::args(AM_ABORT));
    assert!(ok, "{out}");
    assert_eq!(head(&dir), mine);
    let snap = crate::repo::load(&dir, &crate::repo::LoadOptions::all(50)).unwrap();
    assert_eq!(snap.am, None);
    cleanup(&dir, &patch);
}

#[test]
fn without_three_way_it_stops_on_the_problem() {
    let (dir, patch, _) = conflicting("am-no-3way");
    let (ok, out) = run(&dir, &am_cmd(&patch.display().to_string(), false));
    assert!(!ok);
    let stop = am_stop(&out);
    assert_eq!(stop.at, Some((2, "Second".to_owned())), "{out}");
    assert!(!stop.conflicts, "{out}");
    assert!(stop.problem.as_deref().is_some_and(|p| p.contains("a.txt")), "{out}");
    let repo = Repository::open(&dir).unwrap();
    assert_eq!(progress_of(&repo).map(|p| p.conflicted), Some(false));
    git(&dir, &["am", "--abort"]);
    cleanup(&dir, &patch);
}

#[test]
fn a_plain_diff_is_not_mail() {
    let dir = scratch_repo("am-plain");
    write(&dir, "a.txt", "one\n");
    commit(&dir, "base");
    write(&dir, "a.txt", "one\ntwo\n");
    let patch = dir.with_file_name(format!("{}-plain.patch", dir.file_name().unwrap().to_string_lossy()));
    let (_, diff) = gitcli::output_bytes(&dir, &gitcli::args(&["diff"]), &[], None);
    std::fs::write(&patch, diff).unwrap();
    git(&dir, &["checkout", "--", "a.txt"]);
    let (ok, out) = run(&dir, &am_cmd(&patch.display().to_string(), true));
    assert!(!ok);
    assert!(am_stop(&out).not_mail, "{out}");
    cleanup(&dir, &patch);
}

#[test]
fn am_stop_parses_output() {
    let out = "Applying: First\nApplying: Second: with colon\nerror: patch failed: a.txt:2\nerror: a.txt: patch does not apply\n\
               Patch failed at 0002 Second: with colon\nhint: Use 'git am --show-current-patch=diff' to see the failed patch";
    let stop = am_stop(out);
    assert_eq!(stop.at, Some((2, "Second: with colon".to_owned())));
    assert_eq!(stop.problem.as_deref(), Some("a.txt has changed where the hunk at line 2 goes"));
    assert!(!stop.conflicts && !stop.not_mail);
    assert_eq!(am_stop(""), AmStop::default());
}
