//! German for the file row menus (with the Git LFS entries and the tools' "Set up…"), the
//! Staged files header menu, the diff file header and its View popup, blame (header, gutter and
//! line menu), the signature badge with its tooltip and menu, and the reasons in the commit
//! menus: no English text from these areas, checked as `snapshot_tests_language_main` does.

use super::blame::menu::{LineTarget, line_menu};
use super::file_menu::lfs_track_items;
use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_blame::with_blame;
use super::snapshot_tests_diff::with_diff;
use super::snapshot_tests_file_menu::{Menu, harness_in, menu_style};
use super::snapshot_tests_language_check::{english_shown, shown_texts};
use super::snapshot_tests_language_main::german;
use super::snapshot_tests_select::{click, row_y};
use super::snapshot_tests_signature::{checked, ssh, with_signed_head};
use super::theme::{apply_theme, color};
use crate::msg::Msg;
use crate::state::blame::BlameStep;
use crate::state::lfs::{LfsInstall, TrackContext, track_items};
use crate::state::selection::MultiSelection;
use crate::state::{AppState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, PointerButton, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use trellis_core::i18n::Lang;
use trellis_core::repo::{BlameCommit, FileEntry};
use trellis_core::settings::{DiffLayout, ThemeChoice};
use trellis_core::signing::SigningFormat;

/// What these areas showed in English. A `*` stands for a name or number.
const ENGLISH: &[&str] = &[
    // File row menus.
    "File history",
    "History of this folder",
    "Restore this version…",
    "Revert changes to this file",
    "Open with default app",
    "Show in Explorer",
    "Reveal in Finder",
    "Open containing folder",
    "New file in this folder…",
    "Rename…",
    "Delete…",
    "Unstage and delete…",
    "Stash staged changes…",
    "Stash this file…",
    "Create patch from this file…",
    "Create patch from staged changes…",
    "Open in external editor",
    "Open in external diff tool",
    "Set up…",
    "Track in Git LFS",
    "Only this file",
    "Every .* file",
    "Everything in */",
    // The diff file header.
    "View",
    "Inline",
    "Split",
    "Whole file",
    "Layout",
    "Show",
    "Ignore whitespace",
    "Wrap long lines",
    "Expand all",
    "Collapse all",
    "Next change",
    "Previous change",
    "Restore…",
    "Open",
    "More file actions",
    "Close diff",
    "Stage file",
    "Unstage file",
    "Unstaged",
    "Staged",
    // Blame.
    "Moves: off",
    "Moves: In file",
    "Moves: Across files",
    "Wrap lines",
    "Back",
    "Show all",
    "Skipping * commit",
    "Skipping * commits",
    "* commits",
    "Updating…",
    "Blame before this commit",
    "Skip this commit",
    "Show in file history",
    "Select on the graph",
    "Copy line",
    "Copy commit SHA",
    "Select * on the graph",
    "Click to select this commit on the graph · right-click for more",
    // The signature badge.
    "Verified",
    "Signed",
    "Bad signature",
    "Check again",
    "Copy key",
    "Commit signing settings…",
    "Signer",
    "Fingerprint",
    "Good signature from a key your allowed signers file lists for this signer.",
    // Commit menu reasons.
    "Merge commits can't be cherry-picked from a multi-selection.",
    "Merge commits can't be reverted from a multi-selection.",
    "Merge commits can't be saved as a patch.",
    "Merge commits can't be cherry-picked from a multi-selection",
    "Merge commits can't be reverted from a multi-selection",
    "Merge commits can't be saved as a patch",
];

fn assert_no_english<S>(h: &Harness<'_, S>, what: &str) {
    let found = english_shown(h, ENGLISH, 60);
    assert!(found.is_empty(), "{what} still shows English: {found:?}");
}

fn shows_containing<S>(h: &Harness<'_, S>, text: &str) -> bool {
    shown_texts(h).iter().any(|t| t.contains(text))
}

/// A small window drawing the Track in Git LFS entries for `path` inside a menu frame.
fn lfs_menu(t: Lang, path: &str) -> Harness<'static, Vec<Msg>> {
    let install = LfsInstall::Installed("3.7.1".into());
    let items = track_items(t, path, TrackContext { install: &install, patterns: &[], submodules: &[] });
    Harness::builder().with_size(egui::vec2(440.0, 120.0)).build_ui_state(
        move |ui, msgs: &mut Vec<Msg>| {
            color::set_dark(true);
            apply_theme(ui.ctx());
            ui.ctx().set_theme(egui::Theme::Dark);
            ui.painter().rect_filled(ui.max_rect(), 0.0, color::bg());
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
fn file_menus_german() {
    let de = Lang::De;
    let mut h = harness_in(de, FileEntry::new("src/app.txt", 'M'), Menu::Wip { missing: false, unstages: false }, true, false);
    h.run();
    assert!(shows_containing(&h, de.tr("common.set_up.suffix")), "the tools' Set up… is German");
    assert!(h.query_by_label(de.tr("tools.open_in_editor")).is_some());
    assert_no_english(&h, "a working-tree file's menu");
    h.check_snapshot("file_menu_wip_german");
    drop(h);

    let mut h = harness_in(de, FileEntry::new("src/app.txt", 'M'), Menu::Commit { in_workdir: true }, true, false);
    h.run();
    assert!(h.query_by_label(de.tr("files.menu.revert_file")).is_some());
    assert_no_english(&h, "a commit file's menu");
    drop(h);

    let mut h = lfs_menu(de, "assets/a.psd");
    h.run();
    assert!(h.query_by_label(de.tr("lfs.menu.only_file")).is_some());
    assert_no_english(&h, "the Git LFS entries");
    drop(h);

    let (mut st, dir) = with_repo(ThemeChoice::Dark, "lang-staged-menu");
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged = vec![FileEntry::new("b.txt", 'M')];
    snap.staged = vec![FileEntry::new("a.txt", 'M')];
    st.selection = Selection::Wip;
    german(&mut st);
    let mut h = harness(st, true);
    h.run();
    let header = de.trf("details.wip.staged.header", &[("count", &1)]);
    let r = h.get_all_by_label(&header).filter_map(|n| n.bounding_box()).next().expect("the Staged files header");
    click(&mut h, pos2(((r.x0 + r.x1) / 2.0) as f32, ((r.y0 + r.y1) / 2.0) as f32), PointerButton::Secondary);
    assert!(h.query_by_label(de.tr("files.menu.stash_staged")).is_some(), "the header menu is German");
    assert_no_english(&h, "the Staged files header menu");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn diff_header_german() {
    let de = Lang::De;
    let (mut st, dir) = with_diff(DiffLayout::Inline, "lang-diff-header");
    german(&mut st);
    let mut h = harness(st, true);
    h.run();
    for key in ["diff.header.stage_file", "diff.header.prev.name", "diff.header.next.name", "diff.header.close.name"] {
        assert!(h.query_by_label(de.tr(key)).is_some(), "{key} in German");
    }
    h.get_by_label(de.tr("diff.header.view.button")).click();
    h.run();
    assert!(h.query_by_label(de.tr("diff.header.view.wrap")).is_some(), "the View popup is German");
    assert_no_english(&h, "the diff header and its View popup");
    h.check_snapshot("diff_header_german");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn blame_german() {
    let de = Lang::De;
    let (mut st, dir, grace) = with_blame(ThemeChoice::Dark, "lang-blame");
    let view = st.blame.as_mut().unwrap();
    view.skipped = vec![grace];
    view.back.push(BlameStep { path: "src/main.rs".into(), at: None, skipped: Vec::new(), top_line: 0, cursor: None });
    german(&mut st);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(&de.trn("blame.header.skipping", 1, &[])).is_some(), "the skipped chip is German");
    assert!(h.query_by_label(de.tr("blame.header.back.button")).is_some());
    assert_no_english(&h, "the blame view");
    h.check_snapshot("blame_german");
    drop(h);
    remove_fixture(&dir);

    let c = BlameCommit {
        oid: Oid::from_str("3f2a9c41d07be5a8c6f19e2d4b7a0c3e5f816d29").unwrap(),
        author: "Grace Hopper".into(),
        email: "grace@example.com".into(),
        time: 0,
        summary: "Make Point public with float fields".into(),
        body: String::new(),
    };
    let mut h = Harness::builder().with_size(egui::vec2(440.0, 190.0)).build_ui_state(
        move |ui, msgs: &mut Vec<Msg>| {
            color::set_dark(true);
            apply_theme(ui.ctx());
            ui.ctx().set_theme(egui::Theme::Dark);
            ui.painter().rect_filled(ui.max_rect(), 0.0, color::bg());
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.set_min_width(200.0);
                menu_style(ui.style_mut());
                let line = LineTarget { line: 4, text: "    pub x: f64,", commit: &c, before: false, skipped: false, on_graph: true };
                line_menu(ui, de, msgs, &line);
            });
        },
        Vec::new(),
    );
    h.run();
    assert!(h.query_by_label(de.tr("blame.menu.copy_line")).is_some(), "the line menu is German");
    assert_no_english(&h, "the blame line menu");
    h.check_snapshot("blame_menu_german");
}

#[test]
fn signature_badge_german() {
    let de = Lang::De;
    let key = "SHA256:nu3mXdiXjT/P69lIp2s2NqrOSBxSCmevSqfmgZp4JuA";
    let (mut st, dir, oid) = with_signed_head(ThemeChoice::Dark, "lang-signature", Some(SigningFormat::Ssh));
    let out = format!("G\0ada@example.com\0{key}\0{key}\0fully\0Good \"git\" signature for ada@example.com\n");
    checked(&mut st, oid, ssh(&out, ""));
    german(&mut st);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label(de.tr("signature.verdict.verified")).hover();
    for _ in 0..4 {
        h.run();
    }
    assert!(h.query_by_label_contains(de.tr("signature.reason.good_ssh")).is_some(), "the tooltip is German");
    assert_no_english(&h, "the signature badge and its tooltip");
    h.check_snapshot("signature_badge_german");
    let r = h.get_by_label(de.tr("signature.verdict.verified")).bounding_box().unwrap();
    click(&mut h, pos2(((r.x0 + r.x1) / 2.0) as f32, ((r.y0 + r.y1) / 2.0) as f32), PointerButton::Secondary);
    for item in ["signature.menu.check_again", "signature.menu.copy_key", "signature.menu.settings"] {
        assert!(h.query_by_label(de.tr(item)).is_some(), "{item} in German");
    }
    assert_no_english(&h, "the signature menu");
    drop(h);
    remove_fixture(&dir);

    let (mut st, dir, oid) = with_signed_head(ThemeChoice::Dark, "lang-signature-none", Some(SigningFormat::Ssh));
    let stderr = "error: gpg.ssh.allowedSignersFile needs to be configured and exist for ssh signature verification\n";
    checked(&mut st, oid, ssh("N\0\0\0\0undefined\0\n", stderr));
    german(&mut st);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label(de.tr("signature.verdict.signed")).hover();
    for _ in 0..4 {
        h.run();
    }
    assert!(h.query_by_label_contains(de.tr("signature.reason.no_allowed_signers")).is_some(), "the tooltip is German");
    assert_no_english(&h, "a badge with no allowed signers file");
    drop(h);
    remove_fixture(&dir);
}

fn commit(st: &AppState, summary: &str) -> Oid {
    st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == summary).unwrap().oid
}

#[test]
fn commit_menus_german() {
    let de = Lang::De;
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "lang-commit-menus");
    let (merge, fix) = (commit(&st, "Merge branch 'login-page'"), commit(&st, "Fix build on Windows"));
    let row = st.snap.as_ref().unwrap().row_of[&merge] + 1;
    german(&mut st);
    let mut h = harness(st, true);
    h.run();
    click(&mut h, pos2(700.0, row_y(row)), PointerButton::Secondary);
    assert_no_english(&h, "a merge commit's menu");
    drop(h);
    remove_fixture(&dir);

    let (mut st, dir) = with_repo(ThemeChoice::Dark, "lang-multi-menu");
    st.selection = MultiSelection::build([merge, fix], merge, st.snap.as_ref().unwrap());
    german(&mut st);
    let mut h = harness(st, true);
    h.run_steps(4);
    click(&mut h, pos2(700.0, row_y(row)), PointerButton::Secondary);
    assert_no_english(&h, "the menu of a selection with a merge");
    h.check_snapshot("multi_commit_menu_german");
    let Selection::Commits(m) = &h.state().0.selection else { panic!("not a multi-selection") };
    let why = m.pick_blocked.expect("a merge can't be picked with others").text(de);
    assert_ne!(why, m.pick_blocked.unwrap().text(Lang::En));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_check_catches_english() {
    let mut h = harness_in(Lang::En, FileEntry::new("src/app.txt", 'M'), Menu::Wip { missing: false, unstages: false }, true, false);
    h.run();
    let found = english_shown(&h, ENGLISH, 60);
    for text in ["Open in external editor", "Rename…", "Set up…"] {
        assert!(found.iter().any(|f| f == text), "{text} is caught: {found:?}");
    }
    drop(h);
    let (st, dir) = with_diff(DiffLayout::Inline, "lang-diff-english");
    let mut h = harness(st, true);
    h.run();
    let found = english_shown(&h, ENGLISH, 60);
    for text in ["Stage file", "Next change", "Unstaged"] {
        assert!(found.iter().any(|f| f == text), "{text} is caught: {found:?}");
    }
    drop(h);
    remove_fixture(&dir);
}
