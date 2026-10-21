//! The Clone dialog with its shallow options open: a depth and a branch ready to clone, and a
//! future start date with an invalid branch name (red lines, Clone disabled); with its sparse
//! options open: two folders and a partial clone, and a pattern where folders go (red line,
//! Clone disabled). `render` pins today to 2026-02-01.

use super::snapshot_tests::{harness, render, settings};
use crate::state::clone::{CloneForm, LimitKind};
use crate::state::touched::Touched;
use crate::state::{AppState, Modal};
use eframe::egui::Key;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use std::time::Instant;

fn with_form(theme: ThemeChoice, form: CloneForm) -> AppState {
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    st.modal = Some(Modal::Clone(form));
    st
}

fn form() -> CloneForm {
    // The folder check has answered that C:\src is a folder.
    let checked = Some((r"C:\src".into(), Some(r"C:\src".into())));
    CloneForm { url: "https://example.com/team/app.git".into(), shallow: true, parent_checked: checked, ..CloneForm::new(r"C:\src".into()) }
}

#[test]
fn clone_shallow_depth() {
    let f = CloneForm { limit: LimitKind::Depth, depth: "1".into(), branch: "release/2.0".into(), ..form() };
    render("clone_shallow_depth", with_form(ThemeChoice::Dark, f), true);
}

#[test]
fn clone_shallow_since_future() {
    let f = CloneForm { limit: LimitKind::Since, since: "2026-03-01".into(), branch: "bad name".into(), touched: Touched::all(), ..form() };
    render("clone_shallow_since_future", with_form(ThemeChoice::Light, f), false);
}

#[test]
fn clone_sparse_folders() {
    let f = CloneForm { shallow: false, sparse: true, sparse_folders: "docs\nsrc/app".into(), partial: true, ..form() };
    render("clone_sparse_folders", with_form(ThemeChoice::Dark, f), true);
}

#[test]
fn clone_sparse_pattern() {
    let f = CloneForm { shallow: false, sparse: true, sparse_folders: "src/*.rs".into(), touched: Touched::all(), ..form() };
    render("clone_sparse_pattern", with_form(ThemeChoice::Light, f), false);
}

#[test]
fn clone_missing_folder_waits() {
    // The last repository's parent folder is gone: the check found no folder there.
    let parent = glowfly_core::paths::temp_dir().join("trellis-clone-gone").display().to_string();
    let f = CloneForm {
        url: "https://example.com/team/app.git".into(),
        parent_checked: Some((parent.clone(), None)),
        ..CloneForm::new(parent)
    };
    let mut h = harness(with_form(ThemeChoice::Dark, f), true);
    h.run();
    assert!(h.query_by_label("That folder doesn't exist.").is_none(), "nobody typed the folder");
    h.press_key(Key::Enter);
    h.run();
    assert!(h.query_by_label("That folder doesn't exist.").is_some(), "a submit shows it");
    assert!(matches!(&h.state().0.modal, Some(Modal::Clone(_))), "still open");
}
