//! Headless tests of Trellis's own update check: when automatic checks run, what a check's
//! answer does, downloads and their results, and the palette entries. No network: the effects
//! are checked and their results sent by hand.

use super::tests::state;
use super::update;
use crate::effect::OsEffect;
use crate::effect::{Effect, UpdateEffect};
use crate::msg::{CheckedRelease, Msg, PaletteMsg, UpdateMsg as U};
use crate::state::AppState;
use crate::state::palette::PaletteCommand as C;
use crate::state::palette::entries;
use crate::state::updates::{RETRY_AFTER, STARTUP_DELAY, UpdateStatus};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use trellis_core::app_update::{Arch, Asset, InstallKind, Platform, Release, TargetOs, Version};
use trellis_core::i18n::Lang;

/// 2026-02-01 12:00 UTC.
pub(super) const NOW: i64 = 1_769_947_200;

/// Trellis 0.2.0 on 64-bit Windows, with the clock at `NOW`.
pub(super) fn app() -> AppState {
    let mut st = state();
    st.settings.welcome_done = true;
    st.settings.updates.notes_seen = Some("0.2.0".into());
    st.updates.current = Version::parse("0.2.0").unwrap();
    st.updates.platform = Some(Platform { os: TargetOs::Windows, arch: Arch::X86_64 });
    st.updates.bundled_notes = "";
    st.view.now = NOW;
    st
}

fn asset(name: &str) -> Asset {
    Asset { name: name.into(), size: 100, url: format!("https://github.com/bg9m9r/trellis/releases/download/v0.3.0/{name}") }
}

/// Release `tag` with the Windows files; `files: false` publishes none.
pub(super) fn release(tag: &str, files: bool) -> Release {
    let names = ["trellis-v0.3.0-x86_64-pc-windows-msvc.msi", "trellis-v0.3.0-x86_64-pc-windows-msvc.zip", "SHA256SUMS"];
    Release {
        tag: tag.into(),
        name: tag.into(),
        html_url: format!("https://github.com/bg9m9r/trellis/releases/tag/{tag}"),
        body: "### Features\n\n* **ui:** something new\n".into(),
        published_at: None,
        assets: if files { names.iter().map(|n| asset(n)).collect() } else { Vec::new() },
    }
}

pub(super) fn send(st: &mut AppState, m: U, now: Instant) -> Vec<Effect> {
    update(st, Msg::Updates(m), now)
}

pub(super) fn found(tag: &str, install: InstallKind) -> Result<Option<CheckedRelease>, String> {
    Ok(Some(CheckedRelease { release: release(tag, true), install }))
}

pub(super) fn checks(fx: &[Effect]) -> Vec<bool> {
    fx.iter().filter_map(|e| if let Effect::Updates(UpdateEffect::Check { manual }) = e { Some(*manual) } else { None }).collect()
}

fn saves(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::SaveSettings(_)))
}

pub(super) fn tick(st: &mut AppState, t: Instant) -> Vec<Effect> {
    update(st, Msg::Tick(t), t)
}

/// An app with 0.3.0 on offer (found by an automatic check).
pub(super) fn offered() -> (AppState, Instant) {
    let mut st = app();
    let t = Instant::now();
    send(&mut st, U::Checked { manual: false, result: found("v0.3.0", InstallKind::Installer) }, t);
    assert!(matches!(st.updates.status, UpdateStatus::Available(_)));
    (st, t)
}

#[test]
fn automatic_checks_wait_for_startup_and_the_delay() {
    let mut st = app();
    let t0 = Instant::now();
    assert!(checks(&tick(&mut st, t0 + Duration::from_secs(60))).is_empty(), "not armed without Startup");
    send(&mut st, U::Startup, t0);
    assert!(checks(&tick(&mut st, t0 + Duration::from_secs(14))).is_empty(), "before 15 s");
    let fx = tick(&mut st, t0 + STARTUP_DELAY);
    assert_eq!(checks(&fx), [false]);
    assert!(matches!(st.updates.status, UpdateStatus::Checking { manual: false, .. }));
    assert!(checks(&tick(&mut st, t0 + STARTUP_DELAY + Duration::from_secs(1))).is_empty(), "never twice at once");
}

#[test]
fn automatic_checks_honour_the_setting_and_the_last_check() {
    let t0 = Instant::now();
    let later = t0 + STARTUP_DELAY;
    let mut st = app();
    st.settings.updates.check = false;
    send(&mut st, U::Startup, t0);
    assert!(checks(&tick(&mut st, later)).is_empty(), "turned off");
    let mut st = app();
    st.settings.updates.last_checked = Some(NOW - 3600);
    send(&mut st, U::Startup, t0);
    assert!(checks(&tick(&mut st, later)).is_empty(), "checked an hour ago");
    st.settings.updates.last_checked = Some(NOW - 86_400);
    assert_eq!(checks(&tick(&mut st, later)), [false], "a day later");
}

#[test]
fn a_newer_release_is_offered() {
    let mut st = app();
    let t = Instant::now();
    st.updates.status = UpdateStatus::Checking { manual: false, previous: Box::default() };
    let fx = send(&mut st, U::Checked { manual: false, result: found("v0.3.0", InstallKind::Installer) }, t);
    let UpdateStatus::Available(o) = &st.updates.status else { panic!("{:?}", st.updates.status) };
    assert_eq!(o.version.to_string(), "0.3.0");
    assert_eq!(o.asset.as_ref().map(|a| a.name.as_str()), Some("trellis-v0.3.0-x86_64-pc-windows-msvc.msi"));
    assert_eq!(st.settings.updates.last_checked, Some(NOW));
    assert!(saves(&fx));
    let toast = st.toasts.last().expect("a toast");
    assert_eq!(toast.text, "Trellis 0.3.0 is available.");
    assert_eq!(toast.action.as_ref().map(|a| a.label.as_str()), Some("Download"));
    // Its Download button downloads.
    let id = toast.id;
    let fx = update(&mut st, Msg::ToastAction(id), t);
    assert!(fx.iter().any(|e| matches!(e, Effect::Updates(UpdateEffect::Download(_)))));
    assert!(matches!(st.updates.status, UpdateStatus::Downloading(_)));
    assert_eq!(st.updates.chip(Lang::En).map(|c| c.0).as_deref(), Some("Downloading Trellis 0.3.0…"));
}

#[test]
fn update_texts_follow_the_language() {
    let mut st = app();
    st.settings.language = "de".into();
    let t = Instant::now();
    send(&mut st, U::CheckNow, t);
    send(&mut st, U::Checked { manual: true, result: found("v0.3.0", InstallKind::Installer) }, t);
    let want = Lang::De.trf("updates.available", &[("version", &"0.3.0")]);
    assert_ne!(want, "Trellis 0.3.0 is available.");
    let toast = st.toasts.last().expect("a toast");
    assert_eq!(toast.text, want);
    assert_eq!(toast.action.as_ref().map(|a| a.label.as_str()), Some(Lang::De.tr("updates.download.button")));
    let chip = Lang::De.trf("updates.chip.available", &[("version", &"0.3.0")]);
    assert_eq!(st.updates.chip(Lang::De).map(|c| c.0), Some(chip));
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some(want.as_str()));
}

#[test]
fn the_same_version_is_up_to_date() {
    let mut st = app();
    let t = Instant::now();
    send(&mut st, U::Checked { manual: true, result: found("v0.2.0", InstallKind::Portable) }, t);
    assert_eq!(st.updates.status, UpdateStatus::UpToDate);
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Trellis 0.2.0 is the latest version."));
    let mut st = app();
    send(&mut st, U::Checked { manual: false, result: found("v0.1.0", InstallKind::Portable) }, t);
    assert_eq!(st.updates.status, UpdateStatus::UpToDate);
    assert!(st.toasts.is_empty(), "an automatic check stays quiet");
    // No release at all.
    send(&mut st, U::Checked { manual: false, result: Ok(None) }, t);
    assert_eq!((st.updates.status.clone(), st.settings.updates.last_checked), (UpdateStatus::UpToDate, Some(NOW)));
}

#[test]
fn a_failed_automatic_check_is_quiet_and_waits() {
    let mut st = app();
    let t0 = Instant::now();
    send(&mut st, U::Startup, t0);
    let t1 = t0 + STARTUP_DELAY;
    assert_eq!(checks(&tick(&mut st, t1)), [false]);
    let fx = send(&mut st, U::Checked { manual: false, result: Err("offline".into()) }, t1);
    assert!(st.toasts.is_empty() && !saves(&fx));
    assert!(st.log.iter().any(|l| l.text.contains("offline")) && !st.view.show_log, "logged quietly");
    assert_eq!((st.updates.status.clone(), st.settings.updates.last_checked), (UpdateStatus::Idle, None));
    assert!(checks(&tick(&mut st, t1 + Duration::from_secs(60))).is_empty(), "waits");
    assert_eq!(checks(&tick(&mut st, t1 + RETRY_AFTER)), [false], "tries again an hour later");
}

#[test]
fn a_failed_manual_check_says_so() {
    let mut st = app();
    let t = Instant::now();
    assert_eq!(checks(&send(&mut st, U::CheckNow, t)), [true]);
    assert!(checks(&send(&mut st, U::CheckNow, t)).is_empty(), "not while checking");
    send(&mut st, U::Checked { manual: true, result: Err("GitHub's rate limit was reached".into()) }, t);
    assert_eq!(st.updates.status, UpdateStatus::CheckFailed("GitHub's rate limit was reached".into()));
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Couldn't check for updates: GitHub's rate limit was reached"));
}

#[test]
fn downloads_and_what_follows() {
    let (mut st, t) = offered();
    let fx = send(&mut st, U::Download, t);
    let Some(Effect::Updates(UpdateEffect::Download(o))) = fx.first() else { panic!("no download") };
    assert_eq!(o.asset.as_ref().unwrap().name, "trellis-v0.3.0-x86_64-pc-windows-msvc.msi");
    assert!(checks(&send(&mut st, U::CheckNow, t)).is_empty(), "not while downloading");
    assert!(send(&mut st, U::Download, t).is_empty(), "nor a second download");

    let file = PathBuf::from("Downloads").join("trellis-v0.3.0-x86_64-pc-windows-msvc.msi");
    send(&mut st, U::Downloaded(Ok(file.clone())), t);
    assert_eq!(st.updates.status, UpdateStatus::Downloaded { offer: Box::new(o.as_ref().clone()), file: file.clone(), installer: true });
    let toast = st.toasts.last().unwrap();
    assert_eq!((toast.text.as_str(), toast.action.as_ref().map(|a| a.label.as_str())), ("Downloaded Trellis 0.3.0.", Some("Install")));
    assert_eq!(st.updates.chip(Lang::En).map(|c| c.0).as_deref(), Some("Update downloaded"));
    let fx = send(&mut st, U::Install, t);
    assert!(matches!(fx.as_slice(), [Effect::Updates(UpdateEffect::Install(p))] if *p == file));
    let fx = send(&mut st, U::ShowFile, t);
    assert!(matches!(fx.as_slice(), [Effect::Os(OsEffect::RevealPath(p))] if *p == file));
}

#[test]
fn an_archive_is_shown_not_installed() {
    let (mut st, t) = offered();
    send(&mut st, U::Download, t);
    let zip = PathBuf::from("Downloads").join("trellis-v0.3.0-x86_64-pc-windows-msvc.zip");
    send(&mut st, U::Downloaded(Ok(zip.clone())), t);
    assert!(matches!(&st.updates.status, UpdateStatus::Downloaded { installer: false, .. }));
    assert_eq!(st.toasts.last().and_then(|t| t.action.as_ref()).map(|a| a.label.as_str()), Some("Show file"));
    assert!(send(&mut st, U::Install, t).is_empty(), "an archive isn't opened");
}

#[test]
fn a_failed_download_can_be_retried() {
    let (mut st, t) = offered();
    send(&mut st, U::Download, t);
    send(&mut st, U::Downloaded(Err("The download didn't match its published checksum and was discarded.".into())), t);
    assert!(matches!(&st.updates.status, UpdateStatus::DownloadFailed { .. }));
    assert_eq!(st.updates.chip(Lang::En).map(|c| c.0).as_deref(), Some("Update failed · Try again"));
    assert!(st.toasts.last().unwrap().text.starts_with("The download failed: "));
    let fx = send(&mut st, U::Download, t);
    assert!(matches!(fx.as_slice(), [Effect::Updates(UpdateEffect::Download(_))]));
}

#[test]
fn without_a_build_for_this_system_download_opens_the_page() {
    let mut st = app();
    st.updates.platform = Some(Platform { os: TargetOs::Linux, arch: Arch::Aarch64 });
    let t = Instant::now();
    send(&mut st, U::Checked { manual: true, result: found("v0.3.0", InstallKind::Installer) }, t);
    let fx = send(&mut st, U::Download, t);
    assert!(matches!(fx.as_slice(), [Effect::Os(OsEffect::OpenUrl(u))] if u == "https://github.com/bg9m9r/trellis/releases/tag/v0.3.0"));
    assert!(matches!(st.updates.status, UpdateStatus::Available(_)));
    let fx = send(&mut st, U::OpenReleasePage, t);
    assert!(matches!(fx.as_slice(), [Effect::Os(OsEffect::OpenUrl(u))] if u.ends_with("/releases/tag/v0.3.0")));
    let mut st = app();
    let fx = send(&mut st, U::OpenReleasePage, t);
    assert!(matches!(fx.as_slice(), [Effect::Os(OsEffect::OpenUrl(u))] if u == "https://github.com/bg9m9r/trellis/releases"));
}

#[test]
fn palette_entries_follow_the_status() {
    let labels = |st: &AppState| entries(st).into_iter().map(|e| e.label).collect::<Vec<_>>();
    let mut st = app();
    let l = labels(&st);
    assert!(l.contains(&"View release notes".to_owned()) && l.contains(&"Check for updates".to_owned()), "no repository needed");
    assert!(!l.iter().any(|l| l.starts_with("Download Trellis") || l.starts_with("Install Trellis")));
    let t = Instant::now();
    update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Updates(U::CheckNow))), t);
    assert!(matches!(st.updates.status, UpdateStatus::Checking { manual: true, .. }));
    assert_eq!(st.palette_recent.keys(), &["check-updates"]);
    let busy = entries(&st).into_iter().find(|e| e.label == "Check for updates").and_then(|e| e.detail);
    assert_eq!(busy.as_deref(), Some("checking…"));
    send(&mut st, U::Checked { manual: true, result: found("v0.3.0", InstallKind::Installer) }, t);
    assert!(labels(&st).contains(&"Download Trellis 0.3.0".to_owned()));
    update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Updates(U::Download))), t);
    assert!(matches!(st.updates.status, UpdateStatus::Downloading(_)));
    assert!(!labels(&st).iter().any(|l| l.contains("Trellis 0.3.0")));
    send(&mut st, U::Downloaded(Ok(PathBuf::from("Downloads").join("trellis-v0.3.0-x86_64-pc-windows-msvc.msi"))), t);
    assert!(labels(&st).contains(&"Install Trellis 0.3.0".to_owned()));
    let fx = update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Updates(U::Install))), t);
    assert!(matches!(fx.as_slice(), [Effect::Updates(UpdateEffect::Install(_))]));
    update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Updates(U::ShowNotes))), t);
    assert_eq!(st.palette_recent.keys(), &["release-notes", "check-updates"]);
}
