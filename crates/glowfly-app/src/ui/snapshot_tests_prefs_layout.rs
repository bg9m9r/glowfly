//! Preferences at a narrow window (960 × 700): rows stack or shrink to the room left, so nothing
//! on General, Integrations or Profile runs past the content column (the window less its 24 px
//! right padding).

use super::snapshot_tests::{harness_sized, settings};
use crate::state::prefs::GitConfigValues;
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::{Account, Connection, Provider};
use glowfly_core::git_version::GitInfo;
use glowfly_core::settings::{Profile, ProfileId, ThemeChoice};
use std::time::Instant;

const WIDTH: f32 = 960.0;
/// The content column's right edge, with a point for rounding.
const RIGHT: f64 = 960.0 - 24.0 + 1.0;
/// Where the status bar starts (its version text sits at the window's right edge).
const STATUS_TOP: f64 = 700.0 - 30.0;

fn narrow(st: AppState) -> Harness<'static, (AppState, Vec<crate::msg::Msg>)> {
    let mut h = harness_sized(st, true, egui::vec2(WIDTH, 700.0));
    h.run();
    h
}

/// The widgets above the status bar whose right edge is past the content column, by label.
fn overflowing(h: &Harness<'static, (AppState, Vec<crate::msg::Msg>)>) -> Vec<String> {
    h.query_all_by(|_| true)
        .filter_map(|n| {
            let b = n.bounding_box()?;
            (b.y1 < STATUS_TOP && b.x1 > RIGHT).then(|| format!("{:?} {:?} ends at {}", n.role(), n.label(), b.x1))
        })
        .collect()
}

#[test]
fn prefs_general_narrow() {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    let mut prefs = PrefsState::new(PrefPage::General);
    prefs.git = Some(GitConfigValues::default());
    prefs.git_saved = prefs.git.clone();
    st.prefs = Some(prefs);
    let program = if cfg!(windows) { "C:/Program Files/Git/cmd/git.exe" } else { "/opt/homebrew/bin/git" };
    st.git.info = Some(GitInfo { path: Some(program.into()), version: Some((2, 45, 1)), chosen: false });
    let mut h = narrow(st);
    h.check_snapshot("prefs_general_narrow");
    let over = overflowing(&h);
    assert!(over.is_empty(), "{over:#?}");
}

#[test]
fn prefs_integrations_narrow() {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    let account = Account { provider: Provider::GitLabSelfManaged, host: "gitlab.example.com".into(), org: String::new() };
    st.settings.connections = vec![Connection { account, user: "ada-lovelace".into(), method: "Git Credential Manager".into() }];
    let mut prefs = PrefsState::new(PrefPage::Integrations);
    (prefs.int_provider, prefs.int_host, prefs.int_adding) = (Provider::GitLabSelfManaged, "gitlab.example.com".into(), true);
    st.prefs = Some(prefs);
    let mut h = narrow(st);
    h.check_snapshot("prefs_integrations_narrow");
    let over = overflowing(&h);
    assert!(over.is_empty(), "{over:#?}");
}

#[test]
fn prefs_profile_narrow() {
    let mut s = settings(ThemeChoice::Dark);
    s.profiles = vec![Profile {
        id: ProfileId(1),
        name: "Open source contributions".into(),
        author_name: "Ada Lovelace".into(),
        author_email: "ada.lovelace@analytical-engines.example".into(),
        ..Profile::default()
    }];
    s.active_profile = Some(ProfileId(1));
    let mut st = AppState::new(s, Vec::new(), Instant::now());
    let mut prefs = PrefsState::new(PrefPage::Profile);
    prefs.profiles.fill(&st.settings, Some(ProfileId(1)));
    prefs.git = Some(GitConfigValues::default());
    st.prefs = Some(prefs);
    let h = narrow(st);
    let over = overflowing(&h);
    assert!(over.is_empty(), "{over:#?}");
}
