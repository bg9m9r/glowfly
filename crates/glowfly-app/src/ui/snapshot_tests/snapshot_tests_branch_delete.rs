//! The branch Delete dialog: for a branch with commits on no other branch, the count and Delete
//! anyway, which only a click presses (Enter presses Cancel, Ctrl/Cmd+Enter nothing); while the
//! count runs, a disabled Delete.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_worktrees::{H, pushed};
use crate::msg::{BranchMsg, Msg};
use crate::state::Modal;
use crate::state::branch_delete::{DeleteForm, Lost};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use eframe::egui::{Key, Modifiers};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;

fn open(name: &str, lost: Lost) -> (H, std::path::PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    let tip = st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == "search").and_then(|b| b.oid).unwrap();
    st.modal = Some(Modal::DeleteBranch(DeleteForm { name: "search".into(), tip, lost, in_head: false, role: None }));
    let mut h = harness(st, true);
    // The counting line's spinner keeps repainting.
    h.run_steps(3);
    (h, dir)
}

fn deleted(h: &H) -> bool {
    pushed(h, &|m| matches!(m, Msg::Branch(BranchMsg::Delete { name, force: true, .. }) if name == "search"))
}

#[test]
fn branch_delete_unmerged() {
    let (mut h, dir) = open("branch-delete-unmerged", Lost::Known(3));
    h.check_snapshot("branch_delete_unmerged");
    assert!(h.query_by_label_contains("3 commits aren't on any other branch").is_some());
    assert!(h.get_by_label("Cancel").is_focused(), "Cancel is the default");
    h.press_key_modifiers(Modifiers::COMMAND, Key::Enter);
    h.run();
    assert!(!deleted(&h), "Ctrl/Cmd+Enter never presses Delete anyway");
    h.press_key(Key::Enter);
    h.run();
    assert!(!deleted(&h), "nor does Enter");
    assert!(pushed(&h, &|m| matches!(m, Msg::CloseModal)), "Enter presses Cancel");
    h.state_mut().1.clear();
    h.get_by_role_and_label(Role::Button, "Delete anyway").click();
    h.run();
    assert!(deleted(&h), "a click does, with -D");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn branch_delete_counting() {
    let (mut h, dir) = open("branch-delete-counting", Lost::Counting);
    assert!(h.query_all_by_label_contains("Checking whether its commits are on another branch").next().is_some());
    let delete = h.get_by_role_and_label(Role::Button, "Delete");
    assert!(delete.is_disabled(), "nothing to press until the count is in");
    delete.click();
    h.run_steps(2);
    assert!(!pushed(&h, &|m| matches!(m, Msg::Branch(BranchMsg::Delete { .. }))));
    drop(h);
    remove_fixture(&dir);
}
