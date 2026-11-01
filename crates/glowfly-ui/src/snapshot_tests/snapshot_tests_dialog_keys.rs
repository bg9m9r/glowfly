//! The confirmation dialogs in their two tones, the acknowledgement a step that can't be undone
//! asks for, and the frame's keys: Enter runs the default button (the primary, or Cancel in a
//! danger dialog), Ctrl/Cmd+Enter never presses a danger button, and a dialog's default button
//! takes the focus when it opens.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_worktrees::{H, pushed};
use crate::msg::Msg;
use crate::state::Modal;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use eframe::egui::{Key, Modifiers};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;

const BODY: &str = "Delete local branch feature/login? Commits only on it can be found again in the reflog for a while.";

fn normal() -> Modal {
    Modal::Confirm { title: "Delete branch".into(), body: BODY.into(), button: "Delete".into(), then: Box::new(Msg::ClearLog) }
}

fn danger() -> Modal {
    Modal::danger("Delete branch", BODY, "Delete", Msg::ClearLog)
}

fn accepted(h: &H) -> bool {
    pushed(h, &|m| matches!(m, Msg::ConfirmAccepted))
}

fn closed(h: &H) -> bool {
    pushed(h, &|m| matches!(m, Msg::CloseModal))
}

fn open(theme: ThemeChoice, name: &str, modal: Modal) -> (H, std::path::PathBuf) {
    let dark = theme != ThemeChoice::Light;
    let (mut st, dir) = with_repo(theme, name);
    st.modal = Some(modal);
    let mut h = harness(st, dark);
    h.run();
    (h, dir)
}

#[test]
fn confirm_dialog_normal() {
    let (mut h, dir) = open(ThemeChoice::Dark, "confirm-normal", normal());
    h.check_snapshot("confirm_dialog_normal");
    assert!(h.get_by_role_and_label(Role::Button, "Delete").is_focused(), "the primary is the default");
    h.press_key(Key::Enter);
    h.run();
    assert!(accepted(&h), "Enter right after opening accepts");
    drop(h);
    remove_fixture(&dir);
}

fn danger_keys(theme: ThemeChoice, name: &str) {
    let (mut h, dir) = open(theme, name, danger());
    h.check_snapshot(&name.replace('-', "_"));
    assert!(h.get_by_label("Cancel").is_focused(), "Cancel is the default of a danger dialog");
    h.press_key_modifiers(Modifiers::COMMAND, Key::Enter);
    h.run();
    assert!(!accepted(&h), "Ctrl/Cmd+Enter never presses the danger button");
    h.press_key(Key::Enter);
    h.run();
    assert!(!accepted(&h), "nor does Enter");
    assert!(closed(&h), "Enter presses Cancel");
    h.state_mut().1.clear();
    h.get_by_role_and_label(Role::Button, "Delete").click();
    h.run();
    assert!(accepted(&h), "a click does");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn confirm_dialog_danger() {
    danger_keys(ThemeChoice::Dark, "confirm-dialog-danger");
}

#[test]
fn confirm_dialog_ack() {
    let modal = Modal::danger("Discard changes", "Delete untracked file notes.txt? This cannot be undone.", "Discard", Msg::ClearLog)
        .with_ack("I understand this can't be undone");
    let (mut h, dir) = open(ThemeChoice::Dark, "confirm-ack", modal);
    h.check_snapshot("confirm_dialog_ack_unticked");
    assert!(h.get_by_role_and_label(Role::Button, "Discard").is_disabled());
    h.get_by_role_and_label(Role::Button, "Discard").click();
    h.run();
    assert!(!accepted(&h), "nothing before the tick");
    h.get_by_label("I understand this can't be undone").click();
    h.run();
    assert!(matches!(h.state().0.modal, Some(Modal::ConfirmDanger { acked: true, .. })));
    h.check_snapshot("confirm_dialog_ack_ticked");
    h.get_by_role_and_label(Role::Button, "Discard").click();
    h.run();
    assert!(accepted(&h));
    drop(h);
    remove_fixture(&dir);
}
