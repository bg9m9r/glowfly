//! Headless tests for the answers to a new code comment on diff lines (the open panel, a parked
//! tab's and a closed one), the provisional thread until the page reloads, and cancelling.

use super::calls_tests::{calls, detail, done, err, loads, reply_msg, run, view};
use super::line_comment_tests::{draft, last_log, lc, ready, set_text, start};
use super::tests::github;
use crate::effect::ForgeError;
use crate::msg::{Msg, TabsMsg};
use crate::pull_request::line_comment::LineCommentMsg;
use crate::pull_request::{PrReply, PullRequestMsg};
use crate::state::pull_request_actions::review_button;
use crate::state::pull_request_threads::resolve_blocked;
use crate::state::{AppState, Modal};
use crate::update::update;
use glowfly_core::diff_rows::LineSide;
use glowfly_core::forge::{Auth, NewThread, PrPage, Provider, ReviewThread, ThreadComment};
use std::time::Instant;

fn comment(id: u64, pending: bool) -> ThreadComment {
    ThreadComment { id: Some(id), author: "grace".into(), time: 5, body: "Rename this".into(), url: String::new(), pending }
}

fn new_thread(id: &str, first: u64, pending: bool) -> ReviewThread {
    ReviewThread {
        id: id.into(),
        path: "f.txt".into(),
        side: LineSide::New,
        line: Some(2),
        start_line: None,
        original_line: None,
        outdated: false,
        resolved: false,
        resolved_by: None,
        can_reply: true,
        can_resolve: !id.starts_with("new:"),
        can_unresolve: false,
        context: String::new(),
        comments: vec![comment(first, pending)],
        more: 0,
    }
}

fn single_ok() -> PrReply {
    PrReply::LineCommented { review: false, result: Ok(NewThread { thread: new_thread("new:901", 901, false), pending_review: None }) }
}

/// Starts a draft on new line 2 with text and posts it (`review`: to the pending review).
fn posted(st: &mut AppState, review: bool) {
    start(st, LineSide::New, 2, false);
    set_text(st, "Rename this");
    lc(st, LineCommentMsg::Post { review });
    assert_eq!(draft(st).posting, Some(review));
}

fn threads(st: &AppState) -> Vec<String> {
    view(st).loaded().unwrap().threads.iter().map(|t| t.id.clone()).collect()
}

#[test]
fn posted_single_comment_appears_and_the_page_reloads() {
    let (mut st, dirs) = ready("plc-ok", |_| {});
    posted(&mut st, false);
    let msg = reply_msg(&st, single_ok());
    let fx = run(&mut st, msg);
    let v = view(&st);
    assert!(v.line_draft.is_none());
    assert_eq!(threads(&st), ["new:901"]);
    assert_eq!(v.threads.focus.as_deref(), Some("new:901"));
    assert_eq!(v.threads.open.get("new:901"), Some(&true));
    assert_eq!(st.view.pr_thread_scroll.as_deref(), Some("new:901"));
    assert_eq!(v.busy, None);
    assert!(st.toasts.iter().any(|t| t.ok && t.text == "Comment posted"));
    assert_eq!(loads(&fx), 1);
    assert_eq!(last_log(&st), "Commented on f.txt in #7");

    // Until the reload the thread can't be resolved, but its replies work.
    let t = view(&st).loaded().unwrap().threads[0].clone();
    assert_eq!(resolve_blocked(view(&st), &t).as_deref(), Some("This new thread can be resolved once the page has reloaded"));
    st.session.pull_request.view.as_mut().unwrap().reloading = false;
    let fx = run(&mut st, PullRequestMsg::Resolve { thread: "new:901".into(), resolve: true });
    assert!(calls(&fx).is_empty());

    // The reload hands the reply draft and focus to the real thread.
    st.session.pull_request.view.as_mut().unwrap().forms.replies.insert("new:901".into(), "Also this".into());
    let head = st.session.forge.as_ref().unwrap().prs[0].head.clone().unwrap_or_default();
    let page = PrPage { detail: detail(&head), entries: Vec::new(), threads: vec![new_thread("PRRT_1", 901, false)], threads_note: None };
    let msg = reply_msg(&st, PrReply::Loaded(Ok(page)));
    run(&mut st, msg);
    let v = view(&st);
    assert_eq!(v.forms.replies.get("PRRT_1").map(String::as_str), Some("Also this"));
    assert_eq!(v.threads.focus.as_deref(), Some("PRRT_1"));
    done(st, dirs);
}

#[test]
fn posted_review_comment_sets_the_pending_review() {
    let (mut st, dirs) = ready("plc-review-ok", |_| {});
    posted(&mut st, true);
    let result = Ok(NewThread { thread: new_thread("PRRT_2", 902, true), pending_review: Some(31) });
    let msg = reply_msg(&st, PrReply::LineCommented { review: true, result });
    let fx = run(&mut st, msg);
    let v = view(&st);
    assert_eq!(v.loaded().unwrap().detail.pending_review, Some(31));
    assert_eq!(review_button(v), "Finish review");
    assert!(v.loaded().unwrap().threads[0].comments[0].pending);
    assert!(st.toasts.iter().any(|t| t.text == "Review started. Finish it from Your review on Overview"));
    assert_eq!(loads(&fx), 0);
    assert_eq!(last_log(&st), "Added a review comment on f.txt in #7");

    // The next one adds to it.
    posted(&mut st, true);
    let result = Ok(NewThread { thread: new_thread("PRRT_3", 903, true), pending_review: Some(31) });
    let msg = reply_msg(&st, PrReply::LineCommented { review: true, result });
    run(&mut st, msg);
    assert!(st.toasts.iter().any(|t| t.text == "Added to your review"));
    done(st, dirs);
}

#[test]
fn failed_post_keeps_the_draft_with_the_services_words() {
    let (mut st, dirs) = ready("plc-err", |_| {});
    posted(&mut st, false);
    let words = "the server said \u{201c}Validation Failed pull_request_review_thread.line must be part of the diff\u{201d}";
    let msg = reply_msg(&st, PrReply::LineCommented { review: false, result: Err(err(words)) });
    let fx = run(&mut st, msg);
    let d = draft(&st);
    assert_eq!((d.text.as_str(), d.error.as_deref(), d.posting), ("Rename this", Some(words), None));
    assert_eq!(view(&st).busy, None);
    assert_eq!(loads(&fx), 0);
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == format!("Commenting on #7 failed: {words}")));

    lc(&mut st, LineCommentMsg::Post { review: true });
    let msg = reply_msg(&st, PrReply::LineCommented { review: true, result: Err(err("timed out")) });
    let fx = run(&mut st, msg);
    assert_eq!(loads(&fx), 1, "a review may have been started before the thread failed");
    assert_eq!(draft(&st).error.as_deref(), Some("timed out"));
    done(st, dirs);
}

#[test]
fn an_auth_failure_drops_the_cached_auth() {
    let (mut st, dirs) = ready("plc-auth", |_| {});
    let key = github().account().key();
    st.session.forge.as_mut().unwrap().auth.insert(key.clone(), Auth::from_token(Provider::GitHub, None, "tok", "test"));
    posted(&mut st, false);
    let e = ForgeError { message: "authentication failed: 401".into(), auth: true };
    let msg = reply_msg(&st, PrReply::LineCommented { review: false, result: Err(e) });
    run(&mut st, msg);
    assert!(!st.session.forge.as_ref().unwrap().auth.contains_key(&key));
    done(st, dirs);
}

#[test]
fn parked_tab_applies_the_answer_to_its_panel() {
    let (mut st, dirs) = ready("plc-parked", |_| {});
    posted(&mut st, false);
    let msg = reply_msg(&st, single_ok());
    update(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
    assert!(st.session.pull_request.view.is_none(), "parked with its tab");
    let toasts = st.toasts.len();
    run(&mut st, msg);
    let parked = st.tabs[0].parked.as_ref().and_then(|p| p.session.pull_request.view.as_ref()).expect("the parked panel");
    assert!(parked.line_draft.is_none());
    assert_eq!(parked.loaded().unwrap().threads.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["new:901"]);
    assert!(st.tabs[0].log.last().is_some_and(|l| l.ok && l.text == "Commented on f.txt in #7"), "logged in that tab");
    assert_eq!(st.toasts.len(), toasts, "no toast");
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert_eq!(loads(&fx), 1, "the page reloads on return");

    posted(&mut st, false);
    let msg = reply_msg(&st, PrReply::LineCommented { review: false, result: Err(err("timed out")) });
    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)), Instant::now());
    run(&mut st, msg);
    let parked = st.tabs[0].parked.as_ref().and_then(|p| p.session.pull_request.view.as_ref()).expect("the parked panel");
    let d = parked.line_draft.as_ref().expect("kept");
    assert_eq!((d.text.as_str(), d.error.as_deref(), d.posting), ("Rename this", Some("timed out"), None));
    done(st, dirs);
}

#[test]
fn an_answer_for_a_closed_panel_is_logged_and_toasted() {
    let (mut st, dirs) = ready("plc-closed", |_| {});
    posted(&mut st, false);
    let msg = reply_msg(&st, PrReply::LineCommented { review: false, result: Err(err("timed out")) });
    run(&mut st, PullRequestMsg::Close);
    run(&mut st, msg);
    assert!(st.toasts.iter().any(|t| !t.ok && t.text == "Couldn't post the comment on #7: timed out"));
    assert_eq!(last_log(&st), "Commenting on #7 failed: timed out");
    done(st, dirs);
}

#[test]
fn cancel_drops_an_empty_draft() {
    let (mut st, dirs) = ready("plc-cancel", |_| {});
    start(&mut st, LineSide::New, 2, false);
    set_text(&mut st, "  ");
    lc(&mut st, LineCommentMsg::Cancel);
    assert!(view(&st).line_draft.is_none());
    assert!(st.modal.is_none());
    done(st, dirs);
}

#[test]
fn cancel_asks_before_dropping_an_edited_one() {
    let (mut st, dirs) = ready("plc-cancel-edited", |_| {});
    start(&mut st, LineSide::New, 2, false);
    set_text(&mut st, "Rename this");
    lc(&mut st, LineCommentMsg::Cancel);
    match &st.modal {
        Some(Modal::ConfirmDanger { then, .. }) => {
            assert!(matches!(**then, Msg::PullRequest(PullRequestMsg::LineComment(LineCommentMsg::Discard))))
        }
        _ => panic!("no danger confirmation"),
    }
    assert!(view(&st).line_draft.is_some(), "kept until Discard");
    st.modal = None;
    lc(&mut st, LineCommentMsg::Discard);
    assert!(view(&st).line_draft.is_none());
    done(st, dirs);
}

#[test]
fn cancel_while_posting_does_nothing() {
    let (mut st, dirs) = ready("plc-cancel-posting", |_| {});
    posted(&mut st, false);
    lc(&mut st, LineCommentMsg::Cancel);
    lc(&mut st, LineCommentMsg::Discard);
    assert!(view(&st).line_draft.is_some());
    assert!(st.modal.is_none());
    done(st, dirs);
}
