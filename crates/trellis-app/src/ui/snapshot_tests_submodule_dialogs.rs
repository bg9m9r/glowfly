//! The Add submodule dialog (ready to add, with a problem in every field, and waiting for the
//! folder check), the Edit submodule dialog (with a bad URL) and the Remove submodule
//! confirmation (with everything it warns about), drawn over the shared fixture.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_worktrees::{H, p, pushed};
use crate::msg::Msg;
use crate::state::submodules::add::AddForm;
use crate::state::submodules::edit::EditForm;
use crate::state::submodules::remove::RemoveForm;
use crate::state::{AppState, Modal};
use crate::submodule::SubmoduleMsg;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use std::path::PathBuf;
use trellis_core::clone_ops::BRANCH_INVALID;
use trellis_core::settings::ThemeChoice;

/// The fixture with the Add dialog open, filled in by `fill`.
fn add_dialog(name: &str, fill: impl FnOnce(&mut AddForm)) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    let mut form = AddForm::new(st.snap.as_ref().unwrap().workdir.clone());
    fill(&mut form);
    st.modal = Some(Modal::AddSubmodule(form));
    (st, dir)
}

fn add_disabled(h: &H) -> bool {
    h.get_by_role_and_label(Role::Button, "Add submodule").is_disabled()
}

#[test]
fn submodule_add_dialog() {
    let (st, dir) = add_dialog("submodule-add", |f| {
        f.url = "https://example.com/team/lib.git".into();
        f.path = "lib".into();
        f.branch = "dev".into();
        f.checked = Some(("lib".into(), Ok(Vec::new())));
    });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("submodule_add_dialog");
    assert!(!add_disabled(&h));
    h.get_by_role_and_label(Role::Button, "Add submodule").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Submodule(SubmoduleMsg::Add))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn submodule_add_dialog_problems() {
    let tracked = "README.md is already tracked. Choose a folder that doesn't exist yet, or an empty one.";
    let (st, dir) = add_dialog("submodule-add-problems", |f| {
        f.url = "../lib.git".into();
        f.path = "README.md".into();
        f.path_edited = true;
        f.branch = "bad..name".into();
        f.checked = Some(("README.md".into(), Err(tracked.into())));
    });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("submodule_add_dialog_problems");
    assert!(h.query_by_label_contains("resolved against this repository's default remote URL").is_some());
    assert!(h.query_by_label(tracked).is_some());
    assert!(h.query_by_label(BRANCH_INVALID).is_some());
    assert!(add_disabled(&h));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn submodule_add_dialog_checking() {
    let (st, dir) = add_dialog("submodule-add-checking", |f| {
        f.url = "git@example.com:team/lib.git".into();
        f.path = "lib".into();
    });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("submodule_add_dialog_checking");
    assert!(h.query_by_label("Checking…").is_some());
    assert!(add_disabled(&h));
    drop(h);
    remove_fixture(&dir);
}

fn remove_form() -> RemoveForm {
    let commit = |i: usize| (Oid::from_str(&format!("{:x}", i + 1).repeat(40)[..40]).unwrap(), format!("Local work {}", i + 1));
    RemoveForm {
        path: "libs/lib".into(),
        name: "libs/lib".into(),
        initialized: true,
        module_dir: Some(p("C:/work/app/.git/modules/libs/lib")),
        dirty: true,
        unpushed: (0..7).map(commit).collect(),
        stashes: 1,
    }
}

fn remove_confirm(theme: ThemeChoice, name: &str, dark: bool) {
    let (mut st, dir) = with_repo(theme, name);
    st.modal = Some(Modal::RemoveSubmodule(remove_form()));
    let mut h = harness(st, dark);
    h.run();
    h.check_snapshot(name.replace('-', "_").as_str());
    assert!(h.query_by_label_contains("and 2 more").is_some());
    assert!(h.query_by_label("It has 1 stash; it is deleted with the stored module.").is_some());
    h.get_by_role_and_label(Role::Button, "Remove submodule").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Submodule(SubmoduleMsg::Remove))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn submodule_remove_confirm() {
    remove_confirm(ThemeChoice::Dark, "submodule-remove-confirm", true);
}

#[test]
fn submodule_remove_confirm_light() {
    remove_confirm(ThemeChoice::Light, "submodule-remove-confirm-light", false);
}

/// The fixture with the Edit dialog open for a submodule named `lib` at libs/lib, its URL
/// changed to an invalid one so the problem line shows.
fn edit_dialog(theme: ThemeChoice, name: &str, dark: bool) {
    let (mut st, dir) = with_repo(theme, name);
    let id = Oid::from_str(&"a".repeat(40)).unwrap();
    let sm = trellis_core::repo::SubmoduleInfo {
        name: "lib".into(),
        path: "libs/lib".into(),
        url: Some("../lib.git".into()),
        clone_url: Some("https://example.com/team/lib.git".into()),
        branch: Some("main".into()),
        added: false,
        recorded: id,
        checked_out: Some(id),
        state: trellis_core::repo::SubmoduleState::InSync,
        left_behind: Vec::new(),
    };
    let mut form = EditForm::new(st.snap.as_ref().unwrap().workdir.clone(), &sm);
    form.url = "-x".into();
    st.modal = Some(Modal::EditSubmodule(form));
    let mut h = harness(st, dark);
    h.run();
    h.check_snapshot(name.replace('-', "_").as_str());
    assert!(h.query_by_label("URLs can't start with a dash.").is_some());
    assert!(h.query_by_label("Named lib in .gitmodules").is_some());
    assert!(h.get_by_role_and_label(Role::Button, "Save").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn submodule_edit_dialog() {
    edit_dialog(ThemeChoice::Dark, "submodule-edit-dialog", true);
}

#[test]
fn submodule_edit_dialog_light() {
    edit_dialog(ThemeChoice::Light, "submodule-edit-dialog-light", false);
}

#[test]
fn submodule_edit_dialog_saves() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "submodule-edit-saves");
    let id = Oid::from_str(&"a".repeat(40)).unwrap();
    let sm = trellis_core::repo::SubmoduleInfo {
        name: "libs/lib".into(),
        path: "libs/lib".into(),
        url: Some("../lib.git".into()),
        clone_url: None,
        branch: None,
        added: false,
        recorded: id,
        checked_out: Some(id),
        state: trellis_core::repo::SubmoduleState::InSync,
        left_behind: Vec::new(),
    };
    let mut form = EditForm::new(st.snap.as_ref().unwrap().workdir.clone(), &sm);
    form.branch = "dev".into();
    st.modal = Some(Modal::EditSubmodule(form));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Named libs/lib in .gitmodules").is_none(), "the name is the path");
    h.get_by_role_and_label(Role::Button, "Save").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Submodule(SubmoduleMsg::Edit))));
    drop(h);
    remove_fixture(&dir);
}
