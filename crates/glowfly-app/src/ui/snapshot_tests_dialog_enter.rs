//! Enter in a dialog's single-line field runs the primary on the first press: the Init dialog,
//! whose folder field takes the focus back when nothing has it, and the upstream dialog, whose
//! branch field reports nothing itself.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_worktrees::{H, pushed};
use crate::msg::{BranchMsg, InitMsg, Msg};
use crate::state::init::InitForm;
use crate::state::{AppState, Modal};
use eframe::egui::Key;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{By, Queryable};
use glowfly_core::init_ops::FolderFacts;
use glowfly_core::settings::ThemeChoice;
use std::time::Instant;

fn started(h: &H) -> bool {
    pushed(h, &|m| matches!(m, Msg::Init(InitMsg::Start(_))))
}

#[test]
fn init_dialog_enter_in_folder_creates() {
    let path = glowfly_core::paths::temp_dir().join("trellis-init-enter");
    let folder = path.display().to_string();
    let facts = FolderFacts { path, exists: false, repo: false, has_files: false, gitignore: false, license: None };
    let form = InitForm { folder_checked: Some((folder.clone(), Some(facts))), ..InitForm::new(folder.clone(), "main".into()) };
    let mut st = AppState::new(super::snapshot_tests::settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    st.modal = Some(Modal::Init(form));
    let mut h = harness(st, true);
    h.run();
    assert!(h.get(By::new().role(Role::TextInput).value(folder.as_str())).is_focused(), "the folder field has the focus");
    h.press_key(Key::Enter);
    h.run();
    assert!(started(&h), "one Enter in the folder field creates the repository");
}

#[test]
fn init_dialog_enter_blocked_keeps_folder_focus() {
    let mut st = AppState::new(super::snapshot_tests::settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    // Not checked yet: Create is disabled.
    st.modal = Some(Modal::Init(InitForm::new("new-app".into(), "main".into())));
    let mut h = harness(st, true);
    h.run();
    h.press_key(Key::Enter);
    h.run();
    assert!(!started(&h), "a disabled Create doesn't run");
    assert!(h.get(By::new().role(Role::TextInput).value("new-app")).is_focused(), "the folder field takes the focus back");
}

#[test]
fn set_upstream_dialog_enter_in_branch_saves() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "upstream-enter");
    st.modal =
        Some(Modal::SetUpstream { branch: "search".into(), remote: "origin".into(), name: "topic".into(), touched: Default::default() });
    let mut h = harness(st, true);
    h.run();
    h.get(By::new().role(Role::TextInput).value("topic")).focus();
    h.run();
    assert!(h.get(By::new().role(Role::TextInput).value("topic")).is_focused());
    h.press_key(Key::Enter);
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Branch(BranchMsg::SetUpstream { name, .. }) if name == "topic")), "one Enter saves");
    drop(h);
    remove_fixture(&dir);
}
