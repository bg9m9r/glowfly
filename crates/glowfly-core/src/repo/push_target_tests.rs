//! `push_target` on real repositories: main pushed to a bare `origin` (its upstream) and to a
//! second bare `fork` next to it. Every test sets push.default and blanks remote.pushDefault in
//! the repository's own config, so the developer's global config can't decide them.

use super::*;
use crate::repo::{LoadOptions, load};
use crate::testutil::*;
use std::path::{Path, PathBuf};

/// `with_origin(name)` plus a bare `fork` that has main, fetched, and push.default `mode`.
/// Returns (work, origin, fork).
fn fixture(name: &str, mode: &str) -> (PathBuf, PathBuf, PathBuf) {
    let (work, origin) = with_origin(name);
    let fork = work.with_file_name(format!("{}-fork.git", work.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&fork);
    std::fs::create_dir_all(&fork).unwrap();
    git(&fork, &["init", "-q", "--bare", "-b", "main"]);
    git(&work, &["remote", "add", "fork", &fork.display().to_string()]);
    git(&work, &["push", "-q", "fork", "main"]);
    set_config(&work, &[("push.default", mode), ("remote.pushDefault", "")]);
    (work, origin, fork)
}

/// A branch `name` at main tracking origin/`merge`.
fn tracking_branch(work: &Path, name: &str, merge: &str) {
    git(work, &["branch", name]);
    set_config(work, &[(&format!("branch.{name}.remote"), "origin"), (&format!("branch.{name}.merge"), &format!("refs/heads/{merge}"))]);
}

fn commit(work: &Path, file: &str) {
    write(work, file, file);
    git(work, &["add", "."]);
    git(work, &["commit", "-q", "-m", file]);
}

/// `branch`'s push target, as a load fills it.
fn target(work: &Path, branch: &str) -> Option<PushTarget> {
    let snap = load(work, &LoadOptions::all(100)).unwrap();
    snap.locals.iter().find(|b| b.name == branch).unwrap().push.clone()
}

fn dest(work: &Path, branch: &str) -> (String, PushDest) {
    let t = target(work, branch).unwrap();
    let d = t.dest.clone().unwrap_or_else(|r| panic!("{branch}: refused, {r:?}"));
    (t.remote, d)
}

fn refusal(work: &Path, branch: &str) -> PushRefusal {
    target(work, branch).unwrap().dest.unwrap_err()
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn simple_without_push_remote_goes_to_the_upstream() {
    let (work, origin, fork) = fixture("pt-simple", "simple");
    commit(&work, "two.txt");
    let t = target(&work, "main").unwrap();
    assert_eq!((t.remote.as_str(), t.key.as_deref(), &t.missing), ("origin", None, &None));
    let d = t.dest.clone().unwrap();
    assert_eq!((d.dest_ref.as_str(), d.tracking_ref.as_deref()), ("refs/heads/main", Some("refs/remotes/origin/main")));
    assert_eq!((d.ahead, d.behind, d.rule), (1, 0, PushRule::Default), "the upstream's own counts");
    assert_eq!(t.label(&d), "origin/main");
    cleanup(&[&work, &origin, &fork]);
}

#[test]
fn simple_with_a_push_remote_goes_to_the_same_name_there() {
    let (work, origin, fork) = fixture("pt-simple-fork", "simple");
    set_config(&work, &[("remote.pushDefault", "fork")]);
    let t = target(&work, "main").unwrap();
    assert_eq!((t.remote.as_str(), t.key.as_deref()), ("fork", Some("remote.pushDefault")));
    let d = t.dest.clone().unwrap();
    assert_eq!((d.dest_ref.as_str(), d.tracking_ref.as_deref()), ("refs/heads/main", Some("refs/remotes/fork/main")));
    assert_eq!(t.label(&d), "fork/main");
    set_config(&work, &[("branch.main.pushRemote", "origin")]);
    let t = target(&work, "main").unwrap();
    assert_eq!((t.remote.as_str(), t.key.as_deref()), ("origin", Some("branch.main.pushRemote")), "the branch's own setting wins");
    cleanup(&[&work, &origin, &fork]);
}

#[test]
fn simple_refuses_an_upstream_of_another_name() {
    let (work, origin, fork) = fixture("pt-simple-name", "simple");
    tracking_branch(&work, "feat", "main");
    assert_eq!(refusal(&work, "feat"), PushRefusal::NameDiffers { merge: "main".into() });
    set_config(&work, &[("branch.feat.pushRemote", "fork")]);
    let (remote, d) = dest(&work, "feat");
    assert_eq!((remote.as_str(), d.dest_ref.as_str(), d.tip), ("fork", "refs/heads/feat", None), "triangular: the same name");
    cleanup(&[&work, &origin, &fork]);
}

#[test]
fn current_pushes_the_same_name_with_and_without_a_push_remote() {
    let (work, origin, fork) = fixture("pt-current", "current");
    tracking_branch(&work, "feat", "main");
    let (remote, d) = dest(&work, "feat");
    assert_eq!((remote.as_str(), d.dest_ref.as_str()), ("origin", "refs/heads/feat"));
    set_config(&work, &[("remote.pushDefault", "fork")]);
    let (remote, d) = dest(&work, "feat");
    assert_eq!((remote.as_str(), d.dest_ref.as_str()), ("fork", "refs/heads/feat"));
    cleanup(&[&work, &origin, &fork]);
}

#[test]
fn upstream_pushes_to_the_upstream_and_refuses_another_remote() {
    let (work, origin, fork) = fixture("pt-upstream", "upstream");
    tracking_branch(&work, "feat", "main");
    for mode in ["upstream", "tracking"] {
        set_config(&work, &[("push.default", mode), ("remote.pushDefault", "")]);
        let (remote, d) = dest(&work, "main");
        assert_eq!((remote.as_str(), d.dest_ref.as_str()), ("origin", "refs/heads/main"), "{mode}");
        let (remote, d) = dest(&work, "feat");
        assert_eq!((remote.as_str(), d.dest_ref.as_str()), ("origin", "refs/heads/main"), "{mode}: to the upstream's name");
        set_config(&work, &[("remote.pushDefault", "fork")]);
        let t = target(&work, "main").unwrap();
        assert_eq!(t.remote, "fork");
        assert_eq!(t.dest, Err(PushRefusal::NotUpstreamRemote { upstream_remote: "origin".into() }), "{mode}");
        assert_eq!(t.explicit_cmd("main"), None);
    }
    cleanup(&[&work, &origin, &fork]);
}

#[test]
fn nothing_refuses_with_and_without_a_push_remote() {
    let (work, origin, fork) = fixture("pt-nothing", "nothing");
    assert_eq!(refusal(&work, "main"), PushRefusal::Nothing);
    set_config(&work, &[("remote.pushDefault", "fork")]);
    assert_eq!(refusal(&work, "main"), PushRefusal::Nothing);
    cleanup(&[&work, &origin, &fork]);
}

#[test]
fn invalid_push_default_refuses() {
    let (work, origin, fork) = fixture("pt-invalid", "bogus");
    assert_eq!(refusal(&work, "main"), PushRefusal::Invalid("bogus".into()));
    cleanup(&[&work, &origin, &fork]);
}

#[test]
fn matching_needs_the_branch_on_the_remote() {
    let (work, origin, fork) = fixture("pt-matching", "matching");
    tracking_branch(&work, "feat", "main");
    let (remote, d) = dest(&work, "main");
    assert_eq!((remote.as_str(), d.rule), ("origin", PushRule::Matching));
    assert_eq!(refusal(&work, "feat"), PushRefusal::NotOnRemote, "origin has no feat");
    set_config(&work, &[("remote.pushDefault", "fork")]);
    let (remote, d) = dest(&work, "main");
    assert_eq!((remote.as_str(), d.dest_ref.as_str(), d.rule), ("fork", "refs/heads/main", PushRule::Matching));
    assert_eq!(refusal(&work, "feat"), PushRefusal::NotOnRemote, "fork has no feat");
    cleanup(&[&work, &origin, &fork]);
}

#[test]
fn a_missing_push_remote_falls_back_to_the_upstream() {
    let (work, origin, fork) = fixture("pt-missing", "simple");
    set_config(&work, &[("branch.main.pushRemote", "gone")]);
    let t = target(&work, "main").unwrap();
    assert_eq!((t.remote.as_str(), t.key.as_deref()), ("origin", None));
    assert_eq!(t.missing, Some(("branch.main.pushRemote".into(), "gone".into())));
    assert_eq!(t.dest.as_ref().unwrap().dest_ref, "refs/heads/main");
    assert_eq!(t.explicit_cmd("main").unwrap(), ["push", "--", "origin", "refs/heads/main:refs/heads/main"]);
    set_config(&work, &[("push.default", "upstream")]);
    assert!(target(&work, "main").unwrap().dest.is_ok(), "now the upstream's remote, so upstream accepts it");
    set_config(&work, &[("branch.main.pushRemote", ""), ("remote.pushDefault", "gone")]);
    let t = target(&work, "main").unwrap();
    assert_eq!((t.remote.as_str(), t.missing), ("origin", Some(("remote.pushDefault".into(), "gone".into()))));
    cleanup(&[&work, &origin, &fork]);
}

#[test]
fn a_url_push_remote_is_used_as_git_uses_it() {
    let (work, origin, fork) = fixture("pt-url", "simple");
    let url = fork.display().to_string();
    set_config(&work, &[("branch.main.pushRemote", &url)]);
    let t = target(&work, "main").unwrap();
    assert_eq!((t.remote.as_str(), &t.missing), (url.as_str(), &None));
    let d = t.dest.clone().unwrap();
    assert_eq!((d.dest_ref.as_str(), d.tracking_ref.as_deref(), d.tip), ("refs/heads/main", None, None));
    assert_eq!(t.label(&d), url);
    cleanup(&[&work, &origin, &fork]);
}

#[test]
fn remote_push_refspecs_decide_before_push_default() {
    let (work, origin, fork) = fixture("pt-refspecs", "nothing");
    tracking_branch(&work, "feat", "main");
    set_config(&work, &[("remote.pushDefault", "fork"), ("remote.fork.push", "refs/heads/main:refs/heads/review")]);
    let (remote, d) = dest(&work, "main");
    assert_eq!((remote.as_str(), d.dest_ref.as_str(), d.rule), ("fork", "refs/heads/review", PushRule::Refspec));
    assert_eq!(d.tracking_ref.as_deref(), Some("refs/remotes/fork/review"));
    assert_eq!(refusal(&work, "feat"), PushRefusal::NotInRefspecs);
    set_config(&work, &[("remote.fork.push", "HEAD:refs/for/main")]);
    let t = target(&work, "main").unwrap();
    let d = t.dest.clone().unwrap();
    assert_eq!((d.dest_ref.as_str(), t.label(&d)), ("refs/for/main", "fork refs/for/main".to_owned()));
    assert_eq!(refusal(&work, "feat"), PushRefusal::NotInRefspecs, "HEAD is only the checked-out branch");
    cleanup(&[&work, &origin, &fork]);
}

#[test]
fn a_bare_head_push_spec_keeps_the_branch_name() {
    let (work, origin, fork) = fixture("pt-bare-head", "nothing");
    for spec in ["HEAD", "@", "+HEAD"] {
        set_config(&work, &[("remote.origin.push", spec)]);
        let (remote, d) = dest(&work, "main");
        assert_eq!((remote.as_str(), d.dest_ref.as_str(), d.rule), ("origin", "refs/heads/main", PushRule::Refspec), "{spec}");
        assert_eq!(d.tracking_ref.as_deref(), Some("refs/remotes/origin/main"), "{spec}");
    }
    cleanup(&[&work, &origin, &fork]);
}

#[test]
fn a_mirror_remote_pushes_the_same_ref() {
    let (work, origin, fork) = fixture("pt-mirror", "nothing");
    set_config(&work, &[("remote.pushDefault", "fork"), ("remote.fork.mirror", "true")]);
    let (remote, d) = dest(&work, "main");
    assert_eq!((remote.as_str(), d.dest_ref.as_str(), d.rule), ("fork", "refs/heads/main", PushRule::Mirror));
    cleanup(&[&work, &origin, &fork]);
}

#[test]
fn counts_are_against_the_push_targets_tracking_ref() {
    let (work, origin, fork) = fixture("pt-counts", "simple");
    commit(&work, "two.txt");
    commit(&work, "three.txt");
    git(&work, &["push", "-q", "origin", "main"]);
    tracking_branch(&work, "feat", "main");
    set_config(&work, &[("remote.pushDefault", "fork")]);
    let snap = load(&work, &LoadOptions::all(100)).unwrap();
    let main = snap.locals.iter().find(|b| b.name == "main").unwrap();
    assert_eq!((main.ahead, main.behind), (0, 0), "level with origin/main");
    let d = main.push.as_ref().unwrap().dest.clone().unwrap();
    assert!(d.tip.is_some());
    assert_eq!((d.ahead, d.behind), (2, 0), "fork/main is two commits older");
    let feat = snap.locals.iter().find(|b| b.name == "feat").unwrap();
    let d = feat.push.as_ref().unwrap().dest.clone().unwrap();
    assert_eq!((d.tip, d.ahead, d.behind), (None, 0, 0), "fork has no feat");
    cleanup(&[&work, &origin, &fork]);
}

#[test]
fn load_fills_branch_push() {
    let (work, origin, fork) = fixture("pt-load", "simple");
    git(&work, &["branch", "solo"]);
    let snap = load(&work, &LoadOptions::all(100)).unwrap();
    let head = snap.locals.iter().find(|b| b.is_head).unwrap();
    assert!(head.push.is_some());
    assert_eq!(snap.locals.iter().find(|b| b.name == "solo").unwrap().push, None, "no upstream");
    let repo = Repository::open(&work).unwrap();
    assert_eq!(resolve(&repo, &snap.push_remotes, &["origin", "fork"], head).map(|p| p.remote), Some("origin".into()));
    cleanup(&[&work, &origin, &fork]);
}
