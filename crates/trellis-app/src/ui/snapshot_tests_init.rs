//! The Init dialog: a new folder with a Rust .gitignore, operating system files and an MIT
//! license; a folder that is already a repository with a .gitignore of its own (commit off);
//! and an invalid branch name with an empty copyright holder (red lines, Create disabled).

use super::snapshot_tests::{render, settings};
use crate::state::init::InitForm;
use crate::state::touched::Touched;
use crate::state::{AppState, Modal};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::init_ops::{FolderFacts, GitignoreTemplate, License};
use trellis_core::settings::ThemeChoice;

fn with_form(theme: ThemeChoice, form: InitForm) -> AppState {
    let mut st = AppState::new(settings(theme), Vec::new(), Instant::now());
    st.modal = Some(Modal::Init(form));
    st
}

/// A form for `folder` whose check found `facts`.
fn form(folder: &str, facts: FolderFacts) -> InitForm {
    InitForm { folder_checked: Some((folder.into(), Some(facts))), ..InitForm::new(folder.into(), "main".into()) }
}

fn facts(path: &str) -> FolderFacts {
    FolderFacts { path: PathBuf::from(path), exists: true, repo: false, has_files: false, gitignore: false, license: None }
}

#[test]
fn init_dialog_new_folder() {
    let f = InitForm {
        gitignore: Some(GitignoreTemplate::Rust),
        os_editor: true,
        license: Some(License::Mit),
        holder: "Ada Lovelace".into(),
        ..form(r"C:\src\new-app", FolderFacts { exists: false, ..facts(r"C:\src\new-app") })
    };
    render("init_dialog_new_folder", with_form(ThemeChoice::Dark, f), true);
}

#[test]
fn init_dialog_existing_repo() {
    let found = FolderFacts { repo: true, has_files: true, gitignore: true, ..facts(r"C:\src\old-app") };
    let f = InitForm { gitignore: Some(GitignoreTemplate::Node), license: Some(License::Apache2), ..form(r"C:\src\old-app", found) };
    render("init_dialog_existing_repo", with_form(ThemeChoice::Light, f), false);
}

#[test]
fn init_dialog_problems() {
    let f = InitForm {
        branch: "a..b".into(),
        license: Some(License::Bsd3),
        touched: Touched::all(),
        ..form(r"C:\src\tool", facts(r"C:\src\tool"))
    };
    render("init_dialog_problems", with_form(ThemeChoice::Dark, f), true);
}
