//! Preferences > Trusted repositories: dated and undated decisions, Trusted and Not trusted, a
//! repository with the decisions passed on from it, a folder the check didn't find with Forget
//! missing folders, the page while the check runs, the empty page, the filter and what it
//! doesn't match, German, and the buttons only asking update (`TrustMsg`).

use super::snapshot_check::CheckSnapshot;
use super::snapshot_tests::{NOW, harness, render, settings};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::{RepoTrust, SavedTrust, ThemeChoice};
use glowfly_model::msg::{Msg, TrustMsg};
use glowfly_model::state::{AppState, PrefPage, PrefsState};
use std::path::PathBuf;
use std::time::Instant;

const DAY: i64 = 86_400;

/// A work folder that is absolute on every system.
fn at(name: &str) -> PathBuf {
    PathBuf::from(if cfg!(windows) { r"C:\work" } else { "/home/me/work" }).join(name)
}

fn decide(st: &mut AppState, name: &str, trust: RepoTrust, when: Option<i64>, from: Option<&str>) {
    let path = at(name);
    st.settings.set_trust(&path, Some(SavedTrust { trust, print: None }));
    if when.is_some() || from.is_some() {
        st.settings.stamp_trust(&path, when, from.map(at).as_deref());
    }
}

/// Preferences on the page, its folder check done when `missing` is Some.
fn page(missing: Option<&[&str]>) -> AppState {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    let mut prefs = PrefsState::new(PrefPage::Trust);
    prefs.trust.missing = missing.map(|m| m.iter().map(|n| at(n)).collect());
    st.prefs = Some(prefs);
    st
}

/// Every kind of row: dated Trusted with an heir, the heir, Not trusted, a decision saved
/// before dates were kept, and one whose folder is gone.
fn every_kind(missing: Option<&[&str]>) -> AppState {
    let mut st = page(missing);
    decide(&mut st, "tools", RepoTrust::Trusted, Some(NOW - 3 * DAY), None);
    decide(&mut st, "tools-feature", RepoTrust::Trusted, Some(NOW - 2 * DAY), Some("tools"));
    decide(&mut st, "vendor-lib", RepoTrust::NotTrusted, Some(NOW - 40 * DAY), None);
    decide(&mut st, "legacy", RepoTrust::Trusted, None, None);
    decide(&mut st, "scratch", RepoTrust::Trusted, Some(NOW - 400 * DAY), None);
    st
}

#[test]
fn prefs_trust() {
    let mut h = harness(every_kind(Some(&["scratch"])), true);
    h.run();
    h.check_snapshot("prefs_trust");
    assert_eq!(h.query_all_by_label("Trusted repositories").count(), 2, "the navigation row and the title");
    for label in ["Revoke trust in tools", "Forget the decision for vendor-lib", "Not found", "Forget 1 missing folder"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert!(h.query_by_label("Filter by folder").is_none(), "five decisions need no filter");
}

#[test]
fn prefs_trust_checking() {
    let mut h = harness(every_kind(None), true);
    h.run();
    h.check_snapshot("prefs_trust_checking");
    assert!(h.query_by_label("Not found").is_none());
    assert!(h.query_by_label_contains("missing folder").is_none());
}

#[test]
fn prefs_trust_empty() {
    render("prefs_trust_empty", page(Some(&[])), true);
}

/// Nine decisions: the filter shows, and "lib" keeps the rows whose path has it.
fn nine(filter: &str) -> AppState {
    let mut st = page(Some(&[]));
    for name in ["api", "app", "docs", "libgit-fork", "mobile", "site", "tools", "vendor-lib", "web"] {
        decide(&mut st, name, RepoTrust::Trusted, Some(NOW - DAY), None);
    }
    st.prefs.as_mut().unwrap().trust.filter = filter.into();
    st
}

#[test]
fn prefs_trust_filter() {
    let mut h = harness(nine("lib"), true);
    h.run();
    h.check_snapshot("prefs_trust_filter");
    assert!(h.query_by_label("Filter by folder").is_some());
    assert!(h.query_by_label("Revoke trust in libgit-fork").is_some() && h.query_by_label("Revoke trust in vendor-lib").is_some());
    assert!(h.query_by_label("Revoke trust in docs").is_none());
}

#[test]
fn prefs_trust_no_match() {
    let mut h = harness(nine("nothing-like-this"), true);
    h.run();
    h.check_snapshot("prefs_trust_no_match");
    assert!(h.query_by_label("No repositories match").is_some());
}

#[test]
fn prefs_trust_de() {
    let mut st = every_kind(Some(&["scratch"]));
    st.settings.language = "de".into();
    render("prefs_trust_de", st, false);
}

/// The messages clicking `label` sends.
fn click(st: AppState, label: &str) -> Vec<Msg> {
    let mut h = harness(st, true);
    h.run();
    h.get_by_label(label).click();
    h.run();
    std::mem::take(&mut h.state_mut().1)
}

#[test]
fn the_buttons_only_ask() {
    let msgs = click(every_kind(Some(&["scratch"])), "Revoke trust in tools");
    assert!(msgs.iter().any(|m| matches!(m, Msg::Trust(TrustMsg::Revoke { real }) if *real == at("tools"))));
    let msgs = click(every_kind(Some(&["scratch"])), "Forget 1 missing folder");
    assert!(msgs.iter().any(|m| matches!(m, Msg::Trust(TrustMsg::ForgetMissing))));
}
