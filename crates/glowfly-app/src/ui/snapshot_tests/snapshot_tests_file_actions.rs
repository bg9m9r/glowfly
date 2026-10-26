//! File action buttons in headers: the file history's selected commit (Restore this version,
//! Open with default app, Show in folder), its row menu, and the commit diff header (Restore…,
//! Open, Show; Open and Show on working-tree diffs). Covers enablement when the file is gone
//! from the working tree, the messages clicks send, and that comparison diff headers gain no
//! buttons. Shares the fixture repository and renderer with `snapshot_tests`.

use super::file_menu::{NOT_IN_WORKDIR, OPEN_DEFAULT, SHOW_IN_FOLDER};
use super::history::{BLAME_AT, entry_menu};
use super::snapshot_tests::{harness, harness_sized, remove_fixture, with_history, with_repo};
use super::snapshot_tests_file_menu::menu_style;
use super::snapshot_tests_revert::with_commit_diff;
use super::theme::{apply_theme, color};
use crate::msg::BlameMsg;
use crate::msg::FilesMsg;
use crate::msg::HistoryMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::repo::{self, DiffTarget, HistoryEntry, LoadOptions};
use glowfly_core::settings::{DiffLayout, ThemeChoice};
use std::path::Path;

const RESTORE: &str = "Restore this version…";
const OPEN_IN_EDITOR: &str = "Open in external editor";
const REVERT_FILE: &str = "Revert changes to this file";
/// The diff header's labelled file actions and its Close control.
const RESTORE_SHORT: &str = "Restore…";
const OPEN_SHORT: &str = "Open";
const CLOSE_DIFF: &str = "Close diff";
/// A window wide enough for the diff header to label its file actions rather than fold them
/// into its "More file actions" menu.
const WIDE: egui::Vec2 = egui::vec2(1800.0, 820.0);

/// Takes src/app.txt out of the working tree and reloads status, which then lists it as deleted.
fn remove_app(st: &mut AppState, dir: &Path) {
    std::fs::remove_file(dir.join("src/app.txt")).unwrap();
    st.snap = Some(repo::load(dir, &LoadOptions::all(500)).unwrap());
}

/// Clicks `label` and returns the messages it sent, leaving none behind for the next click.
fn click(h: &mut Harness<'static, (AppState, Vec<Msg>)>, label: &str) -> Vec<Msg> {
    h.get_by_label(label).click();
    h.run();
    std::mem::take(&mut h.state_mut().1)
}

#[test]
fn file_history_header_buttons() {
    let (st, dir) = with_history("history-buttons", ThemeChoice::Dark, 500, 0);
    let mut h = harness(st, true);
    h.run();
    for label in [RESTORE, OPEN_DEFAULT, SHOW_IN_FOLDER] {
        assert!(!h.get_by_label(label).is_disabled(), "{label}");
    }
    h.check_snapshot("file_history_header_buttons");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_history_not_on_disk_light() {
    let (mut st, dir) = with_history("history-not-on-disk", ThemeChoice::Light, 500, 0);
    remove_app(&mut st, &dir);
    assert!(st.session.history.view.as_ref().unwrap().on_disk, "the walk saw it, but status now says it is gone");
    let mut h = harness(st, false);
    h.run();
    assert!(h.get_by_label(OPEN_DEFAULT).is_disabled());
    assert!(h.get_by_label(SHOW_IN_FOLDER).is_disabled());
    assert!(!h.get_by_label(RESTORE).is_disabled(), "restoring brings it back");
    h.check_snapshot("file_history_not_on_disk_light");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_history_buttons_send_msgs() {
    // Entry 2 is from before the move, when the file was app.txt.
    let (st, dir) = with_history("history-buttons-click", ThemeChoice::Dark, 500, 2);
    let e = st.session.history.view.as_ref().unwrap().entries[2].clone();
    assert_eq!(e.path, "app.txt");
    let mut h = harness(st, true);
    h.run();
    let msgs = click(&mut h, RESTORE);
    assert!(
        matches!(&msgs[..], [Msg::Files(FilesMsg::RestoreFile { oid, path, deleted: false, confirmed: false, overwrite: false })] if *oid == e.oid && path == "app.txt"),
        "restore"
    );
    // Open and Show act on the file as it is named today.
    let msgs = click(&mut h, OPEN_DEFAULT);
    assert!(matches!(&msgs[..], [Msg::Files(FilesMsg::OpenWorkingFile(p))] if p == "src/app.txt"));
    let msgs = click(&mut h, SHOW_IN_FOLDER);
    assert!(matches!(&msgs[..], [Msg::Files(FilesMsg::ShowWorkingFile(p))] if p == "src/app.txt"));
    // The arrow keys still move through the list after a button click.
    h.press_key(egui::Key::ArrowUp);
    h.run();
    assert!(matches!(&h.state().1[..], [Msg::History(HistoryMsg::SelectHistoryEntry(1))]), "Up still moves the selection");
    h.state_mut().1.clear();

    remove_app(&mut h.state_mut().0, &dir);
    h.run();
    assert!(click(&mut h, OPEN_DEFAULT).is_empty(), "disabled when the file is gone");
    drop(h);
    remove_fixture(&dir);
}

fn history_entry(status: char) -> HistoryEntry {
    HistoryEntry {
        oid: Oid::from_str("3f2a9c41d07be5a8c6f19e2d4b7a0c3e5f816d29").unwrap(),
        summary: "Move app settings under src".into(),
        author: "Test".into(),
        email: "test@example.com".into(),
        time: 0,
        path: "app.txt".into(),
        old_path: None,
        status,
        files: Vec::new(),
    }
}

/// A small window drawing a history row's menu body for `e`, as the popup would.
fn menu_harness(e: HistoryEntry, in_workdir: bool) -> Harness<'static, Vec<Msg>> {
    crate::ui::snapshot_check::builder().with_size(egui::vec2(440.0, 230.0)).build_ui_state(
        move |ui, msgs: &mut Vec<Msg>| {
            color::set_dark(true);
            apply_theme(ui.ctx());
            ui.ctx().set_theme(egui::Theme::Dark);
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                menu_style(ui.style_mut());
                let en = glowfly_core::i18n::Lang::En;
                entry_menu(ui, en, msgs, &e, "src/app.txt", in_workdir, crate::state::tool_compare::ToolSetup::of(en, &Default::default()));
            });
        },
        Vec::new(),
    )
}

#[test]
fn history_entry_menu() {
    let mut h = menu_harness(history_entry('M'), true);
    h.run();
    h.check_snapshot("history_entry_menu");
    let e = history_entry('M');
    h.get_by_label(BLAME_AT).click();
    h.run();
    assert!(matches!(&h.state()[..], [Msg::Blame(BlameMsg::Open { path, at: Some(o) })] if *path == e.path && *o == e.oid));
    h.state_mut().clear();
    assert!(!h.get_by_label(SHOW_IN_FOLDER).is_disabled());
    assert!(!h.get_by_label(OPEN_IN_EDITOR).is_disabled());
    h.get_by_label(OPEN_DEFAULT).click();
    h.run();
    assert!(matches!(&h.state()[..], [Msg::Files(FilesMsg::OpenWorkingFile(p))] if p == "src/app.txt"), "the current path");
    drop(h);

    let mut h = menu_harness(history_entry('M'), true);
    h.run();
    h.get_by_label(REVERT_FILE).click();
    h.run();
    let want = DiffTarget::Commit(e.oid, e.path.clone());
    assert!(matches!(&h.state()[..], [Msg::Stage(crate::msg::StageMsg::RevertFile(t))] if *t == want), "the entry's change");
    drop(h);

    let mut h = menu_harness(history_entry('M'), false);
    h.run();
    assert!(h.get_by_label(REVERT_FILE).is_disabled());
    assert!(h.get_by_label(OPEN_DEFAULT).is_disabled());
    assert!(h.get_by_label(OPEN_IN_EDITOR).is_disabled());
    assert!(h.get_by_label(SHOW_IN_FOLDER).is_disabled());
    drop(h);

    let mut h = menu_harness(history_entry('D'), true);
    h.run();
    assert!(h.get_by_label(BLAME_AT).is_disabled(), "the commit deletes the file");
    assert!(!h.get_by_label(REVERT_FILE).is_disabled(), "brings the deleted file back");
    h.get_by_label("Restore the deleted file…").click();
    h.run();
    assert!(
        matches!(&h.state()[..], [Msg::Files(FilesMsg::RestoreFile { path, deleted: true, confirmed: false, overwrite: false, .. })] if path == "app.txt")
    );
}

#[test]
fn diff_commit_header_buttons() {
    let (st, dir) = with_commit_diff("diff-commit-actions", ThemeChoice::Dark, DiffLayout::Inline, false);
    let oid = st.detail.as_ref().unwrap().oid;
    let mut h = harness_sized(st, true, WIDE);
    h.run_steps(4);
    h.check_snapshot("diff_commit_header_buttons");
    let msgs = click(&mut h, RESTORE_SHORT);
    assert!(
        matches!(&msgs[..], [Msg::Files(FilesMsg::RestoreFile { oid: o, path, deleted: false, confirmed: false, overwrite: false })] if *o == oid && path == "app.txt"),
        "restore"
    );
    let msgs = click(&mut h, OPEN_SHORT);
    assert!(matches!(&msgs[..], [Msg::Files(FilesMsg::OpenWorkingFile(p))] if p == "app.txt"));
    let msgs = click(&mut h, SHOW_IN_FOLDER);
    assert!(matches!(&msgs[..], [Msg::Files(FilesMsg::ShowWorkingFile(p))] if p == "app.txt"));

    h.state_mut().0.detail.as_mut().unwrap().absent.insert("app.txt".into());
    h.run_steps(4);
    assert!(h.get_by_label(OPEN_SHORT).is_disabled() && h.get_by_label(SHOW_IN_FOLDER).is_disabled());
    assert!(!h.get_by_label(RESTORE_SHORT).is_disabled());

    h.state_mut().0.detail = None;
    h.run_steps(4);
    assert!(h.query_by_label(RESTORE_SHORT).is_none(), "no detail lists the file");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn working_tree_diff_header_opens_the_file() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "diff-wip-actions");
    let target = DiffTarget::Unstaged("README.md".into());
    let diff = repo::load_diff(&dir, &target).unwrap();
    // The commit that changed app.txt and its parent.
    let fix = st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == "Fix build on Windows").expect("the fix commit");
    let (newer, older) = (fix.oid, fix.parents[0]);
    st.diff = Some((target, diff.into()));
    let mut h = harness_sized(st, true, WIDE);
    h.run();
    assert!(h.query_by_label("Stage file").is_some());
    assert!(h.query_by_label(OPEN_SHORT).is_some() && h.query_by_label(SHOW_IN_FOLDER).is_some(), "the working file opens");
    assert!(h.query_by_label(RESTORE_SHORT).is_none());
    let msgs = click(&mut h, OPEN_SHORT);
    assert!(matches!(&msgs[..], [Msg::Files(FilesMsg::OpenWorkingFile(p))] if p == "README.md"));

    // Comparing two commits.
    let target = DiffTarget::Between(older, newer, "app.txt".into());
    let diff = repo::load_diff(&dir, &target).unwrap();
    h.state_mut().0.diff = Some((target, diff.into()));
    h.run();
    assert!(h.query_by_label(CLOSE_DIFF).is_some(), "the header is drawn");
    assert!(h.query_by_label(OPEN_SHORT).is_none() && h.query_by_label(RESTORE_SHORT).is_none());
    assert!(h.query_by_label(SHOW_IN_FOLDER).is_none() && h.query_by_label(NOT_IN_WORKDIR).is_none());
    drop(h);
    remove_fixture(&dir);
}

/// Long lists draw only the rows in view: thousands of changed files or local branches lay out
/// the visible rows, and the rest stay out of the widget tree.
#[test]
fn long_lists_draw_only_visible_rows() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "long-lists");
    let snap = st.snap.as_mut().unwrap();
    let file = |i: usize| glowfly_core::repo::FileEntry::new(format!("gen/file{i}.txt"), 'M');
    snap.unstaged = (0..3000).map(file).collect();
    let branch = snap.locals[0].clone();
    snap.locals =
        (0..3000).map(|i| glowfly_core::repo::BranchInfo { name: format!("topic-{i}"), is_head: false, ..branch.clone() }).collect();
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("gen/file0.txt").is_some());
    assert!(h.query_by_label("gen/file2999.txt").is_none(), "an unstaged row out of view isn't drawn");
    assert!(h.query_by_label("topic-0").is_some());
    assert!(h.query_by_label("topic-2999").is_none(), "a branch row out of view isn't drawn");
    remove_fixture(&dir);
}
