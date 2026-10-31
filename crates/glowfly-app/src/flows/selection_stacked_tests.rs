//! Headless tests for the stacked list of a multi-selection with no combined diff: loading each
//! commit's own changes, opening their diffs without leaving the selection, stepping through
//! them, and keeping or asking again across refreshes.

use super::selection_tests::{fixture, refresh, up};
use super::tests::drive;
use crate::effect::Effect;
use crate::effect::SelectionEffect;
use crate::msg::{Msg, RepoMsg, SelectionMsg};
use crate::state::diff_nav::{Landing, Step, next_file, step};
use crate::state::selection::{MultiSelection, STACK_LIMIT, stack_slice};
use crate::state::{AppState, Selection};
use git2::Oid;
use glowfly_core::repo::DiffTarget;

fn multi(st: &AppState) -> &MultiSelection {
    match &st.session.selection {
        Selection::Commits(m) => m,
        other => panic!("expected a multi-selection, got {other:?}"),
    }
}

fn stacks(fx: &[Effect]) -> Vec<Vec<Oid>> {
    fx.iter().filter_map(|e| if let Effect::Selection(SelectionEffect::Stacked(r)) = e { Some(r.oids.clone()) } else { None }).collect()
}

fn commit_file(oid: Oid) -> DiffTarget {
    DiffTarget::Commit(oid, "f.txt".into())
}

#[test]
fn a_selection_with_a_gap_lists_each_commits_changes() {
    let (mut st, f) = fixture("stacked-load");
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(4))));
    let fx = up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(2))));
    assert_eq!(stacks(&fx), [vec![f.c(5), f.c(4), f.c(2)]]);
    assert!(!fx.iter().any(|e| matches!(e, Effect::Selection(SelectionEffect::Compare { .. }))), "{fx:?}");

    super::tests::finish(&mut st, fx);
    let stack = multi(&st).stacked.clone().expect("loaded").expect("no error");
    assert_eq!(stack.iter().map(|c| c.oid).collect::<Vec<_>>(), [f.c(5), f.c(4), f.c(2)]);
    for c in &stack {
        assert_eq!(c.files.iter().map(|f| (f.path.as_str(), f.status)).collect::<Vec<_>>(), [("f.txt", 'M')]);
    }

    // A row opens that commit's diff; the selection stays.
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(commit_file(f.c(4)))));
    assert!(matches!(st.session.selection, Selection::Commits(_)));
    assert_eq!(st.session.diff.view.as_ref().map(|(t, _)| t.clone()), Some(commit_file(f.c(4))));

    // Stepping on goes to the next commit's file.
    assert_eq!(next_file(&st, &commit_file(f.c(4)), 1), Some(commit_file(f.c(2))));
    assert_eq!(next_file(&st, &commit_file(f.c(4)), -1), Some(commit_file(f.c(5))));
    assert_eq!(next_file(&st, &commit_file(f.c(2)), 1), None);
    st.session.tab_view.diff_top_row = 10_000;
    assert_eq!(step(&st, 1), Step::File(commit_file(f.c(2)), Landing::First), "past c4's last change");

    // Ctrl-clicking c4 away closes its diff.
    assert!(st.session.diff.view.is_some());
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(4))));
    assert!(st.session.diff.view.is_none());
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn stale_failed_and_refreshed_lists() {
    let (mut st, f) = fixture("stacked-refresh");
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(4))));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(2))));
    let want = vec![f.c(5), f.c(4), f.c(2)];

    // Another selection's list is dropped.
    up(&mut st, Msg::Selection(SelectionMsg::StackedLoaded { oids: vec![f.c(5), f.c(2)], result: Ok(Vec::new()) }));
    assert!(multi(&st).stacked.is_none());

    // A failure is shown; Retry asks again.
    up(&mut st, Msg::Selection(SelectionMsg::StackedLoaded { oids: want.clone(), result: Err("broken".into()) }));
    assert_eq!(multi(&st).stacked, Some(Err("broken".into())));
    let fx = up(&mut st, Msg::Selection(SelectionMsg::RetryCompare));
    assert!(multi(&st).stacked.is_none());
    assert_eq!(stacks(&fx), std::slice::from_ref(&want));
    super::tests::finish(&mut st, fx);
    let loaded = multi(&st).stacked.clone();
    assert!(matches!(&loaded, Some(Ok(s)) if s.len() == 3));

    // A refresh keeps the list of the same commits.
    let fx = refresh(&mut st);
    assert!(stacks(&fx).is_empty(), "{fx:?}");
    assert_eq!(multi(&st).stacked, loaded);
    // One dropped in flight (a tab switch) is asked for again.
    if let Selection::Commits(m) = &mut st.session.selection {
        m.stacked = None;
    }
    let fx = refresh(&mut st);
    assert_eq!(stacks(&fx), [want]);
    super::tests::finish(&mut st, fx);

    // A commit whose diff is open leaves the history: the diff closes.
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(commit_file(f.c(5)))));
    assert!(st.session.diff.view.is_some());
    super::selection_tests::git(&f.dir, &["reset", "-q", "--hard", &f.c(4).to_string()]);
    refresh(&mut st);
    assert_eq!(multi(&st).oids, [f.c(4), f.c(2)]);
    assert!(st.session.diff.view.is_none());
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn only_the_newest_commits_are_stacked() {
    let oids: Vec<Oid> = (1..=250u32).map(|n| Oid::from_str(&format!("{n:040x}")).unwrap()).collect();
    let shown = stack_slice(&oids);
    assert_eq!(shown.len(), STACK_LIMIT);
    assert_eq!(shown, &oids[..200]);
    assert_eq!(stack_slice(&oids[..3]), &oids[..3]);
}
