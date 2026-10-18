//! Patch dialogs and banners: Apply patch on a series (as commits), on a diff with the
//! three-way fallback chosen, and on a file with nothing to apply; Create patch with Only
//! selected files; the Applying patches banner with conflicts left and without; and the patch
//! entries of the WIP row and Staged Files header menus. Made-up files and a made-up stopped
//! series are injected into the shared fixture's snapshot so the images are deterministic.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::click;
use super::snapshot_tests_worktrees::{H, pushed};
use crate::msg::{JobMsg, Msg};
use crate::patch::{ApplyHow, PatchMsg};
use crate::state::patch::apply::{ApplyForm, ApplyMode, NOTHING_TO_APPLY, THREE_WAY_INDEX};
use crate::state::patch::changes::SaveChangesForm;
use crate::state::{AppState, Modal, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use git2::RepositoryState;
use std::path::PathBuf;
use trellis_core::patch_ops::{AmProgress, OnFail, PatchSummary, WipScope};
use trellis_core::repo::FileEntry;
use trellis_core::settings::ThemeChoice;

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

/// The fixture with the Apply patch dialog open on a file holding `commits` and `paths`.
fn apply_dialog(theme: ThemeChoice, name: &str, commits: usize, paths: &[&str]) -> (AppState, PathBuf) {
    let (mut st, dir) = with_files(theme, name, &[("README.md", 'M')], &[]);
    let summary = PatchSummary { commits, first_subject: None, files: paths.iter().map(|p| (*p).to_owned()).collect() };
    let form = ApplyForm::new(dir.clone(), dir.with_file_name("login-form.patch"), summary);
    st.modal = Some(Modal::ApplyPatch(Box::new(form)));
    (st, dir)
}

/// The lowest node labelled `label` (the dialog's, over the window behind it).
fn lowest<'a>(h: &'a H, label: &'a str) -> egui_kittest::kittest::Node<'a> {
    h.get_all_by_label(label).max_by(|a, b| a.bounding_box().unwrap().y0.total_cmp(&b.bounding_box().unwrap().y0)).expect(label)
}

fn form(st: &AppState) -> &ApplyForm {
    let Some(Modal::ApplyPatch(f)) = &st.modal else { panic!("the dialog") };
    f
}

const SERIES_FILES: [&str; 7] =
    ["src/login.rs", "src/login/form.rs", "src/login/validate.rs", "assets/login.css", "docs/login.md", "tests/login.rs", "Cargo.toml"];

#[test]
fn patch_apply_series() {
    let (st, dir) = apply_dialog(ThemeChoice::Dark, "patch-apply-series", 3, &SERIES_FILES);
    let mut h = harness(st, true);
    h.run();
    for text in
        ["Apply patch", "login-form.patch", "3 commits · 7 files", "Apply as commits", "Three-way fallback when a patch doesn't apply"]
    {
        assert!(h.query_by_label(text).is_some(), "{text}");
    }
    assert!(h.query_by_label("Each patch becomes a commit on main, keeping its author and message.").is_some());
    h.check_snapshot("patch_apply_series");
    lowest(&h, "Apply").click();
    h.run();
    assert!(pushed(
        &h,
        &|m| matches!(m, Msg::Patch(PatchMsg::Apply(r)) if r.how == ApplyHow::Commits { three_way: true } && r.commits == 3)
    ));
    assert!(h.state().0.modal.is_some(), "update closes the dialog once the job starts");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn patch_apply_changes_three_way() {
    let (mut st, dir) = apply_dialog(ThemeChoice::Light, "patch-apply-3way", 0, &["src/login.rs", "Cargo.toml"]);
    let Some(Modal::ApplyPatch(f)) = &mut st.modal else { unreachable!() };
    f.opts.on_fail = OnFail::ThreeWay;
    let mut h = harness(st, false);
    h.run();
    assert_eq!(form(&h.state().0).mode, ApplyMode::Changes);
    assert!(h.query_by_label("Changes to 2 files").is_some());
    assert!(h.get_by_label("Apply as commits").is_disabled(), "a plain diff has no commits");
    assert!(h.get_by_label("Working tree only").is_disabled(), "{THREE_WAY_INDEX}");
    h.check_snapshot("patch_apply_changes_three_way");
    h.get_by_label("Apply what fits and save the rest in .rej files").click();
    h.run();
    assert!(!h.get_by_label("Working tree only").is_disabled());
    lowest(&h, "Apply").click();
    h.run();
    let rejects = |m: &Msg| matches!(m, Msg::Patch(PatchMsg::Apply(r)) if matches!(r.how, ApplyHow::Changes(o) if o.on_fail == OnFail::Reject && !o.index));
    assert!(pushed(&h, &rejects));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn patch_apply_no_changes() {
    let (st, dir) = apply_dialog(ThemeChoice::Dark, "patch-apply-nothing", 0, &[]);
    let mut h = harness(st, true);
    h.run();
    assert_eq!(h.get_all_by_label(NOTHING_TO_APPLY).count(), 1, "said once");
    assert!(lowest(&h, "Apply").is_disabled());
    h.check_snapshot("patch_apply_no_changes");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn patch_save_changes_pick() {
    let (mut st, dir) = with_files(
        ThemeChoice::Dark,
        "patch-save-pick",
        &[("b.txt", 'M'), ("c.txt", '?'), ("docs/notes.md", 'M')],
        &[("a.txt", 'M'), ("src/lib.rs", 'A')],
    );
    st.modal = Some(Modal::SavePatch(SaveChangesForm::new(WipScope::All, Some("b.txt".into()))));
    let mut h = harness(st, true);
    h.run();
    for text in ["Create patch", "From uncommitted changes on main", "Only selected files", "1 of 5 files"] {
        assert!(h.query_by_label(text).is_some(), "{text}");
    }
    assert!(h.query_by_label("Include untracked files (1)").is_none(), "hidden under Only selected files");
    h.check_snapshot("patch_save_changes_pick");
    lowest(&h, "c.txt").click();
    h.run();
    h.get_by_label("Save…").click();
    h.run();
    let saves = |m: &Msg| {
        matches!(m, Msg::Patch(PatchMsg::SaveChanges { scope: WipScope::All, paths: Some(p), .. })
            if p.iter().map(String::as_str).eq(["b.txt", "c.txt"]))
    };
    assert!(pushed(&h, &saves));

    h.get_by_label("Select none").click();
    h.run();
    assert!(h.get_by_label("Save…").is_disabled());
    assert!(h.query_by_label("Choose at least one file.").is_some());
    h.get_by_label("Only selected files").click();
    h.run();
    assert!(h.query_by_label("Include untracked files (1)").is_some());
    drop(h);
    remove_fixture(&dir);
}

/// The fixture with a series stopped at patch 2 of 3 ("Validate the login form").
fn applying(theme: ThemeChoice, name: &str, conflicted: bool) -> (AppState, PathBuf) {
    let unstaged: &[(&str, char)] = if conflicted { &[("src/login.rs", '!')] } else { &[] };
    let (mut st, dir) = with_files(theme, name, unstaged, &[]);
    let snap = st.snap.as_mut().unwrap();
    snap.state = RepositoryState::ApplyMailbox;
    snap.am = Some(AmProgress { step: 2, total: 3, subject: "Validate the login form".into(), conflicted });
    (st, dir)
}

#[test]
fn am_banner_conflict() {
    let (st, dir) = applying(ThemeChoice::Dark, "am-banner-conflict", true);
    let mut h = harness(st, true);
    h.run();
    for text in ["Applying patches · patch 2 of 3", "Stopped at: Validate the login form", "Resolve the conflicted files, then Continue."]
    {
        assert!(h.query_by_label(text).is_some(), "{text}");
    }
    assert!(h.get_by_label("Continue").is_disabled(), "conflicted files remain");
    assert!(!h.get_by_label("Skip patch").is_disabled());
    assert!(h.query_by_label("View conflicts").is_some());
    h.check_snapshot("am_banner_conflict");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn am_banner_stopped() {
    let (st, dir) = applying(ThemeChoice::Light, "am-banner-stopped", false);
    let mut h = harness(st, false);
    h.run();
    assert!(h.query_by_label(crate::state::am_banner::HINT_BY_HAND).is_some());
    assert!(!h.get_by_label("Continue").is_disabled());
    h.check_snapshot("am_banner_stopped");
    h.get_by_label("Skip patch").click();
    h.run();
    let skips = |m: &Msg| matches!(m, Msg::Job(JobMsg::Git { label, cmds }) if label == "Skip patch" && cmds[0] == ["am", "--skip"]);
    assert!(pushed(&h, &skips));
    drop(h);
    remove_fixture(&dir);
}

/// Right-clicks the middle of the topmost node labelled `label`.
fn right_click(h: &mut H, label: &str) {
    let r = h.get_all_by_label(label).filter_map(|n| n.bounding_box()).min_by(|a, b| a.y0.total_cmp(&b.y0)).expect("the node");
    click(h, pos2(((r.x0 + r.x1) / 2.0) as f32, ((r.y0 + r.y1) / 2.0) as f32), PointerButton::Secondary);
}

#[test]
fn wip_menus_open_the_create_patch_dialog() {
    let (st, dir) = with_files(ThemeChoice::Dark, "patch-wip-menus", &[("b.txt", 'M')], &[("a.txt", 'M')]);
    let mut h = harness(st, true);
    h.run();
    right_click(&mut h, "a.txt");
    h.get_by_label("Create patch from this file…").click();
    h.run();
    let staged_row = |m: &Msg| matches!(m, Msg::Patch(PatchMsg::ChangesDialog { scope: WipScope::Staged, path: Some(p) }) if p == "a.txt");
    assert!(pushed(&h, &staged_row));
    right_click(&mut h, "b.txt");
    h.get_by_label("Create patch from this file…").click();
    h.run();
    let unstaged_row =
        |m: &Msg| matches!(m, Msg::Patch(PatchMsg::ChangesDialog { scope: WipScope::Unstaged, path: Some(p) }) if p == "b.txt");
    assert!(pushed(&h, &unstaged_row));
    right_click(&mut h, "Staged Files (1)");
    h.get_by_label("Create patch from staged changes…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Patch(PatchMsg::ChangesDialog { scope: WipScope::Staged, path: None }))));
    drop(h);

    // Nothing staged.
    let (st, _) = with_files(ThemeChoice::Dark, "patch-wip-menus", &[("b.txt", 'M')], &[]);
    let mut h = harness(st, true);
    h.run();
    right_click(&mut h, "Staged Files (0)");
    assert!(h.get_by_label("Create patch from staged changes…").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn commit_row_patch_item() {
    let oid = git2::Oid::from_bytes(&[0x1a; 20]).unwrap();
    for blocked in [None, Some(crate::state::patch::PATCH_BLOCKED_MERGE)] {
        let f = FileEntry::new("src/app.txt", 'M');
        let mut h = egui_kittest::Harness::builder().with_size(eframe::egui::vec2(300.0, 260.0)).build_ui_state(
            move |ui, msgs: &mut Vec<Msg>| {
                super::file_menu::commit_file_items(ui, msgs, super::file_menu::RowSource::Commit(oid), &f, true, &[], None, blocked);
            },
            Vec::new(),
        );
        h.run();
        let item = h.get_by_label("Create patch from this file…");
        assert_eq!(item.is_disabled(), blocked.is_some());
        if blocked.is_none() {
            item.click();
            h.run();
            let sends =
                |m: &Msg| matches!(m, Msg::Patch(PatchMsg::ExportFiles { oid: o, paths }) if *o == oid && paths == &["src/app.txt"]);
            assert!(h.state().iter().any(sends));
        }
    }
}
