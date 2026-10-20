//! Repository colours: set and cleared from the Color submenus (saved only when they change),
//! looked up where a tab really is, followed by the window's top line as the active tab
//! changes, and shared with a listed folder that a tab shows under another spelling.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, RecentMsg, TabsMsg};
use crate::state::repo_colours::{active_colour, listed_colour, tab_colour};
use crate::state::{AppState, Tab};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::settings::RepoColour;

/// A made-up folder (nothing is read from disk), absolute on every OS.
fn fake(name: &str) -> PathBuf {
    trellis_core::paths::temp_dir().join("trellis-repo-colour").join(name)
}

fn set(st: &mut AppState, path: PathBuf, colour: Option<RepoColour>) -> Vec<Effect> {
    update(st, Msg::Recent(RecentMsg::SetColour { path, colour }), Instant::now())
}

fn saves(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::SaveSettings(_))).count()
}

/// Unloaded tabs for made-up folders, with tab `active` selected.
fn tabs(names: &[&str], active: usize) -> AppState {
    let mut st = state();
    st.tabs = names.iter().map(|n| Tab::unloaded(fake(n))).collect();
    st.active_tab = active;
    st
}

#[test]
fn set_and_clear() {
    let mut st = state();
    let fx = set(&mut st, fake("a"), Some(RepoColour::Teal));
    assert_eq!(saves(&fx), 1, "{fx:?}");
    assert_eq!(st.settings.repo_colour_for(&fake("a")), Some(RepoColour::Teal));
    let fx = set(&mut st, fake("a"), Some(RepoColour::Teal));
    assert!(fx.is_empty(), "the same colour again saves nothing: {fx:?}");
    let fx = set(&mut st, fake("a"), None);
    assert_eq!(saves(&fx), 1, "{fx:?}");
    assert!(st.settings.repo_colours.is_empty());
    assert!(set(&mut st, fake("a"), None).is_empty(), "None while there is none saves nothing");
}

#[test]
fn the_tab_colour_follows_where_the_tab_really_is() {
    let mut st = tabs(&["link", "plain"], 0);
    st.tabs.push(Tab::empty());
    st.tabs[0].real = Some(fake("real"));
    st.settings.set_repo_colour(&fake("real"), Some(RepoColour::Iris));
    st.settings.set_repo_colour(&fake("link"), Some(RepoColour::Rose));
    st.settings.set_repo_colour(&fake("plain"), Some(RepoColour::Fern));
    assert_eq!(tab_colour(&st, 0), Some(RepoColour::Iris), "the loaded tab looks where it really is");
    assert_eq!(tab_colour(&st, 1), Some(RepoColour::Fern), "an unloaded tab uses its path");
    assert_eq!(tab_colour(&st, 2), None, "a new tab has none");
    assert_eq!(tab_colour(&st, 9), None, "no such tab");
}

#[test]
fn the_active_colour_follows_the_tab() {
    let mut st = tabs(&["a", "b", "c"], 0);
    st.settings.set_repo_colour(&fake("a"), Some(RepoColour::Teal));
    st.settings.set_repo_colour(&fake("b"), Some(RepoColour::Plum));
    assert_eq!(active_colour(&st), Some(RepoColour::Teal));
    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)), Instant::now());
    assert_eq!(active_colour(&st), Some(RepoColour::Plum));
    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(2)), Instant::now());
    assert_eq!(active_colour(&st), None, "c has no colour, so no line");
    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    update(&mut st, Msg::Tabs(TabsMsg::CloseTab(0)), Instant::now());
    assert_eq!(st.tabs[st.active_tab].path, Some(fake("b")));
    assert_eq!(active_colour(&st), Some(RepoColour::Plum), "closing the active tab moves the line to the next");
}

#[test]
fn a_listed_path_takes_its_open_tabs_colour() {
    let mut st = tabs(&["shown"], 0);
    st.tabs[0].real = Some(fake("real"));
    st.settings.set_repo_colour(&fake("real"), Some(RepoColour::Sky));
    assert_eq!(listed_colour(&st, &fake("shown")), Some(RepoColour::Sky), "the tab shows that folder");
    assert_eq!(listed_colour(&st, &fake("real")), Some(RepoColour::Sky));
    assert_eq!(listed_colour(&st, &fake("other")), None);
    st.settings.set_repo_colour(&fake("shown"), Some(RepoColour::Ochre));
    assert_eq!(listed_colour(&st, &fake("shown")), Some(RepoColour::Ochre), "its own colour comes first");
}

#[test]
fn set_colour_is_never_scoped() {
    assert!(RecentMsg::SetColour { path: fake("a"), colour: Some(RepoColour::Teal) }.on_inactive().is_none());
}
