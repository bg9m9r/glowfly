//! Headless tests for a new code comment left unposted when the pull request panel closes or
//! gives way to another view: nothing is asked, the draft is kept and comes back (on Files
//! changed) when the pull request opens again, an empty one isn't kept, and a draft being posted
//! is settled by its answer whether the panel is closed, opened again or parked with its tab.

use super::calls_tests::{done, err, reply_msg, run, view};
use super::line_comment_reply_tests::{posted, single_ok};
use super::line_comment_tests::{draft, ready, set_text, start};
use super::tests::github;
use glowfly_core::diff_rows::LineSide;
use glowfly_model::msg::{IssueMsg, Msg, TabsMsg};
use glowfly_model::pull_request::{PrReply, PullRequestMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::pull_request::line_comment::LineDraft;
use glowfly_model::state::pull_request::{PrAction, PrTab};
use glowfly_model::update::update;
use std::path::PathBuf;
use std::time::Instant;

/// Lists pull request 8 next to the open one (7).
fn list_another(st: &mut AppState) {
    let fs = st.session.forge.as_mut().unwrap();
    let other = glowfly_core::forge::PullRequest { id: 8, ..fs.prs[0].clone() };
    fs.prs.push(other);
}

/// The line comment kept for pull request 7.
fn kept(st: &AppState) -> Option<&LineDraft> {
    st.session.pr_drafts.get(7).and_then(|(_, d, _)| d)
}

/// A draft "Rename this" on new line 2 of f.txt.
fn drafted(name: &str) -> (AppState, [PathBuf; 2]) {
    let (mut st, dirs) = ready(name, |_| {});
    start(&mut st, LineSide::New, 2, false);
    set_text(&mut st, "Rename this");
    (st, dirs)
}

#[test]
fn closing_keeps_the_draft_and_reopening_restores_it() {
    let (mut st, dirs) = drafted("pkd-close");
    let anchor = draft(&st).anchor.clone();
    run(&mut st, PullRequestMsg::Close);
    assert!(st.modal.is_none(), "nothing is asked");
    assert!(st.session.pull_request.view.is_none());
    assert_eq!(kept(&st).map(|d| d.text.as_str()), Some("Rename this"));

    run(&mut st, PullRequestMsg::Open(7));
    assert_eq!(view(&st).tab, PrTab::Files);
    let d = draft(&st);
    assert_eq!((d.text.as_str(), &d.anchor), ("Rename this", &anchor));
    assert!(st.session.pr_drafts.is_empty());
    done(st, dirs);
}

#[test]
fn opening_another_pull_request_keeps_the_draft() {
    let (mut st, dirs) = drafted("pkd-another");
    list_another(&mut st);
    run(&mut st, PullRequestMsg::Open(8));
    assert!(st.modal.is_none(), "nothing is asked");
    assert_eq!(view(&st).pr.id, 8);
    assert!(view(&st).line_draft.is_none());
    assert_eq!(kept(&st).map(|d| d.text.as_str()), Some("Rename this"));
    done(st, dirs);
}

#[test]
fn opening_an_issue_keeps_the_draft() {
    let (mut st, dirs) = drafted("pkd-issue");
    st.session.forge.as_mut().unwrap().tracker = Some(github());
    update(&mut st, Msg::Issue(IssueMsg::Open("12".into())), Instant::now());
    assert!(st.session.pull_request.view.is_none());
    assert_eq!(kept(&st).map(|d| d.text.as_str()), Some("Rename this"));
    done(st, dirs);
}

#[test]
fn an_empty_draft_is_not_kept() {
    let (mut st, dirs) = ready("pkd-empty", |_| {});
    start(&mut st, LineSide::New, 2, false);
    set_text(&mut st, "  \n");
    run(&mut st, PullRequestMsg::Close);
    assert!(st.session.pr_drafts.is_empty());
    run(&mut st, PullRequestMsg::Open(7));
    assert!(view(&st).line_draft.is_none());
    done(st, dirs);
}

#[test]
fn a_posted_answer_settles_the_kept_draft() {
    let (mut st, dirs) = ready("pkd-posted", |_| {});
    posted(&mut st, false);
    let msg = reply_msg(&st, single_ok());
    run(&mut st, PullRequestMsg::Close);
    assert_eq!(kept(&st).map(|d| d.posting), Some(Some(false)));
    run(&mut st, msg);
    assert!(st.session.pr_drafts.is_empty(), "the posted draft doesn't come back");
    assert!(st.toasts.iter().any(|t| t.ok && t.text == "Commented on f.txt in #7"));
    done(st, dirs);
}

#[test]
fn a_failed_answer_keeps_the_draft_with_the_reason() {
    let (mut st, dirs) = ready("pkd-failed", |_| {});
    posted(&mut st, false);
    let msg = reply_msg(&st, PrReply::LineCommented { review: false, result: Err(err("timed out")) });
    run(&mut st, PullRequestMsg::Close);
    run(&mut st, msg);
    let d = kept(&st).expect("kept");
    assert_eq!((d.text.as_str(), d.posting, d.error.as_deref()), ("Rename this", None, Some("timed out")));
    assert!(st.toasts.iter().any(|t| !t.ok && t.text == "Couldn't post the comment on #7: timed out"));

    run(&mut st, PullRequestMsg::Open(7));
    let v = view(&st);
    assert_eq!(v.busy, None);
    assert_eq!(draft(&st).error.as_deref(), Some("timed out"), "reopened with the reason");
    done(st, dirs);
}

#[test]
fn reopened_before_the_answer_it_waits_and_takes_it() {
    let (mut st, dirs) = ready("pkd-reopened", |_| {});
    posted(&mut st, false);
    let msg = reply_msg(&st, single_ok());
    run(&mut st, PullRequestMsg::Close);
    run(&mut st, PullRequestMsg::Open(7));
    assert_eq!(view(&st).busy, Some(PrAction::LineComment));
    assert_eq!(draft(&st).posting, Some(false));
    run(&mut st, msg);
    assert!(view(&st).line_draft.is_none(), "the open panel took the answer");
    assert!(st.toasts.iter().any(|t| t.ok && t.text == "Comment posted"));
    done(st, dirs);
}

#[test]
fn a_parked_tabs_kept_draft_is_settled() {
    for (name, ok) in [("pkd-parked-ok", true), ("pkd-parked-err", false)] {
        let (mut st, dirs) = ready(name, |_| {});
        posted(&mut st, false);
        let reply = if ok { single_ok() } else { PrReply::LineCommented { review: false, result: Err(err("timed out")) } };
        let msg = reply_msg(&st, reply);
        run(&mut st, PullRequestMsg::Close);
        update(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
        run(&mut st, msg);
        let parked = &st.tabs[0].parked.as_ref().expect("parked").session.pr_drafts;
        if ok {
            assert!(parked.is_empty(), "the posted draft doesn't come back");
        } else {
            let d = parked.get(7).and_then(|(_, d, _)| d).expect("kept");
            assert_eq!((d.text.as_str(), d.posting, d.error.as_deref()), ("Rename this", None, Some("timed out")));
        }
        done(st, dirs);
    }
}
