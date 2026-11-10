//! Stashing the WIP files picked with gaps: the selection menu's Stash N selected files... opens
//! the Stash dialog on Selected files with every stashable pick ticked (a pick with a hazard or
//! no change left isn't), and stashing them runs one git stash that leaves the file between
//! them alone.

use super::stash_push_tests::form;
use super::tests::drive;
use super::wip_picks_tests::{c_text, picked_a_and_c, read};
use glowfly_core::gitcli::{args, run as git_run};
use glowfly_core::repo::Hazard;
use glowfly_core::stash_ops::StashScope;
use glowfly_model::msg::{Msg, StashMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::wip_lists::picked_paths;

/// What the selection menu's Stash N selected files... sends for the Unstaged Files picks.
fn stash_picks(st: &mut AppState) {
    let msg = Msg::Stash(StashMsg::PushDialog { scope: StashScope::Files, paths: picked_paths(st, false) });
    drive(st, msg);
}

fn chosen(st: &AppState) -> Vec<&str> {
    form(st).chosen.iter().map(String::as_str).collect()
}

#[test]
fn the_dialog_opens_with_every_pick_ticked() {
    let (mut st, dir) = picked_a_and_c("stash-picks-open");
    stash_picks(&mut st);
    assert_eq!(form(&st).scope, StashScope::Files);
    assert_eq!(chosen(&st), ["a.txt", "c.txt"], "b.txt, between them, isn't ticked");
    assert_eq!(picked_paths(&st, false), ["a.txt", "c.txt"], "the picks stay while the dialog is up");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_pick_that_cant_be_stashed_isnt_ticked() {
    let (mut st, dir) = picked_a_and_c("stash-picks-unsafe");
    let c = st.snap.as_mut().unwrap().unstaged.iter_mut().find(|f| f.path == "c.txt").unwrap();
    c.hazard = Some(Hazard::Twins(vec!["C.txt".into()]));
    drive(
        &mut st,
        Msg::Stash(StashMsg::PushDialog { scope: StashScope::Files, paths: vec!["a.txt".into(), "c.txt".into(), "clean.txt".into()] }),
    );
    assert_eq!(chosen(&st), ["a.txt"], "neither the hazard nor a file with no change");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stashing_picks_with_gaps_makes_one_stash_of_them_only() {
    let (mut st, dir) = picked_a_and_c("stash-picks-push");
    stash_picks(&mut st);
    let paths = form(&st).paths(st.snap.as_ref().unwrap());
    assert_eq!(paths, ["a.txt", "c.txt"]);
    drive(&mut st, Msg::Stash(StashMsg::Push { scope: StashScope::Files, message: "picks".into(), paths, keep_index: false }));
    let list = git_run(&dir, &args(&["stash", "list"])).1;
    assert_eq!(list.lines().count(), 1, "one stash: {list}");
    assert_eq!(read(&dir, "a.txt").as_deref(), Some("one\n"), "back to HEAD");
    assert_eq!(read(&dir, "c.txt"), None, "the untracked pick went into the stash");
    assert_eq!(read(&dir, "b.txt").as_deref(), Some("b edited\n"), "not picked, untouched");
    let shown = git_run(&dir, &args(&["show", "--format=", "--name-only", "stash@{0}^3"])).1;
    assert!(shown.lines().any(|l| l == "c.txt"), "{shown}: {}", c_text(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}
