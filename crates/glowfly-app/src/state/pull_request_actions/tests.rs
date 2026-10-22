use super::*;
use crate::state::pull_request::{Files, Page, PrTab, Tips};
use glowfly_core::forge::{PrPage, PullRequest};

pub(crate) fn detail() -> PrDetail {
    PrDetail {
        state: PrState::Open,
        draft: false,
        locked: false,
        mergeable: Some(true),
        merge_state: MergeState::Clean,
        head_sha: "h1".into(),
        base_sha: "b1".into(),
        same_repo: true,
        author: "ada".into(),
        me: "grace".into(),
        can_merge: Some(true),
        methods: MergeMethod::ALL.to_vec(),
        deletes_merged_branches: false,
        pending_review: None,
    }
}

pub(crate) fn view(d: Option<PrDetail>) -> PrView {
    let pr = PullRequest {
        id: 12,
        title: "Search page".into(),
        description: String::new(),
        source: "search".into(),
        target: "main".into(),
        author: "ada".into(),
        draft: false,
        reviewers: Vec::new(),
        head: None,
        merge_status: None,
        checks: None,
        url: String::new(),
        facets: Default::default(),
    };
    PrView {
        pr,
        label: "#12".into(),
        noun: "pull request",
        remote: "origin".into(),
        listed: true,
        tips: Tips { target: None, source: None },
        files: Files::Missing,
        head_ref: None,
        shown: None,
        head_fetch: Default::default(),
        asked: None,
        tab: PrTab::Overview,
        page: d.map_or(Page::Loading, |detail| {
            Page::Loaded(Box::new(PrPage { detail, entries: Vec::new(), threads: Vec::new(), threads_note: None }))
        }),
        reloading: false,
        busy: None,
        action_error: None,
        forms: PrForms::default(),
        threads: Default::default(),
        checks: crate::state::pull_request_checks::PrChecks::new(&crate::state::ForgeState::new(None, String::new(), None)),
    }
}

/// `view` with the detail changed by `f`.
fn with(f: impl FnOnce(&mut PrDetail)) -> PrView {
    let mut d = detail();
    f(&mut d);
    view(Some(d))
}

#[test]
fn comment_blocked_reasons() {
    assert_eq!(comment_blocked(&view(None)).as_deref(), Some("The pull request hasn't loaded yet"));
    let mut v = view(Some(detail()));
    assert_eq!(comment_blocked(&v).as_deref(), Some("Write a comment first"));
    v.forms.comment = "  \n".into();
    assert_eq!(comment_blocked(&v).as_deref(), Some("Write a comment first"));
    v.forms.comment = "x".repeat(MAX_BODY);
    assert_eq!(comment_blocked(&v), None);
    v.forms.comment.push('x');
    assert_eq!(comment_blocked(&v).as_deref(), Some("Comments can have at most 65,536 characters"));
    v.forms.comment = "Thanks".into();
    v.busy = Some(PrAction::Review);
    assert_eq!(comment_blocked(&v).as_deref(), Some("Waiting for the service…"));
    let mut v = with(|d| d.locked = true);
    v.forms.comment = "Thanks".into();
    assert_eq!(comment_blocked(&v).as_deref(), Some("The conversation is locked on the service"));
}

#[test]
fn the_author_cannot_judge_their_own_changes() {
    let v = with(|d| d.me = "ADA".into());
    assert_eq!(review_choice_blocked(&v, ReviewEvent::Approve).as_deref(), Some("You opened this pull request, so you can't approve it."));
    assert_eq!(
        review_choice_blocked(&v, ReviewEvent::RequestChanges).as_deref(),
        Some("You opened this pull request, so you can't request changes on it.")
    );
    assert_eq!(review_choice_blocked(&v, ReviewEvent::Comment), None);
    let v = view(Some(detail()));
    assert_eq!(review_choice_blocked(&v, ReviewEvent::Approve), None);
    let v = with(|d| d.state = PrState::Merged);
    assert_eq!(review_choice_blocked(&v, ReviewEvent::Comment).as_deref(), Some("This pull request is merged"));
}

#[test]
fn reviews_need_a_summary_except_approvals_and_finishing() {
    let mut v = view(Some(detail()));
    assert_eq!(review_blocked(&v).as_deref(), Some("Write a summary for this choice"));
    v.forms.review_event = ReviewEvent::RequestChanges;
    assert_eq!(review_blocked(&v).as_deref(), Some("Write a summary for this choice"));
    v.forms.review_event = ReviewEvent::Approve;
    assert_eq!(review_blocked(&v), None);
    v.forms.review_event = ReviewEvent::RequestChanges;
    v.forms.review_body = "Rename it".into();
    assert_eq!(review_blocked(&v), None);
    assert_eq!(review_button(&v), "Submit review");

    let mut v = with(|d| d.pending_review = Some(99));
    assert_eq!(review_blocked(&v), None, "an empty comment finishes a pending review");
    assert_eq!(review_button(&v), "Finish review");
    v.forms.review_event = ReviewEvent::RequestChanges;
    assert_eq!(review_blocked(&v).as_deref(), Some("Write a summary for this choice"));
    let mut v = with(|d| d.state = PrState::Closed);
    v.forms.review_event = ReviewEvent::Approve;
    assert_eq!(review_blocked(&v).as_deref(), Some("This pull request is closed"));
    assert_eq!(review_blocked(&view(None)).as_deref(), Some("The pull request hasn't loaded yet"));
}

/// A change to the detail, and the reason it should block merging.
type Case = (fn(&mut PrDetail), &'static str);

#[test]
fn merge_blocked_in_order_of_precedence() {
    let ready = |f: fn(&mut PrDetail)| {
        let mut v = with(f);
        v.forms.merge_method = Some(MergeMethod::Squash);
        v
    };
    assert_eq!(merge_blocked(&view(None)).as_deref(), Some("The pull request hasn't loaded yet"));
    let mut v = ready(|d| d.state = PrState::Merged);
    v.busy = Some(PrAction::Comment);
    assert_eq!(merge_blocked(&v).as_deref(), Some("Waiting for the service…"));
    let cases: [Case; 6] = [
        (
            |d| {
                d.state = PrState::Merged;
                d.draft = true;
            },
            "This pull request is merged",
        ),
        (|d| d.state = PrState::Closed, "This pull request is closed"),
        (
            |d| {
                d.draft = true;
                d.merge_state = MergeState::Conflicts;
            },
            "Drafts can't be merged; mark it ready on the service",
        ),
        (
            |d| {
                d.merge_state = MergeState::Conflicts;
                d.can_merge = Some(false);
            },
            "Conflicts with main must be resolved first",
        ),
        (
            |d| {
                d.merge_state = MergeState::Blocked;
                d.can_merge = Some(false);
            },
            "The repository's rules block merging (required reviews or checks)",
        ),
        (|d| d.can_merge = Some(false), "Only people with write access can merge"),
    ];
    for (f, want) in cases {
        assert_eq!(merge_blocked(&ready(f)).as_deref(), Some(want));
    }
    let mut v = ready(|d| d.methods = vec![MergeMethod::Merge]);
    assert_eq!(merge_blocked(&v).as_deref(), Some("Not enabled in this repository's settings"));
    v.forms.merge_method = None;
    assert_eq!(merge_blocked(&v).as_deref(), Some("Choose how to merge"));
    let mut v = ready(|_| {});
    assert_eq!(merge_blocked(&v), None);
    v.forms.merge_title = "x".repeat(MAX_TITLE + 1);
    assert_eq!(merge_blocked(&v).as_deref(), Some("The commit title or message is too long"));
    let v = ready(|d| d.merge_state = MergeState::Behind);
    assert_eq!(merge_blocked(&v), None, "the service decides about a branch that is behind");
}

#[test]
fn methods_the_repository_disallows_are_blocked() {
    let mut d = detail();
    d.methods = vec![MergeMethod::Squash];
    assert_eq!(method_blocked(&d, MergeMethod::Merge), Some("Not enabled in this repository's settings"));
    assert_eq!(method_blocked(&d, MergeMethod::Squash), None);
    assert_eq!(method_label(MergeMethod::Rebase), "Rebase and merge");
    assert!(method_help(MergeMethod::Squash, "main").contains("one commit on main"));
}

#[test]
fn deleting_after_the_merge() {
    let v = view(Some(detail()));
    assert_eq!(delete_after_merge_blocked(&v, Some("main")), None);
    let v = with(|d| d.same_repo = false);
    assert_eq!(delete_after_merge_blocked(&v, Some("main")).as_deref(), Some("The source branch is on a fork"));
    let v = view(Some(detail()));
    assert_eq!(delete_after_merge_blocked(&v, Some("search")).as_deref(), Some("search is the repository's default branch"));
    let mut v = with(|d| d.deletes_merged_branches = true);
    assert_eq!(delete_after_merge_blocked(&v, None).as_deref(), Some("The repository deletes merged branches itself"));
    v.forms.delete_branch = true;
    assert!(!deletes_after_merge(&v, None), "asked for, but not allowed");
}

#[test]
fn the_merge_confirmation_names_the_method_and_the_deletion() {
    let mut v = view(Some(detail()));
    v.forms.merge_method = Some(MergeMethod::Squash);
    let Msg::Confirm { title, body, button, then } = merge_confirm(&v, Some("main")) else { panic!("not a confirmation") };
    assert_eq!((title.as_str(), button.as_str()), ("Merge pull request", "Merge"));
    assert_eq!(body, "Merge #12 \u{201c}Search page\u{201d} into main by squashing its commits?");
    assert!(matches!(*then, Msg::PullRequest(crate::pull_request::PullRequestMsg::Merge)));
    v.forms.merge_method = Some(MergeMethod::Merge);
    v.forms.delete_branch = true;
    let Msg::Confirm { body, .. } = merge_confirm(&v, Some("main")) else { panic!() };
    assert_eq!(body, "Merge #12 \u{201c}Search page\u{201d} into main by a merge commit? Then delete search on origin.");
    v.forms.merge_method = Some(MergeMethod::Rebase);
    let Msg::Confirm { body, .. } = merge_confirm(&v, Some("search")) else { panic!() };
    assert!(body.ends_with("by rebasing its commits?"), "the default branch is never deleted: {body}");
}

#[test]
fn prefill_picks_an_allowed_method_and_a_verdict_the_user_may_give() {
    let mut forms = PrForms { review_event: ReviewEvent::Approve, merge_method: Some(MergeMethod::Merge), ..PrForms::default() };
    let mut d = detail();
    d.methods = vec![MergeMethod::Squash, MergeMethod::Rebase];
    prefill(&mut forms, &d);
    assert_eq!(forms.merge_method, Some(MergeMethod::Squash));
    assert_eq!(forms.review_event, ReviewEvent::Approve);
    forms.merge_method = Some(MergeMethod::Rebase);
    d.me = "ada".into();
    prefill(&mut forms, &d);
    assert_eq!(forms.merge_method, Some(MergeMethod::Rebase), "an allowed choice stays");
    assert_eq!(forms.review_event, ReviewEvent::Comment, "the author can only comment");
    d.methods.clear();
    prefill(&mut forms, &d);
    assert_eq!(forms.merge_method, None);
}

#[test]
fn verdicts_are_colored_words() {
    assert_eq!(verdict_text(&EntryKind::Comment), ("commented", Tone::Neutral));
    assert_eq!(verdict_text(&EntryKind::Review(Verdict::Approved)), ("approved", Tone::Good));
    assert_eq!(verdict_text(&EntryKind::Review(Verdict::ChangesRequested)), ("requested changes", Tone::Bad));
    assert_eq!(verdict_text(&EntryKind::Review(Verdict::Commented)), ("reviewed", Tone::Neutral));
    assert_eq!(verdict_text(&EntryKind::Review(Verdict::Dismissed)), ("review dismissed", Tone::Neutral));
}

#[test]
fn mergeability_lines() {
    assert_eq!(mergeability(&view(Some(detail()))), Some((Tone::Good, "No conflicts with main".into())));
    assert_eq!(mergeability(&with(|d| d.merge_state = MergeState::Conflicts)).map(|m| m.0), Some(Tone::Bad));
    assert_eq!(mergeability(&with(|d| d.merge_state = MergeState::Unstable)).map(|m| m.0), Some(Tone::Warn));
    assert_eq!(mergeability(&with(|d| d.draft = true)).map(|m| m.0), Some(Tone::Warn));
    assert_eq!(mergeability(&with(|d| d.can_merge = Some(false))), Some((Tone::Neutral, "Only people with write access can merge".into())));
    assert_eq!(mergeability(&with(|d| d.state = PrState::Merged)), None);
    assert_eq!(mergeability(&view(None)), None);
}
