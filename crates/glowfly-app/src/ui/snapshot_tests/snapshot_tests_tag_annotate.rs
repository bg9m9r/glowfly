//! The Annotate dialog over the whole window: the remotes note, the required message, the
//! message its button and Ctrl+Enter send, Cancel, and the problem line when the tag has moved.
//! Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_worktrees::{H, pushed};
use crate::msg::Msg;
use crate::state::tags::AnnotateForm;
use crate::state::{AppState, Modal};
use crate::tag::TagMsg;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Event, Key, Modifiers};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;

const TAG: &str = "v0.1.0";

/// The commit the fixture's tag points at.
fn tagged_commit(st: &AppState) -> String {
    let snap = st.snap.as_ref().unwrap();
    snap.tags.iter().find(|(n, _)| n == TAG).map(|(_, o)| o.to_string()).expect("the fixture's tag")
}

fn dialog(at: &str, message: &str) -> Modal {
    Modal::AnnotateTag(AnnotateForm { tag: TAG.into(), at: at.into(), message: message.into() })
}

fn set_message(h: &mut H, text: &str) {
    if let Some(Modal::AnnotateTag(f)) = h.state_mut().0.modal.as_mut() {
        f.message = text.into();
    }
    h.run();
}

fn annotates(h: &H, at: &str, message: &str) -> bool {
    let want = TagMsg::Annotate { tag: TAG.into(), at: at.into(), message: message.into() };
    pushed(h, &|m| matches!(m, Msg::Tag(t) if *t == want))
}

#[test]
fn annotate_tag_dialog() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "annotate-tag-dialog");
    st.snap.as_mut().unwrap().remotes = vec![("origin".into(), Vec::new())];
    let at = tagged_commit(&st);
    st.modal = Some(dialog(&at, ""));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label_contains("Copies already pushed to remotes stay lightweight.").is_some());
    assert!(h.get_by_label("Annotate").is_disabled());
    h.check_snapshot("annotate_tag_dialog");

    set_message(&mut h, "Release 0.1.0");
    assert!(!h.get_by_label("Annotate").is_disabled());
    h.get_by_label("Annotate").click();
    h.run();
    assert!(annotates(&h, &at, "Release 0.1.0"));
    // Update closes the dialog once the job starts.
    assert!(!pushed(&h, &|m| matches!(m, Msg::CloseModal)));

    // The field has the keyboard; Ctrl+Enter sends without adding a line.
    h.state_mut().1.clear();
    set_message(&mut h, "");
    h.input_mut().events.push(Event::Text("Second".into()));
    h.run();
    h.press_key_modifiers(Modifiers::COMMAND, Key::Enter);
    h.run();
    assert!(annotates(&h, &at, "Second"));
    assert!(matches!(&h.state().0.modal, Some(Modal::AnnotateTag(f)) if f.message == "Second"));

    h.get_by_label("Cancel").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::CloseModal)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn annotate_tag_dialog_problem_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "annotate-tag-problem");
    let at = tagged_commit(&st);
    let moved = "0123456789abcdef0123456789abcdef01234567";
    assert_ne!(at, moved);
    st.modal = Some(dialog(moved, "Release 0.1.0"));
    let mut h = harness(st, false);
    h.run();
    let short = &at[..7];
    let text = format!("Tag {TAG} has moved to {short}. Open Annotate again from the TAGS list.");
    assert!(h.query_by_label(&text).is_some());
    assert!(h.query_by_label_contains("Copies already pushed").is_none(), "no remotes");
    assert!(h.get_by_label("Annotate").is_disabled());
    h.check_snapshot("annotate_tag_dialog_problem_light");
    drop(h);
    remove_fixture(&dir);
}
