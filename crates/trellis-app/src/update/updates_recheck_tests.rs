//! Headless tests of checking again once an update is on offer or downloaded: the downloaded
//! file, its Install entry and chip outlast later checks, automatic or manual, and a failed
//! check doesn't lose the offer.

use super::updates_tests::{NOW, app, checks, found, offered, send, tick};
use crate::effect::{Effect, UpdateEffect};
use crate::msg::UpdateMsg as U;
use crate::state::AppState;
use crate::state::palette::entries;
use crate::state::updates::{STARTUP_DELAY, UpdateStatus};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use trellis_core::app_update::InstallKind;
use trellis_core::i18n::Lang;

/// An app that has downloaded the 0.3.0 installer.
fn downloaded() -> (AppState, Instant, PathBuf) {
    let (mut st, t) = offered();
    send(&mut st, U::Download, t);
    let file = PathBuf::from("Downloads").join("trellis-v0.3.0-x86_64-pc-windows-msvc.msi");
    send(&mut st, U::Downloaded(Ok(file.clone())), t);
    assert!(matches!(st.updates.status, UpdateStatus::Downloaded { installer: true, .. }));
    (st, t, file)
}

fn has_install(st: &AppState) -> bool {
    entries(st).into_iter().any(|e| e.label == "Install Trellis 0.3.0")
}

#[test]
fn check_now_after_a_download_keeps_the_file() {
    let (mut st, t, file) = downloaded();
    let toasts = st.toasts.len();
    assert_eq!(checks(&send(&mut st, U::CheckNow, t)), [true]);
    // While the check runs, the chip and Install still work.
    assert_eq!(st.updates.chip(Lang::En).map(|c| c.0).as_deref(), Some("Update downloaded"));
    assert!(has_install(&st));
    let fx = send(&mut st, U::Install, t);
    assert!(matches!(fx.as_slice(), [Effect::Updates(UpdateEffect::Install(p))] if *p == file));

    send(&mut st, U::Checked { manual: true, result: found("v0.3.0", InstallKind::Installer) }, t);
    assert!(matches!(&st.updates.status, UpdateStatus::Downloaded { file: f, installer: true, .. } if *f == file));
    assert_eq!(st.updates.chip(Lang::En).map(|c| c.0).as_deref(), Some("Update downloaded"));
    assert!(has_install(&st));
    assert_eq!(st.toasts.len(), toasts + 1);
    let toast = st.toasts.last().unwrap();
    assert_eq!((toast.text.as_str(), toast.action.as_ref().map(|a| a.label.as_str())), ("Downloaded Trellis 0.3.0.", Some("Install")));
}

#[test]
fn the_daily_check_after_a_download_keeps_the_file() {
    let (mut st, t, file) = downloaded();
    st.settings.updates.last_checked = Some(NOW - 86_400);
    send(&mut st, U::Startup, t);
    assert_eq!(checks(&tick(&mut st, t + STARTUP_DELAY)), [false]);
    let toasts = st.toasts.len();
    send(&mut st, U::Checked { manual: false, result: found("v0.3.0", InstallKind::Installer) }, t);
    assert!(matches!(&st.updates.status, UpdateStatus::Downloaded { file: f, .. } if *f == file));
    assert_eq!(st.toasts.len(), toasts, "nothing new to say");
    // Nothing newer published either: still downloaded.
    send(&mut st, U::CheckNow, t);
    send(&mut st, U::Checked { manual: true, result: Ok(None) }, t);
    assert!(matches!(&st.updates.status, UpdateStatus::Downloaded { .. }));
    assert!(has_install(&st));
}

#[test]
fn a_failed_check_after_a_download_keeps_the_file() {
    let (mut st, t, file) = downloaded();
    st.settings.updates.last_checked = Some(NOW - 86_400);
    send(&mut st, U::Startup, t);
    let t1 = t + STARTUP_DELAY;
    assert_eq!(checks(&tick(&mut st, t1)), [false]);
    send(&mut st, U::Checked { manual: false, result: Err("offline".into()) }, t1);
    assert!(matches!(&st.updates.status, UpdateStatus::Downloaded { file: f, .. } if *f == file));
    assert!(checks(&tick(&mut st, t1 + Duration::from_secs(60))).is_empty(), "still waits before retrying");
    send(&mut st, U::CheckNow, t1);
    send(&mut st, U::Checked { manual: true, result: Err("offline".into()) }, t1);
    assert!(matches!(&st.updates.status, UpdateStatus::Downloaded { .. }), "the failure is only toasted");
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Couldn't check for updates: offline"));
    assert!(has_install(&st));
}

#[test]
fn a_newer_release_replaces_an_older_download() {
    let (mut st, t, _) = downloaded();
    send(&mut st, U::CheckNow, t);
    let mut newer = found("v0.4.0", InstallKind::Installer);
    if let Ok(Some(f)) = &mut newer {
        for a in &mut f.release.assets {
            a.name = a.name.replace("0.3.0", "0.4.0");
        }
    }
    send(&mut st, U::Checked { manual: true, result: newer }, t);
    let UpdateStatus::Available(o) = &st.updates.status else { panic!("{:?}", st.updates.status) };
    assert_eq!(o.version.to_string(), "0.4.0");
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Trellis 0.4.0 is available."));
}

#[test]
fn an_offer_outlasts_later_checks() {
    let (mut st, t) = offered();
    let toasts = st.toasts.len();
    // A failed automatic check keeps the offer.
    st.settings.updates.last_checked = Some(NOW - 86_400);
    send(&mut st, U::Startup, t);
    let t1 = t + STARTUP_DELAY;
    assert_eq!(checks(&tick(&mut st, t1)), [false]);
    assert_eq!(st.updates.chip(Lang::En).map(|c| c.0).as_deref(), Some("Update available (0.3.0)"), "while checking");
    send(&mut st, U::Checked { manual: false, result: Err("offline".into()) }, t1);
    assert!(matches!(&st.updates.status, UpdateStatus::Available(o) if o.version.to_string() == "0.3.0"));
    // The next day's check finds the same release: offered still, without a second toast.
    send(&mut st, U::Checked { manual: false, result: found("v0.3.0", InstallKind::Installer) }, t1);
    assert!(matches!(&st.updates.status, UpdateStatus::Available(_)));
    assert_eq!(st.toasts.len(), toasts);
    // A manual failure with nothing on offer still shows the failure in Preferences.
    let mut st = app();
    send(&mut st, U::CheckNow, t);
    send(&mut st, U::Checked { manual: true, result: Err("offline".into()) }, t);
    assert_eq!(st.updates.status, UpdateStatus::CheckFailed("offline".into()));
}
