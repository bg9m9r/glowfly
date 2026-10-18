//! Moving and removing worktrees, part two: the Move worktree dialog (plain, and with a taken
//! folder's error under the field), the Remove dialog deleting the remote branch too and closing
//! another tab, and the context menu of another worktree's WIP row in the graph.

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_select::{click, row_y};
use super::snapshot_tests_worktree_wip::{commit_row, with_wips};
use super::snapshot_tests_worktrees::{p, pushed, with_worktrees};
use crate::msg::Msg;
use crate::state::worktree_manage::{RemoteDelete, RemoveForm};
use crate::state::worktree_move::MoveForm;
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::worktree::WorktreeMsg;
use eframe::egui::{PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::repo::Tracking;
use trellis_core::settings::ThemeChoice;

/// The worktrees fixture with `modal` built for the feature-login worktree.
fn with_dialog(theme: ThemeChoice, name: &str, modal: impl FnOnce(&trellis_core::repo::WorktreeInfo) -> Modal) -> (AppState, PathBuf) {
    let (mut st, dir) = with_worktrees(theme, name);
    let wt = st.snap.as_ref().unwrap().worktrees[1].clone();
    st.modal = Some(modal(&wt));
    (st, dir)
}

#[test]
fn worktree_move_dialog() {
    let (st, dir) = with_dialog(ThemeChoice::Dark, "worktree-move", |wt| {
        let mut form = MoveForm::new(wt);
        form.folder = p("C:/work/moved/feature-login").display().to_string();
        Modal::MoveWorktree(form)
    });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("worktree_move_dialog");
    assert!(h.query_by_label("Move worktree feature-login").is_some());
    h.get_by_label("Move").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Worktree(WorktreeMsg::Move))));
    drop(h);

    // Where it is now: Move waits for another folder.
    let (st, _) = with_dialog(ThemeChoice::Dark, "worktree-move", |wt| Modal::MoveWorktree(MoveForm::new(wt)));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("That is where it is now.").is_some());
    assert!(h.get_by_label("Move").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktree_move_taken_light() {
    let (st, dir) = with_dialog(ThemeChoice::Light, "worktree-move-taken", |wt| {
        let mut form = MoveForm::new(wt);
        form.folder = p("C:/work/elsewhere").display().to_string();
        let error = format!("Something is already at {}. Choose a folder that doesn't exist yet.", form.folder);
        form.error = Some((form.folder.clone(), error));
        Modal::MoveWorktree(form)
    });
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("worktree_move_taken_light");
    assert!(h.query_by_label_contains("Something is already at").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktree_remove_remote() {
    let (st, dir) = with_dialog(ThemeChoice::Dark, "worktree-remove-remote", |wt| {
        let mut form = RemoveForm::new(wt);
        let tracking = Tracking { remote: "origin".into(), merge: "feature/login".into() };
        form.remote = Some(RemoteDelete { tracking, blocked: None });
        form.delete_branch = true;
        form.delete_remote = true;
        Modal::RemoveWorktree(form)
    });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("worktree_remove_remote");
    assert!(h.query_by_label("Also delete feature/login on origin").is_some());
    assert!(h.query_by_label("feature/login is deleted on origin for everyone who uses it.").is_some());
    assert!(h.query_by_label("Remove and delete local and remote branch").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktree_remove_close_tab() {
    let (st, dir) = with_dialog(ThemeChoice::Dark, "worktree-remove-close-tab", |wt| {
        Modal::RemoveWorktree(RemoveForm { close_tabs: true, ..RemoveForm::new(wt) })
    });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("worktree_remove_close_tab");
    assert!(h.query_by_label("It is open in another tab. Remove closes that tab first (and any terminal in it).").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktree_wip_menu() {
    let (st, dir) = with_wips(ThemeChoice::Dark, "worktree-wip-menu");
    let row = commit_row(&st, 2) - 2;
    let mut h = harness(st, true);
    h.run();
    click(&mut h, pos2(700.0, row_y(row)), PointerButton::Secondary);
    h.check_snapshot("worktree_wip_menu");
    for item in ["Open in new tab", "Open folder", "Copy path", "Lock…", "Remove…"] {
        assert!(!h.get_by_label(item).is_disabled(), "{item}");
    }
    h.get_by_label("Move…").click();
    h.run();
    let login = p("C:/work/app-worktrees/feature-login");
    assert!(pushed(&h, &|m| matches!(m, Msg::Worktree(WorktreeMsg::MoveDialog(x)) if *x == login)));
    drop(h);
    remove_fixture(&dir);
}
