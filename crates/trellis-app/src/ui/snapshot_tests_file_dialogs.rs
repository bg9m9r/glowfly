//! The New file and Rename file dialogs over the whole window: a valid path, a refused one, a
//! bare folder prefill, a rename onto a listed path, the staged-rename checkbox, a running
//! request, the messages the buttons and the checkbox send, and the delete-to-trash
//! confirmation. Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::Msg;
use crate::msg::WorkFileMsg;
use crate::state::Modal;
use crate::state::files::PathForm;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::Toggled;
use egui_kittest::kittest::Queryable;
use trellis_core::repo::FileEntry;
use trellis_core::settings::ThemeChoice;

#[test]
fn new_file_dialog() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "new-file");
    st.modal = Some(Modal::NewFile(PathForm::new("docs/notes.md")));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("new_file_dialog");
    assert!(!h.get_by_label("Create").is_disabled());
    assert!(h.query_by_label_contains("already exists").is_none());
    h.get_by_label("Create").click();
    h.run();
    let msgs = &h.state().1;
    assert!(msgs.iter().any(|m| matches!(m, Msg::WorkFile(WorkFileMsg::CreateFile(t)) if t == "docs/notes.md")));
    assert!(!msgs.iter().any(|m| matches!(m, Msg::CloseModal)), "stays open until the result");
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::CloseModal)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn new_file_dialog_folder_prefill() {
    // "New file in this folder…" opens with just the folder: no error yet, Create off.
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "new-file-folder");
    st.modal = Some(Modal::NewFile(PathForm::new("docs/")));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("new_file_dialog_folder");
    assert!(h.query_by_label_contains("not a folder").is_none());
    assert!(h.get_by_label("Create").is_disabled());
    drop(h);

    let (mut st, _) = with_repo(ThemeChoice::Dark, "new-file-folder");
    st.modal = Some(Modal::NewFile(PathForm::new("docs/a.md")));
    let mut h = harness(st, true);
    h.run();
    assert!(!h.get_by_label("Create").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn new_file_dialog_invalid_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "new-file-invalid");
    st.modal = Some(Modal::NewFile(PathForm::new(".git/hooks/x")));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("new_file_dialog_invalid_light");
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
    st.settings.stage_renames = false;
    st.snap.as_mut().unwrap().unstaged.push(FileEntry::new("notes.txt", '?'));
    st.modal = Some(Modal::RenameFile { old: "README.md".into(), form: PathForm::new("notes.txt") });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("rename_file_dialog");
    assert!(h.query_by_label_contains("notes.txt already exists.").is_some());
    assert!(h.get_by_label("Rename").is_disabled());
    assert!(h.query_by_label_contains("until you stage both").is_some(), "README.md is tracked");
    assert_eq!(h.get_by_label(STAGE).toggled(), Some(Toggled::False));
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

const STAGE: &str = "Stage the rename (git mv)";

#[test]
fn rename_file_dialog_staged() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "rename-file-staged");
    st.settings.stage_renames = true;
    st.modal = Some(Modal::RenameFile { old: "README.md".into(), form: PathForm::new("docs/README.md") });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("rename_file_dialog_staged");
    assert_eq!(h.get_by_label(STAGE).toggled(), Some(Toggled::True));
    assert!(h.query_by_label_contains("until you stage both").is_none());
    h.get_by_label("Rename").click();
    h.run();
    let sent = |m: &Msg| matches!(m, Msg::WorkFile(WorkFileMsg::RenameFile { from, to, stage: true }) if from == "README.md" && to == "docs/README.md");
    assert!(h.state().1.iter().any(sent));

    // The checkbox only asks; update saves the setting.
    h.get_by_label(STAGE).click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::WorkFile(WorkFileMsg::SetRenameStages(false)))));
    assert!(h.state().0.settings.stage_renames);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn rename_untracked_has_no_stage_option() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "rename-untracked");
    st.snap.as_mut().unwrap().unstaged.push(FileEntry::new("notes.txt", '?'));
    st.modal = Some(Modal::RenameFile { old: "notes.txt".into(), form: PathForm::new("docs/notes.txt") });
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label_contains("Stage the rename").is_none());
    assert!(h.query_by_label_contains("until you stage both").is_none());
    h.get_by_label("Rename").click();
    h.run();
    let sent = |m: &Msg| matches!(m, Msg::WorkFile(WorkFileMsg::RenameFile { from, stage: false, .. }) if from == "notes.txt");
    assert!(h.state().1.iter().any(sent));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn confirm_delete_trash() {
    // The trash confirmation for an untracked file: the longer body wraps and the button fits.
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "confirm-delete-trash");
    st.snap.as_mut().unwrap().unstaged.push(FileEntry::new("notes/scratch.txt", '?'));
    crate::update::update(
        &mut st,
        Msg::WorkFile(WorkFileMsg::DeleteFile { path: "notes/scratch.txt".into(), confirmed: false }),
        std::time::Instant::now(),
    );
    assert!(matches!(&st.modal, Some(Modal::ConfirmDanger { .. })));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("confirm_delete_trash");
    let button = format!("Move to {}", crate::state::files::TRASH);
    h.get_by_label(&button).click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::ConfirmAccepted)), "the button accepts");
    drop(h);
    remove_fixture(&dir);
}
