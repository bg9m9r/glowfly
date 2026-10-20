//! The dialog frame's look and its close button: the header's × does what Esc does (a form's
//! dismissal, Keep editing on the discard question, Cancel on the credential prompt), the guide
//! pane sits beside the form, stacks above it in a narrow window and grows with long German
//! steps, and the tinted fields, field messages, header and pane in the high-contrast and Sand
//! themes. Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, harness_palette, harness_sized, remove_fixture, with_repo};
use super::snapshot_tests_worktrees::{H, dialog_button, pushed, with_worktrees};
use super::theme::{self, color::Palette};
use crate::msg::{CredentialMsg, DialogMsg, Dismiss, Msg};
use crate::state::askpass::PendingPrompt;
use crate::state::submodules::add::AddForm;
use crate::state::touched::Touched;
use crate::state::worktree_manage::{LockForm, RemoveForm};
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::accesskit::{Rect, Role};
use eframe::egui::vec2;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::askpass::Prompt;
use trellis_core::i18n::Lang;
use trellis_core::settings::ThemeChoice;
use trellis_core::worktree_ops::Refusal;

const CLOSE: &str = "Close this dialog";

fn bounds(h: &H, label: &str) -> Rect {
    h.get_by_label(label).bounding_box().unwrap_or_else(|| panic!("{label} has bounds"))
}

/// The bounds of the text field named `label`.
fn field(h: &H, label: &str) -> Rect {
    let node = h.query_all_by_role_and_label(Role::TextInput, label).next().unwrap_or_else(|| panic!("the {label} field"));
    node.bounding_box().expect("the field has bounds")
}

/// The steps of the guide whose keys are `keys`, in English.
fn steps(keys: &[&'static str]) -> Vec<&'static str> {
    keys.iter().map(|k| Lang::En.tr(k)).collect()
}

fn new_branch(name: &str) -> Modal {
    Modal::NewBranch { start: None, name: name.into(), checkout: true, touched: Default::default() }
}

/// Any message from a dialog, or a close, other than `want`.
fn other_dialog_msg(h: &H, want: &DialogMsg) -> bool {
    pushed(h, &|m| matches!(m, Msg::Dialog(d) if d != want) || matches!(m, Msg::CloseModal))
}

#[test]
fn dialog_close_button_dismisses() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "dialog-close-button");
    st.modal = Some(new_branch(""));
    let mut h = harness(st, true);
    h.run();
    let close = h.get_by_label(CLOSE);
    assert_eq!(close.role(), Role::Button);
    close.click();
    h.run();
    let want = DialogMsg::Dismiss(Dismiss::CloseButton);
    assert!(pushed(&h, &|m| matches!(m, Msg::Dialog(d) if *d == want)), "the × reports a dismissal, as Esc does");
    assert!(!other_dialog_msg(&h, &want), "nothing else from the dialog");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn dialog_close_button_on_question_keeps_editing() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "dialog-close-question");
    st.modal = Some(new_branch("feature/login"));
    st.dialog.asking = true;
    let mut h = harness(st, true);
    h.run();
    // The question is drawn over the form, so its × comes last.
    h.get_all_by_label(CLOSE).last().expect("the question's close button").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Dialog(DialogMsg::KeepEditing))));
    assert!(!pushed(&h, &|m| matches!(m, Msg::Dialog(DialogMsg::Dismiss(_)))), "the form beneath doesn't see it");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn askpass_close_button_cancels() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "askpass-close-button");
    let prompt = Prompt::new("Password for 'https://ada@example.com': ", None);
    st.askpass.queue.push_back(PendingPrompt { id: 1, prompt, retry: false, rejected_before: false });
    let mut h = harness(st, true);
    h.run();
    h.get_by_label(CLOSE).click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Credentials(CredentialMsg::CancelCredentials))));
    assert!(!pushed(&h, &|m| matches!(m, Msg::Credentials(CredentialMsg::AnswerCredentials))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn dialog_guide_beside() {
    let (mut st, dir) = with_worktrees(ThemeChoice::Dark, "dialog-guide-beside");
    let wt = st.snap.as_ref().unwrap().worktrees[1].clone();
    st.modal = Some(Modal::LockWorktree(LockForm::new(&wt)));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("dialog_guide_beside");
    assert!(h.query_by_label("How it works").is_some());
    let steps = steps(&["dialog.guide.worktree_lock.step1", "dialog.guide.worktree_lock.step2", "dialog.guide.worktree_lock.step3"]);
    for s in &steps {
        assert!(h.query_by_label(s).is_some(), "{s}");
    }
    let (first, last) = (bounds(&h, steps[0]), bounds(&h, steps[2]));
    assert!(first.x0 > field(&h, "Reason").x1, "the pane is right of the form");
    let lock = dialog_button(&h, "Lock").bounding_box().expect("the Lock button has bounds");
    assert!(lock.y1 >= last.y1, "the footer lines up with the pane's bottom: {} < {}", lock.y1, last.y1);
    drop(h);
    remove_fixture(&dir);
}

fn add_submodule(fixture: &str, url: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, fixture);
    let mut form = AddForm::new(st.snap.as_ref().unwrap().workdir.clone());
    form.url = url.into();
    form.touched = Touched::all();
    st.modal = Some(Modal::AddSubmodule(form));
    (st, dir)
}

#[test]
fn dialog_guide_stacked() {
    let (st, dir) = add_submodule("dialog-guide-stacked", "");
    let mut h = harness_sized(st, true, vec2(820.0, 820.0));
    h.run();
    h.check_snapshot("dialog_guide_stacked");
    let last = bounds(&h, Lang::En.tr("dialog.guide.submodule_add.step4"));
    assert!(last.y1 < field(&h, "Repository URL").y0, "a narrow window stacks the pane above the form");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn dialog_guide_german() {
    let (mut st, dir) = with_worktrees(ThemeChoice::Dark, "dialog-guide-german");
    st.settings.language = "de".into();
    let wt = st.snap.as_ref().unwrap().worktrees.iter().find(|w| w.folder_name() == "feature-login").unwrap().clone();
    let mut form = RemoveForm::new(&wt);
    form.refusal = Some(Refusal::Changes(3));
    form.delete_branch = true;
    st.modal = Some(Modal::RemoveWorktree(form));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("dialog_guide_german");
    let last = bounds(&h, Lang::De.tr("dialog.guide.worktree_remove.step3"));
    let below = h.query_all_by_role(Role::Button).filter_map(|n| n.bounding_box()).map(|b| b.y0).fold(0.0, f64::max);
    assert!(below >= last.y1, "the footer stays below the long steps");
    drop(h);
    remove_fixture(&dir);
}

/// Add submodule with a touched URL error in the palette `p`.
fn look(name: &str, p: Palette) {
    let (st, dir) = add_submodule(&format!("dialog-look-{name}"), "-bad-url");
    let mut h = harness_palette(st, p);
    h.run();
    h.check_snapshot(&format!("dialog_look_{name}"));
    assert!(h.query_by_label("URLs can't start with a dash.").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn dialog_look_high_contrast_dark() {
    look("high_contrast_dark", theme::HIGH_CONTRAST_DARK);
}

#[test]
fn dialog_look_high_contrast_light() {
    look("high_contrast_light", theme::HIGH_CONTRAST_LIGHT);
}

#[test]
fn dialog_look_sand() {
    look("sand", theme::SAND);
}
