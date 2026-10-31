//! The review and merge forms' rules for a Bitbucket pull request (votes taken back once
//! given, only the repository's merge strategies), and that GitHub's stay as they were.

use super::tests::{detail, view};
use super::*;
use glowfly_core::forge::Vote;

fn votes(mine: Vote) -> ReviewStyle {
    ReviewStyle::Votes { mine }
}

/// A loaded Bitbucket pull request, as `f` changes it.
fn bb(f: impl FnOnce(&mut PrDetail)) -> PrView {
    let mut d = detail();
    d.style = votes(Vote::Neither);
    d.methods = vec![MergeMethod::Merge, MergeMethod::Squash, MergeMethod::FastForward];
    f(&mut d);
    view(Some(d))
}

fn blocked(v: &PrView, ev: ReviewEvent) -> Option<String> {
    review_choice_blocked(v, ev)
}

#[test]
fn vote_choices_follow_my_vote() {
    use ReviewEvent::*;
    let mut d = detail();
    d.style = votes(Vote::Neither);
    assert_eq!(review_choices(&d), [Comment, Approve, RequestChanges]);
    d.style = votes(Vote::Approved);
    assert_eq!(review_choices(&d), [Comment, Unapprove]);
    d.style = votes(Vote::ChangesRequested);
    assert_eq!(review_choices(&d), [Comment, UnrequestChanges]);
    assert_eq!(review_choice_text(UnrequestChanges), ("Remove change request", "Take back your request for changes."));
}

#[test]
fn vote_choice_reasons() {
    let v = bb(|_| {});
    assert_eq!(blocked(&v, ReviewEvent::Approve), None);
    assert_eq!(blocked(&v, ReviewEvent::RequestChanges), None);
    assert_eq!(blocked(&v, ReviewEvent::Unapprove).as_deref(), Some("You haven't approved this"));
    assert_eq!(blocked(&v, ReviewEvent::UnrequestChanges).as_deref(), Some("You haven't requested changes on this"));
    let v = bb(|d| d.me = "ada".into());
    assert_eq!(blocked(&v, ReviewEvent::Approve).as_deref(), Some("You opened this pull request, so you can't approve it."));
    assert_eq!(
        blocked(&v, ReviewEvent::RequestChanges).as_deref(),
        Some("You opened this pull request, so you can't request changes on it.")
    );
    assert_eq!(blocked(&v, ReviewEvent::Comment), None, "the author can still comment");
    let v = bb(|d| d.style = votes(Vote::Approved));
    assert_eq!(blocked(&v, ReviewEvent::Approve).as_deref(), Some("You've approved this already"));
    assert_eq!(blocked(&v, ReviewEvent::Unapprove), None);
    let v = bb(|d| d.style = votes(Vote::ChangesRequested));
    assert_eq!(blocked(&v, ReviewEvent::RequestChanges).as_deref(), Some("You've requested changes already"));
    assert_eq!(blocked(&v, ReviewEvent::UnrequestChanges), None);
    let v = bb(|d| d.state = PrState::Merged);
    assert_eq!(blocked(&v, ReviewEvent::Comment).as_deref(), Some("This pull request is merged"));
    let github = view(Some(detail()));
    assert_eq!(blocked(&github, ReviewEvent::UnrequestChanges).as_deref(), Some("Change requests can't be taken back on this service"));
}

#[test]
fn request_changes_needs_no_summary_on_bitbucket() {
    let mut v = bb(|_| {});
    v.forms.review_event = ReviewEvent::RequestChanges;
    assert_eq!(review_blocked(&v), None);
    v.forms.review_event = ReviewEvent::Comment;
    assert_eq!(review_blocked(&v).as_deref(), Some("Write a summary for this choice"));
    let mut v = bb(|d| d.style = votes(Vote::ChangesRequested));
    v.forms.review_event = ReviewEvent::UnrequestChanges;
    assert_eq!(review_blocked(&v), None);
    let mut github = view(Some(detail()));
    github.forms.review_event = ReviewEvent::RequestChanges;
    assert_eq!(review_blocked(&github).as_deref(), Some("Write a summary for this choice"), "GitHub still asks for one");
}

#[test]
fn prefill_moves_the_author_to_comment() {
    let mut forms = PrForms { review_event: ReviewEvent::Approve, ..PrForms::default() };
    let mut d = detail();
    d.style = votes(Vote::Neither);
    d.methods = vec![MergeMethod::FastForward, MergeMethod::Merge];
    prefill(&mut forms, &d, None);
    assert_eq!((forms.review_event, forms.merge_method), (ReviewEvent::Approve, Some(MergeMethod::FastForward)));
    d.me = "ada".into();
    prefill(&mut forms, &d, None);
    assert_eq!(forms.review_event, ReviewEvent::Comment, "the author can't approve");
    d.me = "grace".into();
    d.style = votes(Vote::ChangesRequested);
    forms.review_event = ReviewEvent::UnrequestChanges;
    prefill(&mut forms, &d, None);
    assert_eq!(forms.review_event, ReviewEvent::UnrequestChanges, "changes are still requested");
    d.style = votes(Vote::Neither);
    prefill(&mut forms, &d, None);
    assert_eq!(forms.review_event, ReviewEvent::Comment, "nothing to take back any more");
}

#[test]
fn merge_choices_are_the_repositorys() {
    let mut d = detail();
    d.style = votes(Vote::Neither);
    d.methods = vec![MergeMethod::Rebase, MergeMethod::Squash];
    assert_eq!(merge_choices(&d), [MergeMethod::Rebase, MergeMethod::Squash]);
}

#[test]
fn no_usable_strategy_blocks_merging() {
    let v = bb(|d| d.methods.clear());
    assert_eq!(v.forms.merge_method, None);
    assert_eq!(merge_blocked(&v).as_deref(), Some(NO_METHODS), "before asking to choose a method");
    let mut v = bb(|_| {});
    v.forms.merge_method = Some(MergeMethod::Squash);
    assert_eq!(merge_blocked(&v), None);
}
