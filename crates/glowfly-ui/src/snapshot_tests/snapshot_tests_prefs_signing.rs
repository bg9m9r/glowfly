//! Preferences > Commit signing around an expired key: Extend by 2 years under the warning, the
//! Learn how link only after extending failed, and neither when gpg wasn't found. Clicks are
//! real pointer presses at the widget, so a widget drawn over the link would show here.

use super::snapshot_tests::{harness, settings};
use crate::msg::{Msg, PrefsMsg};
use crate::state::prefs::GitConfigValues;
use crate::state::{AppState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use glowfly_core::signing::SigningKey;
use std::time::Instant;

const HELP: &str = "https://docs.github.com/en/authentication/managing-commit-signature-verification/updating-an-expired-gpg-key";

/// Commit signing with OpenPGP keys listed as `keys`, `key` chosen.
fn page(key: &str, keys: Vec<SigningKey>) -> AppState {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    let mut prefs = PrefsState::new(PrefPage::Signing);
    prefs.git = Some(GitConfigValues {
        user_name: "Ada Lovelace".into(),
        user_email: "ada@example.com".into(),
        signing_format: "openpgp".into(),
        signing_key: key.into(),
        ..Default::default()
    });
    prefs.signing.keys = Some(keys);
    st.prefs = Some(prefs);
    st
}

fn expired() -> SigningKey {
    SigningKey {
        id: "ABCDEF0123456789".into(),
        label: "Ada Lovelace <ada@example.com>".into(),
        expires: Some(1_700_000_000),
        expired: true,
    }
}

#[test]
fn prefs_signing_expired_key() {
    let mut h = harness(page("ABCDEF0123456789", vec![expired()]), true);
    h.run();
    h.check_snapshot("prefs_signing_expired");
    assert!(h.query_by_label("Learn how").is_none(), "no help link before a try failed");
    h.get_by_label("Extend by 2 years").simulate_click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Prefs(PrefsMsg::ExtendKey))));
}

#[test]
fn prefs_signing_extend_failed() {
    let mut st = page("ABCDEF0123456789", vec![expired()]);
    st.prefs.as_mut().unwrap().signing.extend_error = Some("gpg: agent refused the operation".into());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("prefs_signing_extend_failed");
    h.state_mut().1.clear();
    h.get_by_label("Learn how").simulate_click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::OpenUrl(u) if u == HELP)));
}

#[test]
fn prefs_signing_without_gpg() {
    let mut st = page("", Vec::new());
    st.prefs.as_mut().unwrap().signing.keys_error = Some("could not run gpg: program not found".into());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("prefs_signing_no_gpg");
    for absent in ["Extend by 2 years", "Learn how", "How to extend an expired key"] {
        assert!(h.query_by_label(absent).is_none(), "{absent}");
    }
}
