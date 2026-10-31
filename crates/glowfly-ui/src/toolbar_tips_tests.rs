//! The per-button tooltips on a one-commit fixture: Undo with nothing to undo, Pop without a
//! stash, the terminal button for both right-click choices, and Push and Pull over an upstream.

use super::*;
use crate::state::tip::TipSub;
use crate::state::toolbar_tips::tests::state;

#[test]
fn undo_with_nothing_recorded_says_so() {
    let st = state("tip-undo");
    let tip = undo(&st);
    assert_eq!(tip.title, "Undo");
    assert_eq!(tip.sub, Some(TipSub::Keys(BindingId::Undo)));
    assert_eq!(tip.body.as_deref(), Some("Nothing to undo"));
    assert_eq!(tip.preview, None);
    assert_eq!(redo(&st).body.as_deref(), Some("Nothing to redo"));
}

#[test]
fn pop_without_a_stash_says_why() {
    let st = state("tip-pop");
    let tip = pop(&st);
    assert_eq!((tip.title.as_str(), tip.body.as_deref()), ("Pop the latest stash", Some("No stashes to pop")));
    assert_eq!(tip.preview, None);
}

#[test]
fn terminal_follows_the_right_click_setting() {
    let mut st = state("tip-terminal");
    let tip = terminal(&st);
    assert_eq!(tip.title, "Show the terminal panel");
    assert_eq!(tip.sub, Some(TipSub::Keys(BindingId::ToggleTerminal)));
    assert_eq!(tip.body.as_deref(), Some("Right-click to open an external terminal."));
    st.settings.terminal.open_in_panel = true;
    st.terminal.open = true;
    let tip = terminal(&st);
    assert_eq!(tip.title, "Hide the terminal panel");
    assert_eq!(tip.body.as_deref(), Some("Right-click for a new terminal tab."));
}

#[test]
fn push_and_pull_name_the_upstream() {
    let mut st = state("tip-sync");
    let push_tip = push(&st);
    assert_eq!(push_tip.title, "Publish this repository", "no remote at all");
    crate::state::toolbar_tips::tests::diverge(st.snap.as_mut().unwrap(), 2, 0, true);
    let tip = push(&st);
    assert_eq!(tip.title, "Push main to origin/main");
    assert_eq!(tip.sub, Some(TipSub::State("main is 2 ahead, 0 behind origin/main".into())));
    assert_eq!(tip.preview.map(|p| p.heading).as_deref(), Some("Sends 2 commits"));
    let tip = pull(&st);
    assert_eq!(tip.title, "Pull origin/main into main");
    assert_eq!(tip.body.as_deref(), Some("Nothing new on origin/main since the last fetch; pulling checks again."));
    assert_eq!(tip.footer.as_deref(), Some("Not fetched yet"));
    st.snap.as_mut().unwrap().locals[0].upstream = None;
    st.snap.as_mut().unwrap().remotes = vec![("origin".into(), Vec::new())];
    assert_eq!(push(&st).title, "Push main to origin and track it");
}
