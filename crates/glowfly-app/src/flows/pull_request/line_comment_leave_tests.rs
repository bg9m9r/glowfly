//! Headless tests for leaving the pull request panel while a new code comment is unposted:
//! closing it or opening another pull request asks first with an edited draft, and goes at
//! once with an empty one or one being posted.

use super::calls_tests::{done, run, view};
use super::line_comment_tests::{lc, ready, set_text, start};
use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::pull_request::line_comment::{Leave, LineCommentMsg};
use crate::state::{AppState, Modal};
use glowfly_core::diff_rows::LineSide;

/// The pull request message the open danger confirmation's Discard sends.
fn asked(st: &AppState) -> Option<PullRequestMsg> {
    match &st.modal {
        Some(Modal::ConfirmDanger { then, .. }) => match &**then {
            Msg::PullRequest(m) => Some(m.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// Lists pull request 8 next to the open one (7).
fn list_another(st: &mut AppState) {
    let fs = st.session.forge.as_mut().unwrap();
    let other = glowfly_core::forge::PullRequest { id: 8, ..fs.prs[0].clone() };
    fs.prs.push(other);
}

#[test]
fn closing_asks_with_an_edited_draft() {
    let (mut st, dirs) = ready("plc-leave-close", |_| {});
    start(&mut st, LineSide::New, 2, false);
    set_text(&mut st, "Rename this");
    run(&mut st, PullRequestMsg::Close);
    assert!(st.session.pull_request.view.is_some(), "the panel stays until Discard");
    let then = asked(&st).expect("asks first");
    assert_eq!(then, PullRequestMsg::LineComment(LineCommentMsg::DiscardAnd(Leave::Close)));

    st.modal = None; // Keep editing
    assert_eq!(view(&st).line_draft.as_ref().map(|d| d.text.as_str()), Some("Rename this"));
    run(&mut st, PullRequestMsg::Close);
    let m = asked(&st).expect("asks again");
    st.modal = None;
    run(&mut st, m);
    assert!(st.session.pull_request.view.is_none(), "Discard closes the panel");
    done(st, dirs);
}

#[test]
fn opening_another_asks_with_an_edited_draft() {
    let (mut st, dirs) = ready("plc-leave-open", |_| {});
    list_another(&mut st);
    start(&mut st, LineSide::New, 2, false);
    set_text(&mut st, "Rename this");
    run(&mut st, PullRequestMsg::Open(7));
    assert!(st.modal.is_none(), "the shown pull request again asks nothing");
    run(&mut st, PullRequestMsg::Open(8));
    assert_eq!(view(&st).pr.id, 7, "the panel stays until Discard");
    let m = asked(&st).expect("asks first");
    assert_eq!(m, PullRequestMsg::LineComment(LineCommentMsg::DiscardAnd(Leave::Open(8))));
    st.modal = None;
    run(&mut st, m);
    assert_eq!(view(&st).pr.id, 8, "Discard opens the other one");
    assert!(view(&st).line_draft.is_none());
    done(st, dirs);
}

#[test]
fn an_empty_draft_asks_nothing() {
    let (mut st, dirs) = ready("plc-leave-empty", |_| {});
    list_another(&mut st);
    start(&mut st, LineSide::New, 2, false);
    set_text(&mut st, "  \n");
    run(&mut st, PullRequestMsg::Open(8));
    assert!(st.modal.is_none());
    assert_eq!(view(&st).pr.id, 8);
    run(&mut st, PullRequestMsg::Close);
    assert!(st.modal.is_none());
    assert!(st.session.pull_request.view.is_none());
    done(st, dirs);
}

#[test]
fn a_draft_being_posted_asks_nothing() {
    let (mut st, dirs) = ready("plc-leave-posting", |_| {});
    start(&mut st, LineSide::New, 2, false);
    set_text(&mut st, "Rename this");
    lc(&mut st, LineCommentMsg::Post { review: false });
    assert!(view(&st).line_draft.as_ref().is_some_and(|d| d.posting.is_some()));
    run(&mut st, PullRequestMsg::Close);
    assert!(st.modal.is_none());
    assert!(st.session.pull_request.view.is_none());
    done(st, dirs);
}
