//! Headless tests for the palette's groups with an empty query: Recent, Suggested from the
//! repository's state, then All commands, each command once; no headings for a typed query or
//! when one group would hold everything; and the keys still moving over commands only.

use super::palette_tests::repo_with_topic;
use super::tests::state;
use super::update;
use crate::msg::{Msg, PaletteMsg};
use crate::state::AppState;
use crate::state::palette::{Group, PaletteCommand as C, listed, shown};
use git2::RepositoryState;
use glowfly_core::repo::{FileEntry, RemoteInfo};
use std::time::Instant;

fn labels(st: &AppState) -> Vec<String> {
    shown(st, "").into_iter().map(|s| s.entry.label).collect()
}

/// The repository with an origin remote, main behind its upstream, and a changed file.
fn behind_with_changes(name: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = repo_with_topic(name);
    let snap = st.snap.as_mut().unwrap();
    snap.remote_info = vec![RemoteInfo { name: "origin".into(), urls: vec!["https://example.com/a.git".into()], ..RemoteInfo::default() }];
    snap.remote_urls = vec![("origin".into(), "https://example.com/a.git".into())];
    snap.unstaged = vec![FileEntry::new("a.txt", 'M')];
    let head = snap.locals.iter_mut().find(|b| b.is_head).unwrap();
    head.upstream = Some("origin/main".into());
    head.behind = 2;
    (st, dir)
}

#[test]
fn empty_query_groups_recent_suggested_and_the_rest() {
    let (mut st, dir) = behind_with_changes("palette-groups");
    st.palette_recent.record(C::Refresh.recall_key().unwrap());
    let l = listed(&st, "");
    let all = labels(&st);
    assert_eq!(l.groups[..2], [(Group::Recent, 0), (Group::Suggested, 1)]);
    let (g, rest) = l.groups[2];
    assert_eq!((g, rest), (Group::All, 5), "{all:?}");
    assert_eq!(all[0], "Refresh");
    assert_eq!(all[1..5], ["Pull", "Stage all files", "Create branch at HEAD…", "Fetch all remotes"]);
    assert_eq!(all.iter().filter(|l| *l == "Refresh").count(), 1, "a recent command shows once");
    assert_eq!(all.iter().filter(|l| *l == "Pull").count(), 1, "a suggestion shows once");
    assert!(all[rest..].contains(&"Push".to_owned()), "not ahead: Push stays with the rest");
    assert_eq!(all.len(), crate::state::palette::entries(&st).len());

    // Ahead too: Push joins the suggestions after Pull.
    st.snap.as_mut().unwrap().locals.iter_mut().find(|b| b.is_head).unwrap().ahead = 1;
    assert_eq!(labels(&st)[1..6], ["Pull", "Push", "Stage all files", "Create branch at HEAD…", "Fetch all remotes"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stopped_rebase_suggests_continue_first() {
    let (mut st, dir) = behind_with_changes("palette-groups-rebase");
    st.snap.as_mut().unwrap().state = RepositoryState::RebaseMerge;
    let l = listed(&st, "");
    assert_eq!(l.groups[0], (Group::Suggested, 0));
    assert_eq!(labels(&st)[0], "Continue rebase");

    st.snap.as_mut().unwrap().unstaged = vec![FileEntry::new("a.txt", '!')];
    assert!(!labels(&st).contains(&"Continue rebase".to_owned()), "conflicts left: not offered at all");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_repository_suggests_opening_one() {
    let st = state();
    let l = listed(&st, "");
    assert_eq!(l.groups, [(Group::Suggested, 0), (Group::All, 1)]);
    assert_eq!(labels(&st)[0], "Open repository…");
}

#[test]
fn a_query_has_no_groups_and_keys_move_over_commands() {
    let (mut st, dir) = behind_with_changes("palette-groups-query");
    st.palette_recent.record(C::Refresh.recall_key().unwrap());
    let typed = listed(&st, "pu");
    assert!(typed.groups.is_empty());
    assert_eq!(typed.rows[0].entry.label, "Pull", "the ranking is unchanged");

    // Down from the first row selects the second command, not a heading.
    update(&mut st, Msg::Palette(PaletteMsg::TogglePalette), Instant::now());
    update(&mut st, Msg::Palette(PaletteMsg::PaletteMove(1)), Instant::now());
    assert_eq!(st.palette.as_ref().unwrap().selected, 1);
    assert_eq!(shown(&st, "")[1].entry.label, "Pull");
    let _ = std::fs::remove_dir_all(&dir);
}
