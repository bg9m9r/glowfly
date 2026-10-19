//! The "Discard your changes?" question over an edited form, and what a form's Esc and backdrop
//! click send (update/dialog.rs decides). Shares the fixture repository and renderer with
//! `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::click;
use super::snapshot_tests_worktrees::{H, pushed};
use crate::msg::{DialogMsg, Dismiss, Msg};
use crate::state::Modal;
use crate::state::tags::TagForm;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Key, PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use trellis_core::settings::ThemeChoice;

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
    st.modal = Some(Modal::NewBranch { start: None, name: "feature/login".into(), checkout: true, touched: Default::default() });
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
