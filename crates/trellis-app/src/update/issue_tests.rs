//! Headless tests for the issue panel: opening it from a listed issue (and what that closes),
//! loading its details, stale and misplaced replies, reloads, closing, what closes it, tracker
//! changes and tab switches. Replies are fed by hand; the calls themselves never run here.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, ForgeError, IssueCall, IssueEffect, IssueJob, IssueReply};
use crate::msg::{BlameMsg, ForgeMsg, IssueMsg, Msg, RepoMsg, TabsMsg};
use crate::pull_request::PullRequestMsg;
use crate::state::history::FileHistoryView;
use crate::state::issue::Details;
use crate::state::pull_request::PrView;
use crate::state::{AppState, Status};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use trellis_core::forge::{Account, Issue, IssueDetails, IssueFacets, Provider, PullRequest, Target};
use trellis_core::repo::{DiffTarget, FileDiff};
use trellis_core::settings::{RepoTrust, TrackerChoice};

pub(super) fn github() -> Target {
    Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() }
}

pub(super) fn listed(key: &str) -> Issue {
    Issue {
        key: key.into(),
        title: "Crash on start".into(),
        state: "open".into(),
        kind: "bug".into(),
        assignee: Some("ada".into()),
        url: format!("https://github.com/o/r/issues/{key}"),
        facets: IssueFacets { labels: vec!["bug".into()], milestone: None },
    }
}

/// A repository whose origin is on GitHub (its issues the tracker), opened, with issue 12
/// assigned to the user.
pub(super) fn open_repo(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["remote", "add", "origin", "https://github.com/o/r"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let fs = st.forge.as_mut().expect("the service is detected");
    fs.host_status = Status::Ok;
    fs.tracker_status = Status::Ok;
    fs.loading = 0;
    fs.my_issues = vec![listed("12")];
    (st, dir)
}

pub(super) fn run(st: &mut AppState, m: IssueMsg) -> Vec<Effect> {
    update(st, Msg::Issue(m), Instant::now())
}

/// The tracker calls among `fx`.
pub(super) fn jobs(fx: &[Effect]) -> Vec<&IssueJob> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Issue(IssueEffect::Call(j)) => Some(&**j),
            _ => None,
        })
        .collect()
}

fn loads(fx: &[Effect]) -> Vec<&IssueJob> {
    jobs(fx).into_iter().filter(|j| matches!(j.call, IssueCall::Load(_))).collect()
}

fn details(key: &str) -> IssueDetails {
    IssueDetails {
        issue: Issue { title: "Crash on start (fresh)".into(), ..listed(key) },
        body: "It crashes.".into(),
        author: "grace".into(),
        created: 1_767_348_000,
        assignees: vec!["ada".into()],
        priority: None,
        epic: None,
        comments: Vec::new(),
    }
}

/// A reply from the active repository's tracker to the latest request.
pub(super) fn reply(st: &AppState, reply: IssueReply) -> IssueMsg {
    let target = st.forge.as_ref().and_then(|f| f.tracker.clone()).unwrap_or_else(github);
    IssueMsg::Replied { workdir: st.workdir().unwrap(), target, generation: st.issue_gen, reply: Box::new(reply) }
}

fn loaded_reply(st: &AppState, key: &str, result: Result<IssueDetails, ForgeError>) -> IssueMsg {
    reply(st, IssueReply::Loaded { key: key.into(), result })
}

pub(super) fn err(message: &str) -> ForgeError {
    ForgeError { message: message.into(), auth: false }
}

fn view(st: &AppState) -> &crate::state::issue::IssueView {
    st.issue.as_ref().expect("the panel is open")
}

pub(super) fn done(st: AppState, dir: PathBuf) {
    drop(st);
    let _ = std::fs::remove_dir_all(dir);
}

fn pr(id: u64) -> PullRequest {
    PullRequest {
        id,
        title: "PR".into(),
        description: String::new(),
        source: "feature".into(),
        target: "main".into(),
        author: "ada".into(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        merge_status: None,
        checks: None,
        url: String::new(),
        facets: Default::default(),
    }
}

#[test]
fn opening_a_listed_issue_loads_it_and_closes_the_other_views() {
    let (mut st, dir) = open_repo("issue-open");
    let diff = FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
    st.diff = Some((DiffTarget::Unstaged("a.txt".into()), Arc::new(diff)));
    st.history = Some(FileHistoryView::loading("a.txt".into(), None));
    st.blame = Some(crate::state::blame::BlameView::loading("a.txt".into(), None));
    st.pull_request = Some(PrView::new(pr(7), st.forge.as_ref().unwrap(), st.snap.as_ref().unwrap()));
    let fx = run(&mut st, IssueMsg::Open("12".into()));
    let l = loads(&fx);
    assert_eq!(l.len(), 1);
    assert!(matches!(&l[0].call, IssueCall::Load(k) if k == "12"));
    assert_eq!((&l[0].req.target, l[0].generation), (&github(), st.issue_gen));
    assert_eq!(l[0].req.workdir, st.workdir().unwrap());
    let v = view(&st);
    assert_eq!((v.key.as_str(), v.mention.as_str(), v.issue.title.as_str()), ("12", "#12", "Crash on start"));
    assert_eq!(v.details, Details::Loading);
    assert!(st.diff.is_none() && st.history.is_none() && st.blame.is_none() && st.pull_request.is_none());
    assert!(run(&mut st, IssueMsg::Open("12".into())).is_empty(), "the same issue again changes nothing");
    // A key nobody listed opens with its page only.
    run(&mut st, IssueMsg::Open("99".into()));
    assert_eq!((view(&st).issue.title.as_str(), view(&st).issue.url.as_str()), ("", "https://github.com/o/r/issues/99"));
    done(st, dir);
}

#[test]
fn trackers_without_details_held_and_unconnected() {
    let (mut st, dir) = open_repo("issue-unsupported");
    let bb = Target { provider: Provider::Bitbucket, host: "bitbucket.org".into(), ..github() };
    st.forge.as_mut().unwrap().tracker = Some(bb.clone());
    let fx = run(&mut st, IssueMsg::Open("12".into()));
    assert_eq!(jobs(&fx).iter().map(|j| &j.req.target).collect::<Vec<_>>(), [&bb], "Bitbucket Cloud issues load");
    assert_eq!(view(&st).details, Details::Loading);
    run(&mut st, IssueMsg::Close);
    let dc = Target { provider: Provider::BitbucketDataCenter, host: "bitbucket.acme.io".into(), ..github() };
    st.forge.as_mut().unwrap().tracker = Some(dc);
    assert!(jobs(&run(&mut st, IssueMsg::Open("12".into()))).is_empty());
    assert_eq!(view(&st).details, Details::Unsupported);
    assert!(run(&mut st, IssueMsg::Reload).is_empty());
    run(&mut st, IssueMsg::Close);

    st.forge.as_mut().unwrap().tracker = Some(github());
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    crate::state::trust::record(&mut st.settings, &real, RepoTrust::NotTrusted);
    assert!(jobs(&run(&mut st, IssueMsg::Open("12".into()))).is_empty());
    assert_eq!(view(&st).details, Details::Held);
    run(&mut st, IssueMsg::Close);
    crate::state::trust::record(&mut st.settings, &real, RepoTrust::Trusted);

    let gl = Target { provider: Provider::GitLabSelfManaged, host: "gitlab.acme.io".into(), ..github() };
    st.forge.as_mut().unwrap().tracker = Some(gl);
    assert!(jobs(&run(&mut st, IssueMsg::Open("12".into()))).is_empty());
    assert!(matches!(&view(&st).details, Details::Failed(e) if e.starts_with("not connected")), "{:?}", view(&st).details);
    done(st, dir);
}

#[test]
fn loaded_details_fill_the_panel_and_the_cache() {
    let (mut st, dir) = open_repo("issue-loaded");
    run(&mut st, IssueMsg::Open("12".into()));
    let msg = loaded_reply(&st, "12", Ok(details("12")));
    run(&mut st, msg);
    let v = view(&st);
    assert!(matches!(&v.details, Details::Loaded(d) if d.author == "grace"));
    assert_eq!(v.issue.title, "Crash on start (fresh)");
    assert!(!v.reloading);
    assert_eq!(st.forge.as_ref().unwrap().cache["12"].title, "Crash on start (fresh)");

    // A reload keeps the details; a failure only logs.
    let fx = run(&mut st, IssueMsg::Reload);
    assert_eq!(loads(&fx).len(), 1);
    assert!(view(&st).reloading);
    let msg = loaded_reply(&st, "12", Err(err("timed out")));
    run(&mut st, msg);
    assert!(view(&st).loaded().is_some() && !view(&st).reloading);
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text.contains("timed out")));
    done(st, dir);
}

#[test]
fn a_first_load_failure_shows_and_try_again_asks_again() {
    let (mut st, dir) = open_repo("issue-failed");
    run(&mut st, IssueMsg::Open("12".into()));
    let msg = loaded_reply(&st, "12", Err(err("not found: #12 is a pull request")));
    run(&mut st, msg);
    assert!(matches!(&view(&st).details, Details::Failed(e) if e.contains("pull request")));
    assert!(st.log.last().is_some_and(|l| !l.ok));
    assert_eq!(loads(&run(&mut st, IssueMsg::Reload)).len(), 1);
    assert_eq!(view(&st).details, Details::Loading);
    done(st, dir);
}

#[test]
fn stale_misplaced_and_late_replies_are_dropped() {
    let (mut st, dir) = open_repo("issue-stale");
    run(&mut st, IssueMsg::Open("12".into()));
    let mut stale = loaded_reply(&st, "12", Ok(details("12")));
    if let IssueMsg::Replied { generation, .. } = &mut stale {
        *generation -= 1;
    }
    run(&mut st, stale);
    assert_eq!(view(&st).details, Details::Loading, "an older request");
    let mut elsewhere = loaded_reply(&st, "12", Ok(details("12")));
    if let IssueMsg::Replied { workdir, .. } = &mut elsewhere {
        *workdir = trellis_core::paths::temp_dir().join("elsewhere");
    }
    run(&mut st, elsewhere);
    assert_eq!(view(&st).details, Details::Loading, "another repository");
    let other_key = loaded_reply(&st, "13", Ok(details("13")));
    run(&mut st, other_key);
    assert_eq!(view(&st).details, Details::Loading, "another issue");
    let mut other = loaded_reply(&st, "12", Ok(details("12")));
    if let IssueMsg::Replied { target, .. } = &mut other {
        *target = Target { repo: "other".into(), ..github() };
    }
    run(&mut st, other);
    assert_eq!(view(&st).details, Details::Loading, "another tracker");
    let late = loaded_reply(&st, "12", Ok(details("12")));
    run(&mut st, IssueMsg::Close);
    run(&mut st, late);
    assert!(st.issue.is_none(), "a closed panel stays closed");
    assert!(st.log.is_empty());
    done(st, dir);
}

#[test]
fn a_commit_a_diff_and_the_pull_request_panel_close_it() {
    let (mut st, dir) = open_repo("issue-closers");
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    run(&mut st, IssueMsg::Open("12".into()));
    let before = st.issue_gen;
    update(&mut st, Msg::Repo(RepoMsg::SelectCommit(head)), Instant::now());
    assert!(st.issue.is_none() && st.issue_gen > before, "selecting a commit");
    run(&mut st, IssueMsg::Open("12".into()));
    update(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("a.txt".into()))), Instant::now());
    assert!(st.issue.is_none(), "opening a file's diff");
    run(&mut st, IssueMsg::Open("12".into()));
    st.forge.as_mut().unwrap().prs = vec![pr(7)];
    update(&mut st, Msg::PullRequest(PullRequestMsg::Open(7)), Instant::now());
    assert!(st.issue.is_none() && st.pull_request.is_some(), "opening the pull request panel");
    run(&mut st, IssueMsg::Open("12".into()));
    assert!(st.pull_request.is_none(), "and the other way round");
    done(st, dir);
}

#[test]
fn changing_the_tracker_closes_it() {
    let (mut st, dir) = open_repo("issue-tracker");
    run(&mut st, IssueMsg::Open("12".into()));
    let jira = Account { provider: Provider::JiraCloud, host: "acme.atlassian.net".into(), org: String::new() };
    let choice = TrackerChoice::Jira { account: jira, project: "PROJ".into() };
    update(&mut st, Msg::Forge(ForgeMsg::SetTracker(choice)), Instant::now());
    assert!(st.issue.is_none());
    assert!(st.forge.as_ref().unwrap().tracker.as_ref().is_some_and(|t| t.provider == Provider::JiraCloud));
    done(st, dir);
}

#[test]
fn switching_tabs_parks_the_panel_and_reloads_on_return() {
    let (mut st, dir) = open_repo("issue-tabs");
    run(&mut st, IssueMsg::Open("12".into()));
    let msg = loaded_reply(&st, "12", Ok(details("12")));
    run(&mut st, msg);
    let before = st.issue_gen;
    update(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
    assert!(st.issue.is_none(), "parked with its tab");
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert!(view(&st).loaded().is_some(), "the last details stay on screen");
    let l = loads(&fx);
    assert_eq!(l.len(), 1, "and load again");
    assert!(l[0].generation > before && l[0].generation == st.issue_gen);
    assert!(view(&st).reloading);
    done(st, dir);
}

#[test]
fn blame_and_another_tracker_close_it() {
    let (mut st, dir) = open_repo("issue-blame-remote");
    run(&mut st, IssueMsg::Open("12".into()));
    update(&mut st, Msg::Blame(BlameMsg::Open { path: "a.txt".into(), at: None }), Instant::now());
    assert!(st.issue.is_none(), "opening blame");

    // The same key on another tracker is another issue.
    run(&mut st, IssueMsg::Open("12".into()));
    let r2 = Target { repo: "r2".into(), ..github() };
    st.forge.as_mut().unwrap().tracker = Some(r2.clone());
    let fx = run(&mut st, IssueMsg::Open("12".into()));
    assert_eq!(view(&st).target, r2);
    assert!(loads(&fx).first().is_some_and(|j| j.req.target == r2));
    st.forge.as_mut().unwrap().tracker = Some(github());
    st.issue = None;

    // An edited remote changes the tracker on the next refresh.
    run(&mut st, IssueMsg::Open("12".into()));
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.issue.is_some(), "the same tracker keeps it");
    git(&dir, &["remote", "set-url", "origin", "https://github.com/o/r2"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(st.forge.as_ref().unwrap().tracker.as_ref(), Some(&r2));
    assert!(st.issue.is_none(), "the remote's new tracker closes it");
    done(st, dir);
}
