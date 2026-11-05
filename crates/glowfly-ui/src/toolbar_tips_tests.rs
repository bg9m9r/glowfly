//! The per-button tooltips on a one-commit fixture: Undo with nothing to undo, Pop without a
//! stash, the terminal button for both right-click choices, and Push and Pull over an upstream (the
//! count in Push's title, the pull mode in Pull's).

use super::*;
use glowfly_model::state::toolbar_tips::tests::state;

#[test]
fn undo_with_nothing_recorded_says_so() {
    let st = state("tip-undo");
    let tip = undo(&st);
    assert_eq!(tip.title, "Undo");
    assert_eq!(tip.keys, Some(BindingId::Undo));
    assert_eq!(tip.body.as_deref(), Some("Nothing to undo"));
    assert_eq!(tip.preview, None);
    assert_eq!(redo(&st).body.as_deref(), Some("Nothing to redo"));
}

#[test]
fn pop_without_a_stash_says_why() {
    let st = state("tip-pop");
    let tip = pop(&st);
    assert_eq!((tip.title.as_str(), tip.body.as_deref()), ("Unstash the latest stash", Some("No stashes")));
    assert_eq!(tip.preview, None);
}

#[test]
fn terminal_follows_the_right_click_setting() {
    let mut st = state("tip-terminal");
    let tip = terminal(&st);
    assert_eq!(tip.title, "Show the terminal panel");
    assert_eq!(tip.keys, Some(BindingId::ToggleTerminal));
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
    glowfly_model::state::toolbar_tips::tests::diverge(st.snap.as_mut().unwrap(), 2, 0, true);
    let tip = push(&st);
    assert_eq!(tip.title, "Push main to origin/main · 2 commits");
    assert_eq!(tip.state.as_deref(), Some("main is 2 ahead, 0 behind origin/main"));
    assert_eq!(tip.preview.map(|p| p.heading).as_deref(), Some("Sends 2 commits"));
    let tip = pull(&st);
    assert_eq!(tip.title, "Pull origin/main into main (merge)");
    assert_eq!(tip.body.as_deref(), Some("Nothing new on origin/main since the last fetch; pulling checks again."));
    assert_eq!(tip.footer.as_deref(), Some("Not fetched yet"));
    st.snap.as_mut().unwrap().locals[0].upstream = None;
    st.snap.as_mut().unwrap().remotes = vec![("origin".into(), Vec::new())];
    assert_eq!(push(&st).title, "Publish main to origin (creates origin/main)");
}

#[test]
fn pull_names_the_configured_mode_and_push_drops_a_zero_count() {
    let mut st = state("tip-sync-mode");
    glowfly_model::state::toolbar_tips::tests::diverge(st.snap.as_mut().unwrap(), 0, 3, true);
    assert_eq!(push(&st).title, "Push main to origin/main", "nothing to send: no count");
    st.snap.as_mut().unwrap().pull_rebase = true;
    assert_eq!(pull(&st).title, "Pull origin/main into main (rebase)");
    glowfly_model::state::toolbar_tips::tests::diverge(st.snap.as_mut().unwrap(), 1, 0, true);
    assert_eq!(push(&st).title, "Push main to origin/main · 1 commit");
}

#[test]
fn push_names_the_remote_config_pushes_to() {
    use glowfly_core::repo::PushRefusal;
    use glowfly_model::state::toolbar_tips::tests::{diverge, push_to};
    let mut st = state("tip-push-fork");
    let snap = st.snap.as_mut().unwrap();
    diverge(snap, 2, 0, true);
    let base = snap.commits.iter().find(|c| c.summary == "one").unwrap().oid;
    snap.remotes.push(("fork".into(), Vec::new()));
    // remote.pushDefault=fork, which has no main yet: named, without a count.
    push_to(snap, "fork", Ok("refs/heads/main"), None, (0, 0));
    let tip = push(&st);
    assert_eq!(tip.title, "Push main to fork/main");
    assert_eq!((tip.body, tip.preview), (None, None), "nothing known of fork/main yet");
    // fork/main known: counted and previewed against it.
    push_to(st.snap.as_mut().unwrap(), "fork", Ok("refs/heads/main"), Some(base), (2, 0));
    let tip = push(&st);
    assert_eq!(tip.title, "Push main to fork/main · 2 commits");
    assert_eq!(tip.preview.map(|p| (p.heading, p.rows.len())), Some(("Sends 2 commits".into(), 2)));
    // Level with fork/main: nothing to push there.
    push_to(st.snap.as_mut().unwrap(), "fork", Ok("refs/heads/main"), Some(base), (0, 0));
    assert_eq!(push(&st).body.as_deref(), Some("Nothing to push: fork/main has every commit on main."));
    // Back to the upstream.
    push_to(st.snap.as_mut().unwrap(), "origin", Ok("refs/heads/main"), Some(base), (2, 0));
    assert_eq!(push(&st).title, "Push main to origin/main · 2 commits");
    // Refused: named by remote, the reason as its body, no count or preview.
    push_to(st.snap.as_mut().unwrap(), "fork", Err(PushRefusal::Nothing), None, (0, 0));
    let tip = push(&st);
    assert_eq!(tip.title, "Push main to fork");
    assert!(tip.body.as_deref().is_some_and(|b| b.starts_with("push.default is \"nothing\"")), "{:?}", tip.body);
    assert_eq!(tip.preview, None);
    // A missing push remote passed over: the upstream, with a footer saying so.
    let p = push_to(st.snap.as_mut().unwrap(), "origin", Ok("refs/heads/main"), Some(base), (2, 0));
    p.missing = Some(("branch.main.pushRemote".into(), "gone".into()));
    let tip = push(&st);
    assert_eq!(tip.title, "Push main to origin/main · 2 commits");
    assert_eq!(
        tip.footer.as_deref(),
        Some("branch.main.pushRemote names \"gone\", which isn't a remote here, so Push goes to origin/main, the upstream, instead.")
    );
}
