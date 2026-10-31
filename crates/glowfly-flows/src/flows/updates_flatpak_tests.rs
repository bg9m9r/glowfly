//! Headless tests of the update check for a copy inside Flatpak: it is told about a newer
//! release and how Flatpak installs it, and never downloads or opens a file.

use super::update;
use super::updates_tests::{NOW, app, send};
use crate::effect::OsEffect;
use crate::effect::{Effect, UpdateEffect};
use crate::msg::{CheckedRelease, Msg, UpdateMsg as U};
use crate::state::AppState;
use crate::state::palette::entries;
use crate::state::updates::{UpdateStatus, flatpak_available_text, latest_text};
use glowfly_core::app_update::{Arch, Asset, InstallKind, Platform, Release, TargetOs};
use glowfly_core::i18n::Lang;
use std::time::Instant;

/// Release `tag` with the Linux tar.gz, .deb and .flatpak.
fn linux_release(tag: &str) -> Release {
    let names = ["tar.gz", "deb", "flatpak"].map(|ext| format!("glowfly-{tag}-x86_64-unknown-linux-gnu.{ext}"));
    Release {
        tag: tag.into(),
        name: tag.into(),
        html_url: format!("https://github.com/bg9m9r/glowfly/releases/tag/{tag}"),
        body: "### Features\n\n* **ui:** something new\n".into(),
        published_at: None,
        assets: names
            .iter()
            .chain(std::iter::once(&"SHA256SUMS".to_owned()))
            .map(|n| Asset { name: n.clone(), size: 100, url: format!("https://github.com/bg9m9r/glowfly/releases/download/{tag}/{n}") })
            .collect(),
    }
}

fn found_flatpak(tag: &str) -> Result<Option<CheckedRelease>, String> {
    Ok(Some(CheckedRelease { release: linux_release(tag), install: InstallKind::Flatpak }))
}

/// Glowfly 0.2.0 inside Flatpak on 64-bit Linux.
fn flatpak_app() -> AppState {
    let mut st = app();
    st.updates.platform = Some(Platform { os: TargetOs::Linux, arch: Arch::X86_64 });
    st
}

fn downloads_or_opens(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Updates(UpdateEffect::Download(_)) | Effect::Os(OsEffect::OpenUrl(_))))
}

/// A Flatpak copy with 0.3.0 on offer, found by Check now.
fn offered() -> (AppState, Instant) {
    let mut st = flatpak_app();
    let t = Instant::now();
    send(&mut st, U::CheckNow, t);
    let fx = send(&mut st, U::Checked { manual: true, result: found_flatpak("v0.3.0") }, t);
    assert!(!downloads_or_opens(&fx), "{fx:?}");
    (st, t)
}

#[test]
fn a_flatpak_copy_is_told_how_to_update() {
    let (st, _) = offered();
    let UpdateStatus::Available(o) = &st.updates.status else { panic!("{:?}", st.updates.status) };
    assert_eq!((o.version.to_string(), o.asset.clone()), ("0.3.0".to_owned(), None));
    assert!(o.through_flatpak());
    assert_eq!(st.updates.install, Some(InstallKind::Flatpak));
    assert_eq!(st.settings.updates.last_checked, Some(NOW));
    let toast = st.toasts.last().expect("a toast");
    let text = flatpak_available_text(Lang::En, &o.version);
    assert_eq!(text, "Glowfly 0.3.0 is available. Update it with Flatpak or your software centre.");
    assert_eq!(toast.text, text);
    let action = toast.action.as_ref().expect("an action");
    assert_eq!(action.label, "Release notes");
    assert!(matches!(*action.msg, Msg::Updates(U::ShowNotes)));
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some(text.as_str()));
    assert_eq!(st.updates.chip(Lang::En).map(|c| c.0).as_deref(), Some("Update available (0.3.0)"));
}

#[test]
fn download_does_nothing_for_a_flatpak_offer() {
    let (mut st, t) = offered();
    let before = st.updates.status.clone();
    let fx = send(&mut st, U::Download, t);
    assert!(fx.is_empty(), "{fx:?}");
    assert_eq!(st.updates.status, before);
    // The same through a stale toast's action.
    let id = st.toasts.last().expect("a toast").id;
    let fx = update(&mut st, Msg::ToastAction(id), t);
    assert!(!downloads_or_opens(&fx), "{fx:?}");
    assert!(matches!(st.updates.status, UpdateStatus::Available(_)));
}

#[test]
fn the_palette_offers_no_download_to_a_flatpak_copy() {
    let (st, _) = offered();
    let labels: Vec<String> = entries(&st).into_iter().map(|e| e.label).collect();
    assert!(!labels.iter().any(|l| l.starts_with("Download Glowfly")), "{labels:?}");
    assert!(labels.contains(&"View release notes".to_owned()) && labels.contains(&"Check for updates".to_owned()));
}

#[test]
fn an_automatic_check_toasts_a_flatpak_release_once() {
    let (mut st, t) = offered();
    let shown = st.toasts.len();
    send(&mut st, U::Checked { manual: false, result: found_flatpak("v0.3.0") }, t);
    assert_eq!(st.toasts.len(), shown, "the same version again");
    assert!(matches!(&st.updates.status, UpdateStatus::Available(o) if o.through_flatpak()));
}

#[test]
fn the_release_page_opens_for_a_flatpak_offer() {
    let (mut st, t) = offered();
    let fx = send(&mut st, U::OpenReleasePage, t);
    let want = "https://github.com/bg9m9r/glowfly/releases/tag/v0.3.0".to_owned();
    assert!(matches!(fx.as_slice(), [Effect::Os(OsEffect::OpenUrl(u))] if *u == want), "{fx:?}");
}

#[test]
fn an_up_to_date_flatpak_copy_hears_so() {
    let mut st = flatpak_app();
    let t = Instant::now();
    send(&mut st, U::Checked { manual: true, result: found_flatpak("v0.2.0") }, t);
    assert_eq!(st.updates.status, UpdateStatus::UpToDate);
    assert_eq!(st.toasts.last().map(|t| t.text.clone()), Some(latest_text(Lang::En, &st.updates.current)));
}
