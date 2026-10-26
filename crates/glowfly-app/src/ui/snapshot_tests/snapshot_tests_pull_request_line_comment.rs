//! A new code comment on a pull request file's diff lines: the comment box in the Files changed
//! pane with the draft's marks in the diff, the box with a suggestion, a pending review and an
//! error, the comment button on a hovered line, the hunk header's Comment on N lines, and Esc;
//! and the messages they send.

use super::snapshot_tests::{harness, harness_sized, remove_fixture};
use super::snapshot_tests_esc::{app_harness, esc};
use super::snapshot_tests_pull_request::with_pr;
use super::snapshot_tests_pull_request_threads::{comment, detail, files_loaded, load, thread};
use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::pull_request::line_comment::LineCommentMsg;
use crate::state::AppState;
use crate::state::pull_request::PrTab;
use crate::state::pull_request_line_comment::draft_at;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use eframe::egui::{self, Key, Modifiers};
use egui_kittest::kittest::Queryable;
use glowfly_core::diff_rows::LineSide;
use glowfly_core::forge::PrDetail;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;
use std::sync::Arc;

/// Wide enough for the box's buttons to share a row, so the comment pane shows the whole box.
const WIDE: egui::Vec2 = egui::vec2(1900.0, 900.0);

const TEXT: &str = "Could this name say what it searches?";

/// The fixture's panel on Files changed with search.txt's diff open and its page loaded (as `f`
/// changes the detail, head = the local source tip), plus `threads` on the added line; with
/// `draft`, a draft with `text` on that line. Returns the state, the folder and the line.
fn opened(
    theme: ThemeChoice,
    name: &str,
    f: impl FnOnce(&mut PrDetail),
    with_thread: bool,
    draft: Option<&str>,
) -> (AppState, PathBuf, u32) {
    let (mut st, dir) = with_pr(theme, name, true);
    let (target, d, line) = files_loaded(&mut st, &dir);
    let head = st.session.pull_request.view.as_ref().unwrap().source_tip().unwrap().to_string();
    let mut det = detail(head);
    f(&mut det);
    let threads = if with_thread {
        vec![thread("T1", line, vec![comment(55, "linus", 13, "Should this be case-insensitive?", false)])]
    } else {
        Vec::new()
    };
    load(&mut st, det, threads);
    let v = st.session.pull_request.view.as_mut().unwrap();
    v.tab = PrTab::Files;
    if let Some(text) = draft {
        let (h, i) = glowfly_core::diff_rows::line_index(&d, LineSide::New, line).expect("the added line");
        let mut dr = draft_at(v, &target, &d, h, i, i).unwrap();
        dr.text = text.into();
        v.line_draft = Some(dr);
    }
    st.diff = Some((target, Arc::new(d)));
    (st, dir, line)
}

fn line_msgs(h: &egui_kittest::Harness<'static, (AppState, Vec<Msg>)>, f: impl Fn(&LineCommentMsg) -> bool) -> usize {
    h.state().1.iter().filter(|m| matches!(m, Msg::PullRequest(PullRequestMsg::LineComment(l)) if f(l))).count()
}

#[test]
fn pull_request_line_comment_dark() {
    let (st, dir, line) = opened(ThemeChoice::Dark, "pr-line-comment", |_| {}, false, Some(TEXT));
    let mut h = harness_sized(st, true, WIDE);
    h.run();
    h.check_snapshot("pull_request_line_comment_dark");
    assert!(h.query_by_label("New comment on").is_some());
    assert!(h.query_by_label(&format!("line {line}")).is_some());
    assert!(h.query_by_label("Add single comment").is_some());
    assert!(h.query_by_label("Start a review").is_some());

    // Command+Enter posts alone (no review is pending), once and without a newline.
    let field = h.get_all_by_label("New comment").find(|n| n.role() == Role::MultilineTextInput).expect("the comment field");
    field.type_text(" Thanks");
    h.run();
    h.press_key_modifiers(Modifiers::COMMAND, Key::Enter);
    h.run();
    assert_eq!(line_msgs(&h, |m| *m == LineCommentMsg::Post { review: false }), 1);
    let want = format!("{TEXT} Thanks");
    assert_eq!(h.state().0.session.pull_request.view.as_ref().unwrap().line_draft.as_ref().unwrap().text, want, "no newline added");

    h.get_by_role_and_label(Role::Button, "Cancel").click();
    h.run();
    assert_eq!(line_msgs(&h, |m| *m == LineCommentMsg::Cancel), 1);
    h.get_by_role_and_label(Role::Button, "Your unposted comment").click();
    h.run();
    assert_eq!(line_msgs(&h, |m| *m == LineCommentMsg::Focus), 1);
    h.get_by_label(&format!("line {line}")).click();
    h.run();
    assert_eq!(line_msgs(&h, |m| *m == LineCommentMsg::Show), 1);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pull_request_line_comment_suggestion_light() {
    let (mut st, dir, _) = opened(ThemeChoice::Light, "pr-line-comment-suggest", |d| d.pending_review = Some(9), false, None);
    let (target, d) = st.diff.clone().unwrap();
    let v = st.session.pull_request.view.as_mut().unwrap();
    let (hk, i) = d.hunks.iter().enumerate().find_map(|(h, k)| k.lines.iter().position(|l| l.origin == '+').map(|i| (h, i))).unwrap();
    let mut dr = draft_at(v, &target, &d, hk, i, i).unwrap();
    dr.text = crate::state::pull_request_line_comment::with_suggestion("Maybe:", &dr);
    dr.error = Some("the server said \u{201c}Validation Failed pull_request_review_thread.line must be part of the diff\u{201d}".into());
    v.line_draft = Some(dr);
    let mut h = harness_sized(st, false, WIDE);
    h.run();
    h.check_snapshot("pull_request_line_comment_suggestion_light");
    assert!(h.query_by_label("Add review comment").is_some());
    assert!(h.query_by_label_contains("must be part of the diff").is_some(), "the error note");
    let single = h.get_by_role_and_label(Role::Button, "Add single comment");
    assert!(single.is_disabled(), "a review is pending");
    single.hover();
    for _ in 0..4 {
        h.run();
    }
    assert!(h.query_by_label_contains("You have a review in progress").is_some());
    h.get_by_role_and_label(Role::Button, "Suggest a change").click();
    h.run();
    assert_eq!(line_msgs(&h, |m| *m == LineCommentMsg::Suggest), 1);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pull_request_line_comment_hover_dark() {
    let (st, dir, line) = opened(ThemeChoice::Dark, "pr-line-comment-hover", |_| {}, true, None);
    let mut h = harness(st, true);
    h.run();
    let mark = h.get_by_role_and_label(Role::Button, "1 comment on this line").bounding_box().expect("the thread marker");
    let at = egui::pos2(mark.x0 as f32 - 200.0, ((mark.y0 + mark.y1) / 2.0) as f32);
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    h.run();
    h.check_snapshot("pull_request_line_comment_hover_dark");
    let add = h.get_by_role_and_label(Role::Button, "Comment on this line");
    let r = add.bounding_box().expect("the button");
    assert!(r.x1 <= mark.x0 + 0.5, "left of the thread marker");
    add.click();
    h.run();
    assert_eq!(line_msgs(&h, |m| *m == LineCommentMsg::Start { side: LineSide::New, line, extend: false }), 1);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn diff_hunk_comment_lines_light() {
    let (mut st, dir, _) = opened(ThemeChoice::Light, "pr-line-comment-hunk", |_| {}, false, None);
    let (target, d) = st.diff.clone().unwrap();
    let i = d.hunks[0].lines.iter().position(|l| l.origin == '+').expect("an added line");
    st.view.diff_lines.target = Some(target);
    st.view.diff_lines.hunk = 0;
    st.view.diff_lines.lines = [i].into_iter().collect();
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("diff_hunk_comment_lines_light");
    assert!(h.query_by_label("Revert 1 line").is_some());
    h.get_by_role_and_label(Role::Button, "Comment on 1 line").click();
    h.run();
    assert_eq!(line_msgs(&h, |m| *m == LineCommentMsg::OnSelection), 1);

    // With the service's head elsewhere, no comment can start here.
    if let Some(crate::state::pull_request::Page::Loaded(p)) = h.state_mut().0.session.pull_request.view.as_mut().map(|v| &mut v.page) {
        p.detail.head_sha = "abc1234".into();
    }
    h.run();
    assert!(h.query_by_label("Comment on 1 line").is_none());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn esc_cancels_the_draft_before_closing_the_diff() {
    let (st, dir, _) = opened(ThemeChoice::Dark, "pr-line-comment-esc", |_| {}, false, Some(TEXT));
    let mut h = app_harness(st);
    h.run();
    let msgs = esc(&mut h);
    let cancels =
        |msgs: &[Msg]| msgs.iter().filter(|m| matches!(m, Msg::PullRequest(PullRequestMsg::LineComment(LineCommentMsg::Cancel)))).count();
    assert_eq!(cancels(&msgs), 1, "{}", msgs.len());
    assert!(!msgs.iter().any(|m| matches!(m, Msg::Repo(crate::msg::RepoMsg::CloseDiff) | Msg::PullRequest(PullRequestMsg::Close))));

    // In the focused field, Esc leaves it and cancels once; the app's Esc stays out.
    h.state_mut().0.view.pr_line_comment_focus = true;
    h.run();
    assert!(h.state().0.view.was_typing, "the comment field has the keyboard");
    let msgs = esc(&mut h);
    assert_eq!(cancels(&msgs), 1);
    assert!(!msgs.iter().any(|m| matches!(m, Msg::Repo(crate::msg::RepoMsg::CloseDiff) | Msg::PullRequest(PullRequestMsg::Close))));
    drop(h);
    remove_fixture(&dir);
}
