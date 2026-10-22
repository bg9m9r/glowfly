//! A copy inside Flatpak: Preferences > General's Updates row and the release notes dialog with
//! a newer release (how Flatpak updates it, no Download), and the Glowfly Links row (Flatpak
//! registers links, nothing to register). Click tests check the message each button sends.

use super::{prefs, sent};
use crate::deep_link::DeepLinkMsg;
use crate::msg::{Msg, UpdateMsg};
use crate::state::deep_link::SchemeView;
use crate::state::release_notes::NotesDialog;
use crate::state::updates::UpdateStatus;
use crate::state::{AppState, Modal, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{harness, harness_sized, settings};
use eframe::egui;
use egui_kittest::kittest::Queryable;
use glowfly_core::app_update::{Arch, Asset, InstallKind, Offer, Platform, Release, TargetOs, Version, offer};
use glowfly_core::settings::ThemeChoice;
use glowfly_core::url_scheme::{FLATPAK_NOTE, SchemeStatus};
use std::time::Instant;

const LINUX: Platform = Platform { os: TargetOs::Linux, arch: Arch::X86_64 };

/// v0.3.0 with the Linux downloads, the Flatpak bundle among them.
fn release() -> Release {
    let names = ["tar.gz", "deb", "flatpak"].map(|ext| format!("glowfly-v0.3.0-x86_64-unknown-linux-gnu.{ext}"));
    let url = |n: &str| format!("https://github.com/bg9m9r/glowfly/releases/download/v0.3.0/{n}");
    Release {
        tag: "v0.3.0".into(),
        name: "v0.3.0".into(),
        html_url: "https://github.com/bg9m9r/glowfly/releases/tag/v0.3.0".into(),
        body: "## [0.3.0](https://github.com/bg9m9r/glowfly/compare/v0.2.0...v0.3.0) (2026-02-01)\n\n### Features\n\n\
               * **updates:** check for a new version once a day ([1a2b3c4](https://github.com/bg9m9r/glowfly/commit/1a2b3c4))\n"
            .into(),
        published_at: None,
        assets: names.iter().map(|n| Asset { name: n.clone(), size: 10, url: url(n) }).collect(),
    }
}

fn flatpak_offer() -> Offer {
    offer(&release(), &Version::parse("0.2.0").unwrap(), Some(LINUX), InstallKind::Flatpak).expect("newer")
}

#[test]
fn prefs_updates_flatpak() {
    let mut st = prefs(UpdateStatus::Available(Box::new(flatpak_offer())));
    st.updates.platform = Some(LINUX);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label_contains("installed with Flatpak").is_some());
    assert!(h.query_by_label("Download").is_none());
    h.check_snapshot("prefs_updates_flatpak");
    h.get_by_label("Release notes").click();
    h.run();
    assert!(sent(&h, &UpdateMsg::ShowNotes));
    h.get_by_label("Open release page").click();
    h.run();
    assert!(sent(&h, &UpdateMsg::OpenReleasePage));
}

#[test]
fn release_notes_flatpak_offer() {
    let o = flatpak_offer();
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    st.settings.welcome_done = true;
    st.updates.current = Version::parse("0.2.0").unwrap();
    st.updates.platform = Some(LINUX);
    st.updates.bundled_notes = "";
    let d = NotesDialog::new("", &st.updates.current, Version::parse("0.1.0"), Some(&o));
    st.modal = Some(Modal::ReleaseNotes(Box::new(d)));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label_contains("installed with Flatpak").is_some());
    assert!(h.query_by_label("Download").is_none());
    h.check_snapshot("release_notes_flatpak_offer");
    h.get_by_label("Open release page").click();
    h.run();
    assert!(sent(&h, &UpdateMsg::OpenReleasePage));
}

#[test]
fn prefs_links_flatpak() {
    if cfg!(target_os = "macos") {
        return; // the row shows only the macOS note there
    }
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    st.prefs = Some(PrefsState::new(PrefPage::General));
    st.deep_link.scheme = SchemeView::Known(SchemeStatus::Unsupported(FLATPAK_NOTE));
    let mut h = harness_sized(st, true, egui::vec2(1400.0, 1800.0));
    h.run();
    assert!(h.query_by_label(FLATPAK_NOTE).is_some());
    assert!(h.query_by_label("Register").is_none() && h.query_by_label("Unregister").is_none());
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::DeepLink(DeepLinkMsg::Register | DeepLinkMsg::Unregister))));
    h.check_snapshot("prefs_links_flatpak");
}
