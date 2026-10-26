//! Headless tests for the views that park with their tab as one `RepoSession`: every view comes
//! back after a switch, and the generation counters keep rising across tabs, so a result from
//! before a park, or for another tab, never lands.

use super::issue_tests::{github, listed};
use super::tabs_tests::{cleanup, fake_tabs, two_repos};
use super::update;
use crate::effect::Effect;
use crate::msg::{CloseRequest, HistoryMsg, Msg, TabsMsg};
use crate::state::blame::BlameView;
use crate::state::conflict::ConflictView;
use crate::state::diff_preview::DiffPreview;
use crate::state::history::FileHistoryView;
use crate::state::issue::IssueView;
use crate::state::journal::JournalView;
use crate::state::range_diff::{Origin, RangeDiffView};
use crate::state::rebase::RebasePanel;
use crate::state::session::RepoSession;
use crate::state::{AppState, ForgeState};
use glowfly_core::range_diff::Ends;
use glowfly_core::repo::DiffTarget;
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
fn counters(s: &RepoSession) -> [u64; 11] {
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
    ]
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
    let before = counters(&st.session);

    select(&mut st, 1);
    let s = &st.session;
    assert!(s.conflict.view.is_none() && s.history.view.is_none() && s.blame.view.is_none());
    assert!(s.rebase.view.is_none() && s.journal.view.is_none() && s.range_diff.view.is_none());
    assert!(s.pull_request.view.is_none() && s.issue.view.is_none() && s.diff_preview.is_none());
    assert!(!s.gitflow_panel, "tab b doesn't show a's panels");
    let parked = &st.tabs[0].parked.as_ref().expect("a is parked").session;
    assert!(parked.conflict.view.is_some() && parked.history.view.is_some() && parked.issue.view.is_some());
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
    for (k, (after, was)) in counters(s).into_iter().zip(before).enumerate() {
        assert!(after > was, "counter {k} rose: {was} -> {after}");
    }
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
