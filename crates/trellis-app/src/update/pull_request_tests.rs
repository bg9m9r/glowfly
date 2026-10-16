//! Headless tests for the pull request panel: the files from the merge base of the
//! remote-tracking branches, opening them in the panel, notes for missing or moved branches,
//! closing, refreshes of the refs and of the service's list, and tab switches.

use super::branch_tests::{remote_commit, with_origin};
use super::tests::{drive, git};
use super::update;
use crate::effect::{Effect, ForgeReply, ForgeResult};
use crate::msg::{ForgeMsg, HistoryMsg, Msg, RepoMsg, TabsMsg};
use crate::pull_request::{PrEffect, PullRequestMsg};
use crate::remote::RemoteMsg;
use crate::state::pull_request::{Files, OutOfSync, fetch_blocked, out_of_sync};
use crate::state::{AppState, ForgeState};
use git2::Oid;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::forge::{Provider, PullRequest, Target};
use trellis_core::repo::DiffTarget;

pub(super) fn github() -> Target {
    Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() }
}

pub(super) fn rev(dir: &Path, spec: &str) -> Oid {
    git2::Repository::open(dir).unwrap().revparse_single(spec).unwrap().peel_to_commit().unwrap().id()
}

pub(super) fn pr(id: u64, head: Option<Oid>) -> PullRequest {
    PullRequest {
        id,
        title: "Add f".into(),
        description: String::new(),
        source: "feature".into(),
        target: "main".into(),
        author: "ada".into(),
        draft: false,
        reviewers: Vec::new(),
        head: head.map(|h| h.to_string()),
        merge_status: None,
        checks: None,
        url: "https://github.com/o/r/pull/7".into(),
    }
}

/// A repository with origin: f.txt (and `more`) committed on feature, m.txt on main, both
/// pushed, and pull request 7 from feature into main listed by the service. Returns (state,
/// working repository, bare repository, merge base).
pub(super) fn setup(name: &str, more: &[&str]) -> (AppState, PathBuf, PathBuf, Oid) {
    let (mut st, work, bare) = with_origin(name);
    let base = rev(&work, "main");
    git(&work, &["checkout", "-q", "feature"]);
    for f in ["f.txt"].iter().chain(more).copied() {
        std::fs::write(work.join(f), "feature\n").unwrap();
        git(&work, &["add", f]);
        git(&work, &["commit", "-q", "-m", f]);
    }
    git(&work, &["push", "-q", "origin", "feature"]);
    git(&work, &["checkout", "-q", "main"]);
    std::fs::write(work.join("m.txt"), "main\n").unwrap();
    git(&work, &["add", "m.txt"]);
    git(&work, &["commit", "-q", "-m", "m"]);
    git(&work, &["push", "-q", "origin", "main"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    // After the last refresh: detection finds no host for a local-path remote.
    let mut fs = ForgeState::new(Some(github()), "origin".into(), None);
    fs.prs = vec![pr(7, Some(rev(&work, "feature")))];
    st.forge = Some(fs);
    (st, work, bare, base)
}

fn set_pr(st: &mut AppState, f: impl FnOnce(&mut PullRequest)) {
    f(&mut st.forge.as_mut().unwrap().prs[0]);
}

pub(super) fn open(st: &mut AppState) {
    drive(st, Msg::PullRequest(PullRequestMsg::Open(7)));
}

fn paths(st: &AppState) -> Vec<String> {
    match &st.pull_request.as_ref().unwrap().files {
        Files::Loaded(c) => c.comparison.files.iter().map(|f| f.path.clone()).collect(),
        other => panic!("not loaded: {other:?}"),
    }
}

pub(super) fn pr_target(st: &AppState, path: &str) -> DiffTarget {
    st.pull_request.as_ref().unwrap().diff_target(path).unwrap()
}

pub(super) fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn opening_lists_the_files_from_the_merge_base() {
    let (mut st, work, bare, base) = setup("pr-open", &[]);
    let selection = st.selection.clone();
    open(&mut st);
    let v = st.pull_request.as_ref().expect("the panel is open");
    assert_eq!(v.pr.id, 7);
    assert_eq!((v.label.as_str(), v.noun, v.remote.as_str()), ("#7", "pull request", "origin"));
    let Files::Loaded(c) = &v.files else { panic!("{:?}", v.files) };
    assert_eq!(c.base, base);
    assert_eq!(paths(&st), ["f.txt"]);
    assert!(out_of_sync(st.pull_request.as_ref().unwrap()).is_empty());
    assert_eq!(st.selection, selection);
    assert!(st.diff.is_none());
    // Opening it again changes nothing.
    let fx = update(&mut st, Msg::PullRequest(PullRequestMsg::Open(7)), Instant::now());
    assert!(fx.is_empty());
    // An id the service didn't list opens nothing.
    assert!(update(&mut st, Msg::PullRequest(PullRequestMsg::Open(8)), Instant::now()).is_empty());
    assert_eq!(st.pull_request.as_ref().unwrap().pr.id, 7);
    cleanup(&[&work, &bare]);
}

#[test]
fn a_file_opens_inside_the_panel() {
    let (mut st, work, bare, _) = setup("pr-file", &["g.txt"]);
    open(&mut st);
    assert_eq!(paths(&st), ["f.txt", "g.txt"]);
    let f = pr_target(&st, "f.txt");
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(f.clone())));
    assert!(st.pull_request.is_some());
    assert_eq!(st.diff.as_ref().map(|(t, _)| t), Some(&f));
    let g = pr_target(&st, "g.txt");
    assert_eq!(crate::state::diff_nav::next_file(&st, &f, 1), Some(g.clone()));
    assert_eq!(crate::state::diff_nav::next_file(&st, &g, -1), Some(f.clone()));
    assert_eq!(crate::state::diff_nav::next_file(&st, &g, 1), None);
    // The diff's own Close leaves the panel open.
    drive(&mut st, Msg::Repo(RepoMsg::CloseDiff));
    assert!(st.diff.is_none() && st.pull_request.is_some());
    // A working-tree file is not one of its files.
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("a.txt".into()))));
    assert!(st.pull_request.is_none());
    assert!(matches!(&st.diff, Some((DiffTarget::Unstaged(p), _)) if p == "a.txt"));
    cleanup(&[&work, &bare]);
}

#[test]
fn missing_refs_are_noted_and_nothing_loads() {
    let (mut st, work, bare, _) = setup("pr-missing", &[]);
    set_pr(&mut st, |p| p.source = "from-a-fork".into());
    let fx = update(&mut st, Msg::PullRequest(PullRequestMsg::Open(7)), Instant::now());
    assert!(!fx.iter().any(|e| matches!(e, Effect::PullRequest(PrEffect::Files(_)))));
    let v = st.pull_request.as_ref().unwrap();
    assert_eq!(v.files, Files::Missing);
    assert_eq!(out_of_sync(v), [OutOfSync::SourceMissing]);
    drive(&mut st, Msg::PullRequest(PullRequestMsg::Close));

    set_pr(&mut st, |p| {
        p.source = "feature".into();
        p.target = "release".into();
    });
    let fx = update(&mut st, Msg::PullRequest(PullRequestMsg::Open(7)), Instant::now());
    assert!(!fx.iter().any(|e| matches!(e, Effect::PullRequest(PrEffect::Files(_)))));
    assert_eq!(out_of_sync(st.pull_request.as_ref().unwrap()), [OutOfSync::TargetMissing]);
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(fetch_blocked(snap, "origin", true), None);
    assert_eq!(fetch_blocked(snap, "origin", false).as_deref(), Some("Another operation is still running."));
    assert_eq!(fetch_blocked(snap, "gone", true).as_deref(), Some("gone is no longer a remote"));
    cleanup(&[&work, &bare]);
}

#[test]
fn a_moved_head_is_noted_until_fetched() {
    let (mut st, work, bare, _) = setup("pr-moved", &[]);
    remote_commit(&bare, "feature", "g.txt");
    let head = git2::Repository::open_bare(&bare).unwrap().refname_to_id("refs/heads/feature").unwrap();
    set_pr(&mut st, |p| p.head = Some(head.to_string()));
    open(&mut st);
    let v = st.pull_request.as_ref().unwrap();
    assert!(matches!(&out_of_sync(v)[..], [OutOfSync::SourceDiffers { head: h, .. }] if *h == head.to_string()));
    assert_eq!(paths(&st), ["f.txt"], "listed from the local tip");
    let f = pr_target(&st, "f.txt");
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(f)));
    assert!(st.diff.is_some());

    drive(&mut st, Msg::Remote(RemoteMsg::Fetch("origin".into())));
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.forge.is_none(), "detection found no host for the local-path remote");
    let v = st.pull_request.as_ref().expect("the panel keeps its copy");
    assert_eq!(v.tips.source, Some(head));
    assert!(out_of_sync(v).is_empty());
    assert_eq!(paths(&st), ["f.txt", "g.txt"]);
    assert!(st.diff.is_none(), "the old list's diff closed");
    cleanup(&[&work, &bare]);
}

#[test]
fn close_returns_and_drops_stale_results() {
    let (mut st, work, bare, _) = setup("pr-close", &[]);
    open(&mut st);
    let f = pr_target(&st, "f.txt");
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(f)));
    assert!(st.diff.is_some());
    let old = st.pull_request_gen;
    let tips = st.pull_request.as_ref().unwrap().tips.pair().unwrap();
    drive(&mut st, Msg::PullRequest(PullRequestMsg::Close));
    assert!(st.pull_request.is_none() && st.diff.is_none());

    open(&mut st);
    let stale = PullRequestMsg::FilesLoaded { id: 7, tips, generation: old, result: Ok(None) };
    update(&mut st, Msg::PullRequest(stale), Instant::now());
    assert!(matches!(st.pull_request.as_ref().unwrap().files, Files::Loaded(_)), "the stale result changed nothing");
    // A result for other tips is stale too.
    let other = PullRequestMsg::FilesLoaded { id: 7, tips: (tips.1, tips.0), generation: st.pull_request_gen, result: Ok(None) };
    update(&mut st, Msg::PullRequest(other), Instant::now());
    assert!(matches!(st.pull_request.as_ref().unwrap().files, Files::Loaded(_)));

    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(head)));
    assert!(st.pull_request.is_none());
    open(&mut st);
    drive(&mut st, Msg::History(HistoryMsg::OpenFileHistory { path: "a.txt".into(), at: None }));
    assert!(st.pull_request.is_none());
    cleanup(&[&work, &bare]);
}

#[test]
fn refresh_updates_the_copy() {
    let (mut st, work, bare, _) = setup("pr-refresh", &[]);
    open(&mut st);
    let mut renamed = st.forge.as_ref().unwrap().prs[0].clone();
    renamed.title = "Add f, renamed".into();
    let reply = |prs| ForgeReply { target: github(), auth: None, results: vec![ForgeResult::Prs(Ok(prs))] };
    update(&mut st, Msg::Forge(ForgeMsg::ForgeReplied(reply(vec![renamed]))), Instant::now());
    let v = st.pull_request.as_ref().unwrap();
    assert_eq!(v.pr.title, "Add f, renamed");
    assert!(v.listed);
    update(&mut st, Msg::Forge(ForgeMsg::ForgeReplied(reply(Vec::new()))), Instant::now());
    let v = st.pull_request.as_ref().expect("still open");
    assert!(!v.listed);
    assert_eq!(v.pr.title, "Add f, renamed", "the last copy");
    cleanup(&[&work, &bare]);
}

#[test]
fn parked_tab_reloads_a_pending_list() {
    let (mut st, work, bare, _) = setup("pr-park", &[]);
    let fx = update(&mut st, Msg::PullRequest(PullRequestMsg::Open(7)), Instant::now());
    let Some(Effect::PullRequest(PrEffect::Files(job))) = fx.into_iter().find(|e| matches!(e, Effect::PullRequest(PrEffect::Files(_))))
    else {
        panic!("no request")
    };
    assert_eq!(st.pull_request.as_ref().unwrap().files, Files::Loading);

    // A result for another repository's tab is dropped.
    let loaded = |generation| PullRequestMsg::FilesLoaded { id: 7, tips: job.tips, generation, result: Ok(None) };
    assert!(loaded(job.generation).on_inactive().is_none());
    let scoped = Msg::Scoped { workdir: PathBuf::from("C:/elsewhere"), msg: Box::new(Msg::PullRequest(loaded(job.generation))) };
    update(&mut st, scoped, Instant::now());
    assert_eq!(st.pull_request.as_ref().unwrap().files, Files::Loading);

    let before = st.pull_request_gen;
    update(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
    assert!(st.pull_request.is_none(), "parked with its tab");
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert!(st.pull_request.is_some(), "restored");
    assert!(st.pull_request_gen > before);
    let again = fx.iter().find_map(|e| match e {
        Effect::PullRequest(PrEffect::Files(j)) => Some(j),
        _ => None,
    });
    let again = again.expect("asked for again");
    assert_eq!((again.id, again.tips, again.generation), (7, job.tips, st.pull_request_gen));
    // The first request's result is stale now.
    update(&mut st, Msg::PullRequest(loaded(job.generation)), Instant::now());
    assert_eq!(st.pull_request.as_ref().unwrap().files, Files::Loading);
    cleanup(&[&work, &bare]);
}
