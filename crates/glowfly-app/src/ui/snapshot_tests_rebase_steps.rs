//! The rebase panel's exec and break rows, a selected row, a squash group's combined message
//! and its editor, the command editor, and the panel on a running rebase's remaining steps.

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_rebase::with_panel;
use crate::msg::{Msg, RebaseMsg};
use crate::state::AppState;
use crate::state::rebase::{Editing, RebasePanel};
use crate::state::rebase_steps::NewStep;
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use git2::RepositoryState;
use glowfly_core::rebase::{RebaseAction::*, RebaseProgress, Remaining, Stop, StopKind, TodoStep};
use glowfly_core::settings::ThemeChoice;

fn panel(st: &mut AppState) -> &mut RebasePanel {
    st.rebase.as_mut().unwrap()
}

#[test]
fn rebase_panel_steps() {
    let (mut st, dir) = with_panel(ThemeChoice::Dark, "rebase-steps", &[Pick, Pick, Squash, Fixup, Pick, Pick]);
    let p = panel(&mut st);
    p.items[1].commit_mut().unwrap().combined_message = Some("Build the login flow\n\nOne commit for the page and its checks.".into());
    p.items.insert(1, TodoStep::Exec("cargo test --workspace".into()));
    p.items.insert(6, TodoStep::Break);
    p.selected = Some(5);
    assert_eq!(st.rebase.as_ref().unwrap().start_blocker(st.snap.as_ref().unwrap(), false), None);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("rebase_panel_steps");
    assert!(h.query_by_label("exec cargo test --workspace").is_some() && h.query_by_label("break").is_some());
    assert!(h.query_by_label("Add exec step").is_some() && h.query_by_label("Add break step").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn rebase_panel_combined_editor() {
    let (mut st, dir) = with_panel(ThemeChoice::Light, "rebase-combined", &[Pick, Pick, Squash, Squash, Pick, Pick]);
    let p = panel(&mut st);
    p.open_editor(1);
    assert_eq!(p.editing, Some(Editing::Combined(1)));
    assert_eq!(p.editor_row(), Some(3));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("rebase_panel_combined_editor");
    assert!(h.query_by_label("Use git's message").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn rebase_panel_exec_editor() {
    let (mut st, dir) = with_panel(ThemeChoice::Dark, "rebase-exec-editor", &[Pick, Pick, Pick, Pick, Pick, Pick]);
    panel(&mut st).add_step(Some(2), NewStep::Exec);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("rebase_panel_exec_editor");
    assert!(h.get_by_label("Save").is_disabled(), "an empty command can't be saved");
    h.get_by_label("Save").hover();
    h.run();
    h.run();
    assert!(h.query_by_label("Type a command first.").is_some(), "Save says why it is off");
    // The editor's Cancel, on Save's row (the panel has its own Cancel too).
    let save = h.get_by_label("Save").bounding_box().unwrap();
    let cancel = h.query_all_by_label("Cancel").find(|n| n.bounding_box().is_some_and(|r| r.y0 < save.y1 && r.y1 > save.y0));
    cancel.expect("the editor's Cancel").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Rebase(RebaseMsg::RebaseCancelMessage))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn rebase_panel_remaining() {
    let (mut st, dir) = with_panel(ThemeChoice::Dark, "rebase-remaining", &[Pick, Pick, Squash, Pick, Pick, Pick]);
    let items = std::mem::take(&mut panel(&mut st).items);
    let snap = st.snap.as_mut().unwrap();
    snap.state = RepositoryState::RebaseInteractive;
    let stop = Stop { kind: StopKind::Edit, oid: items[1].oid() };
    snap.rebase =
        Some(RebaseProgress { step: 2, total: 6, stop, summary: String::new(), autostash: false, interactive: true, command: None });
    let mut p = RebasePanel::loading_remaining();
    p.fill_remaining(Remaining {
        branch: Some("main".into()),
        onto_label: "1a2b3c4 Initial commit".into(),
        done: 2,
        locked: vec![items[2].clone()],
        locked_text: "squash x\n".into(),
        steps: items[3..].to_vec(),
        first_may_combine: true,
        todo: String::new(),
    });
    p.add_step(Some(0), NewStep::Break);
    st.rebase = Some(p);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("rebase_panel_remaining");
    assert!(h.query_by_label("Update steps").is_some());
    assert!(h.query_by_label(crate::state::rebase_remaining::LOCKED_NOTE).is_some());
    drop(h);
    remove_fixture(&dir);
}
