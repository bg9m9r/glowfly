//! Preferences > General's Updates row while an update downloads (its progress and Cancel, and
//! cancelling), with Download again once the downloaded file is gone or changed, and the status
//! bar's chip for that. The German download row is also checked at a narrow window. Click tests
//! check the message each button sends.

use super::snapshot_tests::{harness, harness_sized};
use super::snapshot_tests_prefs_layout::overflowing;
use super::snapshot_tests_updates::{prefs, sent, status_bar, the_offer};
use crate::msg::UpdateMsg;
use crate::state::AppState;
use crate::state::updates::{Transfer, UpdateStatus};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::app_update::{Progress, SavedProblem};
use glowfly_core::i18n::Lang;
use std::path::PathBuf;

/// Preferences while 0.3.0 downloads, 4 MB of its 10 MB done.
fn downloading(cancelling: bool) -> AppState {
    let mut st = prefs(UpdateStatus::Downloading(Box::new(the_offer())));
    let progress = Progress { done: 4 * 1024 * 1024, total: Some(10 * 1024 * 1024) };
    st.updates.transfer = Transfer { progress, cancelling };
    st
}

fn stale(problem: SavedProblem) -> UpdateStatus {
    let file = PathBuf::from("Downloads").join("glowfly-v0.3.0-x86_64-pc-windows-msvc.msi");
    UpdateStatus::Stale { offer: Box::new(the_offer()), file, problem }
}

#[test]
fn prefs_updates_downloading() {
    // The status bar's chip spins while it downloads.
    let mut h = harness(downloading(false), true);
    h.run_steps(4);
    h.check_snapshot("prefs_updates_downloading");
    assert!(h.query_by_label("Downloading… 40% (4.0 MB of 10.0 MB)").is_some());
    h.get_by_label("Cancel").click();
    h.run_steps(2);
    assert!(sent(&h, &UpdateMsg::CancelDownload));
}

#[test]
fn prefs_updates_downloading_size_unknown() {
    let mut st = downloading(false);
    st.updates.transfer.progress = Progress { done: 1536, total: None };
    let mut h = harness(st, true);
    h.run_steps(4);
    assert!(h.query_by_label("Downloading… 1.5 KB").is_some());
    assert!(h.query_by_label("Cancel").is_some());
}

#[test]
fn prefs_updates_cancelling() {
    let mut h = harness(downloading(true), true);
    h.run_steps(4);
    h.check_snapshot("prefs_updates_cancelling");
    assert!(h.query_by_label("Cancel").is_none(), "asked once");
}

#[test]
fn prefs_updates_download_again() {
    let mut h = harness(prefs(stale(SavedProblem::Missing)), true);
    h.run();
    h.check_snapshot("prefs_updates_download_again");
    assert!(h.query_by_label_contains("is no longer there").is_some());
    h.get_by_label("Download again").click();
    h.run();
    assert!(sent(&h, &UpdateMsg::Download));
    h.get_by_label("Release notes").click();
    h.run();
    assert!(sent(&h, &UpdateMsg::ShowNotes));
}

#[test]
fn prefs_updates_download_again_changed() {
    let mut h = harness(prefs(stale(SavedProblem::Changed)), true);
    h.run();
    assert!(h.query_by_label_contains("no longer matches its checksum").is_some());
    h.check_snapshot("prefs_updates_download_again_changed");
}

#[test]
fn status_update_download_again() {
    let mut h = harness(status_bar(stale(SavedProblem::Changed)), true);
    h.run();
    h.get_by_label("Update available (0.3.0)").hover();
    h.run();
    h.run();
    assert!(h.query_by_label_contains("no longer matches its checksum").is_some(), "the reason");
    h.check_snapshot("status_update_download_again");
    h.get_by_label("Update available (0.3.0)").click();
    h.run();
    assert!(sent(&h, &UpdateMsg::Download));
}

/// The German download row, the longest, stays inside a narrow window's content column (960 px
/// wide, as Preferences' other narrow checks).
#[test]
fn prefs_updates_downloading_narrow_de() {
    let mut st = downloading(false);
    st.settings.language = "de".into();
    let mut h = harness_sized(st, true, eframe::egui::vec2(960.0, 700.0));
    h.run_steps(4);
    assert!(h.query_by_label(Lang::De.tr("common.cancel.button")).is_some());
    h.check_snapshot("prefs_updates_downloading_narrow_de");
    let over = overflowing(&h);
    assert!(over.is_empty(), "{over:#?}");
}
