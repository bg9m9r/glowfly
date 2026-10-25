//! The pull request panel's Overview tab for a Bitbucket pull request: votes in the review form
//! and the timeline, only the repository's merge strategies (or none Glowfly can send), and the
//! messages its buttons send.

use super::{detail, entry, load};
use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::state::AppState;
use crate::state::pull_request::{Page, PrView};
use crate::state::pull_request_actions::NO_METHODS;
use crate::state::pull_request_checks::ChecksPane;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{harness_sized, remove_fixture};
use crate::ui::snapshot_tests_pull_request::with_pr;
use eframe::egui::accesskit::Role;
use eframe::egui::vec2;
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::{CheckList, EntryKind, MergeMethod, PrDetail, Provider, ReviewEvent, ReviewStyle, Target, Verdict, Vote};
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

/// Tall enough to show the merge form.
const TALL: eframe::egui::Vec2 = vec2(1400.0, 1100.0);

const NOTE: &str = "Code comments on Bitbucket pull requests open in the browser for now (1 on this pull request).";

/// `with_pr` on a Bitbucket Cloud repository, the panel opened afresh.
fn with_bb(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_pr(theme, name, true);
    let fs = st.forge.as_mut().unwrap();
    fs.host = Some(Target {
        provider: Provider::Bitbucket,
        host: "bitbucket.org".into(),
        owner: "acme".into(),
        project: String::new(),
        repo: "demo".into(),
    });
    let mut view = PrView::new(fs.prs[0].clone(), fs, st.snap.as_ref().unwrap());
    assert_eq!((view.label.as_str(), view.noun, &view.page), ("#12", "pull request", &Page::Loading));
    view.checks.pane = ChecksPane::Loaded(CheckList::empty(fs.prs[0].head.clone().unwrap_or_default()));
    st.pull_request = Some(view);
    (st, dir)
}

fn bb_detail(mine: Vote, methods: Vec<MergeMethod>) -> PrDetail {
    PrDetail { style: ReviewStyle::Votes { mine }, methods, can_merge: None, ..detail() }
}

fn pushed(h: &egui_kittest::Harness<'static, (AppState, Vec<Msg>)>, f: impl Fn(&Msg) -> bool) -> bool {
    h.state().1.iter().any(f)
}

#[test]
fn pull_request_bitbucket_review_light() {
    let (mut st, dir) = with_bb(ThemeChoice::Light, "pr-bitbucket-review");
    let entries = vec![
        entry("Linus Torvalds", 12, EntryKind::Comment, "Could the results be paged?"),
        entry("Grace Hopper", 13, EntryKind::Review(Verdict::Approved), ""),
        entry("Linus Torvalds", 14, EntryKind::Review(Verdict::ChangesRequested), ""),
    ];
    load(&mut st, bb_detail(Vote::Neither, vec![MergeMethod::Merge, MergeMethod::Squash, MergeMethod::FastForward]), entries);
    if let Page::Loaded(p) = &mut st.pull_request.as_mut().unwrap().page {
        p.threads_note = Some(NOTE.into());
    }
    let mut h = harness_sized(st, false, TALL);
    h.run();
    h.check_snapshot("pull_request_bitbucket_review_light");
    assert!(!h.get_by_label("Approve").is_disabled() && !h.get_by_label("Request changes").is_disabled());
    assert!(h.query_by_label("Remove approval").is_none() && h.query_by_label("Remove change request").is_none());
    for m in ["Merge commit", "Squash and merge", "Fast-forward"] {
        assert!(h.query_by_label(m).is_some(), "{m}");
    }
    assert!(h.query_by_label("Rebase and merge").is_none(), "only the repository's strategies");
    assert!(h.query_by_label("requested changes").is_some());
    assert!(h.query_by_label(NOTE).is_some());
    h.get_by_label("Merge pull request").click();
    h.run();
    assert!(
        pushed(&h, |m| matches!(m, Msg::Confirm { then, .. } if matches!(**then, Msg::PullRequest(PullRequestMsg::Merge)))),
        "Merge asks first"
    );
    h.state_mut().1.clear();
    h.get_by_label("Request changes").click();
    h.run();
    h.get_by_role_and_label(Role::Button, "Submit review").click();
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::PullRequest(PullRequestMsg::SubmitReview))), "no summary needed");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pull_request_bitbucket_changes_requested_dark() {
    let (mut st, dir) = with_bb(ThemeChoice::Dark, "pr-bitbucket-changes");
    let entries = vec![entry("grace", 13, EntryKind::Review(Verdict::ChangesRequested), "")];
    load(&mut st, bb_detail(Vote::ChangesRequested, vec![MergeMethod::Rebase, MergeMethod::Squash]), entries);
    let mut h = harness_sized(st, true, TALL);
    h.run();
    h.check_snapshot("pull_request_bitbucket_changes_requested_dark");
    assert!(h.query_by_label("Remove change request").is_some());
    assert!(h.query_by_label("Approve").is_none() && h.query_by_label("Request changes").is_none());
    assert_eq!(h.state().0.pull_request.as_ref().unwrap().forms.merge_method, Some(MergeMethod::Rebase));
    assert!(h.query_by_label("Commit title").is_none(), "a rebase makes no new message");
    h.get_by_label("Remove change request").click();
    h.run();
    h.get_by_role_and_label(Role::Button, "Submit review").click();
    h.run();
    assert!(pushed(&h, |m| matches!(m, Msg::PullRequest(PullRequestMsg::SubmitReview))));
    assert_eq!(h.state().0.pull_request.as_ref().unwrap().forms.review_event, ReviewEvent::UnrequestChanges);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pull_request_bitbucket_no_strategy_light() {
    let (mut st, dir) = with_bb(ThemeChoice::Light, "pr-bitbucket-none");
    load(&mut st, bb_detail(Vote::Neither, Vec::new()), Vec::new());
    let mut h = harness_sized(st, false, TALL);
    h.run();
    h.check_snapshot("pull_request_bitbucket_no_strategy_light");
    for m in ["Merge commit", "Squash and merge", "Fast-forward", "Rebase and merge"] {
        assert!(h.query_by_label(m).is_none(), "{m}");
    }
    assert!(h.get_by_label("Merge pull request").is_disabled());
    assert!(h.query_by_label(NO_METHODS).is_some());
    drop(h);
    remove_fixture(&dir);
}
