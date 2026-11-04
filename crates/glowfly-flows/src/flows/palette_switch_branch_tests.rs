//! Headless tests for the palette's Switch branch step: its key (Ctrl+Shift+O, Cmd+Shift+O on
//! macOS) opens it, its rows are the main list's checkouts without "Checkout ", nothing opens
//! without a repository, and a row runs the same checkout as the main list's entry.

use super::branch_tests::with_origin;
use super::tests::{drive, git};
use super::update;
use glowfly_model::effect::Effect;
use glowfly_model::keymap::{BindingId, Chord, KeyName, fold_ctrl};
use glowfly_model::msg::{Msg, PaletteMsg, RepoMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::palette::{Mode, PaletteCommand as C, Pick, PickKind, shown};
use glowfly_model::state::palette_bindings::command_for;
use std::path::Path;
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn pick(st: &AppState) -> &Pick {
    match st.palette.as_ref().map(|p| &p.mode) {
        Some(Mode::Pick(p)) => p,
        _ => panic!("the palette is not in a sub-list"),
    }
}

/// What pressing the Switch branch key does: the binding it finds, run as the key would.
fn press_switch_key(st: &mut AppState) -> Vec<Effect> {
    let id = st.keymap.lookup(Chord::cmd_shift(KeyName::Letter('O')), fold_ctrl());
    assert_eq!(id, Some(BindingId::SwitchBranch));
    let cmd = command_for(BindingId::SwitchBranch).expect("a palette command");
    assert_eq!(cmd, C::ChooseCheckout);
    run(st, Msg::Palette(PaletteMsg::RunBoundCommand(cmd)))
}

/// `with_origin` plus a remote-only branch `other` and a tag `v1.0`.
fn with_refs(name: &str) -> (AppState, std::path::PathBuf, std::path::PathBuf) {
    let (mut st, work, bare) = with_origin(name);
    git(&work, &["push", "-q", "origin", "main:other"]);
    git(&work, &["fetch", "-q", "origin"]);
    git(&work, &["tag", "v1.0"]);
    git(&work, &["commit", "-q", "--allow-empty", "-m", "after the tag"]);
    super::tests::clean_tree(&work);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    (st, work, bare)
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn the_key_opens_the_switch_branch_step() {
    let (mut st, work, bare) = with_refs("pal-switch-key");
    assert!(st.palette.is_none());
    press_switch_key(&mut st);
    let pk = pick(&st);
    assert_eq!(pk.kind, PickKind::SwitchBranch);
    let labels: Vec<&str> = (0..3).map(|i| pk.label(i)).collect();
    assert_eq!(labels, ["feature", "origin/other", "tag v1.0"], "locals, then remote branches and tags");
    assert_eq!(pk.detail(0).as_deref(), Some("origin/feature"));
    assert_eq!(pk.detail(1).as_deref(), Some("creates local branch"));
    assert!(pk.detail(2).is_some_and(|d| d.starts_with("detached at ")));
    assert_eq!(pk.command(0), Some(C::Checkout("feature".into())));
    assert_eq!(pk.command(2), Some(C::CheckoutTag("v1.0".into())));
    let k = PickKind::SwitchBranch;
    assert_eq!((k.title(), k.prompt()), ("Switch branch", "Find a branch or tag to switch to"));
    assert_eq!((k.none_text(), k.empty_text()), ("No matching branches or tags", "No other branches or tags to switch to"));
    assert_eq!(k.footer(), "Up/Down to choose, Enter to switch, Backspace or Esc to go back");
    assert_eq!(k.more_text(1, 2), "Showing 1 of 2 branches and tags. Type more to narrow the list.");
    assert!(!k.dims_folders());
    // The main list keeps its own checkout entries, and lists the step's entry.
    let main: Vec<String> = shown(&st, "").into_iter().map(|s| s.entry.label).collect();
    assert!(main.iter().any(|l| l == "Checkout origin/other") && main.iter().any(|l| l == "Switch branch…"), "{main:?}");
    cleanup(&[&work, &bare]);
}

#[test]
fn no_step_without_a_repository() {
    let mut st = super::tests::state();
    press_switch_key(&mut st);
    assert!(st.palette.is_none());
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::ChooseCheckout)));
    assert!(st.palette.is_none());
}

#[test]
fn a_remote_row_runs_the_main_lists_checkout() {
    let (mut st, work, bare) = with_refs("pal-switch-remote");
    let main = C::CheckoutRemote { remote: "origin".into(), branch: "other".into() };
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    let from_main = run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(main.clone())));
    assert!(st.palette.is_none());

    let (mut st2, work2, bare2) = with_refs("pal-switch-remote-row");
    press_switch_key(&mut st2);
    let row = (0..3).find(|&i| pick(&st2).label(i) == "origin/other").expect("the remote row");
    let cmd = pick(&st2).command(row).expect("a command");
    assert_eq!(cmd, main);
    let from_row = run(&mut st2, Msg::Palette(PaletteMsg::RunPaletteCommand(cmd)));
    assert!(st2.palette.is_none());
    let git_of = |fx: &[Effect]| match fx {
        [Effect::RunGit { cmds, record, .. }] => (cmds.clone(), record.clone()),
        other => panic!("expected one RunGit, got {other:?}"),
    };
    assert_eq!(git_of(&from_row), git_of(&from_main));
    cleanup(&[&work, &bare, &work2, &bare2]);
}
