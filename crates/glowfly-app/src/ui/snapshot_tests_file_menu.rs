//! The file actions in file rows' context menus, drawn as menu bodies in a small window: a
//! commit's modified file, a file the commit deleted (not in the working tree), a row comparing
//! two commits, and working-tree rows (a modified file, one whose file is gone, a conflicted
//! one, and a staged new file present or gone, whose delete also unstages it). New file in this
//! folder is enabled on every row. Open in external diff tool (Resolve in external merge tool
//! on a conflicted row) and Open in external editor stay enabled with "Set up…" until a program
//! is set (they then open Preferences). Disabled items are greyed with an info mark. Revert
//! changes to this file needs the file in the working tree, unless the commit deleted it. Clicks
//! check the messages the items send.

use super::file_menu::{OPEN_DEFAULT, RowSource, SHOW_IN_FOLDER, ToolItem, commit_file_items, diff_item, wip_file_items};
use super::theme::{apply_theme, color};
use crate::msg::FilesMsg;
use crate::msg::Msg;
use crate::msg::WorkFileMsg;
use crate::state::tool_compare::ToolSetup;
use crate::tools::ToolsMsg;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{DiffTarget, FileEntry};

// The English texts the menus show, pinned.
const DELETE: &str = "Delete…";
const NEW_FILE_HERE: &str = "New file in this folder…";
const OPEN_IN_DIFF_TOOL: &str = "Open in external diff tool";
const OPEN_IN_EDITOR: &str = "Open in external editor";
const RENAME: &str = "Rename…";
const RESOLVE_IN_MERGE_TOOL: &str = "Resolve in external merge tool";
const REVERT_FILE: &str = "Revert changes to this file";
const TRACK_IN_LFS: &str = "Track in Git LFS";
const UNSTAGE_DELETE: &str = "Unstage and delete…";

const OID: &str = "3f2a9c41d07be5a8c6f19e2d4b7a0c3e5f816d29";
/// The newer commit of a compared pair (`OID` is the older one).
const NEWER: &str = "8c1d5e07a94f2b63d0e7c1a5b9f4d28e6a3c7b10";

#[derive(Clone, Copy)]
pub(super) enum Menu {
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

/// A small window drawing the extra items for `f` inside a menu frame, as the popup would, with
/// no diff or merge tool set.
fn harness(f: FileEntry, menu: Menu, dark: bool) -> Harness<'static, Vec<Msg>> {
    harness_with(f, menu, dark, false)
}

/// `harness`, with the diff and merge tools set when `tools`.
fn harness_with(f: FileEntry, menu: Menu, dark: bool, tools: bool) -> Harness<'static, Vec<Msg>> {
    harness_in(Lang::En, f, menu, dark, tools)
}

/// `harness_with` in the language `t`.
pub(super) fn harness_in(t: Lang, f: FileEntry, menu: Menu, dark: bool, tools: bool) -> Harness<'static, Vec<Msg>> {
    let setup = if tools { ToolSetup::default() } else { ToolSetup::of(t, &glowfly_core::settings::ExternalTools::default()) };
    // The menu grows to fit its widest item, a disabled one's reason included.
    let width = 440.0;
    Harness::builder().with_size(egui::vec2(width, 260.0)).build_ui_state(
        move |ui, msgs: &mut Vec<Msg>| {
            color::set_dark(dark);
            apply_theme(ui.ctx());
            ui.ctx().set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
            ui.painter().rect_filled(ui.max_rect(), 0.0, color::bg());
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.set_min_width(220.0);
                menu_style(ui.style_mut());
                // The row's existing items, which come first in the real menu.
                let first =
                    if matches!(menu, Menu::Wip { .. }) { t.tr("details.wip.stage.button") } else { t.tr("files.menu.file_history") };
                let _ = ui.button(first);
                let _ = ui.button(t.tr("common.copy_path"));
                match menu {
                    Menu::Commit { in_workdir } => commit_file_items(
                        ui,
                        t,
                        msgs,
                        RowSource::Commit(oid()),
                        &f,
                        in_workdir,
                        &[],
                        setup,
                        None,
                        &crate::state::custom_commands::MenuCommands::none(),
                    ),
                    Menu::Between { in_workdir } => {
                        let src = RowSource::Between { from: oid(), to: newer() };
                        commit_file_items(
                            ui,
                            t,
                            msgs,
                            src,
                            &f,
                            in_workdir,
                            &[],
                            setup,
                            None,
                            &crate::state::custom_commands::MenuCommands::none(),
                        )
                    }
                    Menu::Wip { missing, unstages } => {
                        let tool = if f.status == '!' {
                            let msg = Msg::Tools(ToolsMsg::ResolveInMergeTool(f.path.clone()));
                            ToolItem { label: t.tr("tools.resolve_in_merge_tool"), msg, blocked: None, setup: setup.merge }
                        } else {
                            diff_item(t, DiffTarget::Unstaged(f.path.clone()), setup.diff)
                        };
                        wip_file_items(
                            ui,
                            t,
                            msgs,
                            &f,
                            missing,
                            unstages,
                            &[],
                            setup.editor,
                            tool,
                            &crate::state::custom_commands::MenuCommands::none(),
                        )
                    }
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
    FileEntry::new(path, status)
}

#[test]
fn file_menu_commit() {
    let mut h = harness(entry("src/app.txt", 'M'), Menu::Commit { in_workdir: true }, true);
    h.run();
    h.check_snapshot("file_menu_commit");
    assert!(!h.get_by_label(OPEN_DEFAULT).is_disabled());
    assert!(!h.get_by_label(OPEN_IN_EDITOR).is_disabled());
    assert!(!h.get_by_label(SHOW_IN_FOLDER).is_disabled());
    assert!(!h.get_by_label(NEW_FILE_HERE).is_disabled());
    assert!(h.query_by_label(TRACK_IN_LFS).is_none(), "no submenu without entries");
}

#[test]
fn file_menu_commit_deleted_light() {
    let mut h = harness(entry("old/notes.txt", 'D'), Menu::Commit { in_workdir: false }, false);
    h.run();
    h.check_snapshot("file_menu_commit_deleted_light");
    assert!(h.query_by_label("Restore the deleted file…").is_some());
    assert!(h.get_by_label(OPEN_DEFAULT).is_disabled());
    assert!(h.get_by_label(OPEN_IN_EDITOR).is_disabled());
    assert!(h.get_by_label(SHOW_IN_FOLDER).is_disabled());
    assert!(!h.get_by_label(NEW_FILE_HERE).is_disabled(), "a sibling can still be created");
}

#[test]
fn file_menu_between_deleted() {
    let mut h = harness(entry("old/notes.txt", 'D'), Menu::Between { in_workdir: false }, true);
    h.run();
    h.check_snapshot("file_menu_between_deleted");
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
            matches!(&h.state()[..], [Msg::Files(FilesMsg::RestoreFile { oid: o, path, deleted: false, confirmed: false, overwrite: false })] if *o == want && path == "src/app.txt"),
            "{status}"
        );
    }
}

#[test]
fn revert_file_sends_the_row_target() {
    use crate::msg::StageMsg;
    let cases = [
        (Menu::Commit { in_workdir: true }, DiffTarget::Commit(oid(), "src/app.txt".into())),
        (Menu::Between { in_workdir: true }, DiffTarget::Between(oid(), newer(), "src/app.txt".into())),
    ];
    for (menu, want) in cases {
        let mut h = harness(entry("src/app.txt", 'M'), menu, true);
        h.run();
        h.get_by_label(REVERT_FILE).click();
        h.run();
        assert!(matches!(&h.state()[..], [Msg::Stage(StageMsg::RevertFile(t))] if *t == want), "{want:?}");
    }
    // Not in the working tree: disabled, unless the commit deleted the file (it is brought back).
    for (status, disabled) in [('M', true), ('A', true), ('D', false)] {
        for menu in [Menu::Commit { in_workdir: false }, Menu::Between { in_workdir: false }] {
            let mut h = harness(entry("src/app.txt", status), menu, true);
            h.run();
            assert_eq!(h.get_by_label(REVERT_FILE).is_disabled(), disabled, "{status}");
        }
    }
}

#[test]
fn file_menu_wip() {
    let mut h = harness(entry("src/app.txt", 'M'), Menu::Wip { missing: false, unstages: false }, true);
    h.run();
    h.check_snapshot("file_menu_wip");
    for label in [OPEN_DEFAULT, OPEN_IN_EDITOR, SHOW_IN_FOLDER, NEW_FILE_HERE, RENAME, DELETE] {
        assert!(!h.get_by_label(label).is_disabled(), "{label}");
    }
    assert!(h.query_by_label(TRACK_IN_LFS).is_none(), "no submenu without entries");
}

#[test]
fn file_menu_wip_missing() {
    let mut h = harness(entry("src/gone.txt", 'D'), Menu::Wip { missing: true, unstages: false }, true);
    h.run();
    h.check_snapshot("file_menu_wip_missing");
    for label in [OPEN_DEFAULT, OPEN_IN_EDITOR, SHOW_IN_FOLDER, RENAME, DELETE] {
        assert!(h.get_by_label(label).is_disabled(), "{label}");
    }
    assert!(!h.get_by_label(NEW_FILE_HERE).is_disabled());
}

#[test]
fn file_menu_wip_conflicted() {
    let mut h = harness(entry("src/both.txt", '!'), Menu::Wip { missing: false, unstages: false }, true);
    h.run();
    h.check_snapshot("file_menu_wip_conflicted");
    assert!(!h.get_by_label(RESOLVE_IN_MERGE_TOOL).is_disabled(), "no merge tool set: it opens Preferences");
    assert!(h.query_by_label(OPEN_IN_DIFF_TOOL).is_none());
    assert!(!h.get_by_label(OPEN_DEFAULT).is_disabled());
    assert!(!h.get_by_label(NEW_FILE_HERE).is_disabled());
    assert!(h.get_by_label(RENAME).is_disabled());
    assert!(h.get_by_label(DELETE).is_disabled());
    h.get_by_label(RESOLVE_IN_MERGE_TOOL).click();
    h.run();
    assert!(matches!(&h.state()[..], [Msg::Tools(ToolsMsg::ResolveInMergeTool(p))] if p == "src/both.txt"));
    drop(h);

    let mut h = harness(entry("src/both.txt", '!'), Menu::Wip { missing: false, unstages: true }, true);
    h.run();
    assert!(h.get_by_label(UNSTAGE_DELETE).is_disabled());
    drop(h);

    let mut h = harness_with(entry("src/both.txt", '!'), Menu::Wip { missing: false, unstages: false }, true, true);
    h.run();
    h.get_by_label(RESOLVE_IN_MERGE_TOOL).click();
    h.run();
    assert!(matches!(&h.state()[..], [Msg::Tools(ToolsMsg::ResolveInMergeTool(p))] if p == "src/both.txt"));
}

#[test]
fn diff_tool_entry_without_a_tool_is_enabled() {
    // Enabled without a tool too: update opens Preferences > External Tools.
    for menu in [Menu::Commit { in_workdir: true }, Menu::Wip { missing: false, unstages: false }] {
        let mut h = harness(entry("src/app.txt", 'M'), menu, true);
        h.run();
        assert!(!h.get_by_label(OPEN_IN_DIFF_TOOL).is_disabled());
        h.get_by_label(OPEN_IN_DIFF_TOOL).click();
        h.run();
        assert!(matches!(&h.state()[..], [Msg::Tools(ToolsMsg::OpenDiff(_))]));
    }
}

#[test]
fn diff_tool_items_send_their_targets() {
    // Enabled, even for a file gone from the working tree.
    let cases = [
        (Menu::Commit { in_workdir: false }, DiffTarget::Commit(oid(), "src/app.txt".into())),
        (Menu::Between { in_workdir: true }, DiffTarget::Between(oid(), newer(), "src/app.txt".into())),
        (Menu::Wip { missing: true, unstages: false }, DiffTarget::Unstaged("src/app.txt".into())),
    ];
    for (menu, want) in cases {
        let mut h = harness_with(entry("src/app.txt", 'M'), menu, true, true);
        h.run();
        assert!(!h.get_by_label(OPEN_IN_DIFF_TOOL).is_disabled());
        h.get_by_label(OPEN_IN_DIFF_TOOL).click();
        h.run();
        assert!(matches!(&h.state()[..], [Msg::Tools(ToolsMsg::OpenDiff(t))] if *t == want), "{want:?}");
    }
}

/// Clicks the "Unstage and delete…" item and checks it asks to delete `path`.
fn click_unstage_delete(mut h: Harness<'static, Vec<Msg>>, path: &str) {
    h.get_by_label(UNSTAGE_DELETE).click();
    h.run();
    assert!(matches!(&h.state()[..], [Msg::WorkFile(WorkFileMsg::DeleteFile { path: p, confirmed: false })] if p == path));
}

#[test]
fn file_menu_wip_staged_new() {
    let mut h = harness(entry("src/new.txt", 'A'), Menu::Wip { missing: false, unstages: true }, true);
    h.run();
    h.check_snapshot("file_menu_wip_staged_new");
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
    h.check_snapshot("file_menu_wip_staged_new_gone");
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
        matches!(&h.state()[..], [Msg::Files(FilesMsg::RestoreFile { oid: o, path, deleted: false, confirmed: false, overwrite: false })] if *o == oid() && path == "src/app.txt")
    );
    drop(h);

    let mut h = harness(entry("src/app.txt", 'M'), Menu::Wip { missing: false, unstages: false }, true);
    h.run();
    h.get_by_label(SHOW_IN_FOLDER).click();
    h.run();
    assert!(matches!(&h.state()[..], [Msg::Files(FilesMsg::ShowWorkingFile(p))] if p == "src/app.txt"));
    drop(h);

    for menu in [Menu::Wip { missing: false, unstages: false }, Menu::Commit { in_workdir: true }] {
        let mut h = harness(entry("src/app.txt", 'M'), menu, true);
        h.run();
        h.get_by_label(OPEN_IN_EDITOR).click();
        h.run();
        assert!(matches!(&h.state()[..], [Msg::Tools(ToolsMsg::OpenFile { path, line: None })] if path == "src/app.txt"));
        drop(h);
    }

    let mut h = harness(entry("src/app.txt", 'M'), Menu::Wip { missing: false, unstages: false }, true);
    h.run();
    h.get_by_label(RENAME).click();
    h.run();
    assert!(matches!(&h.state()[..], [Msg::WorkFile(WorkFileMsg::RenameFileDialog(p))] if p == "src/app.txt"));
    drop(h);

    let mut h = harness(entry("src/app.txt", 'M'), Menu::Wip { missing: false, unstages: false }, true);
    h.run();
    h.get_by_label(DELETE).click();
    h.run();
    assert!(matches!(&h.state()[..], [Msg::WorkFile(WorkFileMsg::DeleteFile { path, confirmed: false })] if path == "src/app.txt"));
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
        assert!(matches!(&h.state()[..], [Msg::WorkFile(WorkFileMsg::NewFileDialog(p))] if p == want), "{path}");
        drop(h);
    }

    // A disabled item sends nothing.
    let mut h = harness(entry("src/gone.txt", 'D'), Menu::Wip { missing: true, unstages: false }, true);
    h.run();
    h.get_by_label(OPEN_DEFAULT).click();
    h.run();
    assert!(h.state().is_empty());
}
