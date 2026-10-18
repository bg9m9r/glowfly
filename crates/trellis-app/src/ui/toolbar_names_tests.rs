//! Accessible names of painted widgets and hint-only fields: every toolbar tool button by its
//! label (disabled exactly when it can't be used), graph rows by their summary ("// WIP" for
//! the working tree) and the commit box's fields by their hint. No image comparison.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use trellis_core::settings::ThemeChoice;
use trellis_core::undo::Action;

/// Whether the toolbar's button `label` is disabled; panics unless exactly one button has it.
fn disabled(h: &egui_kittest::Harness<'_, (crate::state::AppState, Vec<crate::msg::Msg>)>, label: &str) -> bool {
    let buttons: Vec<_> = h.query_all_by_role_and_label(Role::Button, label).collect();
    assert_eq!(buttons.len(), 1, "one {label:?} button");
    buttons[0].is_disabled()
}

#[test]
fn tool_buttons_are_named_and_report_whether_they_work() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "toolbar-names");
    let snap = st.snap.as_ref().unwrap();
    let (dirty, stashed) = (snap.is_dirty(), !snap.stashes.is_empty());
    let mut h = harness(st, true);
    h.run();
    for label in ["Pull", "Push", "Branch", "Folder", "Settings", "Search", "Activity", "Refresh"] {
        assert!(!disabled(&h, label), "{label}");
    }
    assert!(disabled(&h, "Undo") && disabled(&h, "Redo"), "nothing to undo or redo yet");
    assert_eq!(disabled(&h, "Stash"), !dirty);
    assert_eq!(disabled(&h, "Pop"), !stashed);
    drop(h);

    // With something to undo, Undo works.
    let (mut st, _) = with_repo(ThemeChoice::Dark, "toolbar-names");
    let snap = st.snap.as_ref().unwrap();
    let to = snap.head_oid.unwrap();
    let from = snap.commits[snap.row_of[&to]].parents[0];
    st.undo.record(Action::Commit { branch: Some("main".into()), from, to, message: "Last".into(), amend: false });
    let mut h = harness(st, true);
    h.run();
    assert!(!disabled(&h, "Undo") && disabled(&h, "Redo"));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_rows_and_commit_fields_are_named() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "graph-names");
    let summary = st.snap.as_ref().unwrap().commits[0].summary.clone();
    let mut h = harness(st, true);
    h.run();
    assert_eq!(h.query_all_by_role_and_label(Role::Button, "// WIP").count(), 1);
    assert!(h.query_all_by_role_and_label(Role::Button, &summary).next().is_some(), "{summary}");
    let field = h.get_by_role_and_label(Role::TextInput, "Commit summary");
    field.type_text("Name the fields");
    h.run();
    assert_eq!(h.state().0.draft.summary, "Name the fields");
    assert!(h.query_by_label("Description").is_some());
    drop(h);
    remove_fixture(&dir);
}
