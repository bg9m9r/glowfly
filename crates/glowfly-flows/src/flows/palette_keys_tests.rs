//! Headless tests for palette commands run from their keys (Preferences > Keyboard): the
//! palette's own message runs only while the palette would list the entry, two-step commands
//! open their step, nothing is recorded among recent commands, blocked commands say why, and
//! the keys are saved and loaded like any other.

use super::palette_tests::repo_with_topic;
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::keymap::{BindingId, Chord, KeyName, Keymap, Slot};
use crate::msg::{KeysMsg, Msg, PaletteMsg, RepoMsg};
use crate::state::palette::{InputKind, Mode, PaletteCommand as C, PickKind, shown};
use crate::state::toolbar_tips;
use crate::state::{AppState, Modal, PrefPage, PrefsState, Selection};
use std::collections::BTreeMap;
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn bound(st: &mut AppState, cmd: C) -> Vec<Effect> {
    run(st, Msg::Palette(PaletteMsg::RunBoundCommand(cmd)))
}

fn last_log(st: &AppState) -> Option<&str> {
    st.log.last().map(|e| e.text.as_str())
}

/// A scratch repository with a bare clone of itself as `origin`, opened in a new state.
fn repo_with_remote(name: &str) -> (AppState, Vec<std::path::PathBuf>) {
    let dir = scratch_repo(name);
    let bare = dir.with_extension("git");
    let _ = std::fs::remove_dir_all(&bare);
    git(&dir, &["clone", "-q", "--bare", ".", bare.to_str().unwrap()]);
    git(&dir, &["remote", "add", "origin", bare.to_str().unwrap()]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.snap.is_some());
    (st, vec![dir, bare])
}

fn remove(dirs: &[std::path::PathBuf]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn bound_fetch_runs_the_palette_fetch() {
    let (mut st, dirs) = repo_with_remote("bound-fetch");
    let fx = bound(&mut st, C::Fetch);
    assert!(matches!(&fx[..], [Effect::RunGit { .. }]), "{fx:?}");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Fetch all"));
    assert!(st.palette.is_none(), "a one-step command leaves the palette closed");
    assert!(st.palette_recent.is_empty(), "a key press isn't a recent palette command");
    remove(&dirs);
}

#[test]
fn bound_command_shows_the_bound_key_in_the_palette() {
    let (mut st, dirs) = repo_with_remote("bound-fetch-hint");
    let hint = |st: &AppState| {
        let list = shown(st, "");
        let e = list.iter().find(|s| s.entry.label == "Fetch all remotes").expect("listed");
        e.entry.shortcut(&st.keymap).map(str::to_owned)
    };
    assert_eq!(hint(&st), None, "no key by default");
    st.keymap.set(BindingId::Fetch, vec![Chord::plain(KeyName::F(8))]);
    assert_eq!(hint(&st).as_deref(), Some("F8"));
    remove(&dirs);
}

#[test]
fn unavailable_fetch_logs_the_toolbar_reason() {
    let dir = scratch_repo("bound-fetch-no-remote");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let why = toolbar_tips::fetch_blocked(&st).expect("no remote");
    assert!(bound(&mut st, C::Fetch).is_empty());
    assert!(st.job.is_none());
    assert_eq!(last_log(&st), Some(why.as_str()));

    let why = toolbar_tips::pop_blocked(&st).expect("no stash");
    assert!(bound(&mut st, C::PopStash).is_empty());
    assert_eq!(last_log(&st), Some(why.as_str()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bound_two_step_commands_open_their_step() {
    let (mut st, dir) = repo_with_topic("bound-two-step");
    let fx = bound(&mut st, C::FileHistory);
    assert!(matches!(&st.palette.as_ref().expect("open").mode, Mode::Pick(p) if matches!(p.kind, PickKind::FileHistory)));
    assert!(matches!(&fx[..], [Effect::LoadTrackedPaths { .. }]), "{fx:?}");

    st.palette = None;
    assert!(bound(&mut st, C::StashMessage).is_empty());
    assert!(matches!(st.palette.as_ref().expect("open").mode, Mode::Input(InputKind::StashMessage)));
    assert!(st.palette_recent.is_empty(), "opening a step from a key isn't recorded either");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bound_create_branch_opens_the_dialog() {
    let (mut st, dir) = repo_with_topic("bound-branch");
    let oid = st.snap.as_ref().unwrap().head_oid.unwrap();
    st.session.selection = Selection::Commit(oid);
    let dialog = |st: &AppState| match &st.modal {
        Some(Modal::NewBranch { start, name, .. }) => Some((start.clone(), name.clone())),
        _ => None,
    };
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::CreateBranch)));
    let from_palette = dialog(&st);
    st.modal = None;
    st.palette_recent = Default::default();
    bound(&mut st, C::CreateBranch);
    assert_eq!(dialog(&st), Some((Some(oid.to_string()), String::new())));
    assert_eq!(dialog(&st), from_palette, "the same dialog as the palette's entry");
    assert!(st.palette_recent.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_repository_ignores_bound_keys() {
    let mut st = state();
    for cmd in [C::Push, C::Fetch, C::OpenTerminal, C::FileHistory] {
        assert!(bound(&mut st, cmd.clone()).is_empty(), "{cmd:?}");
    }
    assert!(st.log.is_empty(), "nothing to say without a repository");
    assert!(st.palette.is_none() && st.modal.is_none());
}

fn page() -> AppState {
    let mut st = state();
    st.prefs = Some(PrefsState::new(PrefPage::Keyboard));
    st
}

fn keys(st: &mut AppState, m: KeysMsg) -> Vec<Effect> {
    run(st, Msg::Keys(m))
}

fn saves(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::SaveSettings(_))).count()
}

#[test]
fn assigning_a_command_key_saves_and_reloads() {
    let mut st = page();
    let f8 = Chord::plain(KeyName::F(8));
    assert!(keys(&mut st, KeysMsg::Record { id: BindingId::Fetch, slot: Slot::Add }).is_empty());
    let fx = keys(&mut st, KeysMsg::Recorded(f8));
    assert_eq!(saves(&fx), 1);
    let want: BTreeMap<String, Vec<String>> = [("fetch".to_owned(), vec!["F8".to_owned()])].into();
    assert_eq!(st.settings.keybindings, want);
    assert_eq!(Keymap::from_overrides(&st.settings.keybindings), (st.keymap.clone(), Vec::new()));

    keys(&mut st, KeysMsg::Remove { id: BindingId::Fetch, index: 0 });
    assert!(st.settings.keybindings.is_empty(), "no key is the default");
    assert!(st.keymap.is_default(BindingId::Fetch));

    keys(&mut st, KeysMsg::Record { id: BindingId::Fetch, slot: Slot::Add });
    keys(&mut st, KeysMsg::Recorded(f8));
    assert_eq!(st.settings.keybindings, want);
    keys(&mut st, KeysMsg::Reset(BindingId::Fetch));
    assert!(st.settings.keybindings.is_empty());
    assert_eq!(st.keymap.hint(BindingId::Fetch), None);
}
