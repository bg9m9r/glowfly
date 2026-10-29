//! Headless tests for author avatars: fetched in batches only when lookups are turned on and the
//! repository is on a public hosting service, and handed to the shell.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::{AvatarMsg, Msg};
use crate::state::AppState;
use std::time::Instant;

/// A repository open with one `url` remote (or none), lookups turned `on` or off; `name` keeps
/// each fixture's folder apart.
fn with_remote(name: &str, on: bool, url: Option<&str>) -> AppState {
    let mut st = state();
    st.settings.avatar_lookups = on;
    let mut snap = crate::state::op_fixture::snapshot(&format!("avatars-{name}"));
    snap.remote_urls = url.map(|u| vec![("origin".to_owned(), u.to_owned())]).unwrap_or_default();
    st.snap = Some(snap);
    st
}

/// The batch sizes of the fetches asked for these ten authors.
fn fetches(st: &mut AppState) -> Vec<usize> {
    let emails: Vec<String> = (0..10).map(|i| format!("dev{i}@example.com")).collect();
    let fx = update(st, Msg::Avatars(AvatarMsg::WantAvatars(emails)), Instant::now());
    fx.iter().map(|e| if let Effect::FetchAvatars(b) = e { b.len() } else { 0 }).collect()
}

#[test]
fn avatars_are_fetched_in_batches_and_handed_to_the_shell() {
    let mut st = with_remote("batches", true, Some("https://github.com/o/r.git"));
    assert_eq!(fetches(&mut st), [8, 2]);
    update(&mut st, Msg::Avatars(AvatarMsg::AvatarsLoaded(vec![("dev0@example.com".into(), None)])), Instant::now());
    assert_eq!(st.view.avatars_loaded.len(), 1);
}

#[test]
fn nothing_is_looked_up_unless_turned_on_and_publicly_hosted() {
    assert!(!glowfly_core::settings::Settings::default().avatar_lookups, "off by default");
    assert!(fetches(&mut with_remote("off", false, Some("https://github.com/o/r.git"))).is_empty(), "off");
    assert!(fetches(&mut with_remote("self-hosted", true, Some("git@git.example.com:o/r.git"))).is_empty(), "self-hosted");
    assert!(fetches(&mut with_remote("no-remote", true, None)).is_empty(), "no remote");
    let mut st = state();
    st.settings.avatar_lookups = true;
    assert!(fetches(&mut st).is_empty(), "no repository open");
}
