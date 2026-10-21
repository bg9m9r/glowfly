//! Headless tests for tab labels (set, cleared, saved, restored, kept when a tab is closed and
//! reopened) and for moving the active tab with the keyboard.

use super::tabs_close_tests::{fake, fake_tabs, names, saves, send};
use crate::effect::{Effect, UiEffect};
use crate::msg::TabsMsg;
use crate::state::menu_bar::tab_name;
use crate::state::{AppState, Modal, Tab};
use glowfly_core::repo::CommonDir;

fn label(st: &AppState, i: usize) -> Option<&str> {
    st.tabs[i].label.as_deref()
}

fn saved_labels(fx: &[Effect]) -> Vec<Vec<Option<String>>> {
    fx.iter().filter_map(|e| if let Effect::SaveTabs { labels, .. } = e { Some(labels.clone()) } else { None }).collect()
}

fn titles(fx: &[Effect]) -> Vec<&str> {
    fx.iter().filter_map(|e| if let Effect::Ui(UiEffect::SetTitle(t)) = e { Some(t.as_str()) } else { None }).collect()
}

#[test]
fn set_and_clear_a_label() {
    let mut st = fake_tabs(&["a", "b"], 1);
    let b = st.tabs[1].id;
    let fx = send(&mut st, TabsMsg::SetTabLabel { tab: b, label: Some("Frontend".into()) });
    assert_eq!(label(&st, 1), Some("Frontend"));
    assert_eq!(tab_name(&st, 1), "Frontend");
    assert_eq!(saved_labels(&fx), [vec![None, Some("Frontend".to_owned())]]);
    assert_eq!(titles(&fx), ["Frontend - Trellis"]);

    // An inactive tab's label leaves the title alone.
    let a = st.tabs[0].id;
    let fx = send(&mut st, TabsMsg::SetTabLabel { tab: a, label: Some("  Docs \n".into()) });
    assert_eq!(label(&st, 0), Some("Docs"), "cleaned again");
    assert!(titles(&fx).is_empty());

    for cleared in [None, Some("  ".to_owned()), Some("\t\u{7}\n".to_owned()), Some("b".to_owned())] {
        send(&mut st, TabsMsg::SetTabLabel { tab: b, label: Some("Frontend".into()) });
        let fx = send(&mut st, TabsMsg::SetTabLabel { tab: b, label: cleared.clone() });
        assert_eq!(label(&st, 1), None, "{cleared:?} clears the label");
        assert_eq!(tab_name(&st, 1), "b");
        assert_eq!(titles(&fx), ["b - Trellis"]);
    }
}

#[test]
fn rename_dialog_opens_for_repository_tabs_only() {
    let mut st = fake_tabs(&["a", "b"], 0);
    send(&mut st, TabsMsg::RenameTabDialog(1));
    match &st.modal {
        Some(Modal::RenameTab(form)) => {
            assert_eq!((form.tab, form.text.as_str(), form.folder.as_str(), form.current.as_deref()), (st.tabs[1].id, "b", "b", None));
            assert!(!form.changed(), "the folder name as it is changes nothing");
        }
        _ => panic!("the Rename tab dialog opens"),
    }
    st.tabs[0].label = Some("API".into());
    send(&mut st, TabsMsg::RenameTabDialog(usize::MAX));
    match &st.modal {
        Some(Modal::RenameTab(form)) => assert_eq!((form.text.as_str(), form.current.as_deref()), ("API", Some("API"))),
        _ => panic!("the Rename tab dialog opens for the active tab"),
    }
    let a = st.tabs[0].id;
    send(&mut st, TabsMsg::SetTabLabel { tab: a, label: Some("Server".into()) });
    assert!(st.modal.is_none(), "setting the label closes the dialog");

    st.tabs.push(Tab::empty());
    send(&mut st, TabsMsg::RenameTabDialog(2));
    assert!(st.modal.is_none(), "an empty tab has nothing to rename");
}

#[test]
fn labels_survive_restore_and_reopen() {
    let mut st = super::tests::state();
    let paths = vec![fake("api"), fake("web")];
    send(&mut st, TabsMsg::RestoreTabs { paths: paths.clone(), labels: vec![Some("API".into()), None], active: 1 });
    assert_eq!((label(&st, 0), label(&st, 1)), (Some("API"), None));
    assert_eq!(tab_name(&st, 0), "API");

    send(&mut st, TabsMsg::CloseTab(0));
    assert_eq!(st.closed_tabs.last().and_then(|c| c.label.as_deref()), Some("API"));
    let fx = send(&mut st, TabsMsg::ReopenClosedTab);
    let api = st.tabs.iter().position(|t| t.is(&fake("api"))).expect("reopened");
    assert_eq!(label(&st, api), Some("API"), "it comes back with its label");
    assert!(saved_labels(&fx).last().is_some_and(|l| l.contains(&Some("API".to_owned()))));

    let mut st = super::tests::state();
    send(&mut st, TabsMsg::RestoreTabs { paths, labels: Vec::new(), active: 0 });
    assert_eq!(names(&st), ["api", "web"]);
    assert!(st.tabs.iter().all(|t| t.label.is_none()));
}

fn group(g: &str) -> CommonDir {
    CommonDir { dir: glowfly_core::paths::temp_dir().join(g).join(".git"), folder: None, name: g.into() }
}

#[test]
fn keyboard_move_moves_the_active_tab() {
    let mut st = fake_tabs(&["a", "b", "c"], 1);
    let fx = send(&mut st, TabsMsg::MoveActiveTab(1));
    assert_eq!((names(&st), st.active_tab), (vec!["a".to_owned(), "c".into(), "b".into()], 2));
    assert_eq!(saves(&fx), 1);
    assert!(send(&mut st, TabsMsg::MoveActiveTab(1)).is_empty(), "no wrap at the right end");
    send(&mut st, TabsMsg::SelectTab(0));
    assert!(send(&mut st, TabsMsg::MoveActiveTab(-1)).is_empty(), "no wrap at the left end");
    assert_eq!(names(&st), ["a", "c", "b"]);

    // With groups: inside the group a swap, at its edge the whole group moves.
    let mut st = fake_tabs(&["x", "a1", "a2", "y"], 1);
    st.settings.group_worktree_tabs = true;
    for i in [1, 2] {
        st.tabs[i].group = Some(group("a"));
    }
    let a1 = st.tabs[1].id;
    send(&mut st, TabsMsg::MoveActiveTab(1));
    assert_eq!(names(&st), ["x", "a2", "a1", "y"]);
    send(&mut st, TabsMsg::MoveActiveTab(1));
    assert_eq!(names(&st), ["x", "y", "a2", "a1"]);
    assert_eq!(st.tabs[st.active_tab].id, a1, "the moved tab stays active");
}
