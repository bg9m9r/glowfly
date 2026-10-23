//! The graph's search bar: dimmed and filtered rows, the match count, the author picker, and
//! the messages the bar and its keys send. Shares the fixture repository and renderer with
//! `snapshot_tests`.

use super::snapshot_tests::{git, harness, reload, remove_fixture, render, with_repo};
use super::{UiCache, avatars::Avatars, draw, theme::color};
use crate::msg::{Msg, RepoMsg, SearchMsg};
use crate::state::{AppState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::{Role, Toggled};
use eframe::egui::{self, Event, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use glowfly_core::repo::LoadOptions;
use glowfly_core::settings::ThemeChoice;
use glowfly_core::spell::Speller;
use std::path::PathBuf;
use std::time::Instant;

/// The fixture with one more commit on main, by Grace Hopper.
pub(super) fn with_two_authors(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    std::fs::write(dir.join("notes.txt"), "notes\n").unwrap();
    let d = "2026-01-11T10:00:00+00:00";
    git(&dir, &["add", "notes.txt"], d);
    git(&dir, &["commit", "-q", "-m", "Write release notes", "--author", "Grace Hopper <grace@example.com>"], d);
    reload(&mut st, &dir, &LoadOptions::all(500));
    let snap = st.snap.as_ref().unwrap();
    let grace = snap.commits.iter().find(|c| c.summary == "Write release notes").expect("the new commit");
    assert_eq!((grace.author.as_str(), grace.email.as_str()), ("Grace Hopper", "grace@example.com"));
    assert!(snap.graph.wip.is_some(), "README.md is still modified");
    (st, dir)
}

pub(super) fn send(st: &mut AppState, msg: SearchMsg) {
    crate::update::update(st, Msg::Search(msg), Instant::now());
}

/// The bar shown with `q` typed into it.
pub(super) fn searching(st: &mut AppState, q: &str) {
    st.view.show_search = true;
    st.view.search = q.into();
    send(st, SearchMsg::QueryChanged);
}

fn select(st: &mut AppState, summary: &str) {
    let oid = st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == summary).unwrap().oid;
    st.selection = Selection::Commit(oid);
}

/// Clicks the author picker open.
fn open_picker(h: &mut Harness<'_, (AppState, Vec<Msg>)>) {
    h.get(By::new().role(Role::ComboBox)).click();
    h.run();
}

fn search_msgs(h: &Harness<'_, (AppState, Vec<Msg>)>) -> Vec<String> {
    h.state()
        .1
        .iter()
        .filter_map(|m| match m {
            Msg::Search(s) => Some(match s {
                SearchMsg::Open => "Open".to_owned(),
                SearchMsg::QueryChanged => "QueryChanged".to_owned(),
                SearchMsg::Next => "Next".to_owned(),
                SearchMsg::ToggleFilter => "ToggleFilter".to_owned(),
                SearchMsg::Close => "Close".to_owned(),
                SearchMsg::Clear => "Clear".to_owned(),
                SearchMsg::WantAuthors => "WantAuthors".to_owned(),
                SearchMsg::PickAuthor(a) => format!("PickAuthor({a:?})"),
                SearchMsg::PathsSearched { request, .. } => format!("PathsSearched({request})"),
                SearchMsg::IndexProgress { build, .. } => format!("IndexProgress({build})"),
                SearchMsg::IndexBuilt { build, .. } => format!("IndexBuilt({build})"),
                SearchMsg::IndexQueried { request, .. } => format!("IndexQueried({request})"),
                SearchMsg::LoadedMatched { request, .. } => format!("LoadedMatched({request})"),
            }),
            _ => None,
        })
        .collect()
}

#[test]
fn graph_search_dim() {
    let (mut st, dir) = with_two_authors(ThemeChoice::Dark, "search-dim");
    searching(&mut st, "login");
    assert_eq!(st.search.matches.len(), 2);
    select(&mut st, "Merge branch 'login-page'");
    let mut h = harness(st, true);
    h.run_steps(4);
    assert!(h.query_by_label("1 of 2").is_some());
    h.check_snapshot("graph_search_dim");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_search_filtered() {
    for (theme, name, dark) in
        [(ThemeChoice::Dark, "graph_search_filtered", true), (ThemeChoice::Light, "graph_search_filtered_light", false)]
    {
        let (mut st, dir) = with_two_authors(theme, name);
        searching(&mut st, "page");
        send(&mut st, SearchMsg::ToggleFilter);
        assert!(st.search.filter);
        assert_eq!(st.search.matches.len(), 3);
        let mut h = harness(st, dark);
        h.run();
        assert_eq!(h.get_by_label("Filter").toggled(), Some(Toggled::True), "the Filter mode is raised");
        h.check_snapshot(name);
        drop(h);
        remove_fixture(&dir);
    }
}

/// Sends `msg`, runs the path searches it asks for and feeds their reports back.
fn send_and_search(st: &mut AppState, msg: SearchMsg) {
    let fx = crate::update::update(st, Msg::Search(msg), Instant::now());
    for effect in fx {
        if matches!(effect, crate::effect::Effect::SearchPaths { .. })
            && let Some(reply) = crate::runner::execute(effect)
        {
            crate::update::update(st, reply, Instant::now());
        }
    }
}

#[test]
fn graph_search_paths_pending() {
    let (mut st, dir) = with_two_authors(ThemeChoice::Dark, "search-paths-pending");
    send(&mut st, SearchMsg::ToggleFilter);
    // The effect isn't run: the changed files are still being looked through.
    searching(&mut st, "path:notes.txt");
    assert!(st.search.searching_files());
    let mut h = harness(st, true);
    h.run_steps(4);
    assert!(h.query_by_label("Searching files…").is_some());
    assert!(h.query_by_label("Searching changed files…").is_some());
    h.check_snapshot("graph_search_paths_pending");

    // A first chunk found some: they show, and the count says the search goes on.
    let st = &mut h.state_mut().0;
    let request = st.search.paths.pending.expect("a request");
    let commits = &st.snap.as_ref().unwrap().commits;
    let first: Vec<_> = commits[..3].iter().map(|c| c.oid).collect();
    let touching = vec![commits[0].oid];
    let chunk = crate::msg::PathsChunk { searched: first, touching, done: false };
    send(st, SearchMsg::PathsSearched { request, result: Ok(chunk) });
    h.run_steps(4);
    assert!(h.query_by_label("1 so far, searching…").is_some());
    h.check_snapshot("graph_search_paths_partial");
    drop(h);

    remove_fixture(&dir);
}

#[test]
fn graph_search_paths() {
    let (mut st, dir) = with_two_authors(ThemeChoice::Dark, "search-paths");
    st.view.show_search = true;
    send(&mut st, SearchMsg::ToggleFilter);
    st.view.search = "path:notes.txt".into();
    send_and_search(&mut st, SearchMsg::QueryChanged);
    assert!(!st.search.searching_files());
    let snap = st.snap.as_ref().unwrap();
    let shown: Vec<&str> = st.search.matches.iter().map(|&i| snap.commits[i].summary.as_str()).collect();
    assert_eq!(shown, ["Write release notes"]);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("1 match").is_some());
    h.check_snapshot("graph_search_paths");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_search_hidden_bar() {
    let (mut st, dir) = with_two_authors(ThemeChoice::Dark, "search-hidden");
    searching(&mut st, "login");
    send(&mut st, SearchMsg::ToggleFilter);
    // Esc or Ctrl+F only hide the bar: the query and filter stay, but nothing dims or hides.
    st.view.show_search = false;
    render("graph_search_hidden_bar", st, true);
    remove_fixture(&dir);
}

#[test]
fn graph_search_author_picker() {
    let (mut st, dir) = with_two_authors(ThemeChoice::Dark, "search-authors");
    searching(&mut st, "");
    send(&mut st, SearchMsg::WantAuthors);
    assert_eq!(st.search.authors.as_ref().map(Vec::len), Some(2));
    let mut h = harness(st, true);
    h.run();
    open_picker(&mut h);
    assert!(h.query_by_label("Ada Lovelace <ada@example.com>").is_some());
    h.check_snapshot("graph_search_author_picker");

    h.state_mut().1.clear();
    h.get_by_label("Grace Hopper <grace@example.com>").click();
    h.run();
    assert_eq!(search_msgs(&h), ["PickAuthor(Some(\"grace@example.com\"))"]);

    open_picker(&mut h);
    h.state_mut().1.clear();
    h.get_by_label("Any author").click();
    h.run();
    assert_eq!(search_msgs(&h), ["PickAuthor(None)"]);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_bar_sends_search_messages() {
    let (mut st, dir) = with_two_authors(ThemeChoice::Dark, "search-msgs");
    st.view.show_search = true;
    let mut h = harness(st, true);
    h.run();
    // The empty field has the keyboard.
    h.input_mut().events.push(Event::Text("log".into()));
    h.run();
    assert_eq!(h.state().0.view.search, "log");
    assert_eq!(search_msgs(&h), ["QueryChanged"]);

    // Highlight is the mode at first; clicking it again changes nothing.
    assert_eq!(h.get_by_label("Highlight").toggled(), Some(Toggled::True));
    assert_eq!(h.get_by_label("Filter").toggled(), Some(Toggled::False));
    h.state_mut().1.clear();
    h.get_by_label("Highlight").click();
    h.run();
    assert!(search_msgs(&h).is_empty(), "the current mode sends nothing");
    h.get_by_label("Filter").click();
    h.run();
    assert_eq!(search_msgs(&h), ["ToggleFilter"]);

    // Opening the picker before the authors are known asks for them.
    h.state_mut().1.clear();
    open_picker(&mut h);
    assert!(search_msgs(&h).iter().any(|m| m == "WantAuthors"));
    assert!(h.query_by_label("Reading authors…").is_some());
    h.press_key(Key::Escape);
    h.run();

    h.state_mut().1.clear();
    h.get_by_label("Close").click();
    h.run();
    assert_eq!(search_msgs(&h), ["Close"]);
    drop(h);
    remove_fixture(&dir);
}

/// A harness that also runs the shortcuts, as the app's input handling does.
fn keys_harness(st: AppState) -> Harness<'static, (AppState, Vec<Msg>)> {
    let mut cache: Option<UiCache> = None;
    Harness::builder().with_size(egui::vec2(1400.0, 820.0)).build_state(
        move |ctx, (st, msgs): &mut (AppState, Vec<Msg>)| {
            color::set_dark(true);
            super::shortcuts::input(ctx, st, msgs);
            let cache = cache.get_or_insert_with(|| UiCache {
                avatars: Avatars::default(),
                speller: Speller::new(),
                diff_rows: Default::default(),
                history_text: Default::default(),
                history_preview: Default::default(),
                blame_rows: Default::default(),
                diff_images: Default::default(),
                markdown: Default::default(),
            });
            draw(ctx, st, cache, msgs);
        },
        (st, Vec::new()),
    )
}

#[test]
fn find_keys_show_the_bar_and_toggle_the_filter() {
    let (st, dir) = with_two_authors(ThemeChoice::Dark, "search-keys");
    let mut h = keys_harness(st);
    h.run();
    h.press_key_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::F);
    h.run();
    assert!(h.state().0.view.show_search);
    assert_eq!(search_msgs(&h), ["ToggleFilter"]);

    h.state_mut().1.clear();
    h.press_key_modifiers(Modifiers::COMMAND, Key::F);
    h.run();
    assert!(!h.state().0.view.show_search);
    assert!(search_msgs(&h).is_empty(), "Ctrl+F only hides the bar");
    h.press_key_modifiers(Modifiers::COMMAND, Key::F);
    h.run();
    assert!(h.state().0.view.show_search);
    // Ctrl+Shift+F with the bar shown keeps it shown.
    h.press_key_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::F);
    h.run();
    assert!(h.state().0.view.show_search);
    assert_eq!(search_msgs(&h), ["ToggleFilter"]);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn arrow_keys_move_through_the_shown_matches() {
    let (mut st, dir) = with_two_authors(ThemeChoice::Dark, "search-arrows");
    searching(&mut st, "page");
    send(&mut st, SearchMsg::ToggleFilter);
    let snap = st.snap.as_ref().unwrap();
    let shown: Vec<_> = st.search.matches.iter().map(|&i| snap.commits[i].oid).collect();
    assert_eq!(shown.len(), 3);
    // Merge branch 'login-page', then Add login page with Fix build on Windows hidden between.
    st.selection = Selection::Commit(shown[1]);
    let mut h = harness(st, true);
    h.run_steps(4);
    h.state_mut().1.clear();
    h.press_key(Key::ArrowDown);
    h.run_steps(4);
    let selected: Vec<_> =
        h.state().1.iter().filter_map(|m| if let Msg::Repo(RepoMsg::SelectCommit(o)) = m { Some(*o) } else { None }).collect();
    assert_eq!(selected, [shown[2]], "the next shown match, past the hidden rows");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_search_clear() {
    let (mut st, dir) = with_two_authors(ThemeChoice::Dark, "search-clear");
    searching(&mut st, "release");
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("1 match").is_some());
    let close_x = |h: &Harness<'_, (AppState, Vec<Msg>)>| h.get_by_label("Close").bounding_box().expect("Close is drawn").x0;
    let before = close_x(&h);
    let field = h.get(By::new().role(Role::TextInput).value("release")).bounding_box().unwrap();
    let clear = h.get_by_label("Clear search").bounding_box().expect("the clear button is drawn");
    assert!(clear.x0 > field.x0 && clear.x1 <= field.x1 && clear.y0 >= field.y0 && clear.y1 <= field.y1, "inside the field");
    h.check_snapshot("graph_search_clear");

    // The count changes length; Close stays where it was.
    let st = &mut h.state_mut().0;
    searching(st, "path:notes.txt");
    let request = st.search.paths.pending.expect("a request");
    let commits = &st.snap.as_ref().unwrap().commits;
    let searched: Vec<_> = commits[..3].iter().map(|c| c.oid).collect();
    let chunk = crate::msg::PathsChunk { searched, touching: vec![commits[0].oid], done: false };
    send(st, SearchMsg::PathsSearched { request, result: Ok(chunk) });
    h.run_steps(4);
    assert!(h.query_by_label("1 so far, searching…").is_some());
    assert_eq!(close_x(&h), before, "the count's slot keeps its width");

    h.state_mut().1.clear();
    h.get_by_label("Clear search").click();
    h.run_steps(4);
    assert_eq!(search_msgs(&h), ["Clear"]);
    h.state_mut().0.view.search.clear();
    h.run_steps(4);
    assert!(h.query_by_label("Clear search").is_none(), "nothing to clear");
    drop(h);
    remove_fixture(&dir);
}
