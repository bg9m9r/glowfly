//! The release notes dialog (bundled notes with a breaking section and New chips, a newer
//! release on top, nothing bundled), Preferences > General's Updates row in each state, and the
//! status bar's update chip. Click tests check the message each button sends.

use super::snapshot_tests::{harness, settings};
use crate::msg::{Msg, PrefsMsg, UpdateMsg};
use crate::state::release_notes::NotesDialog;
use crate::state::updates::UpdateStatus;
use crate::state::{AppState, Modal, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::app_update::{Arch, Asset, InstallKind, Offer, Platform, Release, TargetOs, Version, offer};
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;
use std::time::Instant;

const NOTES: &str = "# Changelog

## [0.2.0](https://github.com/bg9m9r/trellis/compare/v0.1.0...v0.2.0) (2026-01-20)


### ⚠ BREAKING CHANGES

* **settings:** themes saved by 0.1 load as Dark ([abc1234](https://github.com/bg9m9r/trellis/commit/abc1234))

### Features

* **diff:** a split view for wide windows ([def5678](https://github.com/bg9m9r/trellis/commit/def5678))
* **palette:** `View release notes` lists what changed in each version ([0a1b2c3](https://github.com/bg9m9r/trellis/commit/0a1b2c3))

### Bug Fixes

* keep the selected commit after a refresh ([4d5e6f7](https://github.com/bg9m9r/trellis/commit/4d5e6f7))

## 0.1.0 (2026-01-05)


### Features

* first release: the commit graph, diffs and staging
";

/// 2026-02-01 11:00 UTC, an hour before the harness's clock.
const CHECKED: i64 = 1_769_943_600;

fn app(theme: ThemeChoice) -> AppState {
    let mut s = settings(theme);
    s.welcome_done = true;
    let mut st = AppState::new(s, Vec::new(), Instant::now());
    st.updates.current = Version::parse("0.2.0").unwrap();
    st.updates.platform = Some(Platform { os: TargetOs::Windows, arch: Arch::X86_64 });
    st.updates.bundled_notes = NOTES;
    st
}

fn release(with_files: bool) -> Release {
    let names = ["glowfly-v0.3.0-x86_64-pc-windows-msvc.msi", "SHA256SUMS"];
    let url = |n: &str| format!("https://github.com/bg9m9r/trellis/releases/download/v0.3.0/{n}");
    Release {
        tag: "v0.3.0".into(),
        name: "v0.3.0".into(),
        html_url: "https://github.com/bg9m9r/trellis/releases/tag/v0.3.0".into(),
        body: "## [0.3.0](https://github.com/bg9m9r/trellis/compare/v0.2.0...v0.3.0) (2026-02-01)\n\n### Features\n\n\
               * **updates:** check for a new version once a day ([1a2b3c4](https://github.com/bg9m9r/trellis/commit/1a2b3c4))\n"
            .into(),
        published_at: None,
        assets: if with_files { names.iter().map(|n| Asset { name: (*n).into(), size: 10, url: url(n) }).collect() } else { Vec::new() },
    }
}

pub(super) fn the_offer() -> Offer {
    offer(
        &release(true),
        &Version::parse("0.2.0").unwrap(),
        Some(Platform { os: TargetOs::Windows, arch: Arch::X86_64 }),
        InstallKind::Installer,
    )
    .unwrap()
}

/// The notes dialog as an upgrade from 0.1.0 opens it (0.2.0 is new), with `offer` on top.
fn notes(theme: ThemeChoice, offer: Option<&Offer>, bundled: &str) -> AppState {
    let mut st = app(theme);
    let d = NotesDialog::new(bundled, &st.updates.current, Version::parse("0.1.0"), offer);
    st.modal = Some(Modal::ReleaseNotes(Box::new(d)));
    st
}

fn prefs(status: UpdateStatus) -> AppState {
    let mut st = app(ThemeChoice::Dark);
    st.settings.updates.last_checked = Some(CHECKED);
    st.updates.status = status;
    st.prefs = Some(PrefsState::new(PrefPage::General));
    st
}

pub(super) fn status_bar(status: UpdateStatus) -> AppState {
    let mut st = app(ThemeChoice::Dark);
    st.updates.status = status;
    st
}

pub(super) fn sent(h: &egui_kittest::Harness<'_, (AppState, Vec<Msg>)>, m: &UpdateMsg) -> bool {
    h.state().1.iter().any(|x| matches!(x, Msg::Updates(u) if u == m))
}

#[test]
fn release_notes_dark_and_light() {
    let mut h = harness(notes(ThemeChoice::Dark, None, NOTES), true);
    h.run();
    h.check_snapshot("release_notes_dark");
    h.get_by_label("Close").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::CloseModal)));
    let mut h = harness(notes(ThemeChoice::Light, None, NOTES), false);
    h.run();
    h.check_snapshot("release_notes_light");
}

#[test]
fn release_notes_offer() {
    let o = the_offer();
    let mut h = harness(notes(ThemeChoice::Dark, Some(&o), NOTES), true);
    h.run();
    h.check_snapshot("release_notes_offer");
    h.get_by_label("Download").click();
    h.run();
    assert!(sent(&h, &UpdateMsg::Download));
}

#[test]
fn release_notes_empty() {
    let mut h = harness(notes(ThemeChoice::Dark, None, ""), true);
    h.run();
    h.check_snapshot("release_notes_empty");
    h.get_by_label("Open releases page").click();
    h.run();
    assert!(sent(&h, &UpdateMsg::OpenReleasePage));
}

#[test]
fn prefs_updates_up_to_date() {
    let mut h = harness(prefs(UpdateStatus::UpToDate), true);
    h.run();
    h.check_snapshot("prefs_updates_up_to_date");
    h.get_by_label("Check now").click();
    h.run();
    assert!(sent(&h, &UpdateMsg::CheckNow));
}

#[test]
fn prefs_updates_available() {
    let mut h = harness(prefs(UpdateStatus::Available(Box::new(the_offer()))), true);
    h.run();
    h.check_snapshot("prefs_updates_available");
    h.get_by_label("Release notes").click();
    h.run();
    assert!(sent(&h, &UpdateMsg::ShowNotes));
}

#[test]
fn prefs_updates_failed() {
    let mut h = harness(prefs(UpdateStatus::CheckFailed("network error: couldn't reach api.github.com".into())), true);
    h.run();
    h.check_snapshot("prefs_updates_failed");
}

#[test]
fn prefs_updates_checking() {
    let mut st = prefs(UpdateStatus::Checking { manual: true, previous: Box::default() });
    st.settings.updates.last_checked = None;
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("prefs_updates_checking");
}

#[test]
fn status_update_available() {
    let mut h = harness(status_bar(UpdateStatus::Available(Box::new(the_offer()))), true);
    h.run();
    h.check_snapshot("status_update_available");
    h.get_by_label("Update available (0.3.0)").click();
    h.run();
    assert!(sent(&h, &UpdateMsg::ShowNotes));
}

#[test]
fn status_update_downloaded() {
    let file = PathBuf::from("Downloads").join("glowfly-v0.3.0-x86_64-pc-windows-msvc.msi");
    let mut h = harness(status_bar(UpdateStatus::Downloaded { offer: Box::new(the_offer()), file, installer: true }), true);
    h.run();
    h.check_snapshot("status_update_downloaded");
    h.get_by_label("Update downloaded").click();
    h.run();
    assert!(sent(&h, &UpdateMsg::Install));
}

#[test]
fn status_update_failed() {
    let failed = UpdateStatus::DownloadFailed { offer: Box::new(the_offer()), error: "offline".into() };
    let mut h = harness(status_bar(failed), true);
    h.run();
    h.check_snapshot("status_update_failed");
    h.get_by_label("Update failed · Try again").click();
    h.run();
    assert!(sent(&h, &UpdateMsg::Download));
    // Details, right of the chip, opens the Updates row in Preferences > General.
    let chip = h.get_by_label("Update failed · Try again").bounding_box().unwrap();
    let details = h.get_by_label("Details").bounding_box().unwrap();
    assert!(details.x0 >= chip.x1, "{details:?} {chip:?}");
    h.get_by_label("Details").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)))));
}

#[test]
fn status_update_failed_tip() {
    let failed = UpdateStatus::DownloadFailed { offer: Box::new(the_offer()), error: "offline".into() };
    let mut h = harness(status_bar(failed), true);
    h.run();
    h.get_by_label("Update failed · Try again").hover();
    h.run();
    h.run();
    assert!(h.query_by_label_contains("The download failed: offline").is_some(), "the reason");
    assert!(h.query_by_label_contains("Download again").is_some());
    h.check_snapshot("status_update_failed_tip");
}

#[test]
fn status_update_downloading() {
    let mut h = harness(status_bar(UpdateStatus::Downloading(Box::new(the_offer()))), true);
    h.run_steps(4);
    h.check_snapshot("status_update_downloading");
}

#[test]
fn an_offer_without_files_opens_its_page() {
    let o = offer(&release(false), &Version::parse("0.2.0").unwrap(), None, InstallKind::Portable).unwrap();
    let mut h = harness(notes(ThemeChoice::Dark, Some(&o), NOTES), true);
    h.run();
    h.get_by_label("Open release page").click();
    h.run();
    assert!(sent(&h, &UpdateMsg::OpenReleasePage));
}
