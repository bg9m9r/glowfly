//! Lost commit snapshots: a States row nothing keeps any more (its Lost chip and preview
//! sentence), and the Lost commits tab with a dropped stash (Dark) or a plain commit (Light)
//! selected, empty, loading, failed and cut with fsck's problems, and with the discarded changes
//! Glowfly kept above them (a kept file selected); and what its buttons and the States segment
//! send. The lost commits are built by hand on the fixture's commits.

use super::snapshot_tests::{NOW, harness, remove_fixture, with_repo};
use super::snapshot_tests_journal::journal;
use crate::journal::{JournalMsg, LostMsg};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::journal::JournalView;
use crate::state::journal_lost::{JournalTab, LostPreviewState, LostView};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::journal::{CommitLine, LostCommit, LostCommits, LostKind, Preview};
use glowfly_core::settings::ThemeChoice;
use glowfly_core::undo::kept::KeptWhat;
use std::path::PathBuf;

fn commit(st: &AppState, summary: &str) -> Oid {
    st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == summary).unwrap_or_else(|| panic!("{summary}")).oid
}

/// The fixture's first-parent history from `from`, as the preview reads it.
fn history(st: &AppState, from: Oid) -> Vec<CommitLine> {
    let snap = st.snap.as_ref().unwrap();
    let mut out = Vec::new();
    let mut next = Some(from);
    while let Some(o) = next.take() {
        let Some(c) = snap.commits.iter().find(|c| c.oid == o) else { break };
        out.push(CommitLine { oid: c.oid, summary: c.summary.to_string(), author: c.author.name().to_owned(), time: c.time });
        next = c.parents.first().copied();
    }
    out.truncate(8);
    out
}

fn sent(h: &egui_kittest::Harness<'_, (AppState, Vec<Msg>)>, want: fn(&JournalMsg) -> bool) -> bool {
    h.state().1.iter().any(|m| matches!(m, Msg::Journal(j) if want(j)))
}

fn lost_commit(oid: Oid, kind: LostKind, summary: &str, author: &str, ago: i64) -> LostCommit {
    LostCommit { oid, kind, summary: summary.into(), message: format!("{summary}\n"), author: author.into(), time: NOW - ago }
}

/// The Lost tab showing `result`.
fn on_lost(name: &str, theme: ThemeChoice, result: Option<Result<LostCommits, String>>) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let lost = LostView { generation: 1, loading: result.is_none(), result, ..Default::default() };
    st.session.journal.view = Some(JournalView { tab: JournalTab::Lost, lost: Some(lost), ..Default::default() });
    (st, dir)
}

/// A dropped stash on "Fix build on Windows" and two plain lost commits, `selected` (0: the
/// stash) with its history loaded.
fn three(name: &str, theme: ThemeChoice, selected: usize) -> (AppState, PathBuf) {
    let (st, dir) = on_lost(name, theme, None);
    let (stash, base) = (commit(&st, "Search results page"), commit(&st, "Fix build on Windows"));
    let kind = LostKind::Stash { base, branch: Some("main".into()), untracked: false };
    let commits = vec![
        lost_commit(stash, kind, "On main: try the wider search box", "Test", 1_800),
        lost_commit(commit(&st, "Add login page"), LostKind::Commit, "Add login page", "Test", 7_200),
        lost_commit(commit(&st, "Add app skeleton"), LostKind::Commit, "Add app skeleton", "Test", 3 * 86_400),
    ];
    let c = &commits[selected];
    let start = if matches!(c.kind, LostKind::Stash { .. }) { base } else { c.oid };
    let preview = Preview { history: history(&st, start), leaving: None, arriving: None };
    let mut st = st;
    let lost = st.session.journal.view.as_mut().unwrap().lost.as_mut().unwrap();
    lost.loading = false;
    lost.selected = Some(c.oid);
    lost.preview = Some(LostPreviewState { oid: c.oid, generation: 1, result: Some(Ok(preview)) });
    lost.result = Some(Ok(LostCommits { commits, ..Default::default() }));
    (st, dir)
}

#[test]
fn journal_unreachable() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "journal-unreachable");
    journal(&mut st, 1);
    if let Some(Ok(t)) = &mut st.session.journal.view.as_mut().unwrap().timeline {
        t.rows[1].unreachable = true;
    }
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("journal_unreachable");
    assert!(h.query_by_label_contains("Git keeps it only while this reflog entry lasts").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn journal_lost_stash() {
    let (st, dir) = three("journal-lost-stash", ThemeChoice::Dark, 0);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("journal_lost_stash");
    assert!(h.query_by_label("Create branch here…").is_none(), "a stash offers no branch");
    h.get_by_label("Apply stash").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, JournalMsg::Lost(LostMsg::Apply(_)))));
    h.get_by_label("Put back in stash list").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, JournalMsg::Lost(LostMsg::Store(_)))));
    h.get_by_label("States").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, JournalMsg::Tab(JournalTab::States))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn journal_lost_commit_light() {
    let (st, dir) = three("journal-lost-commit-light", ThemeChoice::Light, 1);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("journal_lost_commit_light");
    h.get_by_label("Create branch here…").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, JournalMsg::Lost(LostMsg::CreateBranch(_)))));
    h.get_by_label("Restore…").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, JournalMsg::Lost(LostMsg::Restore(_)))), "Restore is enabled");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn journal_lost_empty() {
    let (st, dir) = on_lost("journal-lost-empty", ThemeChoice::Dark, Some(Ok(LostCommits::default())));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("journal_lost_empty");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn journal_lost_loading() {
    let (st, dir) = on_lost("journal-lost-loading", ThemeChoice::Dark, None);
    let mut h = harness(st, true);
    // The spinner keeps repainting.
    h.run_steps(4);
    h.check_snapshot("journal_lost_loading");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn journal_lost_failed() {
    let (st, dir) = on_lost("journal-lost-failed", ThemeChoice::Dark, Some(Err("git fsck took too long".into())));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("journal_lost_failed");
    h.get_by_label("Try again").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, JournalMsg::Refresh)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn journal_lost_capped() {
    let (mut st, dir) = three("journal-lost-capped", ThemeChoice::Dark, 1);
    let lost = st.session.journal.view.as_mut().unwrap().lost.as_mut().unwrap();
    if let Some(Ok(l)) = &mut lost.result {
        l.capped = true;
        l.incomplete = true;
    }
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("journal_lost_capped");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn journal_lost_discarded() {
    let (mut st, dir) = three("journal-lost-discarded", ThemeChoice::Dark, 1);
    let kept = |n: u8, path: &str, what, ago| {
        let kind = LostKind::Discarded { path: path.into(), blob: Oid::from_bytes(&[n + 100; 20]).unwrap(), what };
        LostCommit {
            oid: Oid::from_bytes(&[n; 20]).unwrap(),
            kind,
            summary: path.into(),
            message: path.into(),
            author: String::new(),
            time: NOW - ago,
        }
    };
    let lost = st.session.journal.view.as_mut().unwrap().lost.as_mut().unwrap();
    let Some(Ok(l)) = &mut lost.result else { unreachable!() };
    // The stash moves up: its record keeps it, so it is listed with the discarded changes.
    let stash = l.commits.remove(0);
    let first = kept(1, "src/search/results.rs", KeptWhat::Hunk, 120);
    l.discarded = vec![first.clone(), kept(2, "notes/todo.md", KeptWhat::Delete, 900), stash];
    lost.selected = Some(first.oid);
    lost.preview = None;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("journal_lost_discarded");
    h.get_by_label("Restore as a copy").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, JournalMsg::Lost(LostMsg::RestoreFile { copy: true, .. }))));
    h.get_by_label("Restore").click();
    h.run();
    assert!(sent(&h, |m| matches!(m, JournalMsg::Lost(LostMsg::RestoreFile { copy: false, .. }))));
    drop(h);
    remove_fixture(&dir);
}
