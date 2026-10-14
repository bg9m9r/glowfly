//! The New file and Rename file dialogs over the whole window: a valid path, a refused one, a
//! rename onto a listed path, a running request, and the messages the buttons send. Shares the
//! fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::Msg;
use crate::state::Modal;
use crate::state::files::PathForm;
use egui_kittest::kittest::Queryable;
use trellis_core::repo::FileEntry;
use trellis_core::settings::ThemeChoice;

#[test]
fn new_file_dialog() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "new-file");
    st.modal = Some(Modal::NewFile(PathForm::new("docs/notes.md")));
    let mut h = harness(st, true);
    h.run();
    h.snapshot("new_file_dialog");
    assert!(!h.get_by_label("Create").is_disabled());
    assert!(h.query_by_label_contains("already exists").is_none());
    h.get_by_label("Create").click();
    h.run();
    let msgs = &h.state().1;
    assert!(msgs.iter().any(|m| matches!(m, Msg::CreateFile(t) if t == "docs/notes.md")));
    assert!(!msgs.iter().any(|m| matches!(m, Msg::CloseModal)), "stays open until the result");
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::CloseModal)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn new_file_dialog_invalid_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "new-file-invalid");
    st.modal = Some(Modal::NewFile(PathForm::new(".git/hooks/x")));
    let mut h = harness(st, false);
    h.run();
    h.snapshot("new_file_dialog_invalid_light");
    assert!(h.query_by_label_contains("inside the .git folder").is_some());
    assert!(h.get_by_label("Create").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn new_file_dialog_shows_the_last_error_for_its_text() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "new-file-error");
    let mut form = PathForm::new("build/out.log");
    form.error = Some(("build/out.log".into(), "build is a file, so build/out.log can't be written".into()));
    st.modal = Some(Modal::NewFile(form));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label_contains("build is a file").is_some());
    drop(h);

    // An error produced for other text is not shown.
    let (mut st, _) = with_repo(ThemeChoice::Dark, "new-file-error");
    let mut form = PathForm::new("build/other.log");
    form.error = Some(("build/out.log".into(), "build is a file, so build/out.log can't be written".into()));
    st.modal = Some(Modal::NewFile(form));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label_contains("build is a file").is_none());
    assert!(!h.get_by_label("Create").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn new_file_dialog_busy() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "new-file-busy");
    let mut form = PathForm::new("docs/notes.md");
    form.busy = true;
    st.modal = Some(Modal::NewFile(form));
    let mut h = harness(st, true);
    // The spinner keeps repainting, so run a few frames rather than until idle.
    for _ in 0..3 {
        h.step();
    }
    assert!(h.get_by_label("Create").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn rename_file_dialog() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "rename-file");
    st.snap.as_mut().unwrap().unstaged.push(FileEntry { path: "notes.txt".into(), status: '?' });
    st.modal = Some(Modal::RenameFile { old: "README.md".into(), form: PathForm::new("notes.txt") });
    let mut h = harness(st, true);
    h.run();
    h.snapshot("rename_file_dialog");
    assert!(h.query_by_label_contains("notes.txt already exists.").is_some());
    assert!(h.get_by_label("Rename").is_disabled());
    assert!(h.query_by_label_contains("until you stage both").is_some(), "README.md is tracked");
    drop(h);

    // Unchanged: no message, Rename off. A change of case only is allowed.
    for (text, enabled) in [("README.md", false), ("readme.md", true)] {
        let (mut st, _) = with_repo(ThemeChoice::Dark, "rename-file");
        st.modal = Some(Modal::RenameFile { old: "README.md".into(), form: PathForm::new(text) });
        let mut h = harness(st, true);
        h.run();
        assert!(h.query_by_label_contains("already exists").is_none(), "{text}");
        assert_eq!(!h.get_by_label("Rename").is_disabled(), enabled, "{text}");
    }
    remove_fixture(&dir);
}
