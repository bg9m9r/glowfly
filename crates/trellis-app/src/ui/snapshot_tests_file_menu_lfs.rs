//! The Track in Git LFS submenu of file rows: the closed submenu at the end of a working-tree
//! row's real context menu, its entries with one already tracked, and all of them disabled
//! without git-lfs. Clicks check the messages the entries send, and which rows offer the submenu.

use super::file_menu::{RowSource, commit_file_items, lfs_track_items, wip_file_items};
use super::snapshot_tests_file_menu::menu_style;
use super::theme::{apply_theme, color};
use crate::lfs::LfsMsg;
use crate::msg::Msg;
use crate::state::lfs::{LfsInstall, TrackContext, TrackItem, track_items};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, Event, Modifiers, PointerButton, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use trellis_core::i18n::Lang;
use trellis_core::lfs::{LfsPattern, TrackTarget};
use trellis_core::repo::FileEntry;

const TRACK_IN_LFS: &str = "Track in Git LFS";
const ONLY_THIS_FILE: &str = "Only this file";
const NOT_INSTALLED_MENU: &str = "Git LFS isn't installed. See Preferences > Git LFS.";

const PATH: &str = "assets/a.psd";

/// The entries for `path` with `install`, and `*.psd` already tracked.
fn items(path: &str, install: LfsInstall) -> Vec<TrackItem> {
    let patterns = [LfsPattern { raw: "*.psd".into(), display: "*.psd".into(), lockable: false }];
    track_items(Lang::En, path, TrackContext { install: &install, patterns: &patterns, submodules: &[] })
}

fn installed() -> LfsInstall {
    LfsInstall::Installed("3.7.1".into())
}

fn theme(ui: &mut egui::Ui, dark: bool) {
    color::set_dark(dark);
    apply_theme(ui.ctx());
    ui.ctx().set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
    ui.painter().rect_filled(ui.max_rect(), 0.0, color::bg());
}

/// A window drawing the submenu's entries inside a menu frame.
fn body(items: Vec<TrackItem>, dark: bool) -> Harness<'static, Vec<Msg>> {
    Harness::builder().with_size(egui::vec2(440.0, 120.0)).build_ui_state(
        move |ui, msgs: &mut Vec<Msg>| {
            theme(ui, dark);
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.set_min_width(220.0);
                menu_style(ui.style_mut());
                lfs_track_items(ui, msgs, &items);
            });
        },
        Vec::new(),
    )
}

#[test]
fn file_menu_wip_lfs() {
    let lfs = items(PATH, installed());
    let f = FileEntry::new(PATH, 'M');
    let mut h = Harness::builder().with_size(egui::vec2(260.0, 250.0)).build_ui_state(
        move |ui, msgs: &mut Vec<Msg>| {
            theme(ui, true);
            let r = ui.add(egui::Label::new(PATH).sense(egui::Sense::click()));
            r.context_menu(|ui| {
                let _ = ui.button("Stage");
                let _ = ui.button("Copy path");
                let tool = super::file_menu::diff_item(Lang::En, trellis_core::repo::DiffTarget::Unstaged(PATH.into()), None);
                wip_file_items(
                    ui,
                    Lang::En,
                    msgs,
                    &f,
                    false,
                    false,
                    &lfs,
                    None,
                    tool,
                    &crate::state::custom_commands::MenuCommands::none(),
                );
            });
        },
        Vec::new(),
    );
    h.run();
    let at = pos2(20.0, 12.0);
    for pressed in [true, false] {
        let events = &mut h.input_mut().events;
        events.push(Event::PointerMoved(at));
        events.push(Event::PointerButton { pos: at, button: PointerButton::Secondary, pressed, modifiers: Modifiers::NONE });
        h.step();
    }
    h.run();
    assert!(h.query_by_label(TRACK_IN_LFS).is_some());
    h.check_snapshot("file_menu_wip_lfs");
}

#[test]
fn file_menu_lfs_track() {
    let lfs = items(PATH, installed());
    assert_eq!(lfs.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(), [ONLY_THIS_FILE, "Every .psd file", "Everything in assets/"]);
    let mut h = body(lfs, true);
    h.run();
    h.check_snapshot("file_menu_lfs_track");
    assert!(!h.get_by_label(ONLY_THIS_FILE).is_disabled());
    assert!(h.get_by_label("Every .psd file").is_disabled());
    assert!(!h.get_by_label("Everything in assets/").is_disabled());
    h.get_by_label(ONLY_THIS_FILE).click();
    h.run();
    assert!(matches!(&h.state()[..], [Msg::Lfs(LfsMsg::TrackPath(TrackTarget::File(p)))] if p == PATH));
    drop(h);

    let mut h = body(items(PATH, installed()), true);
    h.run();
    h.get_by_label("Every .psd file").click();
    h.run();
    assert!(h.state().is_empty(), "a disabled entry sends nothing");
}

#[test]
fn file_menu_lfs_not_installed_light() {
    let lfs = items(PATH, LfsInstall::Missing);
    assert!(lfs.iter().all(|i| i.blocked.as_deref() == Some(NOT_INSTALLED_MENU)));
    let mut h = body(lfs, false);
    h.run();
    h.check_snapshot("file_menu_lfs_not_installed_light");
    for label in [ONLY_THIS_FILE, "Every .psd file", "Everything in assets/"] {
        assert!(h.get_by_label(label).is_disabled(), "{label}");
    }
}

/// A commit row's menu body with `lfs` passed in.
fn commit_menu(in_workdir: bool) -> Harness<'static, Vec<Msg>> {
    let lfs = items(PATH, installed());
    let f = FileEntry::new(PATH, 'M');
    let oid = Oid::from_str("3f2a9c41d07be5a8c6f19e2d4b7a0c3e5f816d29").unwrap();
    Harness::builder().with_size(egui::vec2(260.0, 240.0)).build_ui_state(
        move |ui, msgs: &mut Vec<Msg>| {
            theme(ui, true);
            commit_file_items(
                ui,
                Lang::En,
                msgs,
                RowSource::Commit(oid),
                &f,
                in_workdir,
                &lfs,
                Default::default(),
                None,
                &crate::state::custom_commands::MenuCommands::none(),
            );
        },
        Vec::new(),
    )
}

#[test]
fn lfs_submenu_rows() {
    let mut h = commit_menu(false);
    h.run();
    assert!(h.query_by_label(TRACK_IN_LFS).is_none(), "hidden for a file not in the working tree");
    drop(h);
    let mut h = commit_menu(true);
    h.run();
    assert!(h.query_by_label(TRACK_IN_LFS).is_some());
    drop(h);

    let root = items("Makefile", installed());
    assert_eq!(root.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(), [ONLY_THIS_FILE]);
    let mut h = body(root, true);
    h.run();
    assert!(h.query_by_label(ONLY_THIS_FILE).is_some());
    assert!(h.query_by_label_contains("Every").is_none());
}
