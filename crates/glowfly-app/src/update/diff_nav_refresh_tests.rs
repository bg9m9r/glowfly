//! Headless tests for a next/previous change step that a refresh catches while its file loads:
//! the step goes on to the refreshed list's neighbour (or the shown file reloads when none is
//! left), and a diff-options change asks for the step's file again with the new options.

use super::diff_nav_tests::{at_end_of, finish, nav_repo, one_load, pending_step, run, shown, unstaged};
use crate::effect::{Effect, StatusRead};
use crate::msg::{DiffMsg, LoadMsg, Msg};
use crate::runner;
use crate::state::AppState;
use crate::state::diff_nav::Landing;
use glowfly_core::repo::FileEntry;

/// A status refresh arriving with `keep` deciding which unstaged and staged rows remain.
fn refresh(st: &mut AppState, keep: impl Fn(&FileEntry) -> bool) -> Vec<Effect> {
    let snap = st.snap.as_ref().unwrap();
    let unstaged = snap.unstaged.iter().filter(|f| keep(f)).cloned().collect();
    let staged = snap.staged.iter().filter(|f| keep(f)).cloned().collect();
    let status = StatusRead::Whole(unstaged, staged, snap.state);
    let generation = st.load_gen;
    run(
        st,
        Msg::Load(LoadMsg::StatusLoaded {
            generation,
            result: Ok(status),
            submodules: None,
            open_stamp: None,
            slow: None,
            lfs_patterns: None,
            index: None,
        }),
    )
}

#[test]
fn status_refresh_keeps_pending_step() {
    let (mut st, dir) = nav_repo("refresh-keep");
    let stale = pending_step(&mut st);
    let fx = refresh(&mut st, |_| true);
    assert_eq!(one_load(&fx), (&unstaged("b.bin"), false), "the step's file is asked for again");
    assert_eq!(st.view.diff_landing, Some((unstaged("b.bin"), Landing::First)));
    run(&mut st, runner::execute(stale).unwrap());
    assert_eq!(shown(&st), Some(&unstaged("a.txt")), "the superseded request doesn't land");
    assert!(st.view.diff_landing.is_some(), "and doesn't end the step");
    finish(&mut st, fx);
    // b.bin has nothing to land on, so the step goes on to c.txt.
    assert_eq!(shown(&st), Some(&unstaged("c.txt")));
    assert_eq!(st.view.diff_landing, None);
    assert!(matches!(&st.view.file_reveal, Some((t, g)) if *t == unstaged("c.txt") && *g == st.diff_shown_gen));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refresh_that_changes_the_list_steps_to_the_new_neighbour() {
    let (mut st, dir) = nav_repo("refresh-neighbour");
    let stale = pending_step(&mut st);
    // b.bin left the list while it loaded: a.txt's next file is now c.txt.
    let fx = refresh(&mut st, |f| f.path != "b.bin");
    assert_eq!(one_load(&fx), (&unstaged("c.txt"), false));
    assert_eq!(st.view.diff_landing, Some((unstaged("c.txt"), Landing::First)));
    run(&mut st, runner::execute(stale).unwrap());
    assert_eq!(shown(&st), Some(&unstaged("a.txt")));
    finish(&mut st, fx);
    assert_eq!(shown(&st), Some(&unstaged("c.txt")));
    assert_eq!(st.view.diff_cursor, Some(st.view.diff_top_row), "landed on its first change");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refresh_with_no_neighbour_left_reloads_the_shown_file() {
    let (mut st, dir) = nav_repo("refresh-none");
    pending_step(&mut st);
    let fx = refresh(&mut st, |f| f.path == "a.txt");
    assert_eq!(one_load(&fx), (&unstaged("a.txt"), true), "the shown file reloads");
    assert_eq!(st.view.diff_landing, None, "the step ends");
    finish(&mut st, fx);
    assert_eq!(shown(&st), Some(&unstaged("a.txt")));
    assert_eq!(st.view.file_reveal, None, "a reload reveals nothing");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn options_change_during_step_reopens_the_target() {
    let (mut st, dir) = nav_repo("refresh-options");
    pending_step(&mut st);
    let fx = run(&mut st, Msg::Diff(DiffMsg::SetIgnoreWhitespace(true)));
    let loads: Vec<_> = fx.iter().filter(|e| matches!(e, Effect::LoadDiff { .. })).collect();
    assert!(
        matches!(&loads[..], [Effect::LoadDiff { target, reload: false, ignore_whitespace: true, .. }] if *target == unstaged("b.bin")),
        "{fx:?}"
    );
    assert_eq!(st.view.diff_landing, Some((unstaged("b.bin"), Landing::First)));
    finish(&mut st, fx);
    assert_eq!(shown(&st), Some(&unstaged("c.txt")));
    // A plain open reveals nothing.
    at_end_of(&mut st, unstaged("a.txt"));
    assert_eq!(st.view.file_reveal, None);
    let _ = std::fs::remove_dir_all(&dir);
}
