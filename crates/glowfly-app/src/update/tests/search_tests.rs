//! Headless tests for the graph's search bar: matches, stepping through them, the filter, the
//! author picker, refreshes, tabs and jumping to a hidden commit.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::msg::{Msg, RepoMsg, SearchMsg, TabsMsg};
use crate::state::{AppState, Selection};
use git2::Oid;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Commits `msg` (changing a.txt) as `name <email>`.
fn commit_as(dir: &Path, name: &str, email: &str, msg: &str) {
    std::fs::write(dir.join("a.txt"), msg).unwrap();
    git(dir, &["-c", &format!("user.name={name}"), "-c", &format!("user.email={email}"), "commit", "-q", "-am", msg]);
}

/// A repository whose history is, newest first: 0 readme, 1 page 8, 2 page 7, 3 page 6,
/// 4 page 5, 5 page 4, 6 page 3, 7 fix typo, 8 page 2, 9 page 1, 10 first. Even pages and
/// "fix typo" are Grace's, the rest Test's. The working tree has a change (a WIP row).
fn repo(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let grace = ("Grace Hopper", "grace@example.com");
    let test = ("Test", "test@example.com");
    for (msg, (n, e)) in [
        ("page 1", test),
        ("page 2", grace),
        ("fix typo", grace),
        ("page 3", test),
        ("page 4", grace),
        ("page 5", test),
        ("page 6", grace),
        ("page 7", test),
        ("page 8", grace),
        ("readme", test),
    ] {
        commit_as(&dir, n, e, msg);
    }
    std::fs::write(dir.join("a.txt"), "changed\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let summaries: Vec<&str> = st.snap.as_ref().unwrap().commits.iter().map(|c| c.summary.as_str()).collect();
    assert_eq!(summaries[..3], ["readme", "page 8", "page 7"]);
    assert_eq!(summaries.len(), 11);
    assert!(st.snap.as_ref().unwrap().graph.wip.is_some());
    (st, dir)
}

fn up(st: &mut AppState, msg: SearchMsg) {
    update(st, Msg::Search(msg), Instant::now());
}

fn search(st: &mut AppState, q: &str) {
    st.session.tab_view.search = q.into();
    up(st, SearchMsg::QueryChanged);
}

fn oid(st: &AppState, i: usize) -> Oid {
    st.snap.as_ref().unwrap().commits[i].oid
}

const PAGES: [usize; 8] = [1, 2, 3, 4, 5, 6, 8, 9];

#[test]
fn query_changes_find_matches_and_enter_steps_through_them() {
    let (mut st, dir) = repo("search-next");
    search(&mut st, "Page");
    assert_eq!(st.session.search.matches, PAGES);
    assert_eq!(st.session.search.cursor, 10, "the next Enter lands on the newest match");
    up(&mut st, SearchMsg::Next);
    assert_eq!(st.session.selection, Selection::Commit(oid(&st, 1)));
    up(&mut st, SearchMsg::Next);
    assert_eq!(st.session.selection, Selection::Commit(oid(&st, 2)));
    for _ in 0..6 {
        up(&mut st, SearchMsg::Next);
    }
    assert_eq!(st.session.selection, Selection::Commit(oid(&st, 9)));
    up(&mut st, SearchMsg::Next);
    assert_eq!(st.session.selection, Selection::Commit(oid(&st, 1)), "wraps to the newest");

    // A new query starts over from the newest match.
    search(&mut st, "fix");
    assert_eq!(st.session.search.matches, [7]);
    up(&mut st, SearchMsg::Next);
    assert_eq!(st.session.selection, Selection::Commit(oid(&st, 7)));

    search(&mut st, "nothing like it");
    assert!(st.session.search.matches.is_empty());
    up(&mut st, SearchMsg::Next);
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some("No commits match \"nothing like it\""));
    assert_eq!(st.session.selection, Selection::Commit(oid(&st, 7)));

    // No query: Enter does nothing.
    let logged = st.log.len();
    search(&mut st, "author: ");
    up(&mut st, SearchMsg::Next);
    assert_eq!(st.log.len(), logged);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_filter_keeps_the_selection_in_view_and_close_clears_it() {
    let (mut st, dir) = repo("search-filter");
    st.view.show_search = true;
    search(&mut st, "page");
    st.session.selection = Selection::Commit(oid(&st, 9));
    up(&mut st, SearchMsg::ToggleFilter);
    assert!(st.session.search.filter);
    assert_eq!(st.view.scroll_to_row, Some(7 - 4), "page 1 is the 8th shown row");
    up(&mut st, SearchMsg::ToggleFilter);
    assert!(!st.session.search.filter);
    assert_eq!(st.view.scroll_to_row, Some(9 + 1 - 4), "all rows again, after the WIP row");

    // A commit the filter hides: back to the top.
    st.session.selection = Selection::Commit(oid(&st, 7));
    up(&mut st, SearchMsg::ToggleFilter);
    assert_eq!(st.view.scroll_to_row, Some(0));
    // A query change while filtering also keeps the selection in view.
    st.session.selection = Selection::Commit(oid(&st, 9));
    st.view.scroll_to_row = None;
    search(&mut st, "page 1");
    assert_eq!(st.view.scroll_to_row, Some(0));
    assert_eq!(st.session.search.matches, [9]);

    up(&mut st, SearchMsg::Close);
    assert!(!st.view.show_search && st.session.tab_view.search.is_empty() && !st.session.search.filter);
    assert!(st.session.search.matches.is_empty() && !st.session.search.active());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn author_terms_and_the_author_picker() {
    let (mut st, dir) = repo("search-authors");
    search(&mut st, "author:grace");
    assert_eq!(st.session.search.matches, [1, 3, 5, 7, 8]);
    search(&mut st, "author:\"Grace Hopper\" page");
    assert_eq!(st.session.search.matches, [1, 3, 5, 8]);

    search(&mut st, "page");
    st.view.author_filter = "gra".into();
    up(&mut st, SearchMsg::PickAuthor(Some("grace@example.com".into())));
    assert_eq!(st.session.tab_view.search, "author:grace@example.com page");
    assert!(st.view.author_filter.is_empty());
    assert_eq!(st.session.search.matches, [1, 3, 5, 8]);
    up(&mut st, SearchMsg::PickAuthor(Some("test@example.com".into())));
    assert_eq!(st.session.tab_view.search, "author:test@example.com page");
    assert_eq!(st.session.search.matches, [2, 4, 6, 9]);
    up(&mut st, SearchMsg::PickAuthor(None));
    assert_eq!(st.session.tab_view.search, "page");
    assert_eq!(st.session.search.matches, PAGES);

    assert!(st.session.search.authors.is_none());
    up(&mut st, SearchMsg::WantAuthors);
    let list = st.session.search.authors.clone().expect("the author list");
    let rows: Vec<(&str, usize)> = list.iter().map(|a| (a.email.as_str(), a.commits)).collect();
    assert_eq!(rows, [("test@example.com", 6), ("grace@example.com", 5)]);
    // Asked for again while open: worked out only once.
    st.session.search.authors.as_mut().unwrap().pop();
    up(&mut st, SearchMsg::WantAuthors);
    assert_eq!(st.session.search.authors.as_ref().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refresh_matches_the_new_history() {
    let (mut st, dir) = repo("search-refresh");
    search(&mut st, "page");
    up(&mut st, SearchMsg::WantAuthors);
    st.session.search.cursor = 999;
    commit_as(&dir, "Test", "test@example.com", "page 9");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.commits.len(), 12);
    assert_eq!(snap.commits[0].summary, "page 9");
    assert_eq!(st.session.search.matches, [0, 2, 3, 4, 5, 6, 7, 9, 10]);
    assert!(st.session.search.authors.is_none(), "worked out again when the picker opens");
    assert_eq!(st.session.search.cursor, 11);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn each_tab_keeps_its_own_search() {
    let (mut st, a) = repo("search-tabs-a");
    st.view.show_search = true;
    search(&mut st, "page");
    up(&mut st, SearchMsg::ToggleFilter);
    up(&mut st, SearchMsg::Next);
    let cursor = st.session.search.cursor;

    let b = scratch_repo("search-tabs-b");
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    assert_eq!(st.active_tab, 1);
    assert!(st.session.tab_view.search.is_empty() && !st.session.search.filter && st.session.search.matches.is_empty());
    search(&mut st, "first");
    assert_eq!(st.session.search.matches, [0]);

    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert_eq!(st.session.tab_view.search, "page");
    assert!(st.session.search.filter);
    assert_eq!(st.session.search.matches, PAGES);
    assert_eq!(st.session.search.cursor, cursor);

    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)), Instant::now());
    assert_eq!(st.session.tab_view.search, "first");
    assert!(!st.session.search.filter);
    assert_eq!(st.session.search.matches, [0]);
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn jumping_to_a_hidden_commit_turns_the_filter_off() {
    let (mut st, dir) = repo("search-jump");
    st.view.show_search = true;
    search(&mut st, "page");
    up(&mut st, SearchMsg::ToggleFilter);

    // Shown: the filter stays and the graph scrolls to its filtered row.
    let page1 = oid(&st, 9);
    update(&mut st, Msg::Repo(RepoMsg::JumpTo(page1)), Instant::now());
    assert!(st.session.search.filter);
    assert_eq!(st.view.scroll_to_row, Some(7 - 4));

    // Hidden: the filter goes off (the query stays) and the graph scrolls to its row.
    let typo = oid(&st, 7);
    update(&mut st, Msg::Repo(RepoMsg::JumpTo(typo)), Instant::now());
    assert!(!st.session.search.filter);
    assert_eq!(st.session.tab_view.search, "page");
    assert_eq!(st.view.scroll_to_row, Some(7 + 1 - 4));
    assert_eq!(st.session.selection, Selection::Commit(typo));
    let _ = std::fs::remove_dir_all(&dir);
}
