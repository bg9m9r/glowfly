//! Tests of the Chapters view's rows, expanding, syncing, keyboard moves and texts.

use super::moves::{current, parent_row, step};
use super::words::{authors_text, count_text, row_name, span_text};
use super::*;
use crate::state::list_keys::ListMove;
use crate::testing::{git, history_repo};
use glowfly_core::repo::{LoadOptions, load};
use glowfly_core::settings::Settings;

const DOWN: ListMove = ListMove::Step { down: true };
const UP: ListMove = ListMove::Step { down: false };

/// A snapshot of: a1 a2 a3 by ada (one run), b1 by bob tagged v1.0.0, "Land it (#4)" by bob, and
/// a merge of side (one commit) into main.
fn snapshot(name: &str) -> Snapshot {
    let dir = history_repo(
        name,
        &[("ada", 1000, "a1"), ("ada", 1060, "a2"), ("ada", 1120, "a3"), ("bob", 1200, "b1"), ("bob", 1300, "Land it (#4)")],
    );
    git(&dir, &["tag", "v1.0.0", "HEAD~1"]);
    git(&dir, &["checkout", "-q", "-b", "side", "HEAD~2"]);
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "s1"]);
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["merge", "-q", "--no-ff", "side", "-m", "Merge branch 'side'"]);
    load(&dir, &LoadOptions::all(100)).unwrap()
}

fn chapters_view(snap: &Snapshot) -> ChaptersView {
    let mut v = ChaptersView { mode: HistoryMode::Chapters, ..Default::default() };
    v.sync(snap);
    v
}

fn id_of(snap: &Snapshot, summary: &str) -> Oid {
    snap.commits.iter().find(|c| &*c.summary == summary).unwrap().oid
}

#[test]
fn collapsed_rows_have_a_release_row_only_before_a_tagged_chapter() {
    let snap = snapshot("chapters-state-rows");
    let v = chapters_view(&snap);
    let list = v.list.as_ref().unwrap();
    let titles: Vec<&str> = list.list.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles, ["side", "Land it (#4)", "b1", "a1"]);
    assert_eq!(v.rows, [Row::Chapter(0), Row::Chapter(1), Row::Release(2), Row::Chapter(2), Row::Chapter(3)]);
}

#[test]
fn expanding_shows_a_runs_commits_and_a_merges_brought_in_commits() {
    let snap = snapshot("chapters-state-expand");
    let mut v = chapters_view(&snap);
    let list = v.list.clone().unwrap();
    let merge = list.list[0].id;
    let run = list.list[3].id;
    assert!(v.toggle(run));
    assert!(v.toggle(merge));
    let commit_rows: Vec<Row> = v.rows.iter().copied().filter(|r| matches!(r, Row::Commit { .. })).collect();
    let s1 = snap.row_of[&id_of(&snap, "s1")] as u32;
    assert_eq!(commit_rows.len(), 4);
    assert_eq!(v.rows[1], Row::Commit { chapter: 0, row: s1 }, "the merge shows what it brought in, not itself");
    assert_eq!(v.rows[0], Row::Chapter(0));
    assert!(v.toggle(run));
    assert!(!v.rows.iter().any(|r| matches!(r, Row::Commit { chapter: 3, .. })));
}

#[test]
fn toggle_refuses_a_chapter_of_one_and_an_unknown_id() {
    let snap = snapshot("chapters-state-refuse");
    let mut v = chapters_view(&snap);
    let before = v.rows.clone();
    assert!(!v.toggle(id_of(&snap, "Land it (#4)")));
    assert!(!v.toggle(id_of(&snap, "s1")));
    assert!(!v.toggle(Oid::from_bytes(&[7; 20]).unwrap()));
    assert_eq!(v.rows, before);
    assert!(v.expanded.is_empty());
}

#[test]
fn sync_keeps_expanded_ids_that_survive_and_drops_the_others() {
    let snap = snapshot("chapters-state-sync");
    let mut v = chapters_view(&snap);
    let run = v.list.as_ref().unwrap().list[3].id;
    v.toggle(run);
    let gone = Oid::from_bytes(&[9; 20]).unwrap();
    v.expanded.insert(gone);
    let short = load(&snap.workdir, &LoadOptions::all(6)).unwrap();
    v.sync(&short);
    assert!(!v.expanded.contains(&gone));
    assert!(v.expanded.contains(&run), "the run is still there with its two newest commits");
    assert!(v.rows.iter().any(|r| matches!(r, Row::Commit { .. })));
}

#[test]
fn the_commits_view_drops_the_grouping_but_keeps_what_is_expanded() {
    let snap = snapshot("chapters-state-commits");
    let mut v = chapters_view(&snap);
    let run = v.list.as_ref().unwrap().list[3].id;
    v.toggle(run);
    v.mode = HistoryMode::Commits;
    v.sync(&snap);
    assert!(v.list.is_none() && v.rows.is_empty() && v.expanded.contains(&run));
}

#[test]
fn follow_takes_the_saved_choice_once_per_repository() {
    let snap = snapshot("chapters-state-follow");
    let mut settings = Settings::default();
    settings.set_history_mode(&snap.real_workdir, HistoryMode::Chapters);
    let mut v = ChaptersView::default();
    v.follow(&snap, &settings);
    assert_eq!(v.mode, HistoryMode::Chapters);
    v.mode = HistoryMode::Commits;
    v.follow(&snap, &settings);
    assert_eq!(v.mode, HistoryMode::Commits, "not read again for the same repository");
}

#[test]
fn moves_skip_release_rows_and_stop_at_the_ends() {
    let rows = [Row::Chapter(0), Row::Release(1), Row::Chapter(1), Row::Chapter(2)];
    assert_eq!(step(&rows, false, Some(0), DOWN), Some(2), "over the heading");
    assert_eq!(step(&rows, false, Some(2), UP), Some(0));
    assert_eq!(step(&rows, false, Some(0), UP), None);
    assert_eq!(step(&rows, false, Some(3), DOWN), None);
    assert_eq!(step(&rows, false, None, DOWN), Some(0));
    assert_eq!(step(&rows, false, Some(1), DOWN), Some(2), "from a heading");
    assert_eq!(step(&rows, false, Some(1), UP), Some(0));
    assert_eq!(step(&rows, false, Some(0), ListMove::Edge { end: true }), Some(3));
    assert_eq!(step(&rows, false, Some(3), ListMove::Edge { end: false }), Some(0));
    assert_eq!(step(&rows, false, Some(0), ListMove::Page { down: true, rows: 10 }), Some(3));
}

#[test]
fn moves_count_the_work_tree_row_first() {
    let rows = [Row::Chapter(0), Row::Chapter(1)];
    assert_eq!(step(&rows, true, None, ListMove::Edge { end: false }), Some(0));
    assert_eq!(step(&rows, true, Some(0), DOWN), Some(1));
    assert_eq!(step(&rows, true, Some(2), DOWN), None);
    assert_eq!(step(&rows, true, Some(1), UP), Some(0));
    assert_eq!(step(&[], true, None, DOWN), Some(0), "only the work tree row");
    assert_eq!(step(&[], false, None, DOWN), None);
}

#[test]
fn the_current_row_follows_the_selection() {
    let snap = snapshot("chapters-state-current");
    let mut v = chapters_view(&snap);
    let list = v.list.clone().unwrap();
    let run = list.list[3].id;
    v.toggle(run);
    // Rows: 0 merge, 1 squash, 2 release heading, 3 b1, 4 the run, then its three commits.
    let a2 = id_of(&snap, "a2");
    assert_eq!(current(&v, &snap, &Selection::Commit(a2), false, false), Some(6));
    assert_eq!(current(&v, &snap, &Selection::Commit(a2), false, true), Some(7), "the work tree row counts");
    assert_eq!(current(&v, &snap, &Selection::Commit(list.list[1].id), false, false), Some(1));
    assert_eq!(current(&v, &snap, &Selection::Wip, true, true), Some(0));
    assert_eq!(current(&v, &snap, &Selection::Wip, false, true), None, "not lit");
    let sel = selection_of(&list.list[3], &snap);
    assert!(matches!(sel, Selection::Commits(_)));
    assert!(is_selected(&list.list[3], &snap, &sel));
    assert_eq!(current(&v, &snap, &sel, false, false), Some(4));
    assert!(!is_selected(&list.list[3], &snap, &Selection::Commit(run)));
    assert_eq!(parent_row(&v.rows, false, 6), Some(4));
    assert_eq!(parent_row(&v.rows, true, 7), Some(5));
    assert_eq!(parent_row(&v.rows, false, 1), None);
}

#[test]
fn texts() {
    let snap = snapshot("chapters-state-words");
    let v = chapters_view(&snap);
    let list = v.list.as_ref().unwrap();
    let mut settings = Settings { language: "en".into(), ..Settings::default() };
    let t = settings.lang();
    let run = &list.list[3];
    assert_eq!(count_text(t, run), "3 commits");
    assert_eq!(count_text(t, &list.list[2]), "1 commit");
    assert_eq!(row_name(t, run), "a1, 3 commits");
    assert_eq!(authors_text(t, run), "ada");
    let mut many = run.clone();
    many.more_authors = 2;
    assert_eq!(authors_text(t, &many), "ada +2");
    many.partial = true;
    assert_eq!(count_text(t, &many), "3+ commits");
    settings.relative_dates = false;
    let one_day = Chapter { oldest: 1_700_000_000, newest: 1_700_000_000, ..run.clone() };
    let two_days = Chapter { oldest: 1_700_000_000, newest: 1_700_200_000, ..run.clone() };
    assert!(!span_text(&settings, &one_day, 1_800_000_000, 0).contains(" to "));
    assert!(span_text(&settings, &two_days, 1_800_000_000, 0).contains(" to "));
}
