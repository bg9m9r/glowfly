//! A new code comment on lines of a pull request file's diff: starting the draft from a line's
//! button or the selected lines, stretching and moving it, a suggestion, posting it alone or to
//! the pending review (through `calls::act`, one action at a time), cancelling it, and applying
//! the answer to the open panel or to a parked tab's.

use super::calls::{act, action_text, load, service_request};
use crate::effect::{Effect, ForgeError};
use crate::pull_request::line_comment::LineCommentMsg;
use crate::pull_request::{PrCall, PrReply};
use crate::state::AppState;
use crate::state::pull_request::{Page, PrAction, PrTab, PrView};
use crate::state::pull_request_line_comment::{
    ONE_HUNK, OUTSIDE, discard_confirm, draft_at, edited, post_blocked, start_blocked, suggest_blocked, with_suggestion,
};
use glowfly_core::diff_rows::{LineSide, line_index, row_of_file_line};
use glowfly_core::forge::{NewComment, NewThread, Target};
use std::path::Path;
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: LineCommentMsg, _now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        LineCommentMsg::Start { side, line, extend } => start(st, side, line, extend),
        LineCommentMsg::OnSelection => on_selection(st),
        LineCommentMsg::Suggest => suggest(st),
        LineCommentMsg::Post { review } => post(st, review, fx),
        LineCommentMsg::Cancel => cancel(st),
        LineCommentMsg::Discard => {
            if let Some(v) = st.session.pull_request.view.as_mut()
                && v.line_draft.as_ref().is_some_and(|d| d.posting.is_none())
            {
                v.line_draft = None;
            }
        }
        LineCommentMsg::Focus => {
            if let Some(v) = st.session.pull_request.view.as_mut().filter(|v| v.line_draft.is_some()) {
                v.tab = PrTab::Files;
                st.view.pr_line_comment_focus = true;
            }
        }
        LineCommentMsg::Show => show(st, fx),
    }
}

fn provider(st: &AppState) -> Option<glowfly_core::forge::Provider> {
    st.forge.as_ref().and_then(|f| f.host_provider())
}

/// The draft can start on the open diff: it is one of the panel's files and nothing blocks it.
fn may_start(st: &mut AppState) -> bool {
    let Some(v) = st.session.pull_request.view.as_ref() else { return false };
    if !st.session.diff.view.as_ref().is_some_and(|(t, _)| v.shows_file(t)) {
        return false;
    }
    match start_blocked(provider(st), v) {
        Some(why) => {
            st.log_quiet(false, why);
            false
        }
        None => true,
    }
}

/// A line's comment button: a draft on that line, on the selection it is part of, or (with
/// `extend`) the open draft stretched to it within its hunk.
fn start(st: &mut AppState, side: LineSide, n: u32, extend: bool) {
    if !may_start(st) {
        return;
    }
    match span_at(st, side, n, extend) {
        Ok((h, first, last)) => build(st, h, first, last),
        Err(why) => st.log_quiet(false, why),
    }
}

/// The (hunk, first, last) lines a button on file line `n` of `side` puts the draft on.
fn span_at(st: &AppState, side: LineSide, n: u32, extend: bool) -> Result<(usize, usize, usize), &'static str> {
    let (target, diff) = st.session.diff.view.as_ref().ok_or(OUTSIDE)?;
    let (h, i) = line_index(diff, side, n).ok_or(OUTSIDE)?;
    let sel = &st.view.diff_lines;
    if extend {
        return match st.session.pull_request.view.as_ref().and_then(|v| v.line_draft.as_ref()) {
            Some(d) if d.target == *target && d.hunk == h => Ok((h, d.first.min(i), d.last.max(i))),
            _ => Err(ONE_HUNK),
        };
    }
    match (sel.contains(target, h, i), sel.lines.first(), sel.lines.last()) {
        (true, Some(&a), Some(&b)) => Ok((h, a, b)),
        _ => Ok((h, i, i)),
    }
}

/// The hunk header's Comment on N lines: a draft on the selection's span.
fn on_selection(st: &mut AppState) {
    if !may_start(st) {
        return;
    }
    let sel = &st.view.diff_lines;
    let (Some((target, _)), Some(first), Some(last)) = (st.session.diff.view.as_ref(), sel.lines.first(), sel.lines.last()) else { return };
    if sel.target.as_ref() != Some(target) {
        return;
    }
    let (h, first, last) = (sel.hunk, *first, *last);
    build(st, h, first, last);
}

/// Puts the draft on lines `first..=last` of hunk `h` of the open diff, keeping an existing
/// draft's text, and focuses its box.
fn build(st: &mut AppState, h: usize, first: usize, last: usize) {
    let (Some((target, diff)), Some(v)) = (st.session.diff.view.as_ref(), st.session.pull_request.view.as_mut()) else { return };
    match draft_at(v, target, diff, h, first, last) {
        Ok(mut d) => {
            if let Some(old) = v.line_draft.take() {
                d.text = old.text;
            }
            v.line_draft = Some(d);
            v.tab = PrTab::Files;
            st.view.pr_line_comment_focus = true;
        }
        Err(why) => st.log_quiet(false, why),
    }
}

fn suggest(st: &mut AppState) {
    let Some(d) = st.session.pull_request.view.as_mut().and_then(|v| v.line_draft.as_mut()) else { return };
    if d.posting.is_some() || suggest_blocked(d).is_some() {
        return;
    }
    d.text = with_suggestion(&d.text, d);
    st.view.pr_line_comment_focus = true;
}

fn post(st: &mut AppState, review: bool, fx: &mut Vec<Effect>) {
    let Some(v) = st.session.pull_request.view.as_ref() else { return };
    if post_blocked(provider(st), v, review).is_some() {
        return;
    }
    if let Err(e) = service_request(st) {
        let text = format!("Commenting on {} failed: {e}", v.label);
        if let Some(d) = st.session.pull_request.view.as_mut().and_then(|v| v.line_draft.as_mut()) {
            d.error = Some(e);
        }
        st.log(false, text);
        return;
    }
    let (Some(d), Some(page)) = (v.line_draft.as_ref(), v.loaded()) else { return };
    let comment = NewComment { head_sha: d.head.clone(), anchor: d.anchor.clone(), body: d.text.clone() };
    let call = if review { PrCall::ReviewComment { comment, pending: page.detail.pending_review } } else { PrCall::LineComment(comment) };
    act(st, PrAction::LineComment, call, fx);
    if let Some(v) = st.session.pull_request.view.as_mut().filter(|v| v.busy == Some(PrAction::LineComment))
        && let Some(d) = v.line_draft.as_mut()
    {
        d.posting = Some(review);
        d.error = None;
    }
}

/// Cancel and Esc: an empty draft goes at once, an edited one after a confirmation.
fn cancel(st: &mut AppState) {
    let Some(v) = st.session.pull_request.view.as_mut() else { return };
    let Some(d) = v.line_draft.as_ref().filter(|d| d.posting.is_none()) else { return };
    if edited(d) {
        st.modal = Some(discard_confirm(d));
    } else {
        v.line_draft = None;
    }
}

/// The box's line link: scrolls the open diff to the draft's last line, or opens its file
/// there.
fn show(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(d) = st.session.pull_request.view.as_ref().and_then(|v| v.line_draft.as_ref()) else { return };
    let (target, side, n) = (d.target.clone(), d.anchor.side, d.anchor.line);
    match &st.session.diff.view {
        Some((open, diff)) if *open == target => {
            let (s, ex) = (&st.settings, &st.view.diff_expanded);
            if let Some(row) = row_of_file_line(diff, s.diff_mode, s.diff_layout, ex, side, n) {
                st.view.diff_scroll_to_row = Some(row);
                st.view.diff_top_row = row;
            }
        }
        _ if st.session.pull_request.view.as_ref().is_some_and(|v| v.shows_file(&target)) => {
            super::super::repo::open_file_diff(st, target.clone(), fx);
            st.view.diff_open_line = Some((target, side, n));
        }
        _ => {}
    }
}

/// Applies a post's answer to panel `v`: the new thread joins the page (open and focused) and
/// the draft goes, or the draft stays with the reason. Returns the new thread's id.
fn apply(v: &mut PrView, review: bool, result: &Result<NewThread, ForgeError>) -> Option<String> {
    if v.busy == Some(PrAction::LineComment) {
        v.busy = None;
    }
    match result {
        Ok(n) => {
            v.line_draft = None;
            let id = n.thread.id.clone();
            if let Page::Loaded(p) = &mut v.page {
                p.threads.push(n.thread.clone());
                if review && n.pending_review.is_some() {
                    p.detail.pending_review = n.pending_review;
                }
            }
            v.threads.open.insert(id.clone(), true);
            v.threads.focus = Some(id.clone());
            Some(id)
        }
        Err(e) => {
            if let Some(d) = v.line_draft.as_mut() {
                d.posting = None;
                d.error = Some(e.message.clone());
            }
            None
        }
    }
}

/// A new comment's answer for pull request `id` of `target` in `workdir` (`active`: the
/// active tab's repository and service). It goes to the open panel, else to the parked panel
/// of the tab showing `workdir`, and only while that panel's draft is being posted. Without
/// one it is logged and toasted.
#[allow(clippy::too_many_arguments)]
pub(super) fn replied(
    st: &mut AppState,
    workdir: &Path,
    target: &Target,
    id: u64,
    active: bool,
    label: &str,
    reply: PrReply,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    let (ok, text) = action_text(label, "", &reply);
    let PrReply::LineCommented { review, result } = reply else { return };
    let mine = |v: &&mut PrView| v.pr.id == id && v.line_draft.as_ref().is_some_and(|d| d.posting.is_some());
    let view = if active {
        st.session.pull_request.view.as_mut().filter(mine)
    } else {
        st.tabs
            .iter_mut()
            .find(|t| t.is(workdir))
            .and_then(|t| t.parked.as_mut())
            .filter(|p| p.forge.as_ref().and_then(|f| f.host.as_ref()) == Some(target))
            .and_then(|p| p.session.pull_request.view.as_mut())
            .filter(mine)
    };
    let found = view.map(|v| {
        let was_pending = v.loaded().is_some_and(|p| p.detail.pending_review.is_some());
        (was_pending, apply(v, review, &result))
    });
    if active {
        st.log(ok, text.clone());
    } else {
        st.log_in(Some(workdir), ok, text.clone());
    }
    match found {
        Some((was_pending, thread)) if active => {
            if let Some(thread) = thread {
                st.view.pr_thread_scroll = Some(thread);
                let toast = match (review, was_pending) {
                    (false, _) => "Comment posted",
                    (true, false) => "Review started. Finish it from Your review on Overview",
                    (true, true) => "Added to your review",
                };
                st.toast(true, toast, now);
            }
            // A single comment's thread gets its real id and rights from the page; a failed
            // review comment may have started a review before the thread was refused.
            if review == result.is_err() {
                load(st, fx);
            }
        }
        Some(_) => {}
        None => match &result {
            Ok(_) => st.toast(true, text, now),
            Err(e) => st.toast(false, format!("Couldn't post the comment on {label}: {}", e.message), now),
        },
    }
}
