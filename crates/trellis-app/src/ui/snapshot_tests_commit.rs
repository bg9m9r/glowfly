//! The commit box: its options row and button suffixes, the more-actions menu with Skip hooks,
//! the commit template note, and the
//! Ctrl+Enter / Ctrl+Shift+Enter shortcuts. Shares the fixture repository and renderer with
//! `snapshot_tests`.

use super::snapshot_tests::{git, harness, reload, remove_fixture, with_repo};
use crate::msg::CommitMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use eframe::egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use trellis_core::repo::{CommitTemplate, LoadOptions};
use trellis_core::settings::ThemeChoice;

type H = Harness<'static, (AppState, Vec<Msg>)>;

fn count(h: &H, want: fn(&Msg) -> bool) -> usize {
    h.state().1.iter().filter(|m| want(m)).count()
}

#[test]
fn commit_box_options() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "commit-options");
    git(&dir, &["add", "README.md"], "2026-01-11T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    st.draft.summary = "Add login form".into();
    st.settings.push_after_commit = true;
    st.draft.skip_hooks = true;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("commit_box_options");
    assert!(h.query_by_label_contains("and push (skip hooks)").is_some(), "the button names both options");
    assert!(h.query_by_label("Skip hooks").is_none(), "Skip hooks is in the more-actions menu");
    let more = h.get_by_label("More commit options");
    assert_eq!(more.role(), Role::Button);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn commit_box_more_menu() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "commit-more-menu");
    git(&dir, &["add", "README.md"], "2026-01-11T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    st.draft.summary = "Add login form".into();
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label_contains("(skip hooks)").is_none());
    h.get_by_label("More commit options").click();
    h.run();
    h.check_snapshot("commit_box_more_menu");
    // The menu is as wide as its label, not the caret: the label stays on one line, on screen.
    let check = h.get_by_label("Skip hooks").bounding_box().unwrap();
    let caret = h.get_by_label("More commit options").bounding_box().unwrap();
    assert!(check.width() > 2.0 * caret.width(), "the menu is wider than the caret: {check:?}");
    assert!(check.height() < caret.height(), "the label is one line: {check:?}");
    let right = f64::from(h.ctx.screen_rect().right());
    assert!(check.x1 <= right, "the menu stays inside the window: {check:?} {right}");
    h.get_by_label("Skip hooks").click();
    h.run();
    assert!(h.state().0.draft.skip_hooks);
    assert!(h.query_by_label("Skip hooks").is_none(), "the menu closes so the tint shows");
    assert!(h.query_by_label_contains("(skip hooks)").is_some(), "the button says hooks are skipped");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn commit_box_split_halves() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "commit-split");
    git(&dir, &["add", "README.md"], "2026-01-11T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    st.draft.summary = "Add login form".into();
    let mut h = harness(st, true);
    h.run();
    let caret = h.get_by_label("More commit options").bounding_box().unwrap();
    let commit = h.get(By::new().role(Role::Button).label_contains("Commit 1 file")).bounding_box().unwrap();
    assert!((caret.x0 - commit.x1).abs() < 0.5 && (caret.y0 - commit.y0).abs() < 0.5, "one pill: {commit:?} {caret:?}");
    h.get_by_label("More commit options").click();
    h.run();
    assert!(h.query_by_label("Skip hooks").is_some(), "the caret opens the menu");
    assert_eq!(count(&h, |m| matches!(m, Msg::Commit(CommitMsg::Commit))), 0, "without committing");
    h.get_by_label("Skip hooks").click();
    h.run();
    assert!(h.state().0.draft.skip_hooks);
    assert!(h.query_by_label("Skip hooks").is_none(), "ticking it closes the menu");
    h.get_by_label("More commit options").hover();
    h.run();
    h.run();
    h.check_snapshot("design_split_commit");
    h.get(By::new().role(Role::Button).label_contains("Commit 1 file")).click();
    h.run();
    assert_eq!(count(&h, |m| matches!(m, Msg::Commit(CommitMsg::Commit))), 1, "the action half commits");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn commit_box_says_why() {
    const NO_SUMMARY: &str = "Write a summary for the commit.";
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "commit-says-why");
    git(&dir, &["add", "README.md"], "2026-01-11T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(NO_SUMMARY).is_some(), "under the disabled button");
    h.state_mut().0.draft.summary = "Add login form".into();
    h.run();
    assert!(h.query_by_label(NO_SUMMARY).is_none(), "gone once it can commit");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn commit_box_template_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "commit-template");
    let t = CommitTemplate::parse("\n# Why is this change needed?\n# What does it change?\n", '#').unwrap();
    st.draft.apply_template(&t);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("commit_box_template_light");
    assert!(h.query_by_label_contains("left out of the message").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn commit_shortcuts() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "commit-shortcuts");
    st.draft.summary = "Add login".into();
    let mut h = harness(st, true);
    h.run();
    h.get(By::new().role(Role::TextInput).value("Add login")).type_text(" form");
    h.run();
    assert_eq!(h.state().0.draft.summary, "Add login form", "the summary field has focus");

    h.press_key_modifiers(Modifiers::COMMAND, Key::Enter);
    h.run();
    assert_eq!(count(&h, |m| matches!(m, Msg::Commit(CommitMsg::Commit))), 1);
    assert_eq!(h.state().0.draft.summary, "Add login form", "no newline or lost text");

    h.state_mut().1.clear();
    h.press_key_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Enter);
    h.run();
    assert_eq!(count(&h, |m| matches!(m, Msg::Commit(CommitMsg::StageAllAndCommit))), 1);
    assert_eq!(count(&h, |m| matches!(m, Msg::Commit(CommitMsg::Commit))), 0, "Shift is matched first");

    // Without focus in the message fields the shortcuts do nothing.
    h.state_mut().1.clear();
    // The header, not the list (which has the same name).
    h.get_all_by_label_contains("Unstaged files").find(|n| n.role() == Role::Label).expect("the header").simulate_click();
    h.run();
    h.press_key_modifiers(Modifiers::COMMAND, Key::Enter);
    h.run();
    assert_eq!(count(&h, |m| matches!(m, Msg::Commit(CommitMsg::Commit) | Msg::Commit(CommitMsg::StageAllAndCommit))), 0);

    h.get_by_label("Push after committing").click();
    h.run();
    assert_eq!(count(&h, |m| matches!(m, Msg::Commit(CommitMsg::SetPushAfterCommit(true)))), 1);
    assert!(!h.state().0.settings.push_after_commit, "only update changes the setting");
    drop(h);
    remove_fixture(&dir);
}
