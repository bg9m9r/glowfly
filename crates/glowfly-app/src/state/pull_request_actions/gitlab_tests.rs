//! The review and merge forms' rules for a GitLab merge request (approvals, the project's
//! merge methods), and that GitHub's stay as they were.

use super::tests::{detail, view};
use super::*;

fn approvals(by_me: bool, can_approve: Option<bool>) -> ReviewStyle {
    ReviewStyle::Approvals { by_me, can_approve }
}

/// A loaded merge request in GitLab's style, as `f` changes it.
fn mr(f: impl FnOnce(&mut PrDetail)) -> PrView {
    let mut d = detail();
    d.style = approvals(false, Some(true));
    d.methods = vec![MergeMethod::Merge, MergeMethod::Squash];
    f(&mut d);
    let mut v = view(Some(d));
    v.noun = "merge request";
    v.label = "!12".into();
    v
}

fn blocked(v: &PrView, ev: ReviewEvent) -> Option<String> {
    review_choice_blocked(v, ev)
}

#[test]
fn review_choices_follow_the_style() {
    use ReviewEvent::*;
    assert_eq!(review_choices(&detail()), [Comment, Approve, RequestChanges]);
    let mut d = detail();
    d.style = approvals(false, None);
    assert_eq!(review_choices(&d), [Comment, Approve]);
    d.style = approvals(true, None);
    assert_eq!(review_choices(&d), [Comment, Unapprove]);
    assert_eq!(review_choice_text(Unapprove), ("Remove approval", "Take back your approval of these changes."));
    assert_eq!(review_choice_text(Approve).0, "Approve");
}

#[test]
fn approval_choice_reasons() {
    let v = mr(|_| {});
    assert_eq!(blocked(&v, ReviewEvent::Approve), None);
    assert_eq!(blocked(&v, ReviewEvent::Unapprove).as_deref(), Some("You haven't approved this"));
    assert_eq!(blocked(&v, ReviewEvent::RequestChanges).as_deref(), Some("GitLab has no Request changes"));
    assert_eq!(blocked(&v, ReviewEvent::UnrequestChanges).as_deref(), Some("GitLab has no Request changes"));
    let v = mr(|d| d.style = approvals(true, Some(true)));
    assert_eq!(blocked(&v, ReviewEvent::Approve).as_deref(), Some("You've approved this already"));
    assert_eq!(blocked(&v, ReviewEvent::Unapprove), None);
    let v = mr(|d| d.style = approvals(false, Some(false)));
    assert_eq!(blocked(&v, ReviewEvent::Approve).as_deref(), Some("The project's approval rules don't let you approve this"));
    let v = mr(|d| {
        d.style = approvals(false, Some(false));
        d.me = "ada".into();
    });
    assert_eq!(blocked(&v, ReviewEvent::Approve).as_deref(), Some("You opened this merge request, so you can't approve it."));
    let v = mr(|d| {
        d.style = approvals(false, None);
        d.me = "ada".into();
    });
    assert_eq!(blocked(&v, ReviewEvent::Approve).as_deref(), Some("You opened this merge request, so you can't approve it."));
    let v = mr(|d| d.me = "ada".into());
    assert_eq!(blocked(&v, ReviewEvent::Approve), None, "the project lets its author approve");
    let v = mr(|d| d.state = PrState::Merged);
    assert_eq!(blocked(&v, ReviewEvent::Comment).as_deref(), Some("This merge request is merged"));
}

#[test]
fn approvals_need_no_summary_but_comments_do() {
    let mut v = mr(|_| {});
    v.forms.review_event = ReviewEvent::Approve;
    assert_eq!(review_blocked(&v), None);
    v.forms.review_event = ReviewEvent::Comment;
    assert_eq!(review_blocked(&v).as_deref(), Some("Write a summary for this choice"));
    let mut v = mr(|d| d.style = approvals(true, None));
    v.forms.review_event = ReviewEvent::Unapprove;
    assert_eq!(review_blocked(&v), None);
    assert_eq!(review_button(&v), "Submit review");
}

#[test]
fn prefill_keeps_to_the_listed_choices() {
    let mut forms = PrForms { review_event: ReviewEvent::Approve, ..PrForms::default() };
    let mut d = detail();
    d.style = approvals(true, None);
    d.methods = vec![MergeMethod::Squash, MergeMethod::Merge];
    prefill(&mut forms, &d);
    assert_eq!((forms.review_event, forms.merge_method), (ReviewEvent::Comment, Some(MergeMethod::Squash)));
    forms.review_event = ReviewEvent::Unapprove;
    prefill(&mut forms, &d);
    assert_eq!(forms.review_event, ReviewEvent::Unapprove, "still approved");
    d.style = approvals(false, None);
    prefill(&mut forms, &d);
    assert_eq!(forms.review_event, ReviewEvent::Comment);
    forms.review_event = ReviewEvent::RequestChanges;
    prefill(&mut forms, &d);
    assert_eq!(forms.review_event, ReviewEvent::Comment);
    forms.review_event = ReviewEvent::Approve;
    d.me = "ada".into();
    prefill(&mut forms, &d);
    assert_eq!(forms.review_event, ReviewEvent::Approve, "GitLab's rules decide whether its author may approve");
    let mut github = detail();
    github.me = "ada".into();
    prefill(&mut forms, &github);
    assert_eq!(forms.review_event, ReviewEvent::Comment, "GitHub's author can't approve");
}

#[test]
fn merge_choices_and_texts() {
    let mut d = detail();
    d.methods = vec![MergeMethod::Squash];
    assert_eq!(merge_choices(&d), MergeMethod::GITHUB, "GitHub lists every method");
    d.style = approvals(false, None);
    d.methods = vec![MergeMethod::FastForward, MergeMethod::Squash];
    assert_eq!(merge_choices(&d), [MergeMethod::FastForward, MergeMethod::Squash]);
    assert_eq!(method_label(MergeMethod::FastForward), "Fast-forward");
    assert_eq!(
        method_help(MergeMethod::FastForward, "main"),
        "main moves forward to the source branch's last commit, with no merge commit."
    );
    assert_eq!(merge_button(&mr(|_| {})), "Merge");
    assert_eq!(merge_button(&view(Some(detail()))), "Merge pull request");
    assert_eq!(verdict_text(&EntryKind::Review(Verdict::ApprovalRemoved)), ("removed their approval", Tone::Neutral));
}

#[test]
fn fast_forward_confirmation() {
    let mut v = mr(|d| d.methods = vec![MergeMethod::FastForward]);
    v.forms.merge_method = Some(MergeMethod::FastForward);
    let Msg::Confirm { title, body, .. } = merge_confirm(&v, None) else { panic!("a confirmation") };
    assert!(body.contains("into main by a fast-forward?"), "{body}");
    assert_eq!(title, "Merge");
}
