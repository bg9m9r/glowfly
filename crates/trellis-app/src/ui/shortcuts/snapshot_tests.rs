//! The shortcut sheet (whole and filtered), its empty state and Close button, and the commit
//! summary taking focus for Ctrl+Shift+M. Shares the fixture repository and renderer with
//! `ui::snapshot_tests`.

use crate::msg::Msg;
use crate::state::{Modal, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{harness, remove_fixture, with_repo};
use eframe::egui;
use egui_kittest::kittest::Queryable;
use trellis_core::settings::ThemeChoice;

#[test]
fn shortcut_sheet() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "shortcut-sheet");
    st.modal = Some(Modal::Shortcuts { filter: String::new() });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("shortcut_sheet");
    for label in ["Staging and committing", "Navigation", "Stage all files", "Go to the commit message"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    #[cfg(not(target_os = "macos"))]
    assert!(h.query_by_label("F7 or Alt+Down").is_some(), "the key caps are labelled with their keys");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn shortcut_sheet_filtered_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "shortcut-sheet-filtered");
    st.modal = Some(Modal::Shortcuts { filter: "stage".into() });
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("shortcut_sheet_filtered_light");
    assert!(h.query_by_label("Stage all files").is_some());
    assert!(h.query_by_label("New tab").is_none());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn shortcut_sheet_empty_and_close() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "shortcut-sheet-empty");
    st.modal = Some(Modal::Shortcuts { filter: "zzz".into() });
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("No shortcuts match").is_some());
    assert!(h.query_by_label("Stage all files").is_none());
    h.get_by_label("Close").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::CloseModal)));
    drop(h);

    // Esc closes it too, with the filter field focused.
    let (mut st, _) = with_repo(ThemeChoice::Dark, "shortcut-sheet-empty");
    st.modal = Some(Modal::Shortcuts { filter: String::new() });
    let mut h = harness(st, true);
    h.run();
    assert!(h.ctx.memory(|m| m.focused()).is_some(), "the filter has focus");
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::CloseModal)));
    h.press_key(egui::Key::Escape);
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::CloseModal)), "Esc closes the sheet");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn commit_summary_takes_focus() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "shortcut-focus-message");
    st.selection = Selection::Wip;
    st.view.focus_message = true;
    let mut h = harness(st, true);
    h.run();
    assert!(!h.state().0.view.focus_message, "the commit box took the request");
    assert!(h.ctx.memory(|m| m.focused()).is_some(), "the summary has focus");
    // Typing goes to the focused field.
    h.input_mut().events.push(egui::Event::Text("Add login".into()));
    h.run();
    assert_eq!(h.state().0.draft.summary, "Add login");
    drop(h);
    remove_fixture(&dir);
}

#[cfg(not(target_os = "macos"))]
#[test]
fn sheet_shows_rebound_keys() {
    use crate::keymap::{BindingId, Chord, KeyName};
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "shortcut-sheet-rebound");
    st.keymap.set(BindingId::Refresh, vec![Chord::cmd(KeyName::Letter('R'))]);
    st.keymap.set(BindingId::NewTab, Vec::new());
    st.modal = Some(Modal::Shortcuts { filter: String::new() });
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Ctrl+R").is_some(), "Refresh shows its new key");
    assert!(h.query_by_label("F5").is_none(), "and not the old one");
    assert!(h.query_by_label("No key").is_some(), "New tab has none");
    drop(h);
    remove_fixture(&dir);
}
