//! Headless tests for what a multi-selection spans (a pair, a line of history and its combined
//! comparison, or neither), Swap direction, select all, and the selection rules for the cursor
//! and the working directory.

use super::selection_tests::{fixture, refresh, up};
use super::tests::drive;
use crate::effect::Effect;
use crate::effect::SelectionEffect;
use crate::msg::{Msg, RepoMsg, SelectionMsg};
use crate::state::selection::{MultiSelection, extend_from, ranged, toggled};
use crate::state::selection_span::{Span, span_of};
use crate::state::selection_wip::{WORKDIR_NEEDS_ONE, WorkdirCompare, wip_toggled};
use crate::state::{AppState, Selection};
use git2::Oid;
use glowfly_core::repo::{self, Comparison, DiffTarget, FileDiff, FileEntry};

fn multi(st: &AppState) -> &MultiSelection {
    match &st.session.selection {
        Selection::Commits(m) => m,
        other => panic!("expected a multi-selection, got {other:?}"),
    }
}

fn compares(fx: &[Effect]) -> Vec<(Oid, Oid)> {
    fx.iter()
        .filter_map(|e| if let Effect::Selection(SelectionEffect::Compare { from, to, .. }) = e { Some((*from, *to)) } else { None })
        .collect()
}

fn select(st: &mut AppState, oid: Oid) {
    up(st, Msg::Repo(RepoMsg::SelectCommit(oid)));
}

#[test]
fn a_line_of_commits_combines_and_others_explain() {
    let (mut st, f) = fixture("span-kinds");
    select(&mut st, f.c(5));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(4))));
    let fx = up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(3))));
    assert_eq!(multi(&st).span, Span::Range { base: f.c(2), newest: f.c(5) }, "c4's first parent is c3");
    assert_eq!(compares(&fx), [(f.c(2), f.c(5))]);

    // A range across the side branch's row isn't one line.
    select(&mut st, f.c(5));
    let fx = up(&mut st, Msg::Selection(SelectionMsg::SelectCommitRange(f.c(3))));
    assert_eq!(multi(&st).oids, [f.c(5), f.c(4), f.side, f.c(3)]);
    assert_eq!(multi(&st).span, Span::Gaps { gap: None }, "every first parent is selected, but side branches off");
    assert!(compares(&fx).is_empty() && multi(&st).compare_ends().is_none());

    // c4's parent c3 is left out: c4 is named.
    select(&mut st, f.c(5));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(4))));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(2))));
    assert_eq!(multi(&st).span, Span::Gaps { gap: Some(f.c(4)) });
    assert_eq!(multi(&st).gap(), Some(f.c(4)));

    // From the first commit, the changes run from the empty tree.
    select(&mut st, f.c(3));
    let fx = up(&mut st, Msg::Selection(SelectionMsg::SelectCommitRange(f.c(1))));
    assert_eq!(multi(&st).span, Span::Range { base: repo::empty_tree(), newest: f.c(3) });
    assert_eq!(compares(&fx), [(repo::empty_tree(), f.c(3))]);

    // Two commits are a pair, adjacent or not.
    select(&mut st, f.c(5));
    let fx = up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(2))));
    assert_eq!(multi(&st).span, Span::Pair { older: f.c(2), newer: f.c(5) });
    assert_eq!(compares(&fx), [(f.c(2), f.c(5))]);
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn a_root_commit_is_never_the_gap() {
    let (mut st, f) = fixture("span-root-gap");
    let snap = st.snap.as_mut().unwrap();
    let oids = [f.c(5), f.c(4), f.c(3), f.c(1)];
    assert_eq!(span_of(&oids, snap), Span::Gaps { gap: Some(f.c(3)) }, "c3's parent c2 isn't selected");
    // With c3 a root (as in a repository with two histories), nothing above c1 has a gap.
    let row = snap.row_of[&f.c(3)];
    std::sync::Arc::get_mut(&mut snap.commits).unwrap()[row].parents.clear();
    assert_eq!(span_of(&oids, snap), Span::Gaps { gap: None });
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn a_line_from_the_first_commit_compares_with_an_empty_tree() {
    let (mut st, f) = fixture("span-root");
    select(&mut st, f.c(3));
    drive(&mut st, Msg::Selection(SelectionMsg::SelectCommitRange(f.c(1))));
    let c = multi(&st).changes.clone().expect("loaded").expect("no error");
    let files: Vec<_> = c.files.iter().map(|f| (f.path.as_str(), f.status)).collect();
    assert_eq!(files, [("f.txt", 'A'), ("g.txt", 'A')]);

    let target = DiffTarget::Between(repo::empty_tree(), f.c(3), "f.txt".into());
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(target.clone())));
    let (shown, diff) = st.session.diff.view.clone().expect("the diff opens");
    assert_eq!(shown, target);
    assert!(!diff.hunks.is_empty() && diff.hunks.iter().flat_map(|h| &h.lines).all(|l| l.origin == '+'));

    let fx = up(&mut st, Msg::Selection(SelectionMsg::SwapCompare));
    assert_eq!(multi(&st).compare_ends(), Some((f.c(3), repo::empty_tree())));
    assert_eq!(compares(&fx), [(f.c(3), repo::empty_tree())]);
    assert!(st.session.diff.view.is_none());
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn a_line_from_a_shallow_boundary_has_no_combined_diff() {
    let (mut st, f) = fixture("span-shallow");
    st.snap.as_mut().unwrap().shallow_roots = vec![f.c(1)];
    select(&mut st, f.c(3));
    let fx = up(&mut st, Msg::Selection(SelectionMsg::SelectCommitRange(f.c(1))));
    assert_eq!(multi(&st).span, Span::ShallowEnd);
    let want = [f.c(3), f.c(2), f.c(1)];
    assert!(
        matches!(&fx[..], [Effect::Selection(SelectionEffect::Stacked(r))] if r.oids == want),
        "each commit's own changes instead: {fx:?}"
    );
    assert_eq!(multi(&st).compare_ends(), None);
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn swap_direction_reverses_the_comparison() {
    let (mut st, f) = fixture("span-swap");
    select(&mut st, f.c(5));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(4))));
    let c = Comparison { files: vec![FileEntry::new("f.txt", 'M')], absent: Default::default() };
    up(&mut st, Msg::Selection(SelectionMsg::CompareLoaded { from: f.c(4), to: f.c(5), result: Ok(c.clone()) }));
    let empty = FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
    st.session.diff.view = Some((DiffTarget::Between(f.c(4), f.c(5), "f.txt".into()), empty.into()));

    let fx = up(&mut st, Msg::Selection(SelectionMsg::SwapCompare));
    assert_eq!(compares(&fx), [(f.c(5), f.c(4))]);
    assert!(multi(&st).swapped && multi(&st).changes.is_none());
    assert!(st.session.diff.view.is_none(), "the old direction's diff closes");
    up(&mut st, Msg::Selection(SelectionMsg::CompareLoaded { from: f.c(4), to: f.c(5), result: Ok(c.clone()) }));
    assert!(multi(&st).changes.is_none(), "a late result of the old direction is dropped");
    up(&mut st, Msg::Selection(SelectionMsg::CompareLoaded { from: f.c(5), to: f.c(4), result: Ok(c.clone()) }));
    assert_eq!(multi(&st).changes, Some(Ok(c.clone())));

    // A refresh keeps the direction and the list while the ends stay.
    let fx = refresh(&mut st);
    assert!(multi(&st).swapped && compares(&fx).is_empty());
    assert_eq!(multi(&st).compare_ends(), Some((f.c(5), f.c(4))));
    assert_eq!(multi(&st).changes, Some(Ok(c)));
    // A new selection starts unswapped.
    let fx = up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(3))));
    assert!(!multi(&st).swapped);
    assert_eq!(compares(&fx), [(f.c(2), f.c(5))]);
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn select_all_takes_every_shown_commit() {
    let (mut st, f) = fixture("span-select-all");
    select(&mut st, f.c(4));
    up(&mut st, Msg::Selection(SelectionMsg::SelectAll));
    assert_eq!(multi(&st).oids, [f.c(5), f.c(4), f.side, f.c(3), f.c(2), f.c(1)]);
    assert_eq!(multi(&st).anchor, f.c(4), "the commit in focus stays the anchor");
    // While the search filters, only the matches.
    st.view.show_search = true;
    st.session.search.query = glowfly_core::commit_query::CommitQuery::parse("c");
    st.session.search.matches = vec![0, 1, 3, 4, 5];
    st.session.search.filter = true;
    select(&mut st, f.c(4));
    up(&mut st, Msg::Selection(SelectionMsg::SelectAll));
    assert_eq!(multi(&st).oids, [f.c(5), f.c(4), f.c(3), f.c(2), f.c(1)]);
    let _ = std::fs::remove_dir_all(&f.dir);

    let dir = super::tests::scratch_repo("span-select-all-one");
    let mut st = super::tests::state();
    super::tests::drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    up(&mut st, Msg::Selection(SelectionMsg::SelectAll));
    let head = st.snap.as_ref().and_then(|s| s.head_oid).unwrap();
    assert_eq!(st.session.selection, Selection::Commit(head), "one shown commit");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cursor_anchor_and_working_directory_rules() {
    let (st, f) = fixture("span-rules");
    let snap = st.snap.as_ref().unwrap();
    let Selection::Commits(m) = ranged(&Selection::Commit(f.c(5)), f.c(3), snap, None) else { panic!() };
    assert_eq!((m.anchor, m.cursor), (f.c(5), f.c(3)), "Shift moves the cursor and keeps the anchor");
    assert_eq!(extend_from(&Selection::Commits(m.clone())), Some(f.c(3)));
    assert!(m.oids.iter().all(|&o| m.contains(o)) && m.members.len() == m.oids.len() && !m.contains(f.c(1)));
    let Selection::Commits(t) = toggled(&Selection::Commits(m), f.c(1), snap) else { panic!() };
    assert_eq!((t.anchor, t.cursor), (f.c(1), f.c(1)), "Ctrl sets both");

    let vs = Selection::VsWorkdir(WorkdirCompare::new(f.c(3)));
    assert_eq!(toggled(&Selection::Wip, f.c(3), snap), vs);
    assert_eq!(toggled(&vs, f.c(3), snap), Selection::Wip);
    let Selection::Commits(two) = toggled(&vs, f.c(5), snap) else { panic!() };
    assert_eq!((two.oids.clone(), two.anchor), (vec![f.c(5), f.c(3)], f.c(5)));
    let Selection::Commits(r) = ranged(&vs, f.c(2), snap, None) else { panic!() };
    assert_eq!((r.anchor, r.oids.len()), (f.c(3), 2), "Shift ranges from the compared commit");
    assert_eq!(extend_from(&vs), Some(f.c(3)));
    assert_eq!(extend_from(&Selection::Wip), None);

    assert_eq!(wip_toggled(&Selection::Commit(f.c(3))), Ok(vs.clone()));
    assert_eq!(wip_toggled(&vs), Ok(Selection::Commit(f.c(3))));
    assert_eq!(wip_toggled(&Selection::Wip), Ok(Selection::Wip));
    assert_eq!(wip_toggled(&Selection::None), Ok(Selection::Wip));
    assert_eq!(wip_toggled(&Selection::Commits(two.clone())), Err(WORKDIR_NEEDS_ONE));

    let mut swapped = two.clone();
    swapped.swapped = true;
    assert_eq!(two.compare_ends(), Some((f.c(3), f.c(5))));
    assert_eq!(swapped.compare_ends(), Some((f.c(5), f.c(3))));
    let _ = std::fs::remove_dir_all(&f.dir);
}
