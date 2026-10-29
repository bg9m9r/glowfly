//! Headless tests for the per-tab state that parks with its tab as one `RepoSession`: every view,
//! the open diff, the draft, the selection, undo, searches and view state come back after a
//! switch, and the generation counters (the repository reads' included) keep rising across tabs,
//! so a result from before a park, or for another tab or a closed one, never lands.

use super::issue_tests::{github, listed};
use super::tabs_tests::{cleanup, fake_tabs, two_repos};
use super::update;
use crate::conflict_check::{CheckRequest, ConflictCheckMsg};
use crate::effect::Effect;
use crate::msg::{CloseRequest, HistoryMsg, Msg, RepoMsg, TabsMsg};
use crate::state::blame::BlameView;
use crate::state::conflict::ConflictView;
use crate::state::diff_preview::DiffPreview;
use crate::state::file_folds::FileListId;
use crate::state::history::FileHistoryView;
use crate::state::issue::IssueView;
use crate::state::journal::JournalView;
use crate::state::lfs::LfsDiffNote;
use crate::state::range_diff::{Origin, RangeDiffView};
use crate::state::rebase::RebasePanel;
use crate::state::session::RepoSession;
use crate::state::{AppState, ForgeState, Selection};
use glowfly_core::conflict_check::{MergeTarget, Prediction, TargetKind};
use glowfly_core::lfs::pointer::{LfsChange, LfsSide};
use glowfly_core::range_diff::Ends;
use glowfly_core::repo::{DiffTarget, FileDiff};
use glowfly_core::undo::Action;
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn select(st: &mut AppState, i: usize) -> Vec<Effect> {
    run(st, Msg::Tabs(TabsMsg::SelectTab(i)))
}

fn open_history(st: &mut AppState, path: &str) -> Vec<Effect> {
    run(st, Msg::History(HistoryMsg::OpenFileHistory { path: path.into(), at: None }))
}

/// The generations of the history loads among `fx`.
fn history_loads(fx: &[Effect]) -> Vec<u64> {
    fx.iter().filter_map(|e| if let Effect::LoadFileHistory { generation, .. } = e { Some(*generation) } else { None }).collect()
}

/// A failed history load of `path` at `generation`, as the runner would report it.
fn stale_history(path: &str, generation: u64) -> Msg {
    Msg::History(HistoryMsg::FileHistoryLoaded { path: path.into(), generation, result: Err("stale".into()) })
}

/// The history view is still waiting: no result landed on it.
fn still_loading(st: &AppState) -> bool {
    st.session.history.view.as_ref().is_some_and(|h| h.loading && h.error.is_none())
}

/// Every counter of the session, in a fixed order (the pull request panel's four included).
fn counters(s: &RepoSession) -> [u64; 12] {
    let pr = &s.pull_request;
    [
        s.conflict.generation,
        s.history.generation,
        s.blame.generation,
        pr.generation,
        pr.page_gen,
        pr.checks_gen,
        pr.lines_gen,
        s.issue.generation,
        s.rebase.generation,
        s.journal.generation,
        s.range_diff.generation,
        s.diff.generation,
    ]
}

/// An empty diff of `a.txt`'s working copy.
fn a_diff() -> Option<(DiffTarget, std::sync::Arc<FileDiff>)> {
    let d = FileDiff { binary: false, hunks: vec![], highlight: None, intraline: Default::default(), whole: None };
    Some((DiffTarget::Unstaged("a.txt".into()), d.into()))
}

fn a_note() -> LfsDiffNote {
    let change = LfsChange { old: LfsSide::Absent, new: LfsSide::Regular { size: 4 }, new_in_workdir: true, filtered: false };
    LfsDiffNote { target: DiffTarget::Unstaged("a.txt".into()), change, show_pointer: true }
}

#[test]
fn a_history_result_from_before_the_switch_is_dropped() {
    let (mut st, a, b) = two_repos("tabs-session-stale");
    select(&mut st, 0);
    let fx = open_history(&mut st, "a.txt");
    let g = st.session.history.generation;
    assert_eq!(history_loads(&fx), vec![g], "the load carries the view's generation");

    select(&mut st, 1);
    let fx = select(&mut st, 0);
    let now = st.session.history.generation;
    assert!(now > g, "the counter moved past the parked load");
    assert_eq!(history_loads(&fx), vec![now], "the load that was in flight is asked for again");

    let lines = st.log.len();
    run(&mut st, stale_history("a.txt", g));
    assert!(still_loading(&st), "the old result is dropped");
    assert_eq!(st.log.len(), lines, "and nothing is logged for it");
    cleanup(&[&a, &b]);
}

#[test]
fn a_result_for_another_tab_never_matches() {
    let (mut st, a, b) = two_repos("tabs-session-other");
    select(&mut st, 0);
    open_history(&mut st, "a.txt");
    let ga = st.session.history.generation;

    select(&mut st, 1);
    assert!(st.session.history.view.is_none(), "B has no history open");
    assert!(st.session.history.generation > ga, "B's counter is past everything A handed out");
    let fx = open_history(&mut st, "a.txt");
    let gb = st.session.history.generation;
    assert!(gb > ga);
    assert_eq!(history_loads(&fx), vec![gb]);

    // A's load answers while B is active: it carries only a generation, which isn't B's.
    let lines = st.log.len();
    run(&mut st, stale_history("a.txt", ga));
    assert!(still_loading(&st), "B's view is untouched");
    assert_eq!(st.log.len(), lines);
    cleanup(&[&a, &b]);
}

#[test]
fn every_session_view_round_trips_a_switch() {
    let mut st = fake_tabs(&["a", "b"], 0);
    let s = &mut st.session;
    s.conflict.view = Some(ConflictView { saving: true, ..ConflictView::loading("c.txt".into()) });
    s.history.view = Some(FileHistoryView::loading("h.txt".into(), None));
    s.blame.view = Some(BlameView::loading("b.txt".into(), None));
    s.rebase.view = Some(RebasePanel::loading_remaining());
    s.journal.view = Some(JournalView::default());
    let origin = Origin { ends: Ends::LastRewrite { refname: "refs/heads/topic".into() }, label: Some("topic".into()), at: None };
    s.range_diff.view = Some(RangeDiffView::loading(origin, 0));
    s.pull_request.view = Some(crate::state::pull_request_actions::tests::view(None));
    let fs = ForgeState::new(Some(github()), "origin".into(), Some(github()));
    s.issue.view = IssueView::new(listed("12"), &fs);
    assert!(s.issue.view.is_some(), "the issue panel needs a tracker");
    s.gitflow_panel = true;
    s.diff_preview = Some(DiffPreview { target: DiffTarget::Unstaged("img.png".into()), generation: 0, sides: None });
    s.conflict_files.generation = 7;
    s.history.generation = 3;
    s.pull_request.page_gen = 5;
    s.diff.view = a_diff();
    s.diff.lfs = Some(a_note());
    s.draft.summary = "wip".into();
    s.selection = Selection::Wip;
    s.forge = Some(ForgeState::new(Some(github()), "origin".into(), Some(github())));
    s.forge.as_mut().unwrap().loading = 2;
    let before = counters(&st.session);

    select(&mut st, 1);
    let s = &st.session;
    assert!(s.conflict.view.is_none() && s.history.view.is_none() && s.blame.view.is_none());
    assert!(s.rebase.view.is_none() && s.journal.view.is_none() && s.range_diff.view.is_none());
    assert!(s.pull_request.view.is_none() && s.issue.view.is_none() && s.diff_preview.is_none());
    assert!(!s.gitflow_panel, "tab b doesn't show a's panels");
    assert!(s.diff.view.is_none() && s.diff.lfs.is_none() && s.draft.summary.is_empty(), "nor a's diff or draft");
    let parked = &st.tabs[0].parked.as_ref().expect("a is parked").session;
    assert!(parked.conflict.view.is_some() && parked.history.view.is_some() && parked.issue.view.is_some());
    assert!(parked.diff.view.is_some() && parked.diff.lfs.is_some() && parked.draft.summary == "wip");
    assert_eq!(parked.conflict_files.generation, 7);

    select(&mut st, 0);
    let s = &st.session;
    assert_eq!(s.conflict.view.as_ref().map(|c| (c.path.as_str(), c.saving)), Some(("c.txt", false)), "a save in flight isn't waited for");
    assert_eq!(s.history.view.as_ref().map(|h| h.path.as_str()), Some("h.txt"));
    assert_eq!(s.blame.view.as_ref().map(|b| b.path.as_str()), Some("b.txt"));
    assert!(s.rebase.view.is_some() && s.journal.view.is_some() && s.range_diff.view.is_some());
    assert_eq!(s.pull_request.view.as_ref().map(|v| v.pr.id), Some(12));
    assert_eq!(s.issue.view.as_ref().map(|v| v.key.as_str()), Some("12"));
    assert!(s.gitflow_panel);
    assert!(s.diff_preview.as_ref().is_some_and(|p| p.target.path() == "img.png"));
    assert!(s.conflict_files.generation >= 7);
    assert_eq!(s.diff.view.as_ref().map(|(t, _)| t.path()), Some("a.txt"), "the diff comes back");
    assert_eq!(s.diff.lfs, Some(a_note()), "with its note and Show pointer text choice");
    assert_eq!(s.draft.summary, "wip");
    assert_eq!(s.selection, Selection::None, "no selection without a loaded snapshot");
    assert_eq!(s.forge.as_ref().map(|f| f.loading), Some(0), "the forge replies in flight count as dropped");
    assert!(s.tab_view.parked_in.is_none(), "the diff's mode and layout are taken on return");
    for (k, (after, was)) in counters(s).into_iter().zip(before).enumerate() {
        assert!(after > was, "counter {k} rose: {was} -> {after}");
    }
}

#[test]
fn selection_undo_searches_and_view_round_trip() {
    let (mut st, a, b) = two_repos("tabs-session-view");
    st.session.selection = Selection::None; // b's
    select(&mut st, 0);
    run(&mut st, Msg::Repo(RepoMsg::SelectWip));
    assert_eq!(st.session.selection, Selection::Wip);
    st.session.undo.record(Action::DeleteBranch { name: "old".into(), oid: git2::Oid::zero(), upstream: None });
    st.session.search.filter = true;
    let v = &mut st.session.tab_view;
    (v.search, v.ref_filter, v.graph_scroll) = ("fix".into(), "main".into(), (120.0, 400.0));
    v.file_folds.toggle(FileListId::Unstaged, "src".into());

    select(&mut st, 1);
    let s = &st.session;
    assert_eq!(s.selection, Selection::None, "b has its own selection");
    assert!(s.undo.undo.is_empty() && !s.search.filter, "and its own undo history and search");
    let v = &s.tab_view;
    assert!(v.search.is_empty() && v.ref_filter.is_empty() && v.graph_scroll == (0.0, 0.0), "and its own view state");
    assert!(!v.file_folds.is_closed(FileListId::Unstaged, "src"));

    select(&mut st, 0);
    let s = &st.session;
    assert_eq!(s.selection, Selection::Wip, "a's selection is back");
    assert!(s.undo.undo.len() == 1 && s.search.filter, "with its undo history and search");
    let v = &s.tab_view;
    assert_eq!((v.search.as_str(), v.ref_filter.as_str(), v.graph_scroll), ("fix", "main", (120.0, 400.0)));
    assert!(v.file_folds.is_closed(FileListId::Unstaged, "src"));
    assert_eq!(st.view.scroll_offset, Some(120.0), "the graph scrolls back to where it was");
    cleanup(&[&a, &b]);
}

#[test]
fn closing_the_active_tab_keeps_counters_rising() {
    let mut st = fake_tabs(&["a", "b"], 0);
    st.session.blame.generation = 50;
    // Confirmed: closes without asking, whatever the tab holds.
    let id = st.tabs[0].id;
    run(&mut st, Msg::Tabs(TabsMsg::CloseConfirmed(CloseRequest::One(id))));
    assert_eq!(st.tabs.len(), 1, "a is closed");
    assert!(st.session.blame.generation > 50, "b's counter moved past a's, though b's parked session had 0");
}

#[test]
fn a_diff_result_from_before_the_switch_is_dropped() {
    let (mut st, a, b) = two_repos("tabs-session-diff");
    select(&mut st, 0);
    let target = DiffTarget::Unstaged("a.txt".into());
    let fx = run(&mut st, Msg::Repo(RepoMsg::OpenDiff(target.clone())));
    let g = fx.iter().find_map(|e| if let Effect::LoadDiff { generation, .. } = e { Some(*generation) } else { None });
    let g = g.expect("the diff is asked for");

    select(&mut st, 1);
    select(&mut st, 0);
    assert!(st.session.diff.generation > g, "the diff's counter moved past the parked request");

    let (lines, shown) = (st.log.len(), st.session.diff.view.is_some());
    run(
        &mut st,
        Msg::Repo(RepoMsg::DiffLoaded { generation: g, reload: false, result: Err("stale".into()), lfs: None, raw: None, target }),
    );
    assert_eq!(st.log.len(), lines, "the old result is dropped: nothing is logged for it");
    assert_eq!(st.session.diff.view.is_some(), shown, "and the diff view is untouched");
    cleanup(&[&a, &b]);
}

#[test]
fn hunk_ops_wait_for_the_reload_after_a_switch() {
    let mut st = fake_tabs(&["a", "b"], 0);
    st.session.diff.view = a_diff();
    st.session.diff.generation = 4;
    st.session.diff.shown_gen = 4;
    assert!(st.hunk_ops_gen().is_some());
    let shown = st.session.diff.shown_gen;

    select(&mut st, 1);
    select(&mut st, 0);
    assert!(st.session.diff.view.is_some(), "the diff is back");
    assert_eq!(st.hunk_ops_gen(), None, "its hunks wait for the reload");
    assert_eq!(st.session.diff.shown_gen, shown, "the last request answered is unchanged");
}

/// A made-up oid for the read tests.
fn oid(n: u8) -> git2::Oid {
    git2::Oid::from_bytes(&[n; 20]).unwrap()
}

/// A conflict check's inputs (the values don't matter here).
fn a_check() -> CheckRequest {
    CheckRequest {
        branch: "main".into(),
        ours: oid(7),
        target: MergeTarget { name: "origin/main".into(), oid: oid(8), kind: TargetKind::Upstream },
    }
}

/// Starts a signature check of `oid(3)`, a conflict check and a working-tree count on the active
/// tab's reads, by hand: their generations.
fn start_reads(st: &mut AppState) -> (u64, u64, u64) {
    let r = &mut st.session.reads;
    (r.signatures.start(oid(3)), r.conflict_checks.start().expect("none running"), r.line_counts.start_wip().expect("none running"))
}

/// Delivers the replies to `start_reads`'s requests, unscoped (as if for the active tab).
fn deliver_reads(st: &mut AppState, (sig, check, wip): (u64, u64, u64)) {
    let result = Err("stale".into());
    run(st, Msg::Repo(RepoMsg::SignatureChecked { oid: oid(3), generation: sig, result }));
    let result = Ok(Prediction::Clean);
    run(st, Msg::ConflictCheck(ConflictCheckMsg::Checked { request: a_check(), manual: false, generation: check, result }));
    run(st, Msg::Repo(RepoMsg::WipLineCountsLoaded { generation: wip, result: Ok(Box::default()) }));
}

/// None of `start_reads`'s replies landed.
fn no_reads_landed(st: &AppState) -> bool {
    let r = &st.session.reads;
    r.signatures.get(oid(3)).is_none() && r.conflict_checks.shown(Some(&a_check())).is_none() && r.line_counts.wip().is_none()
}

#[test]
fn stale_reads_after_a_round_trip_are_dropped() {
    let mut st = fake_tabs(&["a", "b"], 0);
    let asked = start_reads(&mut st);
    select(&mut st, 1);
    select(&mut st, 0);
    deliver_reads(&mut st, asked);
    let r = &st.session.reads;
    assert!(r.signatures.get(oid(3)).is_none(), "a signature check from before the switch is dropped");
    assert!(r.conflict_checks.shown(Some(&a_check())).is_none(), "and a conflict check");
    assert!(r.line_counts.wip().is_none(), "and a working-tree count");
}

#[test]
fn a_closed_tabs_reads_never_reach_the_next_tab() {
    let mut st = fake_tabs(&["a", "b"], 0);
    let asked = start_reads(&mut st);
    let id = st.tabs[0].id;
    run(&mut st, Msg::Tabs(TabsMsg::CloseConfirmed(CloseRequest::One(id))));
    assert_eq!(st.tabs.len(), 1, "a is closed");
    deliver_reads(&mut st, asked);
    assert!(no_reads_landed(&st), "b's reads are untouched by a's replies");
}

#[test]
fn a_stats_result_for_a_parked_tab_is_dropped_and_asked_again() {
    let (mut st, a, b) = two_repos("tabs-session-stats");
    select(&mut st, 0);
    let head = st.snap.as_ref().and_then(|s| s.head_oid).expect("a has a commit");
    st.session.reads.stats.clear();
    let asks = |fx: &[Effect]| fx.iter().any(|e| matches!(e, Effect::Stats { oids, .. } if oids == &[head]));
    let fx = run(&mut st, Msg::Repo(RepoMsg::WantStats(vec![head])));
    assert!(asks(&fx), "a's stats are asked for");

    select(&mut st, 1);
    let workdir = st.tabs[0].path.clone().expect("a has a path");
    run(&mut st, Msg::Scoped { workdir, msg: Box::new(Msg::Repo(RepoMsg::StatsLoaded(vec![(head, 9, 9, 9)]))) });
    assert_ne!(st.session.reads.stats.get(&head), Some(&(9, 9, 9)), "b doesn't get a's stats");
    let parked = &st.tabs[0].parked.as_ref().expect("a is parked").session;
    assert_ne!(parked.reads.stats.get(&head), Some(&(9, 9, 9)), "nor does a while parked");

    select(&mut st, 0);
    assert!(st.session.reads.stats_pending.is_empty(), "nothing is waiting on return");
    let fx = run(&mut st, Msg::Repo(RepoMsg::WantStats(vec![head])));
    assert!(asks(&fx), "so the stats are asked for again");
    cleanup(&[&a, &b]);
}
