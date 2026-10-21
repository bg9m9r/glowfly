//! Operation journal snapshots: the state list with a selected row's preview (Dark and Light),
//! Restore disabled on the current state, the empty, loading and failed journal, the stacked
//! layout of a narrow window, the Restore confirmation and the German view; and what its rows,
//! buttons and Esc send. The rows are built by hand on the fixture's commits so the image never
//! depends on when the fixture's reflog was written.

use super::snapshot_tests::{NOW, harness, harness_sized, remove_fixture, with_repo};
use crate::journal::JournalMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::journal::{JournalView, PreviewState};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, Key};
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::journal::{CommitLine, Count, Event, Note, NotedRef, Op, Preview, Row, Subject, Timeline};
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

/// The fixture's commit whose summary is `summary`.
fn commit(st: &AppState, summary: &str) -> Oid {
    st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == summary).unwrap_or_else(|| panic!("{summary}")).oid
}

fn short(o: Oid) -> String {
    o.to_string()[..7].to_owned()
}

fn row(subject: Subject, oid: Oid, ago: i64, event: Event, message: &str, summary: Option<&str>) -> Row {
    Row {
        subject,
        oid,
        prev: None,
        at: NOW - ago,
        event,
        message: message.into(),
        note: None,
        summary: summary.map(Into::into),
        current: false,
        unreachable: false,
    }
}

fn note(op: Op, branch: &str, oid: Oid) -> Note {
    Note { v: 1, at: NOW, op, refs: vec![NotedRef { branch: Some(branch.into()), oid: oid.to_string() }] }
}

/// A journal on the fixture: main reset back from the search branch's tip (the selected,
/// noted row), main's commits, a HEAD checkout, a deleted branch and a commit that is gone.
pub(super) fn journal(st: &mut AppState, selected: usize) {
    let main = |s: &str| Subject::Branch(s.into());
    let (merge, search, fix, skeleton) = (
        commit(st, "Merge branch 'login-page'"),
        commit(st, "Search results page"),
        commit(st, "Fix build on Windows"),
        commit(st, "Add app skeleton"),
    );
    let login = commit(st, "Add login page");
    let reset = format!("reset: moving to {}", short(merge));
    let mut now_row = row(main("main"), merge, 600, Event::Reset { to: short(merge) }, &reset, Some("Merge branch 'login-page'"));
    now_row.current = true;
    let mut before = row(
        main("main"),
        search,
        3_600,
        Event::Merge { from: "search".into(), fast_forward: true },
        "merge search: Fast-forward",
        Some("Search results page"),
    );
    before.note = Some(note(Op::Reset { mode: "hard".into(), to: short(merge) }, "main", search));
    let checkout = row(
        Subject::Head,
        login,
        7_200,
        Event::Checkout { from: "main".into(), to: "login-page".into() },
        "checkout: moving from main to login-page",
        Some("Add login page"),
    );
    let mut deleted = row(main("old-ui"), skeleton, 86_400, Event::Other, "", Some("Add app skeleton"));
    deleted.note = Some(note(Op::DeleteBranch { name: "old-ui".into() }, "old-ui", skeleton));
    let gone = Oid::from_str("0badc0de0badc0de0badc0de0badc0de0badc0de").unwrap();
    let lost = row(main("login-page"), gone, 2 * 86_400, Event::Commit { subject: "Experiment".into() }, "commit: Experiment", None);
    let fixed = row(
        main("main"),
        fix,
        3 * 86_400,
        Event::Commit { subject: "Fix build on Windows".into() },
        "commit: Fix build on Windows",
        Some("Fix build on Windows"),
    );
    let rows = vec![now_row, before, checkout, deleted, lost, fixed];
    let key = rows[selected].key();
    let snap = st.snap.as_ref().unwrap();
    let from = rows[selected].oid;
    let history: Vec<CommitLine> = [search, merge, fix, skeleton]
        .iter()
        .skip_while(|o| **o != from)
        .map(|o| {
            let c = snap.commits.iter().find(|c| c.oid == *o).unwrap();
            CommitLine { oid: c.oid, summary: c.summary.clone(), author: c.author.clone(), time: c.time }
        })
        .collect();
    let preview = Preview { history, leaving: Some(Count { n: 0, more: false }), arriving: Some(Count { n: 1, more: false }) };
    let timeline = Timeline { rows, capped: false, head: None, tips: Vec::new() };
    st.journal = Some(JournalView {
        generation: 1,
        loading: false,
        timeline: Some(Ok(timeline)),
        selected: Some(key.clone()),
        preview: Some(PreviewState { key, generation: 1, result: Some(Ok(preview)) }),
        ..Default::default()
    });
}

fn opened(theme: ThemeChoice, name: &str, selected: usize) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    journal(&mut st, selected);
    (st, dir)
}

fn sent(h: &egui_kittest::Harness<'_, (AppState, Vec<Msg>)>, want: fn(&JournalMsg) -> bool) -> bool {
    h.state().1.iter().any(|m| matches!(m, Msg::Journal(j) if want(j)))
}

#[test]
fn journal_list_preview() {
    let (st, dir) = opened(ThemeChoice::Dark, "journal-list", 1);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("journal_list_preview");
    h.get_by_label("Restore…").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, JournalMsg::Restore(_))), "Restore is enabled");
    h.get_by_label("Create branch here…").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, JournalMsg::CreateBranch(_))));
    h.get_by_label("Range-diff with now").click();
    h.run();
    let (search, merge) = (commit(&h.state().0, "Search results page"), commit(&h.state().0, "Merge branch 'login-page'"));
    let compare = h.state().1.iter().any(|m| {
        matches!(m, Msg::RangeDiff(crate::range_diff::RangeDiffMsg::Open { old, new, label: Some(l), at: Some(_) })
            if *old == search && *new == merge && l == "main")
    });
    assert!(compare, "Range-diff with now compares the state with main now");
    h.get_by_label("Read the journal again").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, JournalMsg::Refresh)));
    h.get_by_label("Close").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, JournalMsg::Close)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn esc_goes_back_to_the_graph() {
    let (st, dir) = opened(ThemeChoice::Dark, "journal-esc", 1);
    let mut h = super::snapshot_tests_esc::app_harness(st);
    h.run();
    h.press_key(Key::Escape);
    h.run();
    assert!(sent(&h, |m| matches!(m, JournalMsg::Close)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn journal_list_preview_light() {
    let (st, dir) = opened(ThemeChoice::Light, "journal-list-light", 1);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("journal_list_preview_light");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn journal_restore_blocked() {
    let (st, dir) = opened(ThemeChoice::Dark, "journal-blocked", 0);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("journal_restore_blocked");
    h.get_by_label("Restore…").click();
    h.run();
    assert!(!sent(&h, |m| matches!(m, JournalMsg::Restore(_))), "disabled on the current state");
    drop(h);
    remove_fixture(&dir);
}

fn with_timeline(name: &str, timeline: Option<Result<Timeline, String>>) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    st.journal = Some(JournalView { generation: 1, loading: timeline.is_none(), timeline, ..Default::default() });
    (st, dir)
}

#[test]
fn journal_empty() {
    let (st, dir) = with_timeline("journal-empty", Some(Ok(Timeline::default())));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("journal_empty");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn journal_loading() {
    let (st, dir) = with_timeline("journal-loading", None);
    let mut h = harness(st, true);
    // The spinner keeps repainting.
    h.run_steps(4);
    h.check_snapshot("journal_loading");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn journal_failed() {
    let (st, dir) = with_timeline("journal-failed", Some(Err("could not read the reflog of refs/heads/main".into())));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("journal_failed");
    h.get_by_label("Try again").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, JournalMsg::Refresh)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn journal_narrow() {
    let (st, dir) = opened(ThemeChoice::Dark, "journal-narrow", 1);
    let mut h = harness_sized(st, true, egui::vec2(900.0, 820.0));
    h.run();
    h.check_snapshot("journal_narrow");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn journal_confirm() {
    let (mut st, dir) = opened(ThemeChoice::Dark, "journal-confirm", 1);
    let t = st.settings.lang();
    let view = st.journal.as_ref().unwrap();
    let r = view.selected_row().unwrap();
    let snap = st.snap.as_ref().unwrap();
    let plan = crate::state::journal_restore::plan(snap, r, false, t).unwrap();
    let dirty = crate::state::auto_stash::blocks(snap);
    let msg = crate::state::journal_restore::confirm(t, &plan, view.preview_of(&r.key()), dirty, snap.head_branch.as_deref());
    let Msg::Confirm { title, body, button, then } = msg else { panic!("a confirmation") };
    st.modal = Some(crate::state::Modal::Confirm { title, body, button, then });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("journal_confirm");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn journal_de() {
    let (mut st, dir) = opened(ThemeChoice::Dark, "journal-de", 1);
    st.settings.language = "de".into();
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("journal_de");
    for english in [
        "Operation journal",
        "Restore…",
        "Create branch here…",
        "History at this state",
        "Git's reflog entry",
        "Now",
        "States",
        "Lost commits",
    ] {
        assert!(h.query_by_label(english).is_none(), "{english} is translated");
    }
    drop(h);
    remove_fixture(&dir);
}
