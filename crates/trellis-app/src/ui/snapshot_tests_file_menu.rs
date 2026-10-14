//! The file actions in file rows' context menus, drawn as menu bodies in a small window: a
//! commit's modified file, a file the commit deleted (not in the working tree), a row comparing
//! two commits, and working-tree rows (a modified file, one whose file is gone, a conflicted
//! one, and a staged new file present or gone, whose delete also unstages it). New file in this
//! folder is enabled on every row. Clicks check the messages the items
//! send.

use super::file_menu::{
    DELETE, NEW_FILE_HERE, OPEN_DEFAULT, RENAME, RowSource, SHOW_IN_FOLDER, UNSTAGE_DELETE, commit_file_items, wip_file_items,
};
use super::theme::{apply_theme, color};
use crate::msg::Msg;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use trellis_core::repo::FileEntry;

const OID: &str = "3f2a9c41d07be5a8c6f19e2d4b7a0c3e5f816d29";
/// The newer commit of a compared pair (`OID` is the older one).
const NEWER: &str = "8c1d5e07a94f2b63d0e7c1a5b9f4d28e6a3c7b10";

#[derive(Clone, Copy)]
enum Menu {
    Commit {
        in_workdir: bool,
    },
    /// A row comparing `OID` (older) with `NEWER`.
    Between {
        in_workdir: bool,
    },
    /// `unstages`: deleting the row's path also takes it out of the index.
    Wip {
        missing: bool,
        unstages: bool,
    },
}

fn oid() -> Oid {
    Oid::from_str(OID).unwrap()
}

fn newer() -> Oid {
    Oid::from_str(NEWER).unwrap()
}

/// A small window drawing the extra items for `f` inside a menu frame, as the popup would.
fn harness(f: FileEntry, menu: Menu, dark: bool) -> Harness<'static, Vec<Msg>> {
    // The comparison's restore item names a commit, so it needs a wider window.
    let width = if matches!(menu, Menu::Between { .. }) { 300.0 } else { 260.0 };
    Harness::builder().with_size(egui::vec2(width, 240.0)).build_ui_state(
        move |ui, msgs: &mut Vec<Msg>| {
            color::set_dark(dark);
            apply_theme(ui.ctx());
            ui.ctx().set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
            ui.painter().rect_filled(ui.max_rect(), 0.0, color::bg());
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.set_width(width - 40.0);
                menu_style(ui.style_mut());
                // The row's existing items, which come first in the real menu.
                let _ = ui.button(if matches!(menu, Menu::Wip { .. }) { "Stage" } else { "File history" });
                let _ = ui.button("Copy path");
                match menu {
                    Menu::Commit { in_workdir } => commit_file_items(ui, msgs, RowSource::Commit(oid()), &f, in_workdir),
                    Menu::Between { in_workdir } => {
                        commit_file_items(ui, msgs, RowSource::Between { older: oid(), newer: newer() }, &f, in_workdir)
                    }
                    Menu::Wip { missing, unstages } => wip_file_items(ui, msgs, &f, missing, unstages),
                }
            });
        },
        Vec::new(),
    )
}

/// What egui does to a context menu's style, so the items look as they do in the app.
pub(super) fn menu_style(style: &mut egui::Style) {
    style.wrap_mode = Some(egui::TextWrapMode::Extend);
    style.spacing.button_padding = egui::vec2(2.0, 0.0);
    style.visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    style.visuals.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
}

fn entry(path: &str, status: char) -> FileEntry {
    FileEntry { path: path.into(), status }
}

#[test]
fn file_menu_commit() {
    let mut h = harness(entry("src/app.txt", 'M'), Menu::Commit { in_workdir: true }, true);
    h.run();
    h.snapshot("file_menu_commit");
    assert!(!h.get_by_label(OPEN_DEFAULT).is_disabled());
    assert!(!h.get_by_label(SHOW_IN_FOLDER).is_disabled());
    assert!(!h.get_by_label(NEW_FILE_HERE).is_disabled());
}

#[test]
fn file_menu_commit_deleted_light() {
    let mut h = harness(entry("old/notes.txt", 'D'), Menu::Commit { in_workdir: false }, false);
    h.run();
    h.snapshot("file_menu_commit_deleted_light");
    assert!(h.query_by_label("Restore the deleted file…").is_some());
    assert!(h.get_by_label(OPEN_DEFAULT).is_disabled());
    assert!(h.get_by_label(SHOW_IN_FOLDER).is_disabled());
    assert!(!h.get_by_label(NEW_FILE_HERE).is_disabled(), "a sibling can still be created");
}

#[test]
fn file_menu_between_deleted() {
    let mut h = harness(entry("old/notes.txt", 'D'), Menu::Between { in_workdir: false }, true);
    h.run();
    h.snapshot("file_menu_between_deleted");
    assert!(h.query_by_label("Restore the version from 3f2a9c4…").is_some(), "the older commit, which has the file");
    assert!(h.get_by_label(OPEN_DEFAULT).is_disabled());
    assert!(h.get_by_label(SHOW_IN_FOLDER).is_disabled());
    assert!(!h.get_by_label(NEW_FILE_HERE).is_disabled());
}

#[test]
fn file_menu_between_restores_the_right_commit() {
    for (status, label, want) in [('D', "Restore the version from 3f2a9c4…", oid()), ('M', "Restore the version from 8c1d5e0…", newer())]
    {
        let mut h = harness(entry("src/app.txt", status), Menu::Between { in_workdir: true }, true);
        h.run();
        h.get_by_label(label).click();
        h.run();
        assert!(
            matches!(&h.state()[..], [Msg::RestoreFile { oid: o, path, deleted: false, confirmed: false, overwrite: false }] if *o == want && path == "src/app.txt"),
            "{status}"
        );
    }
}

#[test]
fn file_menu_wip() {
    let mut h = harness(entry("src/app.txt", 'M'), Menu::Wip { missing: false, unstages: false }, true);
    h.run();
    h.snapshot("file_menu_wip");
    for label in [OPEN_DEFAULT, SHOW_IN_FOLDER, NEW_FILE_HERE, RENAME, DELETE] {
        assert!(!h.get_by_label(label).is_disabled(), "{label}");
    }
}

#[test]
fn file_menu_wip_missing() {
    let mut h = harness(entry("src/gone.txt", 'D'), Menu::Wip { missing: true, unstages: false }, true);
    h.run();
    h.snapshot("file_menu_wip_missing");
    for label in [OPEN_DEFAULT, SHOW_IN_FOLDER, RENAME, DELETE] {
        assert!(h.get_by_label(label).is_disabled(), "{label}");
    }
    assert!(!h.get_by_label(NEW_FILE_HERE).is_disabled());
}

#[test]
fn file_menu_wip_conflicted() {
    let mut h = harness(entry("src/both.txt", '!'), Menu::Wip { missing: false, unstages: false }, true);
    h.run();
    assert!(!h.get_by_label(OPEN_DEFAULT).is_disabled());
    assert!(!h.get_by_label(NEW_FILE_HERE).is_disabled());
    assert!(h.get_by_label(RENAME).is_disabled());
    assert!(h.get_by_label(DELETE).is_disabled());
    drop(h);

    let mut h = harness(entry("src/both.txt", '!'), Menu::Wip { missing: false, unstages: true }, true);
    h.run();
    assert!(h.get_by_label(UNSTAGE_DELETE).is_disabled());
}

/// Clicks the "Unstage and delete…" item and checks it asks to delete `path`.
fn click_unstage_delete(mut h: Harness<'static, Vec<Msg>>, path: &str) {
    h.get_by_label(UNSTAGE_DELETE).click();
    h.run();
    assert!(matches!(&h.state()[..], [Msg::DeleteFile { path: p, confirmed: false }] if p == path));
}

#[test]
fn file_menu_wip_staged_new() {
    let mut h = harness(entry("src/new.txt", 'A'), Menu::Wip { missing: false, unstages: true }, true);
    h.run();
    h.snapshot("file_menu_wip_staged_new");
    assert!(!h.get_by_label(UNSTAGE_DELETE).is_disabled());
    assert!(h.query_by_label(DELETE).is_none());
    for label in [OPEN_DEFAULT, SHOW_IN_FOLDER, NEW_FILE_HERE, RENAME] {
        assert!(!h.get_by_label(label).is_disabled(), "{label}");
    }
    click_unstage_delete(h, "src/new.txt");
}

#[test]
fn file_menu_wip_staged_new_gone() {
    let mut h = harness(entry("src/new.txt", 'D'), Menu::Wip { missing: true, unstages: true }, true);
    h.run();
    h.snapshot("file_menu_wip_staged_new_gone");
    for label in [OPEN_DEFAULT, SHOW_IN_FOLDER, RENAME] {
        assert!(h.get_by_label(label).is_disabled(), "{label}");
    }
    for label in [UNSTAGE_DELETE, NEW_FILE_HERE] {
        assert!(!h.get_by_label(label).is_disabled(), "{label}");
    }
    assert!(h.query_by_label(DELETE).is_none());
    click_unstage_delete(h, "src/new.txt");
}

#[test]
fn file_menu_clicks_send_msgs() {
    let mut h = harness(entry("src/app.txt", 'M'), Menu::Commit { in_workdir: true }, true);
    h.run();
    h.get_by_label("Restore this version…").click();
    h.run();
    assert!(
        matches!(&h.state()[..], [Msg::RestoreFile { oid: o, path, deleted: false, confirmed: false, overwrite: false }] if *o == oid() && path == "src/app.txt")
    );
    drop(h);

    let mut h = harness(entry("src/app.txt", 'M'), Menu::Wip { missing: false, unstages: false }, true);
    h.run();
    h.get_by_label(SHOW_IN_FOLDER).click();
    h.run();
    assert!(matches!(&h.state()[..], [Msg::ShowWorkingFile(p)] if p == "src/app.txt"));
    drop(h);

    let mut h = harness(entry("src/app.txt", 'M'), Menu::Wip { missing: false, unstages: false }, true);
    h.run();
    h.get_by_label(RENAME).click();
    h.run();
    assert!(matches!(&h.state()[..], [Msg::RenameFileDialog(p)] if p == "src/app.txt"));
    drop(h);

    let mut h = harness(entry("src/app.txt", 'M'), Menu::Wip { missing: false, unstages: false }, true);
    h.run();
    h.get_by_label(DELETE).click();
    h.run();
    assert!(matches!(&h.state()[..], [Msg::DeleteFile { path, confirmed: false }] if path == "src/app.txt"));
    drop(h);

    // New file in this folder: the row's folder, "" at the root, and a folder the commit
    // deleted from the working tree.
    for (path, status, menu, want) in [
        ("src/app.txt", 'M', Menu::Wip { missing: false, unstages: false }, "src/"),
        ("README.md", 'M', Menu::Commit { in_workdir: true }, ""),
        ("old/notes.txt", 'D', Menu::Commit { in_workdir: false }, "old/"),
    ] {
        let mut h = harness(entry(path, status), menu, true);
        h.run();
        h.get_by_label(NEW_FILE_HERE).click();
        h.run();
        assert!(matches!(&h.state()[..], [Msg::NewFileDialog(p)] if p == want), "{path}");
        drop(h);
    }

    // A disabled item sends nothing.
    let mut h = harness(entry("src/gone.txt", 'D'), Menu::Wip { missing: true, unstages: false }, true);
    h.run();
    h.get_by_label(OPEN_DEFAULT).click();
    h.run();
    assert!(h.state().is_empty());
}
