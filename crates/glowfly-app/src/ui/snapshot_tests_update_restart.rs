//! Install and restart in Preferences > General's Updates row (an MSI that restarts, a disk
//! image that is opened, an archive that is only shown, and the update being prepared) and the
//! status bar's Installing chip. Click tests check the message each button sends.

use super::snapshot_tests::harness;
use super::snapshot_tests_updates::{prefs, sent, status_bar, the_offer};
use crate::msg::UpdateMsg;
use crate::state::AppState;
use crate::state::updates::UpdateStatus;
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::app_update::InstallMethod;
use std::path::PathBuf;

fn downloaded(name: &str, method: InstallMethod) -> UpdateStatus {
    UpdateStatus::Downloaded { offer: Box::new(the_offer()), file: PathBuf::from("Downloads").join(name), sha256: [0; 32], method }
}

fn restarting(mut st: AppState) -> AppState {
    st.updates.restarting = true;
    st
}

#[test]
fn prefs_updates_downloaded_restart() {
    let mut h = harness(prefs(downloaded("glowfly-v0.3.0-x86_64-pc-windows-msvc.msi", InstallMethod::RunMsi)), true);
    h.run();
    h.check_snapshot("prefs_updates_downloaded_restart");
    h.get_by_label("Install and restart").click();
    h.run();
    assert!(sent(&h, &UpdateMsg::InstallAndRestart));
    h.get_by_label("Show file").click();
    h.run();
    assert!(sent(&h, &UpdateMsg::ShowFile));
}

#[test]
fn prefs_updates_downloaded_open_quit() {
    let mut h = harness(prefs(downloaded("glowfly-v0.3.0-aarch64-apple-darwin.dmg", InstallMethod::OpenAndQuit)), true);
    h.run();
    assert!(h.query_by_label_contains("Drag Glowfly to Applications").is_some());
    h.check_snapshot("prefs_updates_downloaded_open_quit");
    h.get_by_label("Open installer and quit").click();
    h.run();
    assert!(sent(&h, &UpdateMsg::InstallAndRestart));
}

#[test]
fn prefs_updates_downloaded_show_only() {
    let mut h = harness(prefs(downloaded("glowfly-v0.3.0-x86_64-pc-windows-msvc.zip", InstallMethod::ShowOnly)), true);
    h.run();
    assert!(h.query_by_label("Install and restart").is_none());
    assert!(h.query_by_label_contains("replace it with the downloaded file by hand").is_some());
    h.check_snapshot("prefs_updates_downloaded_show_only");
}

#[test]
fn prefs_updates_restarting() {
    let mut h = harness(restarting(prefs(downloaded("glowfly-v0.3.0-x86_64-pc-windows-msvc.msi", InstallMethod::RunMsi))), true);
    h.run_steps(4);
    assert!(h.query_by_label("Preparing the update…").is_some());
    h.check_snapshot("prefs_updates_restarting");
}

#[test]
fn status_update_installing() {
    let mut h = harness(restarting(status_bar(downloaded("glowfly-v0.3.0-x86_64-pc-windows-msvc.msi", InstallMethod::RunMsi))), true);
    h.run_steps(4);
    h.check_snapshot("status_update_installing");
    h.get_by_label("Installing Glowfly 0.3.0…").click();
    h.run_steps(2);
    assert!(!sent(&h, &UpdateMsg::InstallAndRestart), "not while installing");
}
