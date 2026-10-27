//! A new code comment on lines of a pull request file's diff: the draft (where it is, its text
//! and whether it is being posted), why one can't be started, posted or given a suggestion, the
//! texts that describe it, what the diff marks for it, and carrying a single comment's
//! provisional thread over to the one the reloaded page has. Pure; the calls are made by
//! update/pull_request/line_comment.rs.

use super::AppState;
use super::Modal;
use super::pull_request::PrView;
use super::pull_request_actions::{MAX_BODY, WAITING, ready};
use super::pull_request_threads::{Placement, placement};
use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::pull_request::line_comment::LineCommentMsg;
use glowfly_core::diff_rows::{LineSide, line_index, line_keys};
use glowfly_core::forge::{
    LineAnchor, PrPage, Provider, line_anchor, line_comments_in_app, line_key, new_thread_comment, suggestion_block,
};
use glowfly_core::repo::{DiffLine, DiffTarget, FileDiff};
use glowfly_core::text::printable;
use std::collections::HashSet;

pub const OUTSIDE: &str = "This line is outside the changes, so the service can't take a comment on it";
pub const ONE_HUNK: &str = "A comment can cover lines of one hunk only";
const UNSUPPORTED: &str = "Comments on lines aren't supported for this service in Glowfly yet";
const LOCKED: &str = "The conversation is locked on the service";
const MOVED: &str = "The changes were compared again since this comment was started; pick its line again";
const PENDING: &str = "You have a review in progress: add this to it with Add review comment, or finish it first";
const EMPTY: &str = "Write a comment first";
const TOO_LONG: &str = "Comments can have at most 65,536 characters";
const DELETED: &str = "Suggestions replace lines of the new version; this comment starts or ends on a deleted line";
const NOTHING: &str = "These lines can't take a comment";

/// The unposted comment of the open pull request panel (one at a time).
#[derive(Clone, Debug, PartialEq)]
pub struct LineDraft {
    /// The file diff (merge base to source tip) it was started on.
    pub target: DiffTarget,
    /// The source tip the lines belong to, in full hex.
    pub head: String,
    /// The hunk and the span of its lines (indexes into the normal hunks).
    pub hunk: usize,
    pub first: usize,
    pub last: usize,
    pub anchor: LineAnchor,
    /// The spanned lines, for the snippet, the suggestion and the diff's marks.
    pub lines: Vec<DiffLine>,
    pub text: String,
    /// Some(review) while it is being posted (alone or to the pending review).
    pub posting: Option<bool>,
    /// The last post's failure, in the service's words.
    pub error: Option<String>,
}

/// A draft on lines `first..=last` of hunk `hunk` of `diff` (the normal hunks of `target`,
/// which the panel must show), its text empty.
pub fn draft_at(v: &PrView, target: &DiffTarget, diff: &FileDiff, hunk: usize, first: usize, last: usize) -> Result<LineDraft, String> {
    let (true, Some(head)) = (v.shows_file(target), v.source_tip()) else { return Err(NOTHING.to_owned()) };
    let h = diff.hunks.get(hunk).ok_or(NOTHING)?;
    let anchor = line_anchor(target.path(), h, first, last).ok_or(NOTHING)?;
    let end = h.lines.len() - 1;
    let (a, b) = (first.min(last).min(end), first.max(last).min(end));
    let commentable = |i: &usize| line_key(&h.lines[*i]).is_some();
    let (Some(first), Some(last)) = ((a..=b).find(commentable), (a..=b).rev().find(commentable)) else { return Err(NOTHING.to_owned()) };
    Ok(LineDraft {
        target: target.clone(),
        head: head.to_string(),
        hunk,
        first,
        last,
        anchor,
        lines: h.lines[first..=last].to_vec(),
        text: String::new(),
        posting: None,
        error: None,
    })
}

/// Why line `dl` (of the whole-file view) can't take a comment: none of the normal hunks shows
/// it, so it is outside the service's diff.
pub fn line_blocked(normal: &FileDiff, dl: &DiffLine) -> Option<&'static str> {
    let (side, n) = line_key(dl)?;
    line_index(normal, side, n).is_none().then_some(OUTSIDE)
}

/// Why no comment can start on the panel's diff lines; the first reason that applies.
pub fn start_blocked(provider: Option<Provider>, v: &PrView) -> Option<String> {
    if !provider.is_some_and(line_comments_in_app) {
        return Some(UNSUPPORTED.to_owned());
    }
    let page = match ready(v) {
        Ok(p) => p,
        Err(why) => return Some(why),
    };
    if v.line_draft.as_ref().is_some_and(|d| d.posting.is_some()) {
        return Some(WAITING.to_owned());
    }
    if page.detail.locked {
        return Some(LOCKED.to_owned());
    }
    if placement(v) != Placement::OnLines {
        let head = &page.detail.head_sha;
        return Some(format!("The service's head is {}; fetch {} to comment on its lines", head.get(..7).unwrap_or(head), v.remote));
    }
    None
}

/// Why the draft can't be posted alone (`review` false) or to the pending review.
pub fn post_blocked(provider: Option<Provider>, v: &PrView, review: bool) -> Option<String> {
    if let Some(why) = start_blocked(provider, v) {
        return Some(why);
    }
    let d = v.line_draft.as_ref()?;
    let pending = v.loaded().is_some_and(|p| p.detail.pending_review.is_some());
    let why = if v.source_tip().is_none_or(|s| s.to_string() != d.head) {
        MOVED
    } else if !review && pending {
        PENDING
    } else if d.text.trim().is_empty() {
        EMPTY
    } else if d.text.chars().count() > MAX_BODY {
        TOO_LONG
    } else {
        return None;
    };
    Some(why.to_owned())
}

/// Why Suggest a change is unavailable: a suggestion replaces new-version lines only.
pub fn suggest_blocked(d: &LineDraft) -> Option<&'static str> {
    let left = d.anchor.side == LineSide::Old || d.anchor.start.is_some_and(|(_, s)| s == LineSide::Old);
    left.then_some(DELETED)
}

/// `text` with a suggestion block of the draft's new-version lines at its end, after a blank
/// line when the text doesn't end a line.
pub fn with_suggestion(text: &str, d: &LineDraft) -> String {
    let lines: Vec<&str> = d.lines.iter().filter(|l| matches!(l.origin, '+' | ' ')).map(|l| l.text.as_str()).collect();
    let block = suggestion_block(&lines);
    if text.is_empty() || text.ends_with('\n') { format!("{text}{block}") } else { format!("{text}\n\n{block}") }
}

/// Where the comment goes: "line 42", "lines 40–42", "deleted line 12", "deleted lines 3–4",
/// or "deleted line 3 to line 5" across the two versions.
pub fn where_text(a: &LineAnchor) -> String {
    let one = |side: LineSide, n: u32| if side == LineSide::Old { format!("deleted line {n}") } else { format!("line {n}") };
    match a.start {
        None => one(a.side, a.line),
        Some((s, side)) if side == a.side => {
            let deleted = if side == LineSide::Old { "deleted " } else { "" };
            format!("{deleted}lines {s}\u{2013}{}", a.line)
        }
        Some((s, side)) => format!("{} to {}", one(side, s), one(a.side, a.line)),
    }
}

pub fn review_label(v: &PrView) -> &'static str {
    if primary_is_review(v) { "Add review comment" } else { "Start a review" }
}

/// Whether the box's main action (Command+Enter) adds to the pending review.
pub fn primary_is_review(v: &PrView) -> bool {
    v.loaded().is_some_and(|p| p.detail.pending_review.is_some())
}

/// Whether dropping the draft loses something the user wrote.
pub fn edited(d: &LineDraft) -> bool {
    !d.text.trim().is_empty()
}

pub fn discard_confirm(d: &LineDraft) -> Modal {
    let body = format!("Discard your comment on {} of {}? It hasn't been posted.", where_text(&d.anchor), printable(&d.anchor.path));
    Modal::danger("Discard comment", body, "Discard", Msg::PullRequest(PullRequestMsg::LineComment(LineCommentMsg::Discard)))
}

/// What a pull request file's diff draws for new comments.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiffComments {
    /// Why no comment can start (every button is disabled with it).
    pub blocked: Option<String>,
    /// Every file line of the draft's lines.
    pub draft: HashSet<(LineSide, u32)>,
    /// The draft's last line, which carries its marker.
    pub end: Option<(LineSide, u32)>,
    /// The normal hunk the draft is in.
    pub draft_hunk: Option<usize>,
}

/// Some for the open panel's file diff `target` on a service that takes line comments.
pub fn diff_comments(st: &AppState, target: &DiffTarget) -> Option<DiffComments> {
    let v = st.session.pull_request.view.as_ref().filter(|v| v.shows_file(target))?;
    let provider = st.session.forge.as_ref().and_then(|f| f.host_provider()).filter(|p| line_comments_in_app(*p))?;
    let mut out = DiffComments { blocked: start_blocked(Some(provider), v), ..Default::default() };
    if let Some(d) = v.line_draft.as_ref().filter(|d| d.target == *target) {
        out.draft = d.lines.iter().flat_map(line_keys).flatten().collect();
        out.end = Some((d.anchor.side, d.anchor.line));
        out.draft_hunk = Some(d.hunk);
    }
    Some(out)
}

/// Whether Esc should go to the panel's draft first.
pub fn esc_cancels(st: &AppState) -> bool {
    st.session.pull_request.view.as_ref().is_some_and(|v| v.line_draft.is_some())
}

/// Whether the panel's draft is on `path` (its comment box shows in that file's pane).
pub fn draft_on(v: &PrView, path: &str) -> bool {
    v.line_draft.as_ref().is_some_and(|d| d.anchor.path == path)
}

/// The page `page` replaces the loaded one: a single comment's provisional thread
/// (`new_thread_id`) hands its reply draft, open state and focus to the page's thread that
/// starts with the same comment.
pub fn adopt_new_threads(v: &mut PrView, page: &PrPage) {
    let real = |id: &str| {
        let c = new_thread_comment(id)?;
        page.threads.iter().find(|t| t.reply_to() == Some(c)).map(|t| t.id.clone())
    };
    let drafts: Vec<String> = v.forms.replies.keys().filter(|k| real(k).is_some()).cloned().collect();
    for old in drafts {
        if let (Some(new), Some(text)) = (real(&old), v.forms.replies.remove(&old)) {
            v.forms.replies.insert(new, text);
        }
    }
    let open: Vec<String> = v.threads.open.keys().filter(|k| real(k).is_some()).cloned().collect();
    for old in open {
        if let (Some(new), Some(o)) = (real(&old), v.threads.open.remove(&old)) {
            v.threads.open.insert(new, o);
        }
    }
    if let Some(new) = v.threads.focus.as_deref().and_then(real) {
        v.threads.focus = Some(new);
    }
}

#[cfg(test)]
mod tests;
