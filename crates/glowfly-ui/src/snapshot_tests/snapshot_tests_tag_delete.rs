//! The Delete tag dialog over the whole window: the question, a checkbox per remote (none
//! ticked), the weak note, the danger button counting the ticked remotes and the message it
//! sends, the button waiting while a job runs, Enter pressing Cancel, Esc closing it, and the
//! dialog without remotes. Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_remotes::running;
use super::snapshot_tests_worktrees::{H, pushed};
use crate::snapshot_check::CheckSnapshot;
use eframe::egui::Key;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use glowfly_model::msg::{DialogMsg, Msg};
use glowfly_model::state::tag_delete::DeleteTagForm;
use glowfly_model::state::{AppState, Modal};
use glowfly_model::tag::TagMsg;
use std::path::PathBuf;

const TAG: &str = "v0.1.0";
const NOTE: &str =
    "A remote without this tag is skipped. Undo brings back only the local tag, and anyone who already fetched it keeps their copy.";

/// The fixture with `remotes` and the Delete tag dialog open for its tag.
fn dialog(name: &str, remotes: &[&str]) -> (AppState, PathBuf, git2::Oid) {
    let (mut st, dir) = with_repo(ThemeChoice::Light, name);
    let snap = st.snap.as_mut().unwrap();
    snap.remotes = remotes.iter().map(|r| ((*r).to_owned(), Vec::new())).collect();
    let form = DeleteTagForm::new(snap, TAG).expect("the fixture's tag");
    let at = form.at;
    st.modal = Some(Modal::DeleteTag(form));
    (st, dir, at)
}

fn closed(h: &H) -> bool {
    pushed(h, &|m| matches!(m, Msg::CloseModal | Msg::Dialog(DialogMsg::Dismiss(_))))
}

fn deletes(h: &H) -> bool {
    pushed(h, &|m| matches!(m, Msg::Tag(TagMsg::Delete { .. })))
}

#[test]
fn tag_delete_dialog() {
    let (st, dir, at) = dialog("tag-delete-dialog", &["origin", "backup"]);
    let mut h = harness(st, false);
    h.run();
    assert!(h.query_by_label("Delete local tag v0.1.0?").is_some());
    assert!(h.query_by_label("Also delete it from:").is_some());
    assert!(h.query_by_label(NOTE).is_some());
    assert!(!h.get_by_label("Delete").is_disabled());
    h.check_snapshot("tag_delete_dialog");

    // Enter presses Cancel, as in every danger confirmation.
    h.press_key(Key::Enter);
    h.run();
    assert!(closed(&h) && !deletes(&h), "Enter cancels");
    h.state_mut().1.clear();

    h.get_by_role_and_label(Role::CheckBox, "backup").click();
    h.run();
    assert!(matches!(&h.state().0.modal, Some(Modal::DeleteTag(f)) if f.ticked() == ["backup"]));
    let button = "Delete here and on 1 remote";
    assert!(h.query_by_label(button).is_some());
    h.get_by_label(button).click();
    h.run();
    let want = TagMsg::Delete { tag: TAG.into(), at, remotes: vec!["backup".into()] };
    assert!(pushed(&h, &|m| matches!(m, Msg::Tag(t) if *t == want)));
    // Update closes the dialog once the job starts.
    assert!(!pushed(&h, &|m| matches!(m, Msg::CloseModal)));

    h.state_mut().1.clear();
    h.press_key(Key::Escape);
    h.run();
    assert!(closed(&h), "Esc closes it");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tag_delete_dialog_no_remotes() {
    let (st, dir, _) = dialog("tag-delete-no-remotes", &[]);
    let mut h = harness(st, false);
    h.run();
    assert!(h.query_by_label("Delete local tag v0.1.0?").is_some());
    assert!(h.query_by_label("Also delete it from:").is_none() && h.query_by_label(NOTE).is_none());
    h.check_snapshot("tag_delete_dialog_no_remotes");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tag_delete_dialog_waits_for_a_job() {
    let (mut st, dir, _) = dialog("tag-delete-busy", &["origin"]);
    running(&mut st);
    let mut h = harness(st, false);
    h.run_steps(2);
    assert!(h.get_by_label("Delete").is_disabled(), "a job is running");
    h.get_by_label("Delete").click();
    h.run_steps(2);
    assert!(!deletes(&h));
    drop(h);
    remove_fixture(&dir);
}
