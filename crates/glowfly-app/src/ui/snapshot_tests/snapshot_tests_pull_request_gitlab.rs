//! The pull request panel's Overview tab for a GitLab merge request: approvals in the review
//! form and the timeline, only the project's merge methods, and the messages its buttons send.

use super::{detail, entry, load};
use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::state::AppState;
use crate::state::pull_request::{Page, PrView};
use crate::state::pull_request_checks::ChecksPane;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::snapshot_tests_pull_request::with_pr;
use crate::ui::snapshot_tests::{harness_sized, remove_fixture};
use eframe::egui::accesskit::Role;
use eframe::egui::vec2;
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::{CheckList, EntryKind, MergeMethod, PrDetail, Provider, ReviewEvent, ReviewStyle, Target, Verdict};
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

/// Tall enough to show the merge form.
const TALL: eframe::egui::Vec2 = vec2(1400.0, 1100.0);

const NOTE: &str = "Code comments on GitLab merge requests open in the browser for now (2 on this merge request).";

/// `with_pr` on a GitLab project, the panel opened afresh (so it names merge request !12).
fn with_mr(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_pr(theme, name, true);
    let fs = st.forge.as_mut().unwrap();
    fs.host = Some(Target {
        provider: Provider::GitLab,
        host: "gitlab.com".into(),
        owner: "acme".into(),
        project: String::new(),
        repo: "demo".into(),
    });
    let mut view = PrView::new(fs.prs[0].clone(), fs, st.snap.as_ref().unwrap());
    assert_eq!((view.label.as_str(), view.noun, &view.page), ("!12", "merge request", &Page::Loading));
    view.checks.pane = ChecksPane::Loaded(CheckList::empty(fs.prs[0].head.clone().unwrap_or_default()));
    st.pull_request = Some(view);
    (st, dir)
}

fn gitlab_detail(by_me: bool, methods: Vec<MergeMethod>) -> PrDetail {
    PrDetail { style: ReviewStyle::Approvals { by_me, can_approve: Some(true) }, methods, ..detail() }
}

#[test]
fn pull_request_gitlab_review_light() {
    let (mut st, dir) = with_mr(ThemeChoice::Light, "pr-gitlab-review");
    let entries = vec![
        entry("Linus", 12, EntryKind::Comment, "Could the results be paged?"),
        entry("Grace", 13, EntryKind::Review(Verdict::Approved), ""),
        entry("Grace", 14, EntryKind::Review(Verdict::ApprovalRemoved), ""),
    ];
    load(&mut st, gitlab_detail(false, vec![MergeMethod::Merge, MergeMethod::Squash]), entries);
    if let Page::Loaded(p) = &mut st.pull_request.as_mut().unwrap().page {
        p.threads_note = Some(NOTE.into());
    }
    let mut h = harness_sized(st, false, TALL);
    h.run();
    h.check_snapshot("pull_request_gitlab_review_light");
    assert!(!h.get_by_label("Approve").is_disabled());
    assert!(h.query_by_label("Request changes").is_none() && h.query_by_label("Remove approval").is_none());
    assert!(h.query_by_label("Merge commit").is_some() && h.query_by_label("Squash and merge").is_some());
    assert!(h.query_by_label("Rebase and merge").is_none(), "only the project's methods");
    assert!(h.query_by_label("Merge merge request").is_none());
    assert!(h.query_by_label("removed their approval").is_some());
    assert!(h.query_by_label(NOTE).is_some());
    h.get_by_role_and_label(Role::Button, "Merge").click();
    h.run();
    let merges =
        h.state().1.iter().any(|m| matches!(m, Msg::Confirm { then, .. } if matches!(**then, Msg::PullRequest(PullRequestMsg::Merge))));
    assert!(merges, "Merge asks first");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pull_request_gitlab_approved_dark() {
    let (mut st, dir) = with_mr(ThemeChoice::Dark, "pr-gitlab-approved");
    let entries = vec![entry("grace", 13, EntryKind::Review(Verdict::Approved), "")];
    load(&mut st, gitlab_detail(true, vec![MergeMethod::FastForward, MergeMethod::Squash]), entries);
    let mut h = harness_sized(st, true, TALL);
    h.run();
    h.check_snapshot("pull_request_gitlab_approved_dark");
    assert!(h.query_by_label("Remove approval").is_some() && h.query_by_label("Approve").is_none());
    assert_eq!(h.state().0.pull_request.as_ref().unwrap().forms.merge_method, Some(MergeMethod::FastForward));
    assert!(h.query_by_label("Fast-forward").is_some());
    assert!(h.query_by_label("Commit title").is_none(), "a fast-forward makes no commit");
    h.get_by_label("Remove approval").click();
    h.run();
    h.get_by_role_and_label(Role::Button, "Submit review").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::PullRequest(PullRequestMsg::SubmitReview))));
    assert_eq!(h.state().0.pull_request.as_ref().unwrap().forms.review_event, ReviewEvent::Unapprove);
    drop(h);
    remove_fixture(&dir);
}
