//! The Stash dialog (Selected files with a message, Only staged changes with nothing staged in
//! the light theme, All changes while a job runs), the Staged Files header's menu and the WIP
//! row's Stash this file… item. Made-up working-tree files are injected into the shared
//! fixture's snapshot so the images are deterministic.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_remotes::running;
use super::snapshot_tests_select::click;
use super::snapshot_tests_worktrees::{H, pushed};
use crate::msg::{Msg, StageMsg, StashMsg};
use crate::state::stashes::StashPushForm;
use crate::state::{AppState, Modal, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use eframe::egui::{Key, PointerButton, pos2};
use egui_kittest::kittest::{By, Queryable};
use std::path::PathBuf;
use trellis_core::repo::FileEntry;
use trellis_core::settings::ThemeChoice;
use trellis_core::stash_ops::StashScope;

fn files(list: &[(&str, char)]) -> Vec<FileEntry> {
    list.iter().map(|(p, s)| FileEntry::new(*p, *s)).collect()
}

/// The fixture on the WIP panel with these unstaged and staged files.
fn with_files(theme: ThemeChoice, name: &str, unstaged: &[(&str, char)], staged: &[(&str, char)]) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged = files(unstaged);
    snap.staged = files(staged);
    st.selection = Selection::Wip;
    (st, dir)
}

const UNSTAGED: [(&str, char); 3] = [("b.txt", 'M'), ("c.txt", '?'), ("docs/notes.md", 'M')];
const STAGED: [(&str, char); 2] = [("a.txt", 'M'), ("src/lib.rs", 'A')];

/// The fixture with the Stash dialog open on `form`.
fn dialog(theme: ThemeChoice, name: &str, staged: &[(&str, char)], form: StashPushForm) -> (AppState, PathBuf) {
    let (mut st, dir) = with_files(theme, name, &UNSTAGED, staged);
    st.modal = Some(Modal::StashPush(form));
    (st, dir)
}

/// The dialog's Stash button (the toolbar's is "Stash all changes").
fn stash_button(h: &H) -> egui_kittest::kittest::Node<'_> {
    h.get_by_label("Stash")
}

/// Right-clicks the middle of the node labelled `label` (the topmost match).
fn right_click(h: &mut H, label: &str) {
    let r = h.get_all_by_label(label).filter_map(|n| n.bounding_box()).min_by(|a, b| a.y0.total_cmp(&b.y0)).expect("the node");
    let at = pos2(((r.x0 + r.x1) / 2.0) as f32, ((r.y0 + r.y1) / 2.0) as f32);
    click(h, at, PointerButton::Secondary);
}

#[test]
fn stash_push_dialog_files() {
    let mut form = StashPushForm::new(StashScope::Files, Some("b.txt".into()));
    form.chosen.insert("src/lib.rs".into());
    form.message = "Login form, half done".into();
    let (st, dir) = dialog(ThemeChoice::Dark, "stash-push-files", &STAGED, form);
    let mut h = harness(st, true);
    h.run();
    for text in ["Stash changes", "on main", "Selected files", "2 of 5 files"] {
        // "Stash changes" is also the STASHES header's +.
        assert!(h.query_all_by_label(text).next().is_some(), "{text}");
    }
    assert!(!stash_button(&h).is_disabled());
    h.check_snapshot("stash_push_dialog_files");

    h.get_by_label("Select none").click();
    h.run();
    assert!(stash_button(&h).is_disabled());
    assert!(h.query_by_label("Choose at least one file.").is_some());

    h.get_by_label("Select all").click();
    h.run();
    assert!(h.query_by_label("5 of 5 files").is_some());
    h.get(By::new().role(Role::TextInput).value("Login form, half done")).focus();
    h.run();
    h.press_key(Key::Enter);
    h.run();
    let all = ["a.txt", "b.txt", "c.txt", "docs/notes.md", "src/lib.rs"];
    let stashes = |m: &Msg| {
        matches!(m, Msg::Stash(StashMsg::Push { scope: StashScope::Files, message, paths })
            if message == "Login form, half done" && paths.iter().map(String::as_str).eq(all))
    };
    assert!(pushed(&h, &stashes), "{}", h.state().1.len());
    assert!(h.state().0.modal.is_some(), "update closes the dialog once the job starts");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn stash_push_dialog_unsafe_path() {
    let (mut st, dir) = dialog(ThemeChoice::Dark, "stash-push-unsafe", &STAGED, StashPushForm::new(StashScope::Files, None));
    let twin = trellis_core::repo::Hazard::Twins(vec!["Docs/notes.md".into()]);
    st.snap.as_mut().unwrap().unstaged[2].hazard = Some(twin);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Select all").click();
    h.run();
    assert!(h.query_by_label("4 of 4 files").is_some());
    // In the WIP list behind the dialog, and in the dialog's checklist.
    assert_eq!(h.get_all_by_label("(same file as another path)").count(), 2);
    h.check_snapshot("stash_push_dialog_unsafe_path");
    stash_button(&h).click();
    h.run();
    let four = ["a.txt", "b.txt", "c.txt", "src/lib.rs"];
    let stashes = |m: &Msg| matches!(m, Msg::Stash(StashMsg::Push { paths, .. }) if paths.iter().map(String::as_str).eq(four));
    assert!(pushed(&h, &stashes));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn stash_push_dialog_staged_nothing_light() {
    let (st, dir) = dialog(ThemeChoice::Light, "stash-push-staged-nothing", &[], StashPushForm::new(StashScope::Staged, None));
    let mut h = harness(st, false);
    h.run();
    assert!(h.query_by_label(crate::state::stashes::NOTHING_STAGED).is_some());
    assert!(stash_button(&h).is_disabled());
    h.check_snapshot("stash_push_dialog_staged_nothing_light");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn stash_push_dialog_all_busy() {
    let (mut st, dir) = dialog(ThemeChoice::Dark, "stash-push-all-busy", &STAGED, StashPushForm::new(StashScope::All, None));
    running(&mut st);
    let mut h = harness(st, true);
    // A spinner keeps repainting, so step.
    h.run_steps(2);
    assert!(stash_button(&h).is_disabled());
    h.check_snapshot("stash_push_dialog_all_busy");
    h.get_by_label("Cancel").click();
    h.run_steps(2);
    assert!(pushed(&h, &|m| matches!(m, Msg::CloseModal)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn staged_header_menu() {
    let (st, dir) = with_files(ThemeChoice::Dark, "stash-push-header-menu", &UNSTAGED[..1], &STAGED[..1]);
    let mut h = harness(st, true);
    h.run();
    right_click(&mut h, "Staged files (1)");
    assert!(h.query_by_label("Stash staged changes…").is_some());
    assert_eq!(h.get_all_by_label("Unstage all").count(), 2, "the header's button and the menu item");
    h.check_snapshot("staged_header_menu");
    h.get_by_label("Stash staged changes…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Stash(StashMsg::PushDialog { scope: StashScope::Staged, path: None }))));
    drop(h);

    // The header's own button keeps its clicks.
    let (st, _) = with_files(ThemeChoice::Dark, "stash-push-header-menu", &UNSTAGED[..1], &STAGED[..1]);
    let mut h = harness(st, true);
    h.run();
    let r = h.get_by_label("Unstage all").bounding_box().unwrap();
    click(&mut h, pos2(((r.x0 + r.x1) / 2.0) as f32, ((r.y0 + r.y1) / 2.0) as f32), PointerButton::Primary);
    assert!(pushed(&h, &|m| matches!(m, Msg::Stage(StageMsg::UnstageAll))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn wip_row_stash_item() {
    let (st, dir) = with_files(ThemeChoice::Dark, "stash-push-row", &UNSTAGED, &STAGED);
    let mut h = harness(st, true);
    h.run();
    right_click(&mut h, "b.txt");
    assert!(!h.get_by_label("Stash this file…").is_disabled());
    h.get_by_label("Stash this file…").click();
    h.run();
    let opens = |m: &Msg| matches!(m, Msg::Stash(StashMsg::PushDialog { scope: StashScope::Files, path: Some(p) }) if p == "b.txt");
    assert!(pushed(&h, &opens));
    drop(h);

    let mut conflicted = UNSTAGED.to_vec();
    conflicted.push(("merge.txt", '!'));
    let (st, _) = with_files(ThemeChoice::Dark, "stash-push-row", &conflicted, &STAGED);
    let mut h = harness(st, true);
    h.run();
    right_click(&mut h, "b.txt");
    assert!(h.get_by_label("Stash this file…").is_disabled());
    drop(h);
    remove_fixture(&dir);
}
