//! Headless tests of Install and restart: which method a downloaded file gets, what the chip,
//! toast and palette offer for it, the checks before anything starts (Preferences' unsaved
//! edits, a running operation, the tabs' unsaved work), and the worker's answer.

use super::askpass_gpg_tests::start_job;
use super::tabs_close_tests::fake;
use super::updates_tests::{app, offered, saved, send};
use super::{prefs_save_tests, update};
use crate::effect::{Effect, RestartJob, UiEffect, UpdateEffect};
use crate::msg::{CheckedRelease, Msg, PrefsMsg, UpdateMsg as U};
use crate::state::palette::entries;
use crate::state::prefs::GitConfigValues;
use crate::state::tabs::Tab;
use crate::state::updates::UpdateStatus;
use crate::state::{AppState, Modal, PrefPage, PrefsState};
use glowfly_core::app_update::{Arch, Asset, InstallKind, InstallMethod, Platform, Release, TargetOs};
use glowfly_core::i18n::Lang;
use std::time::Instant;

const MSI: &str = "glowfly-v0.3.0-x86_64-pc-windows-msvc.msi";
const ZIP: &str = "glowfly-v0.3.0-x86_64-pc-windows-msvc.zip";

/// The 0.3.0 release with these files (and SHA256SUMS).
fn release_with(files: &[&str]) -> Release {
    let asset =
        |n: &str| Asset { name: n.into(), size: 100, url: format!("https://github.com/bg9m9r/glowfly/releases/download/v0.3.0/{n}") };
    let mut assets: Vec<Asset> = files.iter().map(|n| asset(n)).collect();
    assets.push(asset("SHA256SUMS"));
    Release {
        tag: "v0.3.0".into(),
        name: "v0.3.0".into(),
        html_url: "https://github.com/bg9m9r/glowfly/releases/tag/v0.3.0".into(),
        body: String::new(),
        published_at: None,
        assets,
    }
}

/// An app on `platform` that checked (as a `kind` copy), downloaded the offered file and
/// returns its name.
fn downloaded_on(platform: Platform, kind: InstallKind, files: &[&str]) -> (AppState, Instant, String) {
    let mut st = app();
    st.updates.platform = Some(platform);
    let t = Instant::now();
    let found = Ok(Some(CheckedRelease { release: release_with(files), install: kind }));
    send(&mut st, U::Checked { manual: false, result: found }, t);
    send(&mut st, U::Download, t);
    let name = match &st.updates.status {
        UpdateStatus::Downloading(o) => o.asset.as_ref().unwrap().name.clone(),
        s => panic!("not downloading: {s:?}"),
    };
    send(&mut st, U::Downloaded(Ok(saved(&name))), t);
    (st, t, name)
}

/// The 0.3.0 MSI downloaded for an installed Windows copy.
fn msi_downloaded() -> (AppState, Instant) {
    let (mut st, t) = offered();
    send(&mut st, U::Download, t);
    send(&mut st, U::Downloaded(Ok(saved(MSI))), t);
    (st, t)
}

fn method(st: &AppState) -> Option<InstallMethod> {
    st.updates.downloaded().map(|d| d.1)
}

fn toast_action(st: &AppState) -> Option<&str> {
    st.toasts.last().and_then(|t| t.action.as_ref()).map(|a| a.label.as_str())
}

fn restart_job(fx: &[Effect]) -> Option<&RestartJob> {
    match fx {
        [Effect::Updates(UpdateEffect::Restart(job))] => Some(job),
        _ => None,
    }
}

const WIN: Platform = Platform { os: TargetOs::Windows, arch: Arch::X86_64 };
const MAC: Platform = Platform { os: TargetOs::Mac, arch: Arch::Aarch64 };
const LINUX: Platform = Platform { os: TargetOs::Linux, arch: Arch::X86_64 };

#[test]
fn an_installed_copy_installs_the_msi_and_restarts() {
    let (mut st, t) = msi_downloaded();
    assert_eq!(method(&st), Some(InstallMethod::RunMsi));
    assert_eq!(toast_action(&st), Some("Install and restart"));
    let fx = send(&mut st, U::InstallAndRestart, t);
    let job = restart_job(&fx).expect("the restart");
    assert_eq!(job.file, saved(MSI).path);
    assert_eq!((job.sha256, job.method, job.version.to_string()), ([7; 32], InstallMethod::RunMsi, "0.3.0".to_owned()));
    assert!(st.updates.restarting);
    assert_eq!(st.updates.chip(Lang::En).map(|c| c.0).as_deref(), Some("Installing Glowfly 0.3.0…"));
    // Nothing else starts meanwhile.
    for m in [U::InstallAndRestart, U::InstallAndRestartConfirmed, U::CheckNow, U::Download] {
        assert!(send(&mut st, m.clone(), t).is_empty(), "{m:?}");
    }
}

#[test]
fn a_prepared_restart_closes_the_window() {
    let (mut st, t) = msi_downloaded();
    send(&mut st, U::InstallAndRestart, t);
    let fx = send(&mut st, U::RestartReady(Ok(())), t);
    assert!(matches!(fx.as_slice(), [Effect::Ui(UiEffect::CloseWindow)]), "{fx:?}");
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some("Restarting to install Glowfly 0.3.0."));
    assert!(st.updates.restarting, "nothing starts while the window closes");
}

#[test]
fn a_failed_restart_changes_nothing_and_says_why() {
    let (mut st, t) = msi_downloaded();
    send(&mut st, U::InstallAndRestart, t);
    let e = "Couldn't replace Glowfly's program in X: denied. Use Show file to install the update by hand.";
    let fx = send(&mut st, U::RestartReady(Err(e.into())), t);
    assert!(fx.is_empty(), "{fx:?}");
    assert!(!st.updates.restarting);
    assert!(matches!(st.updates.status, UpdateStatus::Downloaded { .. }));
    let toast = st.toasts.last().unwrap();
    assert_eq!(toast.text, format!("Couldn't install the update: {e}"));
    assert!(!toast.ok);
    assert_eq!(toast_action(&st), Some("Show file"));
    // It can be tried again.
    assert!(restart_job(&send(&mut st, U::InstallAndRestart, t)).is_some());
}

#[test]
fn a_portable_windows_copy_swaps_the_zip() {
    let (mut st, t, name) = downloaded_on(WIN, InstallKind::Portable, &[MSI, ZIP]);
    assert_eq!(name, ZIP);
    assert_eq!(method(&st), Some(InstallMethod::SwapPortable));
    assert_eq!(toast_action(&st), Some("Install and restart"));
    assert_eq!(restart_job(&send(&mut st, U::InstallAndRestart, t)).map(|j| j.method), Some(InstallMethod::SwapPortable));
}

#[test]
fn an_installed_copy_with_only_an_archive_shows_it() {
    let (mut st, t, name) = downloaded_on(WIN, InstallKind::Installer, &[ZIP]);
    assert_eq!(name, ZIP);
    assert_eq!(method(&st), Some(InstallMethod::ShowOnly));
    assert_eq!(toast_action(&st), Some("Show file"));
    assert!(send(&mut st, U::InstallAndRestart, t).is_empty());
    assert!(!st.updates.restarting);
}

#[test]
fn a_disk_image_is_opened_and_glowfly_quits() {
    let dmg = "glowfly-v0.3.0-aarch64-apple-darwin.dmg";
    let (mut st, t, name) = downloaded_on(MAC, InstallKind::Installer, &[dmg]);
    assert_eq!(name, dmg);
    assert_eq!(method(&st), Some(InstallMethod::OpenAndQuit));
    assert_eq!(toast_action(&st), Some("Open installer"));
    assert_eq!(restart_job(&send(&mut st, U::InstallAndRestart, t)).map(|j| j.method), Some(InstallMethod::OpenAndQuit));
    send(&mut st, U::RestartReady(Ok(())), t);
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some("Opened the Glowfly 0.3.0 installer."));
}

#[test]
fn linux_packages_open_and_archives_swap() {
    let deb = "glowfly-v0.3.0-x86_64-unknown-linux-gnu.deb";
    let tgz = "glowfly-v0.3.0-x86_64-unknown-linux-gnu.tar.gz";
    let (st, _, name) = downloaded_on(LINUX, InstallKind::Installer, &[deb, tgz]);
    assert_eq!((name.as_str(), method(&st)), (deb, Some(InstallMethod::OpenAndQuit)));
    let (st, _, name) = downloaded_on(LINUX, InstallKind::Portable, &[deb, tgz]);
    assert_eq!((name.as_str(), method(&st)), (tgz, Some(InstallMethod::SwapPortable)));
    let rpm = "glowfly-v0.3.0-x86_64-unknown-linux-gnu.rpm";
    let image = "glowfly-v0.3.0-x86_64-unknown-linux-gnu.AppImage";
    let all = [deb, rpm, image, tgz];
    let (st, _, name) = downloaded_on(LINUX, InstallKind::RpmPackage, &all);
    assert_eq!((name.as_str(), method(&st)), (rpm, Some(InstallMethod::OpenAndQuit)));
    let (st, _, name) = downloaded_on(LINUX, InstallKind::AppImage, &all);
    assert_eq!((name.as_str(), method(&st)), (image, Some(InstallMethod::SwapAppImage)));
}

#[test]
fn an_appimage_install_restarts() {
    let image = "glowfly-v0.3.0-x86_64-unknown-linux-gnu.AppImage";
    let (mut st, t, _) = downloaded_on(LINUX, InstallKind::AppImage, &[image]);
    assert_eq!(toast_action(&st), Some("Install and restart"));
    assert_eq!(restart_job(&send(&mut st, U::InstallAndRestart, t)).map(|j| j.method), Some(InstallMethod::SwapAppImage));
    send(&mut st, U::RestartReady(Ok(())), t);
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some("Restarting to install Glowfly 0.3.0."));
}

#[test]
fn windows_arm_gets_the_arm64_msi() {
    let arm = "glowfly-v0.3.0-aarch64-pc-windows-msvc.msi";
    let win_arm = Platform { os: TargetOs::Windows, arch: Arch::Aarch64 };
    let (st, _, name) = downloaded_on(win_arm, InstallKind::Installer, &[arm, MSI]);
    assert_eq!((name.as_str(), method(&st)), (arm, Some(InstallMethod::RunMsi)));
}

#[test]
fn unsaved_work_in_a_tab_asks_first() {
    let (mut st, t) = msi_downloaded();
    st.tabs = vec![Tab::unloaded(fake("a"))];
    st.active_tab = 0;
    st.draft.summary = "wip".into();
    assert!(send(&mut st, U::InstallAndRestart, t).is_empty());
    let Some(Modal::ConfirmDanger { title, body, button, then, .. }) = &st.modal else { panic!("{:?}", st.modal.is_some()) };
    assert_eq!(title, "Restart to update?");
    assert_eq!(body, "Restarting closes every tab.\na holds work that isn't saved anywhere:\n- its commit message draft");
    assert_eq!(button, "Install and restart");
    assert!(matches!(**then, Msg::Updates(U::InstallAndRestartConfirmed)));
    assert!(!st.updates.restarting);
    st.modal = None;
    assert!(restart_job(&send(&mut st, U::InstallAndRestartConfirmed, t)).is_some());
}

#[test]
fn quitting_to_update_says_so() {
    let dmg = "glowfly-v0.3.0-aarch64-apple-darwin.dmg";
    let (mut st, t, _) = downloaded_on(MAC, InstallKind::Installer, &[dmg]);
    st.tabs = vec![Tab::unloaded(fake("a"))];
    st.active_tab = 0;
    st.draft.summary = "wip".into();
    send(&mut st, U::InstallAndRestart, t);
    let Some(Modal::ConfirmDanger { title, body, button, .. }) = &st.modal else { panic!("no confirmation") };
    assert_eq!((title.as_str(), button.as_str()), ("Quit to update?", "Open installer and quit"));
    assert!(body.starts_with("Quitting closes every tab.\n"), "{body}");
}

#[test]
fn a_running_operation_waits() {
    let (mut st, t) = msi_downloaded();
    start_job(&mut st, "Fetch");
    assert!(send(&mut st, U::InstallAndRestart, t).is_empty());
    assert_eq!(
        st.toasts.last().map(|t| (t.text.as_str(), t.ok)),
        Some(("Wait for the running operation to finish, then install the update.", false))
    );
    assert!(!st.updates.restarting);
}

#[test]
fn edited_preferences_ask_to_save_first() {
    let (mut st, t) = msi_downloaded();
    st.prefs = Some(PrefsState::new(PrefPage::General));
    prefs_save_tests::prefs(&mut st, PrefsMsg::GitConfigLoaded(GitConfigValues::default()));
    prefs_save_tests::git_mut(&mut st).longpaths = true;
    let fx = send(&mut st, U::InstallAndRestart, t);
    assert!(restart_job(&fx).is_none(), "{fx:?}");
    assert!(matches!(&st.modal, Some(Modal::ConfirmChoice { title, .. }) if title == "Save your changes?"));
    // Discard, and the held message goes on.
    let fx = update(&mut st, Msg::ConfirmAlt, t);
    assert!(fx.iter().any(|e| matches!(e, Effect::Updates(UpdateEffect::Restart(_)))), "{fx:?}");
}

#[test]
fn startup_tidies_old_staging_folders() {
    let mut st = app();
    let fx = send(&mut st, U::Startup, Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::Updates(UpdateEffect::Tidy))), "{fx:?}");
}

#[test]
fn palette_entries_follow_the_method() {
    let labels = |st: &AppState| entries(st).into_iter().map(|e| e.label).collect::<Vec<_>>();
    let (st, _) = msi_downloaded();
    assert!(labels(&st).contains(&"Install Glowfly 0.3.0 and restart".to_owned()));
    let (st, _, _) = downloaded_on(MAC, InstallKind::Installer, &["glowfly-v0.3.0-aarch64-apple-darwin.dmg"]);
    assert!(labels(&st).contains(&"Open the Glowfly 0.3.0 installer and quit".to_owned()));
    let (st, _, _) = downloaded_on(WIN, InstallKind::Installer, &[ZIP]);
    assert!(!labels(&st).iter().any(|l| l.contains("Install Glowfly") || l.contains("installer and quit")));
}

#[test]
fn an_answer_without_a_restart_is_ignored() {
    let (mut st, t) = msi_downloaded();
    assert!(send(&mut st, U::RestartReady(Ok(())), t).is_empty());
    assert!(send(&mut st, U::RestartReady(Err("late".into())), t).is_empty());
    assert!(st.toasts.last().is_some_and(|t| !t.text.contains("late")));
}
