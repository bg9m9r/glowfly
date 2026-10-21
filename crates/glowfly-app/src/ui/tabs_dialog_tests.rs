//! The tab dialogs: Rename tab, and the confirmation before closing tabs that hold unsaved work.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::{CloseRequest, Msg, TabsMsg};
use crate::state::conflict::ConflictView;
use crate::state::tab_losses::close_confirm;
use crate::state::tabs::{Parked, RenameTabForm};
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::conflict_pick::ChunkPick;
use glowfly_core::repo::Choice;
use glowfly_core::settings::ThemeChoice;

type H = Harness<'static, (AppState, Vec<Msg>)>;

fn label_set(h: &H) -> Option<Option<String>> {
    h.state().1.iter().find_map(|m| if let Msg::Tabs(TabsMsg::SetTabLabel { label, .. }) = m { Some(label.clone()) } else { None })
}

#[test]
fn tabs_rename_dialog() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "tabs-rename");
    let mut form = RenameTabForm::new(&st.tabs[1]);
    form.text = "Frontend".into();
    st.modal = Some(Modal::RenameTab(form));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("tabs_rename_dialog");
    assert!(h.query_by_label("Use folder name").is_some());
    h.get_by_label("Rename").click();
    h.run();
    assert_eq!(label_set(&h), Some(Some("Frontend".to_owned())));
    drop(h);

    // With a label set, Use folder name clears it.
    let (mut st, _) = with_repo(ThemeChoice::Dark, "tabs-rename");
    st.tabs[1].label = Some("API".into());
    st.modal = Some(Modal::RenameTab(RenameTabForm::new(&st.tabs[1])));
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Use folder name").click();
    h.run();
    assert_eq!(label_set(&h), Some(None));
    drop(h);
    remove_fixture(&dir);
}

/// The Rename tab dialog over a tab labelled API, its field changed to `text`.
fn rename_open(name: &str, text: &str) -> (H, std::path::PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    st.tabs[1].label = Some("API".into());
    let mut form = RenameTabForm::new(&st.tabs[1]);
    form.text = text.into();
    st.modal = Some(Modal::RenameTab(form));
    let mut h = harness(st, true);
    h.run();
    (h, dir)
}

#[test]
fn tabs_rename_enter() {
    use eframe::egui::{Key, PointerButton};
    // Typing then Enter in the field renames.
    let (mut h, dir) = rename_open("tabs-rename-enter", "");
    // The dialog's field is drawn after the window's own.
    h.get_all_by_role(eframe::egui::accesskit::Role::TextInput).last().expect("the field").type_text("Frontend");
    h.run();
    h.press_key(Key::Enter);
    h.run();
    assert_eq!(label_set(&h), Some(Some("Frontend".to_owned())));
    drop(h);

    // A click on the dialog's background, then Enter, also renames.
    let (mut h, _) = rename_open("tabs-rename-enter", "Frontend");
    let help = super::snapshot_tests_worktrees::center(
        &h,
        "Shown in the tab bar, the Window menu and the window title. Leave it empty to use the folder name.",
    );
    super::snapshot_tests_select::click(&mut h, help, PointerButton::Primary);
    h.press_key(Key::Enter);
    h.run();
    assert_eq!(label_set(&h), Some(Some("Frontend".to_owned())));
    drop(h);

    // With Use folder name focused, Enter presses it, not Rename.
    let (mut h, _) = rename_open("tabs-rename-enter", "Frontend");
    h.get_by_label("Use folder name").focus();
    h.run();
    h.press_key(Key::Enter);
    h.run();
    assert_eq!(label_set(&h), Some(None));
    assert_eq!(h.state().1.iter().filter(|m| matches!(m, Msg::Tabs(TabsMsg::SetTabLabel { .. }))).count(), 1);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tabs_close_confirm() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "tabs-close-confirm");
    st.draft.summary = "Fix the login form".into();
    let mut conflict = ConflictView::loading("src/lib.rs".into());
    conflict.picks = vec![ChunkPick::Sides(Choice::Ours), ChunkPick::Sides(Choice::Theirs), ChunkPick::default()];
    conflict.auto = vec![None, None, None];
    st.tabs[1].parked = Some(Box::new(Parked { conflict: Some(conflict), ..Default::default() }));
    let then = Msg::Tabs(TabsMsg::CloseConfirmed(CloseRequest::Others(st.tabs[0].id)));
    st.modal = close_confirm(&st, &[0, 1], then);
    assert!(st.modal.is_some());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("tabs_close_confirm");
    h.get_by_label("Close 2 tabs").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::ConfirmAccepted)));
    drop(h);
    remove_fixture(&dir);
}
