//! Headless tests for a click on a desktop notification (update/notify.rs): the window comes
//! forward and the repository the note was about is shown, switched to or opened again, unless
//! a dialog is open.

use super::tests::{drive, same_place, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadKind, UiEffect};
use crate::msg::{Msg, NotifyMsg, RepoMsg, TabsMsg};
use crate::state::{AppState, Modal};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn click(st: &mut AppState, workdir: Option<&Path>) -> Vec<Effect> {
    let msg = Msg::Notify(NotifyMsg::DesktopClicked { workdir: workdir.map(Path::to_path_buf) });
    update(st, msg, Instant::now())
}

fn focuses(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Ui(UiEffect::FocusWindow)))
}

fn shows(st: &AppState, dir: &Path) -> bool {
    st.tab_showing(dir) == Some(st.active_tab)
}

/// A and B open in two tabs, B active.
fn two_tabs(name: &str) -> (AppState, PathBuf, PathBuf) {
    let (a, b) = (scratch_repo(&format!("{name}-a")), scratch_repo(&format!("{name}-b")));
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    assert_eq!(st.tabs.len(), 2);
    assert!(shows(&st, &b));
    (st, a, b)
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn a_click_focuses_the_window() {
    let mut st = state();
    let tabs = st.tabs.len();
    let fx = click(&mut st, None);
    assert!(matches!(&fx[..], [Effect::Ui(UiEffect::FocusWindow)]), "{fx:?}");
    assert_eq!((st.tabs.len(), st.active_tab), (tabs, 0));
}

#[test]
fn a_click_switches_to_the_repositorys_tab() {
    let (mut st, a, b) = two_tabs("click-switch");
    let fx = click(&mut st, Some(&a));
    assert!(focuses(&fx), "{fx:?}");
    assert_eq!(st.tabs.len(), 2, "no new tab");
    assert!(shows(&st, &a));
    assert!(st.snap.as_ref().is_some_and(|s| same_place(&s.workdir, &a)));
    cleanup(&[&a, &b]);
}

#[test]
fn a_click_reopens_a_closed_repository() {
    let (mut st, a, b) = two_tabs("click-reopen");
    let at = st.tab_showing(&a).expect("A's tab");
    drive(&mut st, Msg::Tabs(TabsMsg::CloseTab(at)));
    assert_eq!(st.tab_showing(&a), None, "closed");
    let fx = click(&mut st, Some(&a));
    assert!(focuses(&fx));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { kind: LoadKind::Open, path, .. } if same_place(path, &a))), "{fx:?}");
    assert_eq!(st.tabs.len(), 2, "in a new tab");
    assert!(shows(&st, &a));
    cleanup(&[&a, &b]);
}

#[test]
fn a_click_on_the_active_repository_only_focuses() {
    let (mut st, a, b) = two_tabs("click-active");
    let fx = click(&mut st, Some(&b));
    assert!(matches!(&fx[..], [Effect::Ui(UiEffect::FocusWindow)]), "no reload: {fx:?}");
    assert!(shows(&st, &b));
    st.dashboard.open = true;
    let fx = click(&mut st, Some(&b));
    assert!(matches!(&fx[..], [Effect::Ui(UiEffect::FocusWindow)]), "{fx:?}");
    assert!(!st.dashboard.open, "out of the dashboard");
    cleanup(&[&a, &b]);
}

#[test]
fn a_click_under_a_dialog_waits() {
    let (mut st, a, b) = two_tabs("click-dialog");
    drive(&mut st, Msg::confirm("Delete branch", "Delete it?".into(), "Delete", Msg::ClearLog));
    assert!(matches!(st.modal, Some(Modal::Confirm { .. })));
    let active = st.active_tab;
    let fx = click(&mut st, Some(&a));
    assert!(matches!(&fx[..], [Effect::Ui(UiEffect::FocusWindow)]), "{fx:?}");
    assert_eq!(st.active_tab, active, "no switch under the dialog");
    assert!(matches!(st.modal, Some(Modal::Confirm { .. })), "the dialog stays");
    let t = st.toasts.last().expect("a toast");
    let name = a.file_name().unwrap().to_string_lossy();
    assert_eq!(t.text, format!("Close the open dialog to show {name}."));
    let action = t.action.as_ref().expect("Show");
    assert_eq!(action.label, "Show");
    assert!(action.workdir.is_none());
    assert!(matches!(&*action.msg, Msg::Notify(NotifyMsg::DesktopClicked { workdir: Some(p) }) if *p == a));
    let id = t.id;

    drive(&mut st, Msg::CloseModal);
    let fx = update(&mut st, Msg::ToastAction(id), Instant::now());
    assert!(focuses(&fx));
    assert!(shows(&st, &a), "Show switches once the dialog is closed");

    // Preferences counts as a dialog too.
    st.prefs = Some(crate::state::PrefsState::new(crate::state::prefs::PrefPage::General));
    let active = st.active_tab;
    click(&mut st, Some(&b));
    assert_eq!(st.active_tab, active);
    cleanup(&[&a, &b]);
}
