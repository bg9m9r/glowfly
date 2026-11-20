use super::*;
use glowfly_core::forge::{HELPER_SOURCE, Provider};

fn auth(source: &str) -> Auth {
    Auth::from_token(Provider::GitHub, None, "t", source)
}

fn dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

#[test]
fn environment_and_saved_answers_serve_every_folder() {
    let mut c = AuthCache::default();
    c.store("gh".into(), &dir("a"), auth("GITHUB_TOKEN"), false);
    for d in [dir("a"), dir("b"), crate::state::AppState::outside_dir()] {
        assert_eq!(c.get("gh", &d, false).map(|a| a.source.as_str()), Some("GITHUB_TOKEN"));
        assert!(c.get("gh", &d, true).is_some(), "an environment answer always serves");
    }
    assert!(c.get("gl", &dir("a"), false).is_none());
}

#[test]
fn a_helper_answer_serves_only_its_folder() {
    let mut c = AuthCache::default();
    c.store("gh".into(), &dir("a"), auth(HELPER_SOURCE), true);
    assert!(c.get("gh", &dir("a"), false).is_some());
    assert!(c.get("gh", &dir("a").join("."), false).is_some(), "same spelling");
    assert!(c.get("gh", &dir("b"), false).is_none());
    assert!(c.get("gh", &crate::state::AppState::outside_dir(), false).is_none());
}

#[test]
fn an_environment_only_lookup_reuses_a_helper_answer_only_when_background_ok() {
    let mut c = AuthCache::default();
    c.store("gh".into(), &dir("a"), auth(HELPER_SOURCE), false);
    assert!(c.get("gh", &dir("a"), false).is_some());
    assert!(c.get("gh", &dir("a"), true).is_none(), "a quiet helper answer doesn't keep a background refresh going");
    c.store("gh".into(), &dir("a"), auth(HELPER_SOURCE), true);
    assert!(c.get("gh", &dir("a"), true).is_some());
    c.store("gh".into(), &dir("a"), auth(HELPER_SOURCE), false);
    assert!(c.get("gh", &dir("a"), true).is_some(), "a later quiet answer keeps what the user asked for");
}

#[test]
fn storing_replaces_the_same_account_and_place() {
    let mut c = AuthCache::default();
    c.store("gh".into(), &dir("a"), auth("GITHUB_TOKEN"), false);
    c.store("gh".into(), &dir("b"), auth("saved token"), false);
    assert_eq!(c.entries.len(), 1);
    assert_eq!(c.get("gh", &dir("a"), false).map(|a| a.source.as_str()), Some("saved token"));
    c.store("gh".into(), &dir("a"), auth(HELPER_SOURCE), false);
    c.store("gh".into(), &dir("b"), auth(HELPER_SOURCE), false);
    assert_eq!(c.entries.len(), 3);
}

#[test]
fn forgetting_drops_the_account_everywhere() {
    let mut c = AuthCache::default();
    c.store("gh".into(), &dir("a"), auth("GITHUB_TOKEN"), true);
    c.store("gh".into(), &dir("a"), auth(HELPER_SOURCE), true);
    c.store("gh".into(), &dir("b"), auth(HELPER_SOURCE), true);
    c.store("gl".into(), &dir("a"), auth("GITLAB_TOKEN"), true);
    c.forget("gh");
    assert!(c.get("gh", &dir("a"), false).is_none() && c.get("gh", &dir("b"), false).is_none());
    assert!(c.get("gl", &dir("a"), false).is_some());
    c.clear();
    assert!(c.is_empty());
}
