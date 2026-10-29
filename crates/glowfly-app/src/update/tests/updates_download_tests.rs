//! Headless tests of an update download's progress and Cancel, and of Download again once the
//! downloaded file is found gone or changed. The runner's reports are sent by hand.

use super::update;
use super::updates_tests::{found, offered, saved, send};
use crate::effect::partial::StopKey;
use crate::effect::{Effect, UpdateEffect};
use crate::msg::{Msg, PrefsMsg, UpdateMsg as U};
use crate::state::palette::entries;
use crate::state::updates::{Transfer, UpdateStatus, progress_text};
use crate::state::{AppState, PrefPage};
use glowfly_core::app_update::{InstallKind, Progress, SavedProblem};
use glowfly_core::i18n::Lang;
use std::path::PathBuf;
use std::time::Instant;

const MSI: &str = "glowfly-v0.3.0-x86_64-pc-windows-msvc.msi";

/// An app downloading the 0.3.0 installer (100 bytes, as the release says).
fn downloading() -> (AppState, Instant) {
    let (mut st, t) = offered();
    let fx = send(&mut st, U::Download, t);
    assert!(matches!(fx.as_slice(), [Effect::Updates(UpdateEffect::Download(_))]));
    (st, t)
}

/// An app that has downloaded the 0.3.0 installer to the returned file.
fn downloaded() -> (AppState, Instant, PathBuf) {
    let (mut st, t) = downloading();
    let s = saved(MSI);
    let file = s.path.clone();
    send(&mut st, U::Downloaded(Ok(s)), t);
    assert!(matches!(st.updates.status, UpdateStatus::Downloaded { .. }));
    (st, t, file)
}

fn checks_saved(fx: &[Effect], file: &PathBuf) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Updates(UpdateEffect::CheckSaved { file: f, sha256 }) if f == file && *sha256 == [7; 32]))
}

#[test]
fn progress_is_kept_while_downloading() {
    let (mut st, t) = downloading();
    assert_eq!(st.updates.transfer, Transfer { progress: Progress { done: 0, total: Some(100) }, cancelling: false });
    send(&mut st, U::DownloadProgress(Progress { done: 40, total: Some(100) }), t);
    assert_eq!(st.updates.transfer.progress.done, 40);
    assert_eq!(progress_text(Lang::En, st.updates.transfer.progress), "Downloading… 40% (40 bytes of 100 bytes)");
    assert_eq!(progress_text(Lang::En, Progress { done: 3 * 1024 * 1024, total: None }), "Downloading… 3.0 MB");
    send(&mut st, U::Downloaded(Ok(saved(MSI))), t);
    assert_eq!(st.updates.transfer, Transfer::default(), "reset at the end");
    // A late report changes nothing once it has ended.
    send(&mut st, U::DownloadProgress(Progress { done: 90, total: Some(100) }), t);
    assert_eq!(st.updates.transfer, Transfer::default());
}

#[test]
fn cancel_stops_the_download_and_offers_it_again() {
    let (mut st, t) = downloading();
    send(&mut st, U::DownloadProgress(Progress { done: 40, total: Some(100) }), t);
    let fx = send(&mut st, U::CancelDownload, t);
    assert!(matches!(fx.as_slice(), [Effect::StopGit(StopKey::UpdateDownload)]), "{fx:?}");
    assert!(st.updates.transfer.cancelling);
    assert!(send(&mut st, U::CancelDownload, t).is_empty(), "asked once");
    assert!(send(&mut st, U::Download, t).is_empty(), "nothing new starts until it has stopped");
    send(&mut st, U::DownloadProgress(Progress { done: 60, total: Some(100) }), t);
    assert_eq!(st.updates.transfer.progress.done, 40, "progress after Cancel is ignored");
    let toasts = st.toasts.len();
    send(&mut st, U::DownloadCancelled, t);
    assert!(matches!(&st.updates.status, UpdateStatus::Available(o) if o.version.to_string() == "0.3.0"));
    assert_eq!(st.updates.transfer, Transfer::default());
    assert_eq!(st.toasts.len(), toasts, "the user asked for it: no toast");
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some("Cancelled the download of Glowfly 0.3.0."));
    let fx = send(&mut st, U::Download, t);
    assert!(matches!(fx.as_slice(), [Effect::Updates(UpdateEffect::Download(_))]), "it can start again");
    // Cancel with nothing downloading does nothing.
    send(&mut st, U::DownloadCancelled, t);
    let (mut st, t) = offered();
    assert!(send(&mut st, U::CancelDownload, t).is_empty());
}

#[test]
fn a_download_that_finished_before_its_cancel_is_kept() {
    let (mut st, t) = downloading();
    send(&mut st, U::CancelDownload, t);
    send(&mut st, U::Downloaded(Ok(saved(MSI))), t);
    assert!(matches!(st.updates.status, UpdateStatus::Downloaded { .. }));
    assert_eq!(st.updates.transfer, Transfer::default());
}

#[test]
fn the_downloaded_file_is_read_again_when_preferences_open() {
    let (mut st, t, file) = downloaded();
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)), t);
    assert!(checks_saved(&fx, &file), "{fx:?}");
    // Still there and unchanged: nothing changes.
    send(&mut st, U::SavedChecked { file: file.clone(), problem: None }, t);
    assert!(matches!(st.updates.status, UpdateStatus::Downloaded { .. }));
    // An answer about another file is ignored.
    send(&mut st, U::SavedChecked { file: PathBuf::from("elsewhere.msi"), problem: Some(SavedProblem::Missing) }, t);
    assert!(matches!(st.updates.status, UpdateStatus::Downloaded { .. }));
    // Nothing to read with no downloaded file.
    let (mut st, t) = offered();
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)), t);
    assert!(!fx.iter().any(|e| matches!(e, Effect::Updates(UpdateEffect::CheckSaved { .. }))));
}

#[test]
fn a_gone_or_changed_file_offers_download_again() {
    for problem in [SavedProblem::Missing, SavedProblem::Changed] {
        let (mut st, t, file) = downloaded();
        send(&mut st, U::SavedChecked { file: file.clone(), problem: Some(problem) }, t);
        let UpdateStatus::Stale { file: f, problem: p, offer } = &st.updates.status else { panic!("{:?}", st.updates.status) };
        assert_eq!((f, *p, offer.version.to_string()), (&file, problem, "0.3.0".to_owned()));
        assert_eq!(st.updates.chip(Lang::En).map(|c| c.0).as_deref(), Some("Update available (0.3.0)"));
        assert!(entries(&st).into_iter().any(|e| e.label == "Download Glowfly 0.3.0"));
        assert!(!entries(&st).into_iter().any(|e| e.label.starts_with("Install Glowfly")));
        assert!(send(&mut st, U::InstallAndRestart, t).is_empty(), "nothing to install");
        let fx = send(&mut st, U::Download, t);
        assert!(matches!(fx.as_slice(), [Effect::Updates(UpdateEffect::Download(o))] if o.version.to_string() == "0.3.0"));
        assert!(matches!(st.updates.status, UpdateStatus::Downloading(_)));
    }
}

#[test]
fn checks_keep_download_again_and_read_a_kept_file() {
    let (mut st, t, file) = downloaded();
    // A check that keeps the downloaded file reads it again.
    send(&mut st, U::CheckNow, t);
    let fx = send(&mut st, U::Checked { manual: true, result: found("v0.3.0", InstallKind::Installer) }, t);
    assert!(checks_saved(&fx, &file), "{fx:?}");
    // The answer may come while the next check runs.
    send(&mut st, U::CheckNow, t);
    send(&mut st, U::SavedChecked { file: file.clone(), problem: Some(SavedProblem::Changed) }, t);
    assert!(matches!(&st.updates.status, UpdateStatus::Checking { previous, .. } if matches!(**previous, UpdateStatus::Stale { .. })));
    let fx = send(&mut st, U::Checked { manual: true, result: found("v0.3.0", InstallKind::Installer) }, t);
    assert!(matches!(st.updates.status, UpdateStatus::Stale { .. }), "the same release still says Download again");
    assert!(!fx.iter().any(|e| matches!(e, Effect::Updates(UpdateEffect::CheckSaved { .. }))));
    send(&mut st, U::CheckNow, t);
    send(&mut st, U::Checked { manual: false, result: Ok(None) }, t);
    assert!(matches!(st.updates.status, UpdateStatus::Stale { .. }));
}

#[test]
fn a_file_found_changed_while_installing_waits_for_the_install() {
    let (mut st, t, file) = downloaded();
    send(&mut st, U::InstallAndRestart, t);
    assert!(st.updates.restarting);
    send(&mut st, U::SavedChecked { file: file.clone(), problem: Some(SavedProblem::Changed) }, t);
    assert!(matches!(st.updates.status, UpdateStatus::Downloaded { .. }), "the install's own check says why");
    let fx = send(&mut st, U::RestartReady(Err("The downloaded file changed after it was checked".into())), t);
    assert!(checks_saved(&fx, &file));
    send(&mut st, U::SavedChecked { file, problem: Some(SavedProblem::Changed) }, t);
    assert!(matches!(st.updates.status, UpdateStatus::Stale { problem: SavedProblem::Changed, .. }));
}
