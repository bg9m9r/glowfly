//! Headless tests for where the pull request panel's files and commits come from: the listed
//! head when it is already here, the background fetch of the service's head ref when no source
//! is here (a fork, a merged branch that was deleted), its failure and the button's retry, the
//! cases with no automatic fetch, a fork known from the page, a newly listed head and a parked
//! tab.

use super::super::tests::{drive, git};
use super::super::update;
use super::calls_tests::loaded;
use super::tests::{cleanup, github, open, paths, pr, rev, set_pr, setup};
use crate::effect::{Effect, ForgeReply, ForgeResult};
use crate::msg::{ForgeMsg, Msg, RepoMsg, TabsMsg};
use crate::pull_request::{FilesJob, HeadJob, PrEffect, PullRequestMsg};
use crate::state::pull_request_source::{HeadFetch, OutOfSync, head_fetch_blocked, out_of_sync};
use crate::state::{AppState, ForgeState};
use git2::Oid;
use glowfly_core::auth_failure::AuthNeed;
use glowfly_core::forge::{PrState, Provider, Target};
use glowfly_core::repo::SourceFrom;
use glowfly_core::settings::RepoTrust;
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Instant;

/// Commits g.txt on top of feature in the bare remote, keeps the commit only at
/// `refs/pull/7/head` (as a service does for a fork's pull request) and returns it.
pub(super) fn head_on_remote(bare: &Path) -> Oid {
    move_head(bare, "feature", "g.txt")
}

/// Commits `file` on top of `from` in the bare remote and moves `refs/pull/7/head` to it.
fn move_head(bare: &Path, from: &str, file: &str) -> Oid {
    git(bare, &["branch", "tmp", from]);
    super::super::tests::branch_tests::remote_commit(bare, "tmp", file);
    git(bare, &["update-ref", "refs/pull/7/head", "refs/heads/tmp"]);
    git(bare, &["branch", "-D", "tmp"]);
    git2::Repository::open_bare(bare).unwrap().refname_to_id("refs/pull/7/head").unwrap()
}

fn refs(dir: &Path) -> BTreeSet<String> {
    let repo = git2::Repository::open(dir).unwrap();
    repo.references().unwrap().filter_map(|r| r.ok()?.name().map(str::to_owned)).collect()
}

fn files_jobs(fx: &[Effect]) -> Vec<&FilesJob> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::PullRequest(PrEffect::Files(j)) => Some(j),
            _ => None,
        })
        .collect()
}

fn head_jobs(fx: &[Effect]) -> Vec<&HeadJob> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::PullRequest(PrEffect::FetchHead(j)) => Some(j),
            _ => None,
        })
        .collect()
}

/// Runs `e` and hands its result to `update`, returning the effects that raised.
fn step(st: &mut AppState, e: Effect) -> Vec<Effect> {
    let msg = crate::runner::execute(e).expect("a result");
    update(st, msg, Instant::now())
}

fn take(fx: Vec<Effect>, pick: impl Fn(&Effect) -> bool) -> Effect {
    fx.into_iter().find(|e| pick(e)).expect("the effect")
}

fn is_files(e: &Effect) -> bool {
    matches!(e, Effect::PullRequest(PrEffect::Files(_)))
}

fn is_head(e: &Effect) -> bool {
    matches!(e, Effect::PullRequest(PrEffect::FetchHead(_)))
}

fn view(st: &AppState) -> &crate::state::pull_request::PrView {
    st.session.pull_request.view.as_ref().unwrap()
}

/// A local-only commit on feature, so the listed head is here but origin/feature is behind it.
#[test]
fn a_head_already_here_is_used() {
    let (mut st, work, bare, _) = setup("pr-head-here", &[]);
    let branch = rev(&work, "origin/feature");
    git(&work, &["checkout", "-q", "feature"]);
    std::fs::write(work.join("h.txt"), "local\n").unwrap();
    git(&work, &["add", "h.txt"]);
    git(&work, &["commit", "-q", "-m", "h.txt"]);
    git(&work, &["checkout", "-q", "main"]);
    let head = rev(&work, "feature");
    set_pr(&mut st, |p| p.head = Some(head.to_string()));
    let fx = update(&mut st, Msg::PullRequest(PullRequestMsg::Open(7)), Instant::now());
    let job = files_jobs(&fx)[0];
    assert_eq!((job.branch, job.head.as_deref()), (Some(branch), Some(head.to_string().as_str())));
    let fx = step(&mut st, take(fx, is_files));
    assert!(head_jobs(&fx).is_empty());
    let v = view(&st);
    let shown = v.shown.as_ref().unwrap();
    assert_eq!((shown.source.oid, shown.source.from), (head, SourceFrom::Head));
    assert_eq!(paths(&st), ["f.txt", "h.txt"]);
    let summaries: Vec<&str> = shown.commits.commits.iter().map(|c| c.summary.as_str()).collect();
    assert_eq!(summaries, ["f.txt", "h.txt"]);
    assert_eq!(out_of_sync(v), [OutOfSync::HeadUsed { branch: Some(branch), head, fork: false }]);
    cleanup(&[&work, &bare]);
}

#[test]
fn a_fork_head_is_fetched_then_compared() {
    let (mut st, work, bare, _) = setup("pr-fork-head", &[]);
    let head = head_on_remote(&bare);
    set_pr(&mut st, |p| {
        p.source = "from-a-fork".into();
        p.head = Some(head.to_string());
    });
    let before = refs(&work);
    let fx = update(&mut st, Msg::PullRequest(PullRequestMsg::Open(7)), Instant::now());
    let fx = step(&mut st, take(fx, is_files));
    let job = head_jobs(&fx)[0];
    assert!(!job.prompts);
    assert_eq!(job.head, head.to_string());
    for arg in ["refs/pull/7/head", "--no-tags", "--refmap=", "--recurse-submodules=no", "origin"] {
        assert!(job.cmd.iter().any(|a| a == arg), "{arg}: {:?}", job.cmd);
    }
    assert_eq!(view(&st).head_fetch, HeadFetch::Running);
    assert_eq!(out_of_sync(view(&st)), [OutOfSync::HeadFetching]);
    let fx = step(&mut st, take(fx, is_head));
    assert_eq!(view(&st).head_fetch, HeadFetch::Done);
    let fx = step(&mut st, take(fx, is_files));
    assert!(head_jobs(&fx).is_empty(), "fetched once");
    let v = view(&st);
    let shown = v.shown.as_ref().unwrap();
    assert_eq!((shown.source.oid, shown.source.from), (head, SourceFrom::Head));
    assert_eq!(paths(&st), ["f.txt", "g.txt"]);
    let summaries: Vec<&str> = shown.commits.commits.iter().map(|c| c.summary.as_str()).collect();
    assert_eq!(summaries, ["f.txt", "g.txt"], "oldest first");
    assert_eq!(out_of_sync(v), [OutOfSync::HeadUsed { branch: None, head, fork: false }]);
    assert_eq!(refs(&work), before, "no ref was written");
    assert!(!refs(&work).contains("refs/remotes/origin/from-a-fork"));
    cleanup(&[&work, &bare]);
}

#[test]
fn a_failed_head_fetch_is_noted_and_retried() {
    let (mut st, work, bare, _) = setup("pr-head-fails", &[]);
    let absent = "0123456789abcdef0123456789abcdef01234567";
    set_pr(&mut st, |p| {
        p.source = "from-a-fork".into();
        p.head = Some(absent.into());
    });
    open(&mut st);
    let v = view(&st);
    let HeadFetch::Failed(e) = &v.head_fetch else { panic!("{:?}", v.head_fetch) };
    assert!(!e.is_empty());
    assert!(matches!(&out_of_sync(v)[..], [OutOfSync::HeadFetchFailed(_)]));
    assert!(st.log.iter().any(|l| l.text.starts_with("Couldn't fetch #7's head from origin: ")), "logged");
    // Comparing again doesn't fetch again by itself.
    let fx = update(&mut st, Msg::PullRequest(PullRequestMsg::ReloadFiles), Instant::now());
    let fx = step(&mut st, take(fx, is_files));
    assert!(head_jobs(&fx).is_empty());
    // The button fetches with prompts.
    let fx = update(&mut st, Msg::PullRequest(PullRequestMsg::FetchHead), Instant::now());
    let jobs = head_jobs(&fx);
    assert_eq!(jobs.len(), 1);
    assert!(jobs[0].prompts);
    assert_eq!(view(&st).head_fetch, HeadFetch::Running);
    // Once running, the button is disabled and does nothing.
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(head_fetch_blocked(snap, view(&st), true).as_deref(), Some("Fetching…"));
    assert!(head_jobs(&update(&mut st, Msg::PullRequest(PullRequestMsg::FetchHead), Instant::now())).is_empty());
    // A result for another head is stale.
    let stale = PullRequestMsg::HeadFetched { id: 7, head: "fedcba9".into(), result: Ok(()) };
    assert!(update(&mut st, Msg::PullRequest(stale), Instant::now()).is_empty());
    assert_eq!(view(&st).head_fetch, HeadFetch::Running);
    cleanup(&[&work, &bare]);
}

/// Opens pull request 7 from "from-a-fork" with a head that isn't here, and returns whether a
/// head fetch started.
fn opens_with_fetch(st: &mut AppState) -> bool {
    set_pr(st, |p| {
        p.source = "from-a-fork".into();
        p.head = Some("0123456789abcdef0123456789abcdef01234567".into());
    });
    let fx = update(st, Msg::PullRequest(PullRequestMsg::Open(7)), Instant::now());
    let fx = match fx.into_iter().find(is_files) {
        Some(e) => step(st, e),
        None => Vec::new(),
    };
    !head_jobs(&fx).is_empty()
}

#[test]
fn no_automatic_fetch_without_a_head_ref_a_head_trust_or_sign_in() {
    // A service without head refs.
    let (mut st, work, bare, _) = setup("pr-no-head-fetch", &[]);
    let azure = Target { provider: Provider::AzureDevOps, ..github() };
    let prs = st.session.forge.as_ref().unwrap().prs.clone();
    let mut fs = ForgeState::new(Some(azure), "origin".into(), None);
    fs.prs = prs;
    st.session.forge = Some(fs);
    assert!(!opens_with_fetch(&mut st));
    assert_eq!(out_of_sync(view(&st)), [OutOfSync::SourceMissing]);
    assert!(view(&st).head_ref.is_none());
    drive(&mut st, Msg::PullRequest(PullRequestMsg::Close));
    let mut fs = ForgeState::new(Some(github()), "origin".into(), None);
    fs.prs = st.session.forge.as_ref().unwrap().prs.clone();
    st.session.forge = Some(fs);

    // An unknown head.
    set_pr(&mut st, |p| p.head = None);
    let fx = update(&mut st, Msg::PullRequest(PullRequestMsg::Open(7)), Instant::now());
    assert!(files_jobs(&fx).is_empty() && head_jobs(&fx).is_empty());
    assert_eq!(out_of_sync(view(&st)), [OutOfSync::SourceMissing]);
    drive(&mut st, Msg::PullRequest(PullRequestMsg::Close));

    // A remote paused for sign-in.
    let workdir = st.workdir().unwrap();
    let paused =
        crate::state::sign_in::SignInNeeded { workdir, remote: "origin".into(), host: None, need: AuthNeed::Https, dismissed: false };
    assert!(st.sign_in.pause(paused));
    assert!(!opens_with_fetch(&mut st));
    assert_eq!(out_of_sync(view(&st)), [OutOfSync::SourceMissing]);
    drive(&mut st, Msg::PullRequest(PullRequestMsg::Close));
    st.sign_in = Default::default();

    // An untrusted repository.
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    crate::state::trust::record(&mut st.settings, &real, RepoTrust::NotTrusted);
    assert!(!opens_with_fetch(&mut st));
    let v = view(&st);
    assert_eq!(out_of_sync(v), [OutOfSync::SourceMissing]);
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(head_fetch_blocked(snap, v, st.runs_git_unasked()).as_deref(), Some("Trust this repository to fetch from it."));
    assert!(head_jobs(&update(&mut st, Msg::PullRequest(PullRequestMsg::FetchHead), Instant::now())).is_empty());
    cleanup(&[&work, &bare]);
}

#[test]
fn a_merged_pr_with_its_branch_deleted_fetches_the_head() {
    let (mut st, work, bare, _) = setup("pr-merged-deleted", &[]);
    let head = head_on_remote(&bare);
    git(&work, &["update-ref", "-d", "refs/remotes/origin/feature"]);
    let fs = st.session.forge.take();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    st.session.forge = fs;
    set_pr(&mut st, |p| p.head = Some(head.to_string()));
    open(&mut st);
    loaded(&mut st, |d| d.state = PrState::Merged);
    let v = view(&st);
    assert_eq!(v.head_fetch, HeadFetch::Done);
    assert_eq!(paths(&st), ["f.txt", "g.txt"]);
    assert_eq!(out_of_sync(v), [OutOfSync::HeadUsed { branch: None, head, fork: false }], "not SourceDeleted");
    cleanup(&[&work, &bare]);
}

#[test]
fn a_fork_known_from_the_page_ignores_a_same_named_branch() {
    let (mut st, work, bare, _) = setup("pr-same-repo", &[]);
    open(&mut st);
    assert_eq!(view(&st).shown.as_ref().unwrap().source.from, SourceFrom::Branch);
    let head = rev(&work, "feature");
    // The page says the source is on a fork: origin/feature is another repository's branch.
    let mut d = super::calls_tests::detail(&head.to_string());
    d.same_repo = false;
    let page = glowfly_core::forge::PrPage { detail: d, entries: Vec::new(), threads: Vec::new(), threads_note: None };
    let msg = super::calls_tests::reply_msg(&st, crate::pull_request::PrReply::Loaded(Ok(page)));
    let fx = update(&mut st, Msg::PullRequest(msg), Instant::now());
    let jobs = files_jobs(&fx);
    assert_eq!(jobs.len(), 1, "compared again");
    assert_eq!((jobs[0].branch, jobs[0].head.as_deref()), (None, Some(head.to_string().as_str())));
    step(&mut st, take(fx, is_files));
    let notes = out_of_sync(view(&st));
    assert_eq!(notes, [OutOfSync::HeadUsed { branch: None, head, fork: true }]);
    assert!(notes[0].text(view(&st)).starts_with("This pull request comes from a fork"));
    cleanup(&[&work, &bare]);
}

#[test]
fn a_new_listed_head_and_a_parked_fetch_ask_again() {
    let (mut st, work, bare, _) = setup("pr-head-again", &[]);
    head_on_remote(&bare);
    set_pr(&mut st, |p| {
        p.source = "from-a-fork".into();
        p.head = Some("0123456789abcdef0123456789abcdef01234567".into());
    });
    open(&mut st);
    // The head ref came, but without the listed head (the list was older than the ref).
    assert_eq!(view(&st).head_fetch, HeadFetch::Done);
    let notes = out_of_sync(view(&st));
    assert!(matches!(&notes[..], [OutOfSync::HeadNotSent { .. }]), "{notes:?}");
    assert!(crate::state::pull_request_source::offers_head_fetch(view(&st), &notes));
    // The pull request moved on, and the list now names its new head: the fetch runs again.
    let head = move_head(&bare, "refs/pull/7/head", "k.txt");
    let mut listed = pr(7, Some(head));
    listed.source = "from-a-fork".into();
    let reply = ForgeReply { target: github(), auth: None, results: vec![ForgeResult::Prs(Ok(vec![listed]))] };
    let fx = update(&mut st, Msg::Forge(ForgeMsg::ForgeReplied(reply)), Instant::now());
    assert_eq!(view(&st).head_fetch, HeadFetch::Idle);
    let fx = step(&mut st, take(fx, is_files));
    assert_eq!(head_jobs(&fx).len(), 1);
    assert_eq!(view(&st).head_fetch, HeadFetch::Running);

    // Parked with the fetch running: its result is dropped, so the return asks again.
    update(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
    assert!(st.session.pull_request.view.is_none());
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert_eq!(view(&st).head_fetch, HeadFetch::Idle);
    assert_eq!(files_jobs(&fx).len(), 1, "compared again");
    super::super::tests::finish(&mut st, fx);
    assert_eq!(view(&st).head_fetch, HeadFetch::Done);
    assert_eq!(paths(&st), ["f.txt", "g.txt", "k.txt"]);
    cleanup(&[&work, &bare]);
}
