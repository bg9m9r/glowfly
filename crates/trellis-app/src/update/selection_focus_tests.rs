//! Headless tests for the Go back to the focused commit key: after select all, a Shift-range or
//! Ctrl-clicks it selects only the commit in focus and says how many were let go.

use super::selection_tests::{fixture, up};
use crate::effect::Effect;
use crate::keymap::{BindingId, Chord, KeyName, Keymap, fold_ctrl};
use crate::msg::{Msg, RepoMsg, SelectionMsg};
use crate::state::journal::short;
use crate::state::selection_wip::WorkdirCompare;
use crate::state::{AppState, Selection};

fn last_toast(st: &AppState) -> Option<&str> {
    st.toasts.last().map(|t| t.text.as_str())
}

#[test]
fn back_from_select_all_to_the_focused_commit() {
    let (mut st, f) = fixture("focus-back");
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(3))));
    up(&mut st, Msg::Selection(SelectionMsg::SelectAll));
    assert!(matches!(&st.selection, Selection::Commits(m) if m.len() == 6));
    let fx = up(&mut st, Msg::Selection(SelectionMsg::BackToFocus));
    assert_eq!(st.selection, Selection::Commit(f.c(3)));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadDetail { oid, .. } if *oid == f.c(3))), "{fx:?}");
    assert!(st.view.scroll_to_row.is_some(), "the graph scrolls to it");
    let want = format!("Back to {}: 5 commits deselected.", short(f.c(3)));
    assert_eq!(last_toast(&st), Some(want.as_str()));
    assert!(st.toasts.last().unwrap().ok);

    // After a Shift-range, in German.
    st.settings.language = "de".into();
    up(&mut st, Msg::Selection(SelectionMsg::SelectCommitRange(f.c(2))));
    up(&mut st, Msg::Selection(SelectionMsg::BackToFocus));
    assert_eq!(st.selection, Selection::Commit(f.c(3)));
    let want = format!("Zurück zu {}: 1 Commit abgewählt.", short(f.c(3)));
    assert_eq!(last_toast(&st), Some(want.as_str()));
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn back_with_one_commit_or_a_working_directory_compare() {
    let (mut st, f) = fixture("focus-back-one");
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(4))));
    let toasts = st.toasts.len();
    assert!(up(&mut st, Msg::Selection(SelectionMsg::BackToFocus)).is_empty());
    assert_eq!((st.selection.clone(), st.toasts.len()), (Selection::Commit(f.c(4)), toasts), "nothing to go back from");

    st.selection = Selection::VsWorkdir(WorkdirCompare::new(f.c(4)));
    up(&mut st, Msg::Selection(SelectionMsg::BackToFocus));
    assert_eq!(st.selection, Selection::Commit(f.c(4)));
    assert_eq!(st.toasts.len(), toasts, "collapsed silently, like Esc");
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn the_default_key_goes_back() {
    let keys = Keymap::default();
    assert_eq!(keys.lookup(Chord::cmd_shift(KeyName::Letter('A')), fold_ctrl()), Some(BindingId::SelectFocusedCommit));
    assert_eq!(BindingId::SelectFocusedCommit.key(), "select-focused-commit");
    assert!(crate::keymap::binding(BindingId::SelectFocusedCommit).rebindable);
}
