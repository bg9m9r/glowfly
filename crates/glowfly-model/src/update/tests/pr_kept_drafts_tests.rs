//! Headless tests for the text of a pull request panel that another view replaced: every path
//! that replaces the panel keeps its unposted text, opening the pull request again restores it
//! (with the tab it was on), blame leaves the panel open, blank text isn't kept, another code
//! host drops what was kept, and a post answered after the panel closed settles the kept text.

use super::tests::state;
use super::update;
use crate::effect::{Effect, ForgeError};
use crate::msg::{BlameMsg, ConflictMsg, ForgeMsg, HistoryMsg, IssueMsg, Msg, RepoMsg};
use crate::pull_request::{PrReply, PullRequestMsg};
use crate::state::pull_request::{PrAction, PrTab, PrView};
use crate::state::{AppState, ForgeState};
use glowfly_core::forge::{Entry, EntryKind, Provider, PullRequest, Target};
use glowfly_core::repo::DiffTarget;
use glowfly_core::settings::TrackerChoice;
use std::time::Instant;

fn target() -> Target {
    Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() }
}

fn pull_request(id: u64) -> PullRequest {
    PullRequest {
        id,
        title: "Add search".into(),
        description: String::new(),
        source: "feature".into(),
        target: "main".into(),
        author: "ada".into(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        head_repo: Default::default(),
        merge_status: None,
        checks: None,
        url: String::new(),
        facets: Default::default(),
    }
}

/// A repository on GitHub (code host and tracker) listing pull requests 7 and 8.
fn setup(name: &str) -> AppState {
    let mut st = state();
    st.snap = Some(crate::state::op_fixture::snapshot(name));
    let mut fs = ForgeState::new(Some(target()), "origin".into(), Some(target()));
    fs.prs = vec![pull_request(7), pull_request(8)];
    st.session.forge = Some(fs);
    st
}

fn send(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn pr(st: &mut AppState, msg: PullRequestMsg) -> Vec<Effect> {
    send(st, Msg::PullRequest(msg))
}

fn view(st: &mut AppState) -> &mut PrView {
    st.session.pull_request.view.as_mut().expect("the panel is open")
}

/// Opens pull request `id` and types a comment, a review summary and a reply in it.
fn typed(st: &mut AppState, id: u64) {
    pr(st, PullRequestMsg::Open(id));
    let f = &mut view(st).forms;
    f.comment = "Looks good".into();
    f.review_body = "Two nits".into();
    f.replies.insert("T1".into(), "Done".into());
}

/// The comment, review summary and reply kept for pull request `id`.
fn kept(st: &AppState, id: u64) -> Option<(String, String, Option<String>)> {
    st.session.pr_drafts.get(id).map(|(f, _, _)| (f.comment.clone(), f.review_body.clone(), f.replies.get("T1").cloned()))
}

fn typed_text() -> Option<(String, String, Option<String>)> {
    Some(("Looks good".into(), "Two nits".into(), Some("Done".into())))
}

/// A message that replaces the panel, made for the state it is sent in.
type Path = fn(&AppState) -> Msg;

#[test]
fn every_replacing_path_keeps_the_text_and_reopening_restores_it() {
    let paths: [(&str, Path); 6] = [
        ("issue", |_| Msg::Issue(IssueMsg::Open("12".into()))),
        ("outside diff", |_| Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("a.txt".into())))),
        ("file history", |_| Msg::History(HistoryMsg::OpenFileHistory { path: "a.txt".into(), at: None })),
        ("conflict", |_| Msg::Conflict(ConflictMsg::OpenConflict("a.txt".into()))),
        ("commit", |st| Msg::Repo(RepoMsg::SelectCommit(st.snap.as_ref().and_then(|s| s.head_oid).unwrap()))),
        ("close", |_| Msg::PullRequest(PullRequestMsg::Close)),
    ];
    let mut st = setup("pr-kept-paths");
    for (name, path) in paths {
        typed(&mut st, 7);
        let msg = path(&st);
        send(&mut st, msg);
        assert!(st.session.pull_request.view.is_none(), "{name} closes the panel");
        assert_eq!(kept(&st, 7), typed_text(), "{name} keeps the text");
        pr(&mut st, PullRequestMsg::Open(7));
        let f = &view(&mut st).forms;
        assert_eq!((f.comment.as_str(), f.review_body.as_str()), ("Looks good", "Two nits"), "{name}: restored");
        assert_eq!(f.replies.get("T1").map(String::as_str), Some("Done"));
        assert!(st.session.pr_drafts.is_empty(), "{name}: taken out");
        pr(&mut st, PullRequestMsg::Close);
        st.session.pr_drafts.clear();
    }
}

#[test]
fn opening_another_pull_request_keeps_the_first_ones_text() {
    let mut st = setup("pr-kept-another");
    typed(&mut st, 7);
    pr(&mut st, PullRequestMsg::Open(8));
    assert_eq!(view(&mut st).pr.id, 8);
    assert_eq!(view(&mut st).forms.comment, "", "8 starts empty");
    assert_eq!(kept(&st, 7), typed_text());

    view(&mut st).forms.comment = "On 8".into();
    pr(&mut st, PullRequestMsg::Open(7));
    assert_eq!(view(&mut st).forms.comment, "Looks good");
    assert_eq!(kept(&st, 8).map(|k| k.0).as_deref(), Some("On 8"), "8's text is kept in turn");
    assert!(st.session.pr_drafts.get(7).is_none());
}

#[test]
fn blame_leaves_the_panel_open() {
    let mut st = setup("pr-kept-blame");
    typed(&mut st, 7);
    send(&mut st, Msg::Blame(BlameMsg::Open { path: "a.txt".into(), at: None }));
    assert_eq!(view(&mut st).forms.comment, "Looks good");
    assert!(st.session.pr_drafts.is_empty());
}

#[test]
fn the_panel_returns_on_its_tab() {
    let mut st = setup("pr-kept-tab");
    typed(&mut st, 7);
    pr(&mut st, PullRequestMsg::Tab(PrTab::Files));
    pr(&mut st, PullRequestMsg::Close);
    pr(&mut st, PullRequestMsg::Open(7));
    assert_eq!(view(&mut st).tab, PrTab::Files);
}

#[test]
fn blank_text_is_not_kept() {
    let mut st = setup("pr-kept-blank");
    pr(&mut st, PullRequestMsg::Open(7));
    view(&mut st).forms.comment = " \n".into();
    view(&mut st).forms.merge_title = "Add search (#7)".into();
    pr(&mut st, PullRequestMsg::Tab(PrTab::Commits));
    pr(&mut st, PullRequestMsg::Close);
    assert!(st.session.pr_drafts.is_empty());
    pr(&mut st, PullRequestMsg::Open(7));
    assert_eq!(view(&mut st).forms.comment, "");
    assert_eq!(view(&mut st).tab, PrTab::Overview);
}

#[test]
fn another_code_host_drops_the_kept_text() {
    let mut st = setup("pr-kept-host");
    typed(&mut st, 7);
    pr(&mut st, PullRequestMsg::Close);
    crate::update::pull_request::host_detected(&mut st);
    assert_eq!(kept(&st, 7), typed_text(), "the same host keeps it");

    // The fixture has no remotes, so working the host out again finds none.
    send(&mut st, Msg::Forge(ForgeMsg::SetTracker(TrackerChoice::Auto)));
    assert!(st.session.pr_drafts.is_empty());
}

fn reply(st: &AppState, reply: PrReply) -> PullRequestMsg {
    let workdir = st.workdir().unwrap();
    PullRequestMsg::Replied { workdir, target: target(), id: 7, generation: 0, reply: Box::new(reply) }
}

fn entry() -> Entry {
    Entry { author: "grace".into(), time: 5, kind: EntryKind::Comment, body: "Looks good".into(), url: String::new() }
}

fn failed() -> ForgeError {
    ForgeError { message: "timed out".into(), auth: false }
}

/// Pull request 7 with only a comment typed, being posted, then closed.
fn posting_then_closed(name: &str) -> AppState {
    let mut st = setup(name);
    pr(&mut st, PullRequestMsg::Open(7));
    view(&mut st).forms.comment = "Looks good".into();
    view(&mut st).busy = Some(PrAction::Comment);
    pr(&mut st, PullRequestMsg::Close);
    assert_eq!(st.session.pr_drafts.get(7).and_then(|(_, _, b)| b.cloned()), Some(PrAction::Comment));
    st
}

#[test]
fn a_comment_posted_after_the_panel_closed_leaves_nothing_kept() {
    let mut st = posting_then_closed("pr-kept-posted");
    let msg = reply(&st, PrReply::Commented(Ok(entry())));
    pr(&mut st, msg);
    assert!(st.session.pr_drafts.is_empty());
    assert!(st.toasts.iter().any(|t| t.ok && t.text == "Comment posted"));
}

#[test]
fn a_comment_that_failed_after_the_panel_closed_stays() {
    let mut st = posting_then_closed("pr-kept-failed");
    let msg = reply(&st, PrReply::Commented(Err(failed())));
    pr(&mut st, msg);
    let (forms, _, busy) = st.session.pr_drafts.get(7).expect("kept");
    assert_eq!((forms.comment.as_str(), busy), ("Looks good", None));
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == "Commenting on #7 failed: timed out"));
}

#[test]
fn reopened_before_the_answer_the_panel_waits_for_it() {
    let mut st = posting_then_closed("pr-kept-reopened");
    pr(&mut st, PullRequestMsg::Open(7));
    assert_eq!(view(&mut st).busy, Some(PrAction::Comment), "Waiting for the service");
    let msg = reply(&st, PrReply::Commented(Ok(entry())));
    pr(&mut st, msg);
    let v = view(&mut st);
    assert_eq!((v.forms.comment.as_str(), v.busy.clone()), ("", None));
    assert!(st.session.pr_drafts.is_empty());
}
