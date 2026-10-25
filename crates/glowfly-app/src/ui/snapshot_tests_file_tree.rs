//! The changed-file lists as a tree: commit details (with a folder collapsed), the WIP lists and
//! a comparison of two commits, each with the List / Tree toggle in its header; a WIP folder
//! row's menu, with Stage folder disabled while a conflicted file is under the folder; and the
//! messages the toggle, a folder row and its menu items send.

use crate::msg::{HistoryMsg, LayoutMsg, Msg, StageMsg};
use crate::state::file_folds::FileListId;
use crate::state::selection::MultiSelection;
use crate::state::{AppState, Selection};
use crate::ui::file_menu::folder_menu::{self, FolderStage};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{git, harness, remove_fixture, with_repo};
use crate::ui::snapshot_tests_details_commit::{MESSAGE, with_commit};
use crate::ui::snapshot_tests_file_menu::menu_style;
use crate::ui::snapshot_tests_line_counts::{lines, with_wip, write};
use crate::ui::snapshot_tests_select::click;
use crate::ui::theme::{apply_theme, color};
use eframe::egui::{self, PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{self, LoadOptions};
use glowfly_core::settings::{FileListMode, ThemeChoice};
use std::path::PathBuf;

type H = Harness<'static, (AppState, Vec<Msg>)>;

const D: &str = "2026-01-14T10:00:00+00:00";

fn tree(mut st: AppState) -> AppState {
    st.settings.file_list_mode = FileListMode::Tree;
    st
}

fn center(h: &H, label: &str) -> Pos2 {
    let b = h.get_by_label(label).bounding_box().expect(label);
    pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
}

fn pushed(h: &mut H) -> Vec<Msg> {
    std::mem::take(&mut h.state_mut().1)
}

/// `with_wip` plus an untracked src/ui/view.rs and a staged lib/core.rs, in Tree mode, with the
/// working tree's counts read again.
fn wip_tree(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_wip(theme, name);
    write(&dir, "src/ui/view.rs", &lines("fn view", 6));
    write(&dir, "lib/core.rs", &lines("fn core", 3));
    git(&dir, &["add", "lib/core.rs"], D);
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let generation = st.line_counts.start_wip().unwrap();
    st.line_counts.finish_wip(generation, Some(repo::wip_line_counts(&dir).unwrap()));
    (tree(st), dir)
}

#[test]
fn details_commit_tree() {
    for (theme, name, dark) in [(ThemeChoice::Dark, "details_commit_tree", true), (ThemeChoice::Light, "details_commit_tree_light", false)]
    {
        let (st, dir, _) = with_commit(theme, "tree-commit", MESSAGE);
        let mut h = harness(tree(st), dark);
        h.run();
        assert!(h.query_by_label("docs/").is_some() && h.query_by_label("src/search/").is_some(), "the folder rows");
        assert!(h.query_by_label("src/search/index.rs").is_some(), "a file row keeps its whole path as its name");
        assert!(h.query_by_label(Lang::En.tr("files.tree.show_list.tip")).is_some(), "the toggle");
        h.check_snapshot(name);
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn details_commit_tree_closed() {
    let (mut st, dir, _) = with_commit(ThemeChoice::Dark, "tree-commit-closed", MESSAGE);
    st.view.file_folds.toggle(FileListId::Commit, "src/search".into());
    let mut h = harness(tree(st), true);
    h.run();
    assert!(h.query_by_label("src/search/").is_some());
    assert!(h.query_by_label("src/search/index.rs").is_none(), "hidden in its folder");
    h.check_snapshot("details_commit_tree_closed");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn details_wip_tree() {
    for (theme, name, dark) in [(ThemeChoice::Dark, "details_wip_tree", true), (ThemeChoice::Light, "details_wip_tree_light", false)] {
        let (st, dir) = wip_tree(theme, "tree-wip");
        let mut h = harness(st, dark);
        h.run();
        assert!(h.query_by_label("src/ui/").is_some() && h.query_by_label("notes/").is_some() && h.query_by_label("lib/").is_some());
        assert_eq!(h.query_all_by_label(Lang::En.tr("files.tree.show_list.tip")).count(), 2, "a toggle in each header");
        h.check_snapshot(name);
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn details_compare_tree() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "tree-compare");
    write(&dir, "docs/intro.md", &lines("Intro", 3));
    write(&dir, "src/lib/parse.rs", &lines("fn parse", 4));
    git(&dir, &["add", "-A"], D);
    git(&dir, &["commit", "-q", "-m", "Add the parser"], D);
    write(&dir, "docs/intro.md", &lines("Intro", 5));
    write(&dir, "src/lib/lex.rs", &lines("fn lex", 2));
    write(&dir, "notes.txt", "notes\n");
    git(&dir, &["add", "-A"], D);
    git(&dir, &["commit", "-q", "-m", "Add the lexer"], D);
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let oid = |s: &str| st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == s).unwrap().oid;
    let oids = [oid("Add the lexer"), oid("Add the parser")];
    let mut sel = MultiSelection::build(oids, oids[0], st.snap.as_ref().unwrap());
    if let Selection::Commits(m) = &mut sel {
        let (from, to) = m.compare_ends().expect("a pair");
        m.changes = Some(repo::compare(&dir, from, to).map_err(|e| e.to_string()));
    }
    st.selection = sel;
    let mut h = harness(tree(st), true);
    h.run();
    assert!(h.query_by_label("docs/").is_some() && h.query_by_label("src/lib/").is_some());
    assert!(h.query_by_label(&Lang::En.trn("details.commit.changed_files", 3, &[])).is_some(), "the new header");
    h.check_snapshot("details_compare_tree");
    drop(h);
    remove_fixture(&dir);
}

/// A small window drawing a WIP folder row's menu body for "src", `blocked` naming why Stage
/// folder is off.
fn menu_harness(blocked: bool) -> Harness<'static, Vec<Msg>> {
    crate::ui::snapshot_check::builder().with_size(egui::vec2(440.0, 160.0)).build_ui_state(
        move |ui, msgs: &mut Vec<Msg>| {
            color::set_dark(true);
            apply_theme(ui.ctx());
            ui.ctx().set_theme(egui::Theme::Dark);
            ui.painter().rect_filled(ui.max_rect(), 0.0, color::bg());
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.set_min_width(220.0);
                menu_style(ui.style_mut());
                let t = Lang::En;
                let blocked = blocked.then(|| t.tr("files.folder_menu.stage_conflicted"));
                folder_menu::items(ui, t, msgs, "src", None, FolderStage::Stage { blocked });
            });
        },
        Vec::new(),
    )
}

#[test]
fn folder_row_menu_wip() {
    let mut h = menu_harness(false);
    h.run();
    h.check_snapshot("folder_row_menu_wip");
    h.get_by_label(Lang::En.tr("files.folder_menu.stage")).click();
    h.run();
    assert!(h.state().iter().any(|m| matches!(m, Msg::Stage(StageMsg::StageFolder(p)) if p == "src")));
    let mut h = menu_harness(true);
    h.run();
    h.check_snapshot("folder_row_menu_wip_blocked");
    h.get_by_label(Lang::En.tr("files.folder_menu.stage")).click();
    h.run();
    assert!(h.state().is_empty(), "disabled while a conflicted file is under the folder");
}

#[test]
fn the_toggle_and_folder_rows_send_their_messages() {
    let (st, dir, oid) = with_commit(ThemeChoice::Dark, "tree-clicks", MESSAGE);
    let mut h = harness(st, true);
    h.run();
    pushed(&mut h);
    h.get_by_label(Lang::En.tr("files.tree.show_tree.tip")).click();
    h.run();
    assert!(pushed(&mut h).iter().any(|m| matches!(m, Msg::Layout(LayoutMsg::SetFileListMode(FileListMode::Tree)))));

    h.state_mut().0.settings.file_list_mode = FileListMode::Tree;
    h.run();
    pushed(&mut h);
    h.get_by_label("src/search/").click();
    h.run();
    let toggled = LayoutMsg::ToggleFolder { list: FileListId::Commit, path: "src/search".into() };
    assert!(pushed(&mut h).iter().any(|m| matches!(m, Msg::Layout(l) if *l == toggled)));

    let at = center(&h, "src/search/");
    click(&mut h, at, PointerButton::Secondary);
    pushed(&mut h);
    h.get_by_label(Lang::En.tr("files.folder_menu.history")).click();
    h.run();
    let history =
        |m: &Msg| matches!(m, Msg::History(HistoryMsg::OpenFolderHistory { path, at: Some(o) }) if path == "src/search" && *o == oid);
    assert!(pushed(&mut h).iter().any(history));

    click(&mut h, at, PointerButton::Secondary);
    pushed(&mut h);
    h.get_by_label(Lang::En.tr("common.copy_path")).click();
    h.run();
    assert!(pushed(&mut h).iter().any(|m| matches!(m, Msg::Copy(p) if p == "src/search")));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn wip_folder_rows_send_their_messages() {
    let (st, dir) = wip_tree(ThemeChoice::Dark, "tree-wip-clicks");
    let mut h = harness(st, true);
    h.run();
    pushed(&mut h);
    let at = center(&h, "src/ui/");
    click(&mut h, at, PointerButton::Secondary);
    pushed(&mut h);
    h.get_by_label(Lang::En.tr("files.folder_menu.history")).click();
    h.run();
    let history = |m: &Msg| matches!(m, Msg::History(HistoryMsg::OpenFolderHistory { path, at: None }) if path == "src/ui");
    assert!(pushed(&mut h).iter().any(history), "the WIP lists' history starts at HEAD");

    click(&mut h, at, PointerButton::Secondary);
    pushed(&mut h);
    h.get_by_label(Lang::En.tr("files.folder_menu.stage")).click();
    h.run();
    assert!(pushed(&mut h).iter().any(|m| matches!(m, Msg::Stage(StageMsg::StageFolder(p)) if p == "src/ui")));

    let at = center(&h, "lib/");
    click(&mut h, at, PointerButton::Secondary);
    pushed(&mut h);
    h.get_by_label(Lang::En.tr("files.folder_menu.unstage")).click();
    h.run();
    assert!(pushed(&mut h).iter().any(|m| matches!(m, Msg::Stage(StageMsg::UnstageFolder(p)) if p == "lib")));
    drop(h);
    remove_fixture(&dir);
}
