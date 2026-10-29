//! Headless tests for a folder a second Glowfly process hands over (`glowfly <path>` while a
//! window runs, `DeepLinkMsg::OpenFolder`): the window comes forward and opens it in a tab the
//! way a picked folder opens, switches to the tab already showing it, and waits behind a toast
//! while a dialog is open.

use super::tests::{drive, same_place, scratch_repo, state};
use super::update;
use crate::deep_link::DeepLinkMsg;
use crate::effect::{Effect, LoadKind, UiEffect};
use crate::msg::{Msg, RepoMsg};
use crate::state::{AppState, Modal};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn handed(st: &mut AppState, dir: &Path) -> Vec<Effect> {
    update(st, Msg::DeepLink(DeepLinkMsg::OpenFolder(dir.to_path_buf())), Instant::now())
}

fn focuses(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Ui(UiEffect::FocusWindow)))
}

fn loads(fx: &[Effect], dir: &Path) -> bool {
    fx.iter().any(|e| matches!(e, Effect::LoadRepo { kind: LoadKind::Open, path, .. } if same_place(path, dir)))
}

fn shows(st: &AppState, dir: &Path) -> bool {
    st.tab_showing(dir) == Some(st.active_tab)
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// A open in the only tab.
fn one_tab(name: &str) -> (AppState, PathBuf) {
    let a = scratch_repo(&format!("{name}-a"));
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    assert_eq!(st.tabs.len(), 1);
    (st, a)
}

#[test]
fn a_handed_folder_opens_in_a_new_tab_and_the_window_comes_forward() {
    let (mut st, a) = one_tab("handed-new");
    let b = scratch_repo("handed-new-b");
    let fx = handed(&mut st, &b);
    assert!(focuses(&fx), "{fx:?}");
    assert!(loads(&fx, &b), "opened as a picked folder is: {fx:?}");
    assert_eq!(st.tabs.len(), 2, "next to A, not in its place");
    assert!(shows(&st, &b));
    assert!(st.tab_showing(&a).is_some());
    cleanup(&[&a, &b]);
}

#[test]
fn a_handed_folder_already_open_is_switched_to() {
    let (mut st, a) = one_tab("handed-switch");
    let b = scratch_repo("handed-switch-b");
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    assert!(shows(&st, &b));
    let fx = handed(&mut st, &a);
    assert!(focuses(&fx));
    assert_eq!(st.tabs.len(), 2, "no new tab");
    assert!(shows(&st, &a));
    // The active one again: only the focus, no reload.
    let fx = handed(&mut st, &a);
    assert!(matches!(&fx[..], [Effect::Ui(UiEffect::FocusWindow)]), "{fx:?}");
    cleanup(&[&a, &b]);
}

#[test]
fn a_handed_folder_waits_behind_an_open_dialog() {
    let (mut st, a) = one_tab("handed-dialog");
    let b = scratch_repo("handed-dialog-b");
    drive(&mut st, Msg::confirm("Delete branch", "Delete it?".into(), "Delete", Msg::ClearLog));
    let fx = handed(&mut st, &b);
    assert!(matches!(&fx[..], [Effect::Ui(UiEffect::FocusWindow)]), "{fx:?}");
    assert_eq!(st.tabs.len(), 1, "nothing opens under the dialog");
    assert!(matches!(st.modal, Some(Modal::Confirm { .. })), "the dialog stays");
    let t = st.toasts.last().expect("a toast");
    assert_eq!(t.text, format!("Close the open dialog to show {}.", b.file_name().unwrap().to_string_lossy()));
    let action = t.action.as_ref().expect("Show");
    assert!(matches!(&*action.msg, Msg::DeepLink(DeepLinkMsg::OpenFolder(p)) if *p == b));
    let id = t.id;

    drive(&mut st, Msg::CloseModal);
    let fx = update(&mut st, Msg::ToastAction(id), Instant::now());
    assert!(focuses(&fx));
    assert!(loads(&fx, &b));
    assert!(shows(&st, &b), "Show opens it once the dialog is closed");
    cleanup(&[&a, &b]);
}
