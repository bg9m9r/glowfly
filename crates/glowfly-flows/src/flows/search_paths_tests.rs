//! Headless tests for the search's path terms: the requests update sends, merging reports,
//! the cache across query changes, refreshes, Close and tab switches, and the filtered graph.

use super::tests::{drive, git, scratch_repo, skipped, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, PathsChunk, RepoMsg, SearchMsg, TabsMsg};
use crate::runner;
use crate::state::graph_rows::GraphRows;
use crate::state::{AppState, Selection};
use git2::Oid;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Writes `files` and commits them as `name`.
fn commit_as(dir: &Path, name: &str, files: &[&str], msg: &str) {
    for f in files {
        let path = dir.join(f);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, msg).unwrap();
    }
    let email = format!("{}@example.com", name.to_lowercase());
    git(dir, &["add", "-A"]);
    git(dir, &["-c", &format!("user.name={name}"), "-c", &format!("user.email={email}"), "commit", "-q", "-m", msg]);
}

/// A repository whose history is, newest first: 0 tweak view (Grace, src/ui/c.rs), 1 docs and
/// code (Grace, docs/b.md and src/a.rs), 2 more docs (Test, docs/c.md), 3 add view (Test,
/// src/ui/c.rs), 4 write docs (Grace, docs/b.md), 5 add source (Test, src/a.rs), 6 readme (Test,
/// README.md), 7 first (Test, a.txt).
fn repo(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    commit_as(&dir, "Test", &["README.md"], "readme");
    commit_as(&dir, "Test", &["src/a.rs"], "add source");
    commit_as(&dir, "Grace", &["docs/b.md"], "write docs");
    commit_as(&dir, "Test", &["src/ui/c.rs"], "add view");
    commit_as(&dir, "Test", &["docs/c.md"], "more docs");
    commit_as(&dir, "Grace", &["docs/b.md", "src/a.rs"], "docs and code");
    commit_as(&dir, "Grace", &["src/ui/c.rs"], "tweak view");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let summaries: Vec<&str> = st.snap.as_ref().unwrap().commits.iter().map(|c| c.summary.as_str()).collect();
    assert_eq!(summaries, ["tweak view", "docs and code", "more docs", "add view", "write docs", "add source", "readme", "first"]);
    (st, dir)
}

fn oid(st: &AppState, i: usize) -> Oid {
    st.snap.as_ref().unwrap().commits[i].oid
}

fn oids(st: &AppState, rows: &[usize]) -> Vec<Oid> {
    rows.iter().map(|&i| oid(st, i)).collect()
}

/// Types `q`: returns the path searches update asked for, as (request, oids).
fn search(st: &mut AppState, q: &str) -> Vec<(u64, Vec<Oid>)> {
    st.session.tab_view.search = q.into();
    let fx = update(st, Msg::Search(SearchMsg::QueryChanged), Instant::now());
    settle(st, fx)
}

/// Runs `fx` and what follows, as `tests::finish` does, except path searches: those are
/// returned as (request, oids) instead.
fn settle(st: &mut AppState, fx: Vec<Effect>) -> Vec<(u64, Vec<Oid>)> {
    let mut asked = Vec::new();
    let mut queue = Vec::new();
    let mut fx = fx;
    loop {
        for effect in fx {
            match effect {
                Effect::SearchPaths { request, oids, .. } => asked.push((request, oids)),
                e if !skipped(&e) => queue.extend(runner::testing::execute(e)),
                _ => {}
            }
        }
        let Some(m) = queue.pop() else { return asked };
        fx = update(st, m, Instant::now());
    }
}

/// Sends `msg` and lets everything it leads to run, path searches included.
fn up(st: &mut AppState, msg: SearchMsg) {
    drive(st, Msg::Search(msg));
}

fn report(st: &mut AppState, request: u64, result: Result<PathsChunk, String>) {
    up(st, SearchMsg::PathsSearched { request, result });
}

/// Types `q` and runs the path search it asks for.
fn search_all(st: &mut AppState, q: &str) {
    st.session.tab_view.search = q.into();
    up(st, SearchMsg::QueryChanged);
}

#[test]
fn a_path_term_asks_for_every_loaded_commit_then_matches() {
    let (mut st, dir) = repo("search-paths-ask");
    let asked = search(&mut st, "path:docs");
    let all: Vec<usize> = (0..8).collect();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].1, oids(&st, &all), "every loaded commit, in index order");
    assert_eq!(st.session.search.paths.pending, Some(asked[0].0));
    assert_eq!(st.session.search.count_text(None, st.history_count()).as_deref(), Some("Searching files…"));
    assert!(st.session.search.matches.is_empty());

    search_all(&mut st, "path:docs");
    assert_eq!(st.session.search.matches, [1, 2, 4]);
    assert_eq!(st.session.search.paths.pending, None);
    assert_eq!(st.session.search.count_text(None, st.history_count()).as_deref(), Some("3 matches"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_commits_passing_the_other_terms_are_searched() {
    let (mut st, dir) = repo("search-paths-author");
    let asked = search(&mut st, "author:grace path:docs");
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].1, oids(&st, &[0, 1, 4]), "only Grace's commits");
    search_all(&mut st, "author:grace path:docs");
    assert_eq!(st.session.search.matches, [1, 4]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn answers_are_kept_while_only_the_text_changes() {
    let (mut st, dir) = repo("search-paths-cache");
    search_all(&mut st, "path:docs docs");
    assert_eq!(st.session.search.matches, [1, 2, 4]);
    assert!(search(&mut st, "path:docs more").is_empty(), "every candidate was searched");
    assert_eq!(st.session.search.matches, [2]);
    assert_eq!(st.session.search.paths.pending, None);
    let asked = search(&mut st, "path:docs");
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].1, oids(&st, &[0, 3, 5, 6, 7]), "only the commits not looked at yet");
    assert_eq!(st.session.search.matches, [1, 2, 4], "the ones known so far");
    assert!(st.session.search.searching_files());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn new_path_terms_start_again_and_old_reports_are_ignored() {
    let (mut st, dir) = repo("search-paths-new-terms");
    let old = search(&mut st, "path:docs")[0].0;
    let asked = search(&mut st, "path:src/ui");
    assert_eq!(asked.len(), 1);
    let new = asked[0].0;
    assert!(new > old);
    assert_eq!(asked[0].1.len(), 8);
    assert_eq!(st.session.search.paths.pending, Some(new));
    let all = oids(&st, &[0, 1, 2, 3, 4, 5, 6, 7]);
    report(&mut st, old, Ok(PathsChunk { searched: all.clone(), touching: all, done: true }));
    assert!(st.session.search.matches.is_empty() && st.session.search.paths.searched.is_empty(), "a stale report");
    assert_eq!(st.session.search.paths.pending, Some(new));
    search_all(&mut st, "path:src/ui");
    assert_eq!(st.session.search.matches, [0, 3]);
    // Same terms in another spelling: the answers stay.
    assert!(search(&mut st, "file:SRC\\UI").is_empty());
    assert_eq!(st.session.search.matches, [0, 3]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn new_path_terms_drop_the_old_terms_matches() {
    let (mut st, dir) = repo("search-paths-new-terms-matches");
    search_all(&mut st, "path:docs");
    assert_eq!(st.session.search.matches, [1, 2, 4]);
    let asked = search(&mut st, "path:src");
    assert_eq!(asked.len(), 1);
    assert!(st.session.search.matches.is_empty(), "no commit is known to touch src yet");
    assert_eq!(st.session.search.count_text(None, st.history_count()).as_deref(), Some("Searching files…"));
    let before = st.session.selection.clone();
    up(&mut st, SearchMsg::Next);
    assert_eq!(st.session.selection, before, "Enter doesn't land on a docs commit");
    let chunk = PathsChunk { searched: oids(&st, &[0, 1, 2]), touching: oids(&st, &[0, 1]), done: false };
    report(&mut st, asked[0].0, Ok(chunk));
    assert_eq!(st.session.search.matches, [0, 1]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn partial_reports_match_and_enter_steps_through_them() {
    let (mut st, dir) = repo("search-paths-partial");
    let request = search(&mut st, "path:docs")[0].0;
    let chunk = PathsChunk { searched: oids(&st, &[0, 1, 2]), touching: oids(&st, &[1, 2]), done: false };
    report(&mut st, request, Ok(chunk));
    assert_eq!(st.session.search.matches, [1, 2]);
    assert_eq!(st.session.search.paths.pending, Some(request));
    assert_eq!(st.session.search.count_text(None, st.history_count()).as_deref(), Some("2 so far, searching…"));
    up(&mut st, SearchMsg::Next);
    assert_eq!(st.session.selection, Selection::Commit(oid(&st, 1)));
    up(&mut st, SearchMsg::Next);
    assert_eq!(st.session.selection, Selection::Commit(oid(&st, 2)));
    up(&mut st, SearchMsg::Next);
    assert_eq!(st.session.selection, Selection::Commit(oid(&st, 1)), "wraps");

    // Nothing found yet: Enter does nothing.
    search(&mut st, "path:nothing");
    assert!(st.session.search.searching_files());
    let logged = st.log.len();
    up(&mut st, SearchMsg::Next);
    assert_eq!(st.log.len(), logged, "no \"No commits match\" while searching");
    assert_eq!(st.session.selection, Selection::Commit(oid(&st, 1)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_search_is_logged_and_ends() {
    let (mut st, dir) = repo("search-paths-error");
    let request = search(&mut st, "path:docs")[0].0;
    report(&mut st, request, Err("no such repository".into()));
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some("Could not search changed files: no such repository"));
    assert_eq!(st.session.search.paths.pending, None);
    assert_eq!(st.session.search.count_text(None, st.history_count()).as_deref(), Some("No matches"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refresh_searches_only_the_new_commits() {
    let (mut st, dir) = repo("search-paths-refresh");
    search_all(&mut st, "path:docs");
    let gone = Oid::from_bytes(&[9; 20]).unwrap();
    st.session.search.paths.searched.insert(gone);
    st.session.search.paths.touching.insert(gone);
    commit_as(&dir, "Test", &["docs/new.md"], "newest docs");
    let fx = update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    let asked = settle(&mut st, fx);
    assert_eq!(st.snap.as_ref().unwrap().commits.len(), 9);
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].1, [oid(&st, 0)], "only the new commit");
    assert!(
        !st.session.search.paths.searched.contains(&gone) && !st.session.search.paths.touching.contains(&gone),
        "pruned to the loaded commits"
    );
    assert_eq!(st.session.search.paths.searched.len(), 8);
    assert_eq!(st.session.search.matches, [2, 3, 5], "the known answers, shifted down");
    let all = oids(&st, &[0]);
    report(&mut st, asked[0].0, Ok(PathsChunk { searched: all.clone(), touching: all, done: true }));
    assert_eq!(st.session.search.matches, [0, 2, 3, 5]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn close_drops_the_answers() {
    let (mut st, dir) = repo("search-paths-close");
    st.view.show_search = true;
    let request = search(&mut st, "path:docs")[0].0;
    up(&mut st, SearchMsg::Close);
    let p = &st.session.search.paths;
    assert!(p.pending.is_none() && p.terms.is_empty() && p.searched.is_empty() && p.touching.is_empty());
    assert_eq!(p.requests, request, "the counter is kept");
    let all = oids(&st, &[1]);
    report(&mut st, request, Ok(PathsChunk { searched: all.clone(), touching: all, done: true }));
    assert!(st.session.search.paths.touching.is_empty() && st.session.search.matches.is_empty(), "a late report");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_tab_switch_drops_reports_and_asks_again_on_return() {
    let (mut st, a) = repo("search-paths-tabs-a");
    let request = search(&mut st, "path:docs")[0].0;
    let all = oids(&st, &[0, 1, 2, 3, 4, 5, 6, 7]);

    let b = scratch_repo("search-paths-tabs-b");
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    assert_eq!(st.active_tab, 1);
    let late = PathsChunk { searched: all.clone(), touching: all.clone(), done: true };
    let msg = Msg::Search(SearchMsg::PathsSearched { request, result: Ok(late) });
    update(&mut st, Msg::Scoped { workdir: a.clone(), msg: Box::new(msg) }, Instant::now());

    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    let asked = settle(&mut st, fx);
    assert_eq!(st.session.tab_view.search, "path:docs");
    assert!(st.session.search.paths.touching.is_empty(), "the report for the parked tab was dropped");
    assert!(!asked.is_empty());
    let last = asked.last().unwrap();
    assert!(last.0 > request);
    assert_eq!(last.1, all, "everything is still unsearched");
    assert_eq!(st.session.search.paths.pending, Some(last.0));
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn the_filtered_graph_fills_in_when_the_search_reports() {
    let (mut st, dir) = repo("search-paths-filter");
    st.view.show_search = true;
    up(&mut st, SearchMsg::ToggleFilter);
    let request = search(&mut st, "path:docs")[0].0;
    let snap = st.snap.as_ref().unwrap();
    let rows = GraphRows::new(snap, &st.session.search, true);
    assert!(rows.filtered() && rows.is_empty());
    let all = oids(&st, &[0, 1, 2, 3, 4, 5, 6, 7]);
    let touching = oids(&st, &[1, 2, 4]);
    report(&mut st, request, Ok(PathsChunk { searched: all, touching, done: true }));
    let rows = GraphRows::new(st.snap.as_ref().unwrap(), &st.session.search, true);
    assert_eq!(rows.shown(), Some(&[1, 2, 4][..]));
    let _ = std::fs::remove_dir_all(&dir);
}
