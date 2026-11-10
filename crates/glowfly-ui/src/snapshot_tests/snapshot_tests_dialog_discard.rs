//! The "Discard your changes?" question over an edited form, and what a form's Esc and backdrop
//! click send (update/dialog.rs decides), and the confirmation for discarding several picked
//! files (to the trash, or deleted with Undo promised), and the one for deleting several picked
//! files. Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::click;
use super::snapshot_tests_worktrees::{H, pushed};
use crate::snapshot_check::CheckSnapshot;
use eframe::egui::{Key, PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use glowfly_model::msg::{DialogMsg, Dismiss, Msg};
use glowfly_model::state::Modal;
use glowfly_model::state::tags::TagForm;

fn dismissed(h: &H, how: Dismiss) -> bool {
    pushed(h, &|m| matches!(m, Msg::Dialog(DialogMsg::Dismiss(d)) if *d == how))
}

fn closed(h: &H) -> bool {
    pushed(h, &|m| matches!(m, Msg::CloseModal))
}

fn sent(h: &H, want: DialogMsg) -> bool {
    pushed(h, &|m| matches!(m, Msg::Dialog(d) if *d == want))
}

#[test]
fn discard_question_over_a_form() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "dialog-discard");
    st.modal =
        Some(Modal::NewBranch { start: None, name: "feature/login".into(), checkout: true, track: None, touched: Default::default() });
    st.dialog.asking = true;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("discard_changes_question");
    assert!(h.get_by_label("Keep editing").is_focused(), "the safe answer takes the focus, not the form's field");
    h.press_key(Key::Enter);
    h.run();
    assert!(sent(&h, DialogMsg::KeepEditing), "Enter keeps editing");
    h.state_mut().1.clear();

    h.get_by_label("Keep editing").click();
    h.run();
    assert!(sent(&h, DialogMsg::KeepEditing));
    h.state_mut().1.clear();
    h.get_by_label("Discard").click();
    h.run();
    assert!(sent(&h, DialogMsg::Discard));
    assert!(!closed(&h), "update closes the form");

    // Esc on the question is Keep editing; the form beneath doesn't see it.
    h.state_mut().1.clear();
    h.press_key(Key::Escape);
    h.run();
    assert!(sent(&h, DialogMsg::KeepEditing));
    assert!(!dismissed(&h, Dismiss::Escape));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn form_esc_and_backdrop_are_reported_not_closed() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "dialog-dismiss");
    st.modal = Some(Modal::NewTag(TagForm { name: "v1.0".into(), ..TagForm::default() }));
    let mut h = harness(st, true);
    h.run();
    h.press_key(Key::Escape);
    h.run();
    assert!(dismissed(&h, Dismiss::Escape));
    assert!(!closed(&h), "update decides");

    // Near the window's corner, well outside the 460 px dialog.
    h.state_mut().1.clear();
    click(&mut h, pos2(20.0, 800.0), PointerButton::Primary);
    assert!(dismissed(&h, Dismiss::Backdrop));
    assert!(!closed(&h));
    drop(h);

    let (mut st, _) = with_repo(ThemeChoice::Dark, "dialog-dismiss");
    st.modal = Some(Modal::About);
    let mut h = harness(st, true);
    h.run();
    click(&mut h, pos2(20.0, 800.0), PointerButton::Primary);
    assert!(dismissed(&h, Dismiss::Backdrop));
    drop(h);
    remove_fixture(&dir);
}

/// The confirmation for discarding README.md, app.txt (tracked) and notes/todo.md (untracked)
/// together, with deleting to the trash `trash` or not, as update opens it.
fn discard_files(name: &str, trash: bool) -> (glowfly_model::state::AppState, std::path::PathBuf) {
    let (mut st, dir) = super::snapshot_tests_line_counts::with_wip(ThemeChoice::Dark, name);
    st.settings.delete_to_trash = trash;
    let paths = ["README.md", "app.txt", "notes/todo.md"].map(String::from).to_vec();
    glowfly_model::update::update(&mut st, glowfly_model::state::files::discard_paths_msg(paths), std::time::Instant::now());
    assert!(matches!(&st.modal, Some(Modal::ConfirmDanger { title, .. }) if title == "Discard changes in 3 files"));
    (st, dir)
}

#[test]
fn dialog_discard_files() {
    let (st, dir) = discard_files("dialog-discard-files", true);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Discard 3 files").is_some());
    h.check_snapshot("dialog_discard_files_dark");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn dialog_discard_files_permanent() {
    let (st, dir) = discard_files("dialog-discard-files-perm", false);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("dialog_discard_files_permanent");
    drop(h);
    remove_fixture(&dir);
}

/// The confirmation for deleting the picked `paths` together, with deleting to the trash
/// `trash` or not, as update opens it.
fn delete_files(theme: ThemeChoice, name: &str, trash: bool, paths: &[&str]) -> (glowfly_model::state::AppState, std::path::PathBuf) {
    let (mut st, dir) = super::snapshot_tests_line_counts::with_wip(theme, name);
    st.settings.delete_to_trash = trash;
    let paths = paths.iter().map(|p| (*p).to_owned()).collect();
    glowfly_model::update::update(&mut st, glowfly_model::state::files::delete_paths_msg(paths), std::time::Instant::now());
    assert!(matches!(&st.modal, Some(Modal::ConfirmDanger { title, .. }) if title.starts_with("Delete ")), "{:?}", st.modal.is_some());
    (st, dir)
}

#[test]
fn dialog_delete_files_trash() {
    let (st, dir) = delete_files(ThemeChoice::Dark, "dialog-delete-files", true, &["README.md", "app.txt", "notes/todo.md"]);
    let mut h = harness(st, true);
    h.run();
    let button = format!("Move 3 files to the {}", glowfly_model::state::files::TRASH);
    assert!(h.query_by_label(&button).is_some());
    h.check_snapshot("dialog_delete_files_trash_dark");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn dialog_delete_files_permanent() {
    // search.txt is staged as new, so it leaves the staging area too.
    let (st, dir) = delete_files(ThemeChoice::Light, "dialog-delete-files-perm", false, &["app.txt", "search.txt"]);
    let mut h = harness(st, true);
    h.run();
    assert_eq!(h.get_all_by_label("Delete 2 files").count(), 2, "the title and the button");
    h.check_snapshot("dialog_delete_files_permanent_light");
    drop(h);
    remove_fixture(&dir);
}
