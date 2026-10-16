//! Headless tests for the menu bar's window actions (Quit, About) and Edit > Find commits.

use super::super::tests::{drive, scratch_repo, state};
use super::super::update;
use crate::effect::{Effect, UiEffect};
use crate::msg::{AppMsg, CredentialMsg, Msg, PaletteMsg, RepoMsg, SearchMsg};
use crate::state::{AppState, Modal, PrefPage, PrefsState};
use std::time::Instant;
use trellis_core::askpass::Prompt;
use trellis_core::repo::{DiffTarget, FileDiff};

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn about_open(st: &AppState) -> bool {
    matches!(st.modal, Some(Modal::About))
}

#[test]
fn quit_closes_the_window_and_changes_nothing() {
    let mut st = state();
    let fx = run(&mut st, Msg::App(AppMsg::Quit));
    assert!(matches!(&fx[..], [Effect::Ui(UiEffect::CloseWindow)]), "{fx:?}");
    assert!(st.modal.is_none() && st.palette.is_none() && st.toasts.is_empty());
}

#[test]
fn about_opens_over_the_palette_and_closes() {
    let mut st = state();
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    assert!(st.palette.is_some());
    assert!(run(&mut st, Msg::App(AppMsg::ShowAbout)).is_empty());
    assert!(about_open(&st));
    assert!(st.palette.is_none(), "About replaces the palette");
    run(&mut st, Msg::CloseModal);
    assert!(st.modal.is_none());
}

#[test]
fn about_waits_for_other_dialogs() {
    let mut st = state();
    run(&mut st, Msg::confirm("Delete", "Sure?".into(), "Delete", Msg::Repo(RepoMsg::Refresh)));
    run(&mut st, Msg::App(AppMsg::ShowAbout));
    assert!(matches!(st.modal, Some(Modal::Confirm { .. })), "another dialog stays");

    let mut st = state();
    st.prefs = Some(PrefsState::new(PrefPage::General));
    run(&mut st, Msg::App(AppMsg::ShowAbout));
    assert!(st.modal.is_none(), "not over Preferences");

    let mut st = state();
    run(&mut st, Msg::Credentials(CredentialMsg::CredentialPrompt { id: 1, prompt: Prompt::new("Password: ", None) }));
    run(&mut st, Msg::App(AppMsg::ShowAbout));
    assert!(st.modal.is_none(), "not under a credential prompt");
}

#[test]
fn app_messages_keep_nothing_for_inactive_tabs() {
    for m in [AppMsg::Quit, AppMsg::ShowAbout] {
        assert!(m.clone().on_inactive().is_none(), "{m:?}");
    }
}

#[test]
fn find_commits_closes_the_diff_and_focuses_search() {
    let dir = scratch_repo("menu-find-commits");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let diff = FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
    st.diff = Some((DiffTarget::Unstaged("a.txt".into()), diff.into()));
    run(&mut st, Msg::Search(SearchMsg::Open));
    assert!(st.diff.is_none());
    assert!(st.view.show_search && st.view.focus_search);
    assert!(SearchMsg::Open.on_inactive().is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn edit_menu_copy_items_follow_the_selection() {
    use crate::state::Selection;
    use crate::state::menu_bar::{NO_BRANCH, NO_COMMIT, ShaCopy, copy_branch, copy_message, copy_sha};
    use crate::state::selection::MultiSelection;
    use trellis_core::repo::{self, LoadOptions};

    let mut st = state();
    assert_eq!(copy_sha(&st), Err(NO_COMMIT));
    assert_eq!(copy_message(&st), Err(NO_COMMIT));
    assert_eq!(copy_branch(&st), Err(NO_BRANCH), "no repository open");

    let dir = scratch_repo("menu-copy");
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    st.selection = Selection::Wip;
    assert_eq!(copy_sha(&st), Err(NO_COMMIT));
    assert_eq!(copy_message(&st), Err(NO_COMMIT));
    assert_eq!(copy_branch(&st), Ok("main".to_owned()));

    let head = st.snap.as_ref().and_then(|s| s.head_oid).unwrap();
    st.selection = Selection::Commit(head);
    assert_eq!(copy_sha(&st), Ok(ShaCopy::One(head.to_string())));
    assert_eq!(copy_sha(&st).unwrap().label(), "Copy commit SHA");
    assert_eq!(copy_message(&st), Ok("first".to_owned()));

    let other = git2::Oid::from_str("1234567890123456789012345678901234567890").unwrap();
    st.selection = Selection::Commits(MultiSelection {
        oids: vec![head, other],
        anchor: head,
        changes: None,
        pick_blocked: None,
        export_blocked: None,
    });
    assert_eq!(copy_sha(&st), Ok(ShaCopy::Many(2)));
    assert_eq!(copy_sha(&st).unwrap().label(), "Copy 2 commit SHAs");
    assert_eq!(copy_message(&st), Err(NO_COMMIT), "one commit only");

    super::super::tests::git(&dir, &["checkout", "-q", "--detach"]);
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    assert_eq!(copy_branch(&st), Err(NO_BRANCH), "detached HEAD");
    let _ = std::fs::remove_dir_all(&dir);
}
