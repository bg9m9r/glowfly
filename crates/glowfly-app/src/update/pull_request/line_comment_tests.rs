//! Headless tests for a new code comment on lines of a pull request file's diff: starting,
//! stretching and moving the draft, suggestions, and posting alone or to the pending review
//! (the answers and cancelling are in line_comment_reply_tests.rs).

use super::calls_tests::{calls, detail, done, opened, reply_msg, run, view};
use super::tests::pr_target;
use crate::pull_request::line_comment::LineCommentMsg;
use crate::pull_request::{PrCall, PrReply, PullRequestMsg};
use crate::state::AppState;
use crate::state::pull_request::{PrAction, PrTab};
use glowfly_core::diff_rows::LineSide;
use glowfly_core::forge::{LineAnchor, PrDetail, PrPage};
use glowfly_core::repo::{DiffLine, FileDiff, Hunk};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

fn dl(origin: char, old: Option<u32>, new: Option<u32>, text: &str) -> DiffLine {
    DiffLine { origin, old, new, text: text.into() }
}

/// [ctx 1/1, - old 2, + new 2, ctx 3/3], then a hunk at line 20.
fn diff() -> FileDiff {
    let one = Hunk {
        header: "@@ -1,3 +1,3 @@".into(),
        lines: vec![
            dl(' ', Some(1), Some(1), "a"),
            dl('-', Some(2), None, "b"),
            dl('+', None, Some(2), "B"),
            dl(' ', Some(3), Some(3), "c"),
        ],
    };
    let two = Hunk { header: "@@ -20,1 +20,2 @@".into(), lines: vec![dl(' ', Some(20), Some(20), "t"), dl('+', None, Some(21), "u")] };
    FileDiff { binary: false, hunks: vec![one, two], highlight: None, intraline: Default::default(), whole: None }
}

/// The opened panel with its page loaded (as `f` changes it) and f.txt's diff open.
pub(super) fn ready(name: &str, f: impl FnOnce(&mut PrDetail)) -> (AppState, [PathBuf; 2]) {
    let (mut st, _, dirs) = opened(name);
    let head = st.forge.as_ref().unwrap().prs[0].head.clone().unwrap_or_default();
    let mut d = detail(&head);
    f(&mut d);
    let page = PrPage { detail: d, entries: Vec::new(), threads: Vec::new(), threads_note: None };
    let msg = reply_msg(&st, PrReply::Loaded(Ok(page)));
    run(&mut st, msg);
    st.session.diff.view = Some((pr_target(&st, "f.txt"), Arc::new(diff())));
    (st, dirs)
}

pub(super) fn lc(st: &mut AppState, m: LineCommentMsg) -> Vec<crate::effect::Effect> {
    run(st, PullRequestMsg::LineComment(m))
}

pub(super) fn start(st: &mut AppState, side: LineSide, line: u32, extend: bool) {
    lc(st, LineCommentMsg::Start { side, line, extend });
}

pub(super) fn draft(st: &AppState) -> &crate::state::pull_request_line_comment::LineDraft {
    view(st).line_draft.as_ref().expect("a draft")
}

pub(super) fn set_text(st: &mut AppState, text: &str) {
    st.session.pull_request.view.as_mut().unwrap().line_draft.as_mut().unwrap().text = text.into();
}

fn select(st: &mut AppState, hunk: usize, lines: &[usize]) {
    let target = pr_target(st, "f.txt");
    let sel = &mut st.view.diff_lines;
    sel.target = Some(target);
    sel.hunk = hunk;
    sel.lines = lines.iter().copied().collect::<BTreeSet<_>>();
}

fn anchor(line: u32, side: LineSide, start: Option<(u32, LineSide)>) -> LineAnchor {
    LineAnchor { path: "f.txt".into(), line, side, start }
}

pub(super) fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

#[test]
fn start_on_a_context_line_opens_a_draft() {
    let (mut st, dirs) = ready("plc-ctx", |_| {});
    start(&mut st, LineSide::New, 1, false);
    assert_eq!(draft(&st).anchor, anchor(1, LineSide::New, None));
    assert_eq!((draft(&st).first, draft(&st).last), (0, 0));
    assert!(st.view.pr_line_comment_focus);
    assert_eq!(view(&st).tab, PrTab::Files);
    done(st, dirs);
}

#[test]
fn start_on_a_removed_line_uses_left_and_the_old_number() {
    let (mut st, dirs) = ready("plc-left", |_| {});
    start(&mut st, LineSide::Old, 2, false);
    assert_eq!(draft(&st).anchor, anchor(2, LineSide::Old, None));
    done(st, dirs);
}

#[test]
fn start_outside_the_hunks_is_refused() {
    let (mut st, dirs) = ready("plc-outside", |_| {});
    start(&mut st, LineSide::New, 40, false);
    assert!(view(&st).line_draft.is_none());
    assert_eq!(last_log(&st), "This line is outside the changes, so the service can't take a comment on it");
    done(st, dirs);
}

#[test]
fn on_selection_spans_the_selection() {
    let (mut st, dirs) = ready("plc-sel", |_| {});
    select(&mut st, 0, &[1, 2]);
    lc(&mut st, LineCommentMsg::OnSelection);
    assert_eq!((draft(&st).first, draft(&st).last), (1, 2));
    assert_eq!(draft(&st).anchor, anchor(2, LineSide::New, Some((2, LineSide::Old))));

    // A click on a selected line's button uses the selection too.
    st.session.pull_request.view.as_mut().unwrap().line_draft = None;
    start(&mut st, LineSide::New, 2, false);
    assert_eq!((draft(&st).first, draft(&st).last), (1, 2));
    // A line outside the selection is on its own.
    start(&mut st, LineSide::New, 3, false);
    assert_eq!((draft(&st).first, draft(&st).last), (3, 3));
    done(st, dirs);
}

#[test]
fn shift_start_extends_in_the_hunk_and_refuses_another_hunk() {
    let (mut st, dirs) = ready("plc-extend", |_| {});
    start(&mut st, LineSide::New, 2, true);
    assert!(view(&st).line_draft.is_none(), "nothing to extend");
    start(&mut st, LineSide::New, 2, false);
    start(&mut st, LineSide::New, 1, true);
    assert_eq!((draft(&st).first, draft(&st).last), (0, 2));
    assert_eq!(draft(&st).anchor, anchor(2, LineSide::New, Some((1, LineSide::New))));
    start(&mut st, LineSide::New, 21, true);
    assert_eq!((draft(&st).first, draft(&st).last), (0, 2), "unchanged");
    assert_eq!(last_log(&st), "A comment can cover lines of one hunk only");
    done(st, dirs);
}

#[test]
fn start_elsewhere_moves_the_draft_and_keeps_its_text() {
    let (mut st, dirs) = ready("plc-move", |_| {});
    start(&mut st, LineSide::New, 2, false);
    set_text(&mut st, "Rename this");
    st.session.pull_request.view.as_mut().unwrap().line_draft.as_mut().unwrap().error = Some("old failure".into());
    start(&mut st, LineSide::New, 21, false);
    assert_eq!(draft(&st).anchor, anchor(21, LineSide::New, None));
    assert_eq!(draft(&st).hunk, 1);
    assert_eq!(draft(&st).text, "Rename this");
    assert_eq!(draft(&st).error, None);
    assert!(st.modal.is_none(), "no confirmation to move it");
    done(st, dirs);
}

#[test]
fn suggest_appends_new_side_lines_and_is_blocked_for_deleted_lines() {
    let (mut st, dirs) = ready("plc-suggest", |_| {});
    start(&mut st, LineSide::New, 2, false);
    set_text(&mut st, "Better:");
    st.view.pr_line_comment_focus = false;
    lc(&mut st, LineCommentMsg::Suggest);
    assert_eq!(draft(&st).text, "Better:\n\n```suggestion\nB\n```");
    assert!(st.view.pr_line_comment_focus);
    start(&mut st, LineSide::Old, 2, false);
    let before = draft(&st).text.clone();
    lc(&mut st, LineCommentMsg::Suggest);
    assert_eq!(draft(&st).text, before, "a deleted line can't take a suggestion");
    done(st, dirs);
}

#[test]
fn post_single_sends_head_anchor_and_text() {
    let (mut st, dirs) = ready("plc-post", |_| {});
    start(&mut st, LineSide::New, 2, false);
    set_text(&mut st, "Rename this");
    let fx = lc(&mut st, LineCommentMsg::Post { review: false });
    let jobs = calls(&fx);
    let [job] = &jobs[..] else { panic!("one call: {jobs:?}") };
    let tip = view(&st).source_tip().unwrap().to_string();
    match &job.call {
        PrCall::LineComment(c) => {
            assert_eq!(c.head_sha, tip);
            assert_eq!(c.anchor, anchor(2, LineSide::New, None));
            assert_eq!(c.body, "Rename this");
        }
        other => panic!("not a line comment: {other:?}"),
    }
    assert_eq!(view(&st).busy, Some(PrAction::LineComment));
    assert_eq!(draft(&st).posting, Some(false));
    assert!(calls(&lc(&mut st, LineCommentMsg::Post { review: false })).is_empty(), "busy");
    done(st, dirs);
}

#[test]
fn post_review_passes_the_pending_review() {
    let (mut st, dirs) = ready("plc-review", |d| d.pending_review = Some(9));
    start(&mut st, LineSide::New, 2, false);
    set_text(&mut st, "Rename this");
    assert!(calls(&lc(&mut st, LineCommentMsg::Post { review: false })).is_empty(), "a review is pending");
    let fx = lc(&mut st, LineCommentMsg::Post { review: true });
    let jobs = calls(&fx);
    assert!(
        matches!(&jobs[..], [j] if matches!(&j.call, PrCall::ReviewComment { pending: Some(9), comment } if comment.body == "Rename this"))
    );
    assert_eq!(draft(&st).posting, Some(true));
    done(st, dirs);
}

#[test]
fn post_is_blocked_when_locked_head_elsewhere_empty_or_the_files_moved_since() {
    let (mut st, dirs) = ready("plc-blocked", |d| d.locked = true);
    start(&mut st, LineSide::New, 2, false);
    assert!(view(&st).line_draft.is_none(), "locked: no draft starts");
    let page = |st: &mut AppState, f: &dyn Fn(&mut PrDetail)| {
        let head = st.forge.as_ref().unwrap().prs[0].head.clone().unwrap_or_default();
        let mut d = detail(&head);
        f(&mut d);
        let msg = reply_msg(st, PrReply::Loaded(Ok(PrPage { detail: d, entries: Vec::new(), threads: Vec::new(), threads_note: None })));
        run(st, msg);
    };
    page(&mut st, &|_| {});
    start(&mut st, LineSide::New, 2, false);
    assert!(calls(&lc(&mut st, LineCommentMsg::Post { review: false })).is_empty(), "empty");
    set_text(&mut st, "Rename this");
    page(&mut st, &|d| d.locked = true);
    assert!(calls(&lc(&mut st, LineCommentMsg::Post { review: false })).is_empty(), "locked");
    page(&mut st, &|d| d.head_sha = "abc1234".into());
    assert!(calls(&lc(&mut st, LineCommentMsg::Post { review: false })).is_empty(), "the service's head is elsewhere");
    page(&mut st, &|_| {});
    st.session.pull_request.view.as_mut().unwrap().line_draft.as_mut().unwrap().head = "0".repeat(40);
    assert!(calls(&lc(&mut st, LineCommentMsg::Post { review: false })).is_empty(), "compared again since");
    assert_eq!(view(&st).busy, None);
    done(st, dirs);
}
