//! Renaming a stash: the STASHES row menu with Rename…, the Rename stash dialog (prefilled from a
//! WIP entry, blank in the light theme, waiting while a job runs) and the palette's rename
//! sub-list. The made-up stashes of `snapshot_tests_palette_stash` are injected into the shared
//! fixture's snapshot so the images are deterministic.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_palette_stash::fixture_stashes;
use super::snapshot_tests_remotes::running;
use super::snapshot_tests_select::click;
use super::snapshot_tests_worktrees::{H, pushed};
use crate::msg::{Msg, StashMsg};
use crate::state::palette::{Mode, Palette, Pick};
use crate::state::stashes::RenameStashForm;
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Key, PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::settings::ThemeChoice;

/// The fixture with the made-up stashes in its snapshot and STASHES in view.
fn with_stashes(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let stashes = fixture_stashes(&st);
    st.snap.as_mut().unwrap().stashes = stashes;
    st.view.sections.local = false;
    st.view.sections.remote = false;
    st.view.sections.stashes = true;
    (st, dir)
}

/// The fixture with the Rename stash dialog open for fixture stash `index`, changed by `edit`.
fn dialog(theme: ThemeChoice, name: &str, index: usize, edit: impl FnOnce(&mut RenameStashForm)) -> (AppState, PathBuf) {
    let (mut st, dir) = with_stashes(theme, name);
    let mut form = RenameStashForm::new(&st.snap.as_ref().unwrap().stashes[index]);
    edit(&mut form);
    st.modal = Some(Modal::RenameStash(form));
    (st, dir)
}

/// Right-clicks the sidebar row labelled `label` (the leftmost match).
fn open_menu(h: &mut H, label: &str) {
    let r = h.get_all_by_label(label).filter_map(|n| n.bounding_box()).min_by(|a, b| a.x0.total_cmp(&b.x0)).expect("the row");
    let at = pos2(((r.x0 + r.x1) / 2.0) as f32, ((r.y0 + r.y1) / 2.0) as f32);
    click(h, at, PointerButton::Secondary);
}

fn form(h: &H) -> &RenameStashForm {
    match &h.state().0.modal {
        Some(Modal::RenameStash(f)) => f,
        _ => panic!("the dialog is closed"),
    }
}

#[test]
fn stash_menu() {
    let (st, dir) = with_stashes(ThemeChoice::Dark, "stash-menu");
    let oid = st.snap.as_ref().unwrap().stashes[0].oid;
    let mut h = harness(st, true);
    h.run();
    open_menu(&mut h, "On main: login form tweaks");
    for item in ["Apply", "Pop", "Rename…", "Delete"] {
        // The toolbar has a Pop button too.
        let want = if item == "Pop" { 2 } else { 1 };
        assert_eq!(h.query_all_by_label(item).count(), want, "{item}");
    }
    h.check_snapshot("stash_menu");
    h.get_by_label("Rename…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Stash(StashMsg::RenameDialog { oid: o }) if *o == oid)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn rename_stash_dialog() {
    let (st, dir) = dialog(ThemeChoice::Dark, "rename-stash-dialog", 1, |_| {});
    let oid = st.snap.as_ref().unwrap().stashes[1].oid;
    let mut h = harness(st, true);
    h.run();
    assert_eq!(form(&h).message, "1a2b3c4 Add search");
    assert!(h.query_by_label("Rename stash").is_some());
    assert!(h.query_by_label("stash@{1} · made on main").is_some());
    assert!(h.query_by_label("Enter a message.").is_none());
    assert!(h.get_by_label("Rename").is_disabled(), "unchanged");
    h.check_snapshot("rename_stash_dialog");

    if let Some(Modal::RenameStash(f)) = h.state_mut().0.modal.as_mut() {
        f.message = "Add search box".into();
    }
    h.run();
    assert!(!h.get_by_label("Rename").is_disabled());
    h.press_key(Key::Enter);
    h.run();
    let renames = |m: &Msg| matches!(m, Msg::Stash(StashMsg::Rename { oid: o, message }) if *o == oid && message == "Add search box");
    assert!(pushed(&h, &renames));
    assert!(h.state().0.modal.is_some(), "update closes the dialog once the job starts");

    h.state_mut().1.clear();
    h.get_by_label("Cancel").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::CloseModal)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn rename_stash_dialog_blank_light() {
    let (st, dir) = dialog(ThemeChoice::Light, "rename-stash-dialog-blank", 0, |f| f.message = "  ".into());
    let mut h = harness(st, false);
    h.run();
    assert!(h.query_by_label("stash@{0} · made on main").is_some());
    assert!(h.query_by_label("Enter a message.").is_some());
    assert!(h.get_by_label("Rename").is_disabled());
    h.check_snapshot("rename_stash_dialog_blank_light");

    // A changed message still waits while a job runs (a spinner keeps repainting, so step).
    let st = &mut h.state_mut().0;
    if let Some(Modal::RenameStash(f)) = st.modal.as_mut() {
        f.message = "new text".into();
    }
    running(st);
    h.run_steps(2);
    assert!(h.query_by_label("Enter a message.").is_none());
    assert!(h.get_by_label("Rename").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn rename_stash_dialog_without_branch() {
    let (mut st, dir) = with_stashes(ThemeChoice::Dark, "rename-stash-dialog-plain");
    let mut s = st.snap.as_ref().unwrap().stashes[2].clone();
    s.message = "Trellis: discarded changes".into();
    st.modal = Some(Modal::RenameStash(RenameStashForm::new(&s)));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("stash@{2}").is_some());
    assert_eq!(form(&h).message, "Trellis: discarded changes", "no prefix: the whole text");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn palette_stash_rename() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-stash-rename");
    let pk = Pick::stashes_to_rename(&fixture_stashes(&st));
    st.palette = Some(Palette { mode: Mode::Pick(pk), selected: 1, ..Default::default() });
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Rename stash").is_some());
    assert!(h.query_by_label_contains("Enter to rename").is_some());
    h.check_snapshot("palette_stash_rename");
    drop(h);
    remove_fixture(&dir);
}
