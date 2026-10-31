//! The issue panel's comment box: empty with Comment disabled and its reason, a failed post
//! above the kept draft, a post in flight, a draft too long for Trello, and Reply on each
//! comment. Shares the issue panel's fixture with `snapshot_tests_issue`.

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_issue::{crash, crash_details, github, set_details, with_issue};
use crate::msg::{IssueMsg, Msg};
use crate::state::issue_comment::FIELD;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::{Account, Issue, IssueDetails, Target};
use glowfly_core::settings::ThemeChoice;

#[test]
fn issue_comment_box_empty() {
    let (mut st, dir) = with_issue(ThemeChoice::Light, "issue-comment-empty", github(), crash(), None);
    set_details(&mut st, crash_details());
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("issue_comment_box_empty");
    assert!(h.get_by_role_and_label(Role::Button, "Comment").is_disabled());
    assert!(h.query_by_label("Write a comment first").is_some(), "the reason is on screen");
    assert_eq!(h.query_all_by_label("Reply").count(), 2, "one per comment");
    h.get_all_by_label("Reply").next().unwrap().click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Issue(IssueMsg::Quote(0)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn issue_comment_box_error() {
    let (mut st, dir) = with_issue(ThemeChoice::Dark, "issue-comment-error", github(), crash(), None);
    set_details(&mut st, crash_details());
    let v = st.session.issue.view.as_mut().unwrap();
    v.comment.text = "Confirmed on 1.2 too.".into();
    v.comment.error = Some("Unable to create comment because issue is locked.".into());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("issue_comment_box_error");
    assert!(h.query_by_label("Couldn't post the comment: Unable to create comment because issue is locked.").is_some());
    assert!(!h.get_by_role_and_label(Role::Button, "Comment").is_disabled(), "the kept draft can be posted again");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn issue_comment_box_posting() {
    let (mut st, dir) = with_issue(ThemeChoice::Light, "issue-comment-posting", github(), crash(), None);
    set_details(&mut st, crash_details());
    let v = st.session.issue.view.as_mut().unwrap();
    v.comment.text = "Confirmed on 1.2 too.".into();
    v.comment.posting = true;
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("issue_comment_box_posting");
    assert!(h.get_by_role_and_label(Role::Button, "Posting…").is_disabled());
    assert!(h.query_by_label("Posting the comment…").is_some());
    assert!(h.get_all_by_label("Reply").all(|r| r.is_disabled()));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn issue_comment_too_long() {
    let account = Account { provider: glowfly_core::forge::Provider::Trello, host: "trello.com".into(), org: String::new() };
    let trello = Target::trello(&account, "5f1a2b3c4d5e6f7a8b9c0d1e", "Website");
    let card = Issue { key: "AbCd1234".into(), url: "https://trello.com/c/AbCd1234".into(), ..crash() };
    let (mut st, dir) = with_issue(ThemeChoice::Light, "issue-comment-long", trello, card.clone(), None);
    set_details(&mut st, IssueDetails { issue: card, ..crash_details() });
    let v = st.session.issue.view.as_mut().unwrap();
    v.comment.text = "x".repeat(16_385);
    v.comment.touched.mark(FIELD);
    let mut h = harness(st, false);
    h.run();
    assert_eq!(h.query_all_by_label_contains("at most 16,384 characters").count(), 1, "under the field only, not again under Comment");
    let problem = h.get_by_label_contains("at most 16,384 characters").bounding_box().expect("the problem line");
    let first = h.get_by_label_contains("I can reproduce").bounding_box().expect("the first comment");
    assert!(problem.y1 < 820.0, "the box stays on screen: {problem:?}");
    assert!(first.y0 > 0.0 && first.y1 < problem.y0 - 160.0, "the draft scrolls instead of covering the conversation: {first:?}");
    drop(h);
    remove_fixture(&dir);
}
