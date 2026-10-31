//! Empty panes in the shared empty-state style: the dashboard without repositories and with a
//! filter that matches nothing, a filtered graph search without matches (also with only part
//! of the history loaded), and a repository without commits.

use super::snapshot_tests::{harness, remove_fixture, settings, try_git};
use super::snapshot_tests_search::{searching, send, with_two_authors};
use crate::msg::{CloneMsg, DashboardMsg, LoadMsg, Msg, PrefsMsg, RepoMsg, SearchMsg};
use crate::state::{AppState, PrefPage, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::{self, LoadOptions};
use glowfly_core::settings::ThemeChoice;
use std::time::Instant;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// Clicks the button labelled `label` and returns what it pushed.
fn click(h: &mut H, label: &str) -> Vec<Msg> {
    h.state_mut().1.clear();
    h.get_by_label(label).click();
    h.run();
    std::mem::take(&mut h.state_mut().1)
}

#[test]
fn dashboard_empty() {
    for (theme, name, dark) in [(ThemeChoice::Dark, "dashboard_empty", true), (ThemeChoice::Light, "dashboard_empty_light", false)] {
        let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
        st.dashboard.open = true;
        let mut h = harness(st, dark);
        h.run();
        h.check_snapshot(name);
        assert!(h.query_by_label("No repositories yet").is_some());
        assert!(matches!(&click(&mut h, "Open repository…")[..], [Msg::Repo(RepoMsg::PickRepo)]));
        assert!(matches!(&click(&mut h, "Clone repository…")[..], [Msg::Clone(CloneMsg::Dialog)]));
        let msgs = click(&mut h, "Connect a service…");
        assert!(matches!(&msgs[..], [Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Integrations))]));
    }
}

#[test]
fn dashboard_filter_no_match() {
    let mut st = super::snapshot_tests_dashboard::state(ThemeChoice::Dark);
    st.dashboard.filter = "nothing like this".into();
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("dashboard_filter_no_match");
    assert!(h.query_by_label(super::dashboard::NO_MATCH_TITLE).is_some());
    let msgs = click(&mut h, super::dashboard::CLEAR_FILTER);
    assert!(matches!(&msgs[..], [Msg::Dashboard(DashboardMsg::ClearFilter)]));
}

#[test]
fn graph_search_no_matches() {
    for (theme, name, dark) in
        [(ThemeChoice::Dark, "graph_search_no_matches", true), (ThemeChoice::Light, "graph_search_no_matches_light", false)]
    {
        let (mut st, dir) = with_two_authors(theme, &format!("search-none-{dark}"));
        searching(&mut st, "nothing like this");
        send(&mut st, SearchMsg::ToggleFilter);
        let mut h = harness(st, dark);
        h.run();
        assert!(h.query_by_label("No matches").is_some());
        assert!(h.query_by_label("No commits match").is_some());
        let hint = glowfly_core::i18n::Lang::En.tr("search.empty.hint");
        assert!(h.query_by_label_contains(hint).is_some(), "the syntax hint");
        let n = h.state().0.snap.as_ref().unwrap().commits.len();
        assert!(h.query_by_label_contains(&format!("Searched the whole history ({n} commits).")).is_some());
        assert!(h.query_by_label("Load more commits").is_none(), "the whole history is loaded");
        assert!(h.query_by_label("Keep searching").is_none());
        h.check_snapshot(name);
        // The pane's button (the lower one: the search field has a clear button by that name too).
        assert_eq!(h.query_all_by_label("Clear search").count(), 2);
        h.state_mut().1.clear();
        let y = |n: &egui_kittest::kittest::Node<'_>| n.bounding_box().map_or(0.0, |r| r.y0);
        h.get_all_by_label("Clear search").max_by(|a, b| y(a).total_cmp(&y(b))).unwrap().click();
        h.run();
        assert!(matches!(&h.state().1[..], [Msg::Search(SearchMsg::Clear)]));
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn graph_search_no_matches_truncated() {
    let (mut st, dir) = with_two_authors(ThemeChoice::Dark, "search-none-truncated");
    st.snap.as_mut().unwrap().truncated = true;
    // Without the preference nothing loads by itself, so the pane offers Load more commits.
    st.settings.load_more_on_scroll = false;
    searching(&mut st, "nothing like this");
    send(&mut st, SearchMsg::ToggleFilter);
    let n = st.snap.as_ref().unwrap().commits.len();
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label_contains(&format!("Only the newest {n} commits are loaded.")).is_some());
    h.check_snapshot("graph_search_no_matches_truncated");
    let msgs = click(&mut h, "Load more commits");
    assert!(msgs.iter().any(|m| matches!(m, Msg::Load(LoadMsg::LoadMoreCommits))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_no_commits() {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-snap-{}-no-commits", std::process::id())).join("empty");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (ok, out) = try_git(&dir, &["init", "-q", "-b", "main"], "2026-01-05T10:00:00+00:00");
    assert!(ok, "{out}");
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    st.session.selection = Selection::Wip;
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(super::graph_empty::NO_COMMITS_TITLE).is_some());
    h.check_snapshot("graph_no_commits");
    drop(h);
    remove_fixture(&dir);
}
