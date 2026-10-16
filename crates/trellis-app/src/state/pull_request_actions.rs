//! The pull request panel's action forms (comment, review, merge) and their rules: why each
//! action or choice is unavailable, the labels and explanations, the merge confirmation and
//! the post-merge branch deletion. Pure; the calls are made by update/pull_request_calls.rs.

use super::pull_request::{PrAction, PrView, Tone};
use crate::msg::Msg;
use trellis_core::forge::{EntryKind, MergeMethod, MergeState, PrDetail, PrPage, PrState, ReviewEvent, Verdict};
use trellis_core::repo::Snapshot;

/// The most characters a comment, review summary or merge message may have.
pub const MAX_BODY: usize = 65_536;
/// The most characters a merge commit title may have.
pub const MAX_TITLE: usize = 1_024;

const WAITING: &str = "Waiting for the service…";

/// What the user has typed and chosen in the panel's forms.
#[derive(Clone, Debug, PartialEq)]
pub struct PrForms {
    pub comment: String,
    pub review_body: String,
    pub review_event: ReviewEvent,
    pub merge_method: Option<MergeMethod>,
    pub merge_title: String,
    pub merge_message: String,
    pub delete_branch: bool,
}

impl Default for PrForms {
    fn default() -> Self {
        PrForms {
            comment: String::new(),
            review_body: String::new(),
            review_event: ReviewEvent::Comment,
            merge_method: None,
            merge_title: String::new(),
            merge_message: String::new(),
            delete_branch: false,
        }
    }
}

fn too_long(s: &str, max: usize) -> bool {
    s.chars().count() > max
}

/// The page, or why it can't be acted on yet (not loaded, or another action in flight).
fn ready(v: &PrView) -> Result<&PrPage, String> {
    let Some(page) = v.loaded() else { return Err(format!("The {} hasn't loaded yet", v.noun)) };
    if v.busy.is_some() {
        return Err(WAITING.to_owned());
    }
    Ok(page)
}

/// "This pull request is merged" or "…is closed"; None while it is open.
fn not_open(v: &PrView, d: &PrDetail) -> Option<String> {
    match d.state {
        PrState::Open => None,
        PrState::Merged => Some(format!("This {} is merged", v.noun)),
        PrState::Closed => Some(format!("This {} is closed", v.noun)),
    }
}

/// Why the Comment button is disabled.
pub fn comment_blocked(v: &PrView) -> Option<String> {
    let page = match ready(v) {
        Ok(p) => p,
        Err(why) => return Some(why),
    };
    let text = v.forms.comment.trim();
    if page.detail.locked {
        Some("The conversation is locked on the service".to_owned())
    } else if text.is_empty() {
        Some("Write a comment first".to_owned())
    } else if too_long(&v.forms.comment, MAX_BODY) {
        Some("Comments can have at most 65,536 characters".to_owned())
    } else {
        None
    }
}

/// Why review choice `ev` is unavailable: the author can't judge their own changes, and only
/// an open pull request is reviewed.
pub fn review_choice_blocked(v: &PrView, ev: ReviewEvent) -> Option<String> {
    let d = &v.loaded()?.detail;
    if d.is_author() {
        match ev {
            ReviewEvent::Approve => return Some(format!("You opened this {}, so you can't approve it.", v.noun)),
            ReviewEvent::RequestChanges => return Some(format!("You opened this {}, so you can't request changes on it.", v.noun)),
            ReviewEvent::Comment => {}
        }
    }
    not_open(v, d)
}

/// Why the Submit review (Finish review) button is disabled.
pub fn review_blocked(v: &PrView) -> Option<String> {
    let page = match ready(v) {
        Ok(p) => p,
        Err(why) => return Some(why),
    };
    let ev = v.forms.review_event;
    if let Some(why) = review_choice_blocked(v, ev) {
        return Some(why);
    }
    let finishing = page.detail.pending_review.is_some() && ev == ReviewEvent::Comment;
    if v.forms.review_body.trim().is_empty() && ev != ReviewEvent::Approve && !finishing {
        Some("Write a summary for this choice".to_owned())
    } else if too_long(&v.forms.review_body, MAX_BODY) {
        Some("A summary can have at most 65,536 characters".to_owned())
    } else {
        None
    }
}

pub fn review_button(v: &PrView) -> &'static str {
    if v.loaded().is_some_and(|p| p.detail.pending_review.is_some()) { "Finish review" } else { "Submit review" }
}

/// The line above the merge form: whether the service would merge it now, and its tone.
/// None unless the page is loaded and the pull request open.
pub fn mergeability(v: &PrView) -> Option<(Tone, String)> {
    let d = &v.loaded()?.detail;
    if d.state != PrState::Open {
        return None;
    }
    let (source, target) = (&v.pr.source, &v.pr.target);
    Some(if d.draft {
        (Tone::Warn, "Drafts can't be merged; mark it ready on the service".to_owned())
    } else if d.merge_state == MergeState::Conflicts {
        (Tone::Bad, format!("Conflicts with {target} must be resolved first"))
    } else if d.can_merge == Some(false) {
        (Tone::Neutral, "Only people with write access can merge".to_owned())
    } else {
        match d.merge_state {
            MergeState::Clean => (Tone::Good, format!("No conflicts with {target}")),
            MergeState::Conflicts => (Tone::Bad, format!("Conflicts with {target} must be resolved first")),
            MergeState::Blocked => (Tone::Warn, "The repository's rules block merging (required reviews or checks)".to_owned()),
            MergeState::Behind => (Tone::Warn, format!("{source} is behind {target}; the service may require updating it")),
            MergeState::Unstable => (Tone::Warn, "Some checks haven't passed".to_owned()),
            MergeState::Unknown => (Tone::Neutral, "The service is still checking whether this can merge".to_owned()),
        }
    })
}

/// Why merge method `m` can't be chosen.
pub fn method_blocked(d: &PrDetail, m: MergeMethod) -> Option<&'static str> {
    (!d.methods.contains(&m)).then_some("Not enabled in this repository's settings")
}

pub fn method_label(m: MergeMethod) -> &'static str {
    match m {
        MergeMethod::Merge => "Merge commit",
        MergeMethod::Squash => "Squash and merge",
        MergeMethod::Rebase => "Rebase and merge",
    }
}

/// One line on what merge method `m` does to `target`.
pub fn method_help(m: MergeMethod, target: &str) -> String {
    match m {
        MergeMethod::Merge => format!("All commits are added to {target} through a merge commit."),
        MergeMethod::Squash => format!("The commits are combined into one commit on {target}."),
        MergeMethod::Rebase => format!("Each commit is replayed onto {target}, with no merge commit."),
    }
}

/// Why the Merge button is disabled; the first reason that applies.
pub fn merge_blocked(v: &PrView) -> Option<String> {
    let page = match ready(v) {
        Ok(p) => p,
        Err(why) => return Some(why),
    };
    let d = &page.detail;
    if let Some(why) = not_open(v, d) {
        return Some(why);
    }
    if d.draft {
        return Some("Drafts can't be merged; mark it ready on the service".to_owned());
    }
    match d.merge_state {
        MergeState::Conflicts => return Some(format!("Conflicts with {} must be resolved first", v.pr.target)),
        MergeState::Blocked => return Some("The repository's rules block merging (required reviews or checks)".to_owned()),
        _ => {}
    }
    if d.can_merge == Some(false) {
        return Some("Only people with write access can merge".to_owned());
    }
    match v.forms.merge_method {
        None => return Some("Choose how to merge".to_owned()),
        Some(m) => {
            if let Some(why) = method_blocked(d, m) {
                return Some(why.to_owned());
            }
        }
    }
    if too_long(&v.forms.merge_title, MAX_TITLE) || too_long(&v.forms.merge_message, MAX_BODY) {
        return Some("The commit title or message is too long".to_owned());
    }
    None
}

/// Why the source branch can't be deleted along with the merge.
pub fn delete_after_merge_blocked(v: &PrView, default_branch: Option<&str>) -> Option<String> {
    let Some(page) = v.loaded() else { return Some(format!("The {} hasn't loaded yet", v.noun)) };
    let d = &page.detail;
    if !d.same_repo {
        Some("The source branch is on a fork".to_owned())
    } else if default_branch == Some(v.pr.source.as_str()) {
        Some(format!("{} is the repository's default branch", v.pr.source))
    } else if d.deletes_merged_branches {
        Some("The repository deletes merged branches itself".to_owned())
    } else {
        None
    }
}

/// Whether the merge will delete the source branch: asked for, and allowed.
pub fn deletes_after_merge(v: &PrView, default_branch: Option<&str>) -> bool {
    v.forms.delete_branch && delete_after_merge_blocked(v, default_branch).is_none()
}

/// The confirmation the Merge button opens; accepting it sends `PullRequestMsg::Merge`.
pub fn merge_confirm(v: &PrView, default_branch: Option<&str>) -> Msg {
    let how = match v.forms.merge_method {
        Some(MergeMethod::Squash) => "squashing its commits",
        Some(MergeMethod::Rebase) => "rebasing its commits",
        _ => "a merge commit",
    };
    let title = trellis_core::text::printable(&v.pr.title);
    let mut body = format!("Merge {} \u{201c}{title}\u{201d} into {} by {how}?", v.label, v.pr.target);
    if deletes_after_merge(v, default_branch) {
        body.push_str(&format!(" Then delete {} on {}.", v.pr.source, v.remote));
    }
    Msg::confirm("Merge pull request", body, "Merge", Msg::PullRequest(crate::pull_request::PullRequestMsg::Merge))
}

/// After a merge: the Delete button's confirmation, when the source branch is still on the
/// remote, in the same repository, not the default branch, and the repository doesn't delete
/// merged branches itself.
pub fn delete_branch_offer(v: &PrView, snap: &Snapshot, default_branch: Option<&str>) -> Option<Msg> {
    if v.state() != Some(PrState::Merged) || delete_after_merge_blocked(v, default_branch).is_some() {
        return None;
    }
    super::pull_request::Tips::of(snap, &v.remote, &v.pr).source?;
    let (remote, source) = (v.remote.as_str(), v.pr.source.as_str());
    Some(Msg::confirm(
        "Delete remote branch",
        format!("Delete branch {source} on {remote}? This affects everyone using the remote."),
        "Delete",
        Msg::git("Delete remote branch", &["push", remote, "--delete", source]),
    ))
}

/// What an entry's author did, and its tone.
pub fn verdict_text(kind: &EntryKind) -> (&'static str, Tone) {
    match kind {
        EntryKind::Comment => ("commented", Tone::Neutral),
        EntryKind::Review(Verdict::Approved) => ("approved", Tone::Good),
        EntryKind::Review(Verdict::ChangesRequested) => ("requested changes", Tone::Bad),
        EntryKind::Review(Verdict::Commented) => ("reviewed", Tone::Neutral),
        EntryKind::Review(Verdict::Dismissed) => ("review dismissed", Tone::Neutral),
    }
}

/// Fits the forms to a freshly loaded page: a merge method the repository allows, and no
/// verdict the author can't give.
pub fn prefill(forms: &mut PrForms, d: &PrDetail) {
    if forms.merge_method.is_none_or(|m| !d.methods.contains(&m)) {
        forms.merge_method = d.methods.first().copied();
    }
    if d.is_author() {
        forms.review_event = ReviewEvent::Comment;
    }
}

/// Which action a failure belongs to, for its place in the panel.
pub fn error_for(v: &PrView, action: PrAction) -> Option<&str> {
    v.action_error.as_ref().filter(|(a, _)| *a == action).map(|(_, e)| e.as_str())
}

#[cfg(test)]
mod tests;
