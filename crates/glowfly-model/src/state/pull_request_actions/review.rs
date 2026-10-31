//! The review form's rules: its choices per service style, their labels, why each is
//! unavailable, and when a summary is needed.

use super::{MAX_BODY, not_open, ready, too_long};
use crate::state::pull_request::PrView;
use glowfly_core::forge::{PrDetail, ReviewEvent, ReviewStyle, Vote};

/// The review form's choices: GitHub's verdicts, GitLab's comment and approval (taken back
/// once given), or Bitbucket's comment, approval and change request (either taken back once
/// given).
pub fn review_choices(d: &PrDetail) -> Vec<ReviewEvent> {
    match d.style {
        ReviewStyle::Verdicts => vec![ReviewEvent::Comment, ReviewEvent::Approve, ReviewEvent::RequestChanges],
        ReviewStyle::Approvals { by_me: false, .. } => vec![ReviewEvent::Comment, ReviewEvent::Approve],
        ReviewStyle::Approvals { by_me: true, .. } => vec![ReviewEvent::Comment, ReviewEvent::Unapprove],
        ReviewStyle::Votes { mine: Vote::Neither } => vec![ReviewEvent::Comment, ReviewEvent::Approve, ReviewEvent::RequestChanges],
        ReviewStyle::Votes { mine: Vote::Approved } => vec![ReviewEvent::Comment, ReviewEvent::Unapprove],
        ReviewStyle::Votes { mine: Vote::ChangesRequested } => vec![ReviewEvent::Comment, ReviewEvent::UnrequestChanges],
    }
}

/// A review choice's label and the line under it.
pub fn review_choice_text(ev: ReviewEvent) -> (&'static str, &'static str) {
    match ev {
        ReviewEvent::Comment => ("Comment", "General feedback, without approving."),
        ReviewEvent::Approve => ("Approve", "Approve these changes for merging."),
        ReviewEvent::RequestChanges => ("Request changes", "Feedback that must be addressed before this can merge."),
        ReviewEvent::Unapprove => ("Remove approval", "Take back your approval of these changes."),
        ReviewEvent::UnrequestChanges => ("Remove change request", "Take back your request for changes."),
    }
}

/// Why review choice `ev` is unavailable: the author can't judge their own changes (unless
/// GitLab's approval rules let them), an approval or change request is given or taken back
/// once, and only an open pull request is reviewed.
pub fn review_choice_blocked(v: &PrView, ev: ReviewEvent) -> Option<String> {
    let d = &v.loaded()?.detail;
    let own = || format!("You opened this {}, so you can't approve it.", v.noun);
    let own_changes = || format!("You opened this {}, so you can't request changes on it.", v.noun);
    match d.style {
        ReviewStyle::Verdicts => match ev {
            ReviewEvent::Approve if d.is_author() => return Some(own()),
            ReviewEvent::RequestChanges if d.is_author() => return Some(own_changes()),
            ReviewEvent::Unapprove => return Some("Approvals can't be taken back on this service".to_owned()),
            ReviewEvent::UnrequestChanges => return Some("Change requests can't be taken back on this service".to_owned()),
            _ => {}
        },
        ReviewStyle::Approvals { by_me, can_approve } => match ev {
            ReviewEvent::Approve if by_me => return Some("You've approved this already".to_owned()),
            ReviewEvent::Approve if can_approve == Some(false) => {
                return Some(if d.is_author() { own() } else { "The project's approval rules don't let you approve this".to_owned() });
            }
            ReviewEvent::Approve if can_approve.is_none() && d.is_author() => return Some(own()),
            ReviewEvent::Unapprove if !by_me => return Some("You haven't approved this".to_owned()),
            ReviewEvent::RequestChanges | ReviewEvent::UnrequestChanges => return Some("GitLab has no Request changes".to_owned()),
            _ => {}
        },
        ReviewStyle::Votes { mine } => match ev {
            ReviewEvent::Approve if d.is_author() => return Some(own()),
            ReviewEvent::RequestChanges if d.is_author() => return Some(own_changes()),
            ReviewEvent::Approve if mine == Vote::Approved => return Some("You've approved this already".to_owned()),
            ReviewEvent::RequestChanges if mine == Vote::ChangesRequested => {
                return Some("You've requested changes already".to_owned());
            }
            ReviewEvent::Unapprove if mine != Vote::Approved => return Some("You haven't approved this".to_owned()),
            ReviewEvent::UnrequestChanges if mine != Vote::ChangesRequested => {
                return Some("You haven't requested changes on this".to_owned());
            }
            _ => {}
        },
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
    let votes = matches!(page.detail.style, ReviewStyle::Votes { .. });
    let optional = matches!(ev, ReviewEvent::Approve | ReviewEvent::Unapprove | ReviewEvent::UnrequestChanges)
        || (ev == ReviewEvent::RequestChanges && votes)
        || finishing;
    if v.forms.review_body.trim().is_empty() && !optional {
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
