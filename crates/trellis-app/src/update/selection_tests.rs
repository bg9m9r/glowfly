//! Headless tests for selecting several commits: Ctrl- and Shift-clicks, the two-commit
//! comparison, refreshes and tab switches. Cherry-picking is in `cherry_pick_tests`.

use super::tests::{drive, state};
use super::update;
use crate::effect::{Effect, LoadKind};
use crate::msg::DiffMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::SelectionMsg;
use crate::msg::TabsMsg;
use crate::runner;
use crate::state::conflict::ConflictView;
use crate::state::history::FileHistoryView;
use crate::state::lines::LineClick;
use crate::state::selection::{MultiSelection, PICK_BLOCKED_ON_HEAD, refreshed};
use crate::state::{AppState, Selection};
use git2::Oid;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::gitcli::{args, run, run_with_input};
use trellis_core::repo::{Comparison, DiffTarget, FileDiff, FileEntry};

/// Main's commits c1..c5 (oldest first) and `side`, a branch off c3 committed between c3 and c4.
pub(super) struct Fixture {
    pub dir: PathBuf,
    c: [Oid; 5],
    pub side: Oid,
}

impl Fixture {
    /// c1..c5 by number (1-based, as the commits are named).
    pub fn c(&self, n: usize) -> Oid {
        self.c[n - 1]
    }
}

pub(super) fn git(dir: &Path, a: &[&str]) -> String {
    let (ok, out) = run(dir, &args(a));
    assert!(ok, "git {a:?} failed: {out}");
    out
}

pub(super) fn head(dir: &Path) -> Oid {
    git2::Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn lines(edits: &[usize]) -> String {
    (1..=10).map(|i| if edits.contains(&i) { format!("line {i} edited\n") } else { format!("line {i}\n") }).collect()
}

/// Commits everything with a fixed date (day `day` of January), so the graph order is known.
pub(super) fn commit_on(dir: &Path, message: &str, day: u32) -> Oid {
    git(dir, &["add", "-A"]);
    let date = format!("2026-01-{day:02}T10:00:00+00:00");
    let env = [("GIT_AUTHOR_DATE", date.as_str()), ("GIT_COMMITTER_DATE", date.as_str())];
    let (ok, out) = run_with_input(dir, &args(&["commit", "-q", "-m", message]), "", &env);
    assert!(ok, "commit failed: {out}");
    head(dir)
}

pub(super) fn fixture(name: &str) -> (AppState, Fixture) {
    let dir = std::env::temp_dir().join(format!("trellis-app-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for a in [&["init", "-q", "-b", "main"][..], &["config", "user.name", "Test"], &["config", "user.email", "test@example.com"]] {
        git(&dir, a);
    }
    git(&dir, &["config", "commit.gpgsign", "false"]);
    let write = |f: &str, t: &str| std::fs::write(dir.join(f), t).unwrap();
    let mut c = [Oid::zero(); 5];
    write("f.txt", &lines(&[]));
    c[0] = commit_on(&dir, "c1", 1);
    write("f.txt", &lines(&[2]));
    c[1] = commit_on(&dir, "c2", 2);
    write("f.txt", &lines(&[2, 4]));
    write("g.txt", "g\n");
    c[2] = commit_on(&dir, "c3", 3);
    git(&dir, &["checkout", "-q", "-b", "side"]);
    write("s.txt", "side\n");
    let side = commit_on(&dir, "side", 4);
    git(&dir, &["checkout", "-q", "main"]);
    write("f.txt", &lines(&[2, 4, 6]));
    c[3] = commit_on(&dir, "c4", 5);
    write("f.txt", &lines(&[2, 4, 6, 8]));
    c[4] = commit_on(&dir, "c5", 6);

    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let rows: Vec<Oid> = st.snap.as_ref().unwrap().commits.iter().map(|c| c.oid).collect();
    assert_eq!(rows, [c[4], c[3], side, c[2], c[1], c[0]], "the side commit is drawn between c4 and c3");
    (st, Fixture { dir, c, side })
}

fn multi(st: &AppState) -> &MultiSelection {
    match &st.selection {
        Selection::Commits(m) => m,
        other => panic!("expected a multi-selection, got {other:?}"),
    }
}

pub(super) fn up(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn compares(fx: &[Effect]) -> Vec<(Oid, Oid)> {
    fx.iter().filter_map(|e| if let Effect::LoadCompare { from, to, .. } = e { Some((*from, *to)) } else { None }).collect()
}

fn loads_detail(fx: &[Effect], oid: Oid) -> bool {
    fx.iter().any(|e| matches!(e, Effect::LoadDetail { oid: o, .. } if *o == oid))
}

/// Refreshes the repository and returns the effects of applying the new snapshot.
pub(super) fn refresh(st: &mut AppState) -> Vec<Effect> {
    let fx = up(st, Msg::Repo(RepoMsg::Refresh));
    let load = fx.into_iter().find(|e| matches!(e, Effect::LoadRepo { kind: LoadKind::Refresh, .. })).expect("a refresh load");
    let loaded = runner::execute(load).unwrap();
    up(st, loaded)
}

#[test]
fn ctrl_click_builds_and_shrinks_a_selection() {
    let (mut st, f) = fixture("multi-ctrl");
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    let fx = up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(3))));
    let m = multi(&st);
    assert_eq!((m.oids.clone(), m.anchor), (vec![f.c(5), f.c(3)], f.c(3)));
    assert!(st.detail.is_none());
    assert_eq!(compares(&fx), [(f.c(3), f.c(5))]);
    assert_eq!(fx.len(), 1);

    let fx = up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(3))));
    assert_eq!(st.selection, Selection::Commit(f.c(5)));
    assert!(loads_detail(&fx, f.c(5)));
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn toggle_edge_cases() {
    let (mut st, f) = fixture("multi-toggle-edges");
    for start in [Selection::Wip, Selection::None] {
        st.selection = start;
        let fx = up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(3))));
        assert_eq!(st.selection, Selection::Commit(f.c(3)));
        assert!(loads_detail(&fx, f.c(3)));
    }
    // Toggling the only selected commit keeps it.
    assert!(up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(3)))).is_empty());
    assert_eq!(st.selection, Selection::Commit(f.c(3)));
    // An oid that is not a graph row does nothing, from one commit or several.
    let unknown = Oid::from_str("0123456789012345678901234567890123456789").unwrap();
    assert!(up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(unknown))).is_empty());
    assert!(up(&mut st, Msg::Selection(SelectionMsg::SelectCommitRange(unknown))).is_empty());
    assert_eq!(st.selection, Selection::Commit(f.c(3)));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(1))));
    let before = st.selection.clone();
    assert!(up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(unknown))).is_empty());
    assert_eq!(st.selection, before);
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn shift_click_selects_graph_rows() {
    let (mut st, f) = fixture("multi-shift");
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    let fx = up(&mut st, Msg::Selection(SelectionMsg::SelectCommitRange(f.c(2))));
    let m = multi(&st);
    assert_eq!(m.oids, [f.c(5), f.c(4), f.side, f.c(3), f.c(2)], "rows drawn in between are included");
    assert_eq!(m.anchor, f.c(5));
    assert!(compares(&fx).is_empty(), "no file list for more than two commits");

    let fx = up(&mut st, Msg::Selection(SelectionMsg::SelectCommitRange(f.c(4))));
    let m = multi(&st);
    assert_eq!((m.oids.clone(), m.anchor), (vec![f.c(5), f.c(4)], f.c(5)));
    assert_eq!(compares(&fx), [(f.c(4), f.c(5))]);

    let fx = up(&mut st, Msg::Selection(SelectionMsg::SelectCommitRange(f.c(5))));
    assert_eq!(st.selection, Selection::Commit(f.c(5)));
    assert!(loads_detail(&fx, f.c(5)));

    st.selection = Selection::Wip;
    up(&mut st, Msg::Selection(SelectionMsg::SelectCommitRange(f.c(3))));
    assert_eq!(st.selection, Selection::Commit(f.c(3)));
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn shift_click_while_filtering_selects_only_shown_rows() {
    let (mut st, f) = fixture("multi-shift-filtered");
    // Every commit but `side` (row 2) matches, and the graph shows only the matches.
    st.view.show_search = true;
    st.search.query = trellis_core::commit_query::CommitQuery::parse("c");
    st.search.matches = vec![0, 1, 3, 4, 5];
    st.search.filter = true;
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    up(&mut st, Msg::Selection(SelectionMsg::SelectCommitRange(f.c(2))));
    assert_eq!(multi(&st).oids, [f.c(5), f.c(4), f.c(3), f.c(2)], "the hidden side commit is left out");
    // With the bar hidden the graph shows every row again, and so does the range.
    st.view.show_search = false;
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    up(&mut st, Msg::Selection(SelectionMsg::SelectCommitRange(f.c(2))));
    assert_eq!(multi(&st).oids, [f.c(5), f.c(4), f.side, f.c(3), f.c(2)]);
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn plain_select_and_collapse() {
    let (mut st, f) = fixture("multi-collapse");
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(3))));
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(4))));
    assert_eq!(st.selection, Selection::Commit(f.c(4)));

    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(3))));
    let fx = up(&mut st, Msg::Selection(SelectionMsg::CollapseSelection));
    assert_eq!(st.selection, Selection::Commit(f.c(3)), "back to the anchor");
    assert!(loads_detail(&fx, f.c(3)));

    // With the anchor deselected, the newest selected commit.
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(3))));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(4))));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(4))));
    assert_eq!(multi(&st).anchor, f.c(4));
    up(&mut st, Msg::Selection(SelectionMsg::CollapseSelection));
    assert_eq!(st.selection, Selection::Commit(f.c(5)));
    assert!(up(&mut st, Msg::Selection(SelectionMsg::CollapseSelection)).is_empty());
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn selection_change_closes_views() {
    let (mut st, f) = fixture("multi-closes");
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    let empty = FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
    st.diff = Some((DiffTarget::Commit(f.c(5), "f.txt".into()), empty.into()));
    st.conflict = Some(ConflictView::loading("f.txt".into()));
    st.history = Some(FileHistoryView::loading("f.txt".into(), None));
    let gen_before = st.diff_gen;
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(3))));
    assert!(st.diff.is_none() && st.conflict.is_none() && st.history.is_none());
    assert!(st.diff_gen > gen_before, "a diff still loading must not reopen");
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn compare_results() {
    let (mut st, f) = fixture("multi-compare");
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(3))));
    let files = vec![FileEntry { path: "f.txt".into(), status: 'M' }];
    let c = Comparison { files, absent: ["f.txt".to_owned()].into() };
    up(&mut st, Msg::Selection(SelectionMsg::CompareLoaded { from: f.c(3), to: f.c(5), result: Ok(c.clone()) }));
    assert_eq!(multi(&st).changes, Some(Ok(c.clone())));
    // The same pair after a refresh keeps the list and what was missing from the working tree.
    match refreshed(multi(&st), st.snap.as_ref().unwrap()) {
        Selection::Commits(m) => assert_eq!(m.changes, Some(Ok(c.clone()))),
        other => panic!("{other:?}"),
    }
    // A result for another pair is stale.
    up(&mut st, Msg::Selection(SelectionMsg::CompareLoaded { from: f.c(2), to: f.c(5), result: Ok(Comparison::default()) }));
    assert_eq!(multi(&st).changes, Some(Ok(c)));
    up(&mut st, Msg::Selection(SelectionMsg::CompareLoaded { from: f.c(3), to: f.c(5), result: Err("bad tree".into()) }));
    assert_eq!(multi(&st).changes, Some(Err("bad tree".into())));
    assert!(st.log.iter().any(|l| !l.ok && l.text == "bad tree"));
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn two_commit_diff_end_to_end() {
    let (mut st, f) = fixture("multi-diff");
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    drive(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(3))));
    let c = multi(&st).changes.clone().unwrap().unwrap();
    assert_eq!(c.files, [FileEntry { path: "f.txt".into(), status: 'M' }]);
    assert!(c.absent.is_empty(), "f.txt is on disk");

    let target = DiffTarget::Between(f.c(3), f.c(5), "f.txt".into());
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(target.clone())));
    let (shown, diff) = st.diff.clone().expect("the diff opens");
    assert_eq!(shown, target);
    let changed: Vec<String> = diff
        .hunks
        .iter()
        .flat_map(|h| &h.lines)
        .filter(|l| matches!(l.origin, '+' | '-'))
        .map(|l| format!("{}{}", l.origin, l.text))
        .collect();
    assert_eq!(changed, ["-line 6", "+line 6 edited", "-line 8", "+line 8 edited"], "only the changes after the older commit");

    // A result from an older request is ignored.
    let stale =
        Msg::Repo(RepoMsg::DiffLoaded { target: target.clone(), reload: false, generation: st.diff_gen - 1, result: Err("old".into()) });
    up(&mut st, stale);
    assert_eq!(st.diff.as_ref().map(|(t, _)| t), Some(&target));

    // Lines of a diff between commits can't be selected.
    let line = diff.hunks[0].lines.iter().position(|l| l.origin == '+').unwrap();
    for how in [LineClick::Only, LineClick::Toggle, LineClick::Range] {
        up(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line, how }));
        assert!(st.view.diff_lines.is_empty());
    }
    up(&mut st, Msg::Diff(DiffMsg::ClearLineSelection));
    assert!(st.view.diff_lines.is_empty());
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn refresh_prunes_selection() {
    let (mut st, f) = fixture("multi-refresh");
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    drive(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(4))));
    assert_eq!(multi(&st).pick_blocked, Some(PICK_BLOCKED_ON_HEAD));
    // HEAD moves to a branch without c4 and c5: they can be picked now. The pair is the same,
    // so its file list is kept.
    git(&f.dir, &["checkout", "-q", "side"]);
    let fx = refresh(&mut st);
    assert_eq!(multi(&st).pick_blocked, None);
    assert!(multi(&st).changes.is_some() && compares(&fx).is_empty());

    git(&f.dir, &["checkout", "-q", "main"]);
    drive(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(3))));
    git(&f.dir, &["reset", "-q", "--hard", &f.c(4).to_string()]);
    let fx = refresh(&mut st);
    assert_eq!(multi(&st).oids, [f.c(4), f.c(3)]);
    assert_eq!(multi(&st).pick_blocked, Some(PICK_BLOCKED_ON_HEAD));
    assert_eq!(compares(&fx), [(f.c(3), f.c(4))]);

    git(&f.dir, &["reset", "-q", "--hard", &f.c(3).to_string()]);
    let fx = refresh(&mut st);
    assert_eq!(st.selection, Selection::Commit(f.c(3)));
    assert!(st.detail.is_none() && loads_detail(&fx, f.c(3)));
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn refresh_closes_the_diff_of_a_vanished_pair() {
    let (mut st, f) = fixture("multi-refresh-diff");
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    drive(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(3))));
    drive(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(4))));
    drive(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(5))));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Between(f.c(3), f.c(4), "f.txt".into()))));
    assert!(st.diff.is_some());
    git(&f.dir, &["reset", "-q", "--hard", &f.c(3).to_string()]);
    refresh(&mut st);
    assert_eq!(st.selection, Selection::Commit(f.c(3)));
    assert!(st.diff.is_none());
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn multi_selection_survives_tab_switch() {
    let (mut st, f) = fixture("multi-tabs");
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    // The file list is still loading when the tab is switched away: its result is dropped.
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(3))));
    let other = super::tests::scratch_repo("multi-tabs-other");
    drive(&mut st, Msg::Repo(RepoMsg::Open(other.clone())));
    let fx = up(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert_eq!(multi(&st).oids, [f.c(5), f.c(3)]);
    assert!(multi(&st).changes.is_none());
    let load = fx.into_iter().find(|e| matches!(e, Effect::LoadRepo { .. })).expect("the tab refreshes");
    let fx = up(&mut st, runner::execute(load).unwrap());
    assert_eq!(compares(&fx), [(f.c(3), f.c(5))], "the dropped file list is asked for again");
    let _ = std::fs::remove_dir_all(&f.dir);
    let _ = std::fs::remove_dir_all(&other);
}
