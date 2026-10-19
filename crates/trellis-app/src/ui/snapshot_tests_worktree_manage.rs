//! Removing, locking and pruning worktrees from the WORKTREES section: the row menu's Lock… /
//! Unlock, Remove… and Prune items (and when they are missing or disabled), the header's Prune
//! menu, the Remove dialog (plain with the branch ticked, force for changes, force for a lock on
//! a detached worktree) and the Lock dialog.

use super::snapshot_tests::{harness, remove_fixture};
use super::snapshot_tests_select::click;
use super::snapshot_tests_worktrees::{H, center, p, pushed, with_worktrees};
use crate::msg::Msg;
use crate::state::worktree_manage::{LockForm, RemoveForm};
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::worktree::WorktreeMsg;
use eframe::egui::{Key, PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::settings::ThemeChoice;
use trellis_core::worktree_ops::Refusal;

/// The fixture with the Remove dialog open for the worktree named `name`, changed by `edit`.
fn remove_dialog(theme: ThemeChoice, fixture: &str, name: &str, edit: impl FnOnce(&mut RemoveForm)) -> (AppState, PathBuf) {
    let (mut st, dir) = with_worktrees(theme, fixture);
    let wt = st.snap.as_ref().unwrap().worktrees.iter().find(|w| w.folder_name() == name).unwrap().clone();
    let mut form = RemoveForm::new(&wt);
    edit(&mut form);
    st.modal = Some(Modal::RemoveWorktree(form));
    (st, dir)
}

fn menu_at(h: &mut H, label: &str) {
    let at = center(h, label);
    click(h, at, PointerButton::Secondary);
}

fn escape(h: &mut H) {
    h.press_key(Key::Escape);
    h.run();
}

#[test]
fn worktree_remove_dialog() {
    let (st, dir) = remove_dialog(ThemeChoice::Dark, "worktree-remove", "feature-login", |f| f.delete_branch = true);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("worktree_remove_dialog");
    assert!(h.query_by_label("Also delete branch feature/login").is_some());
    h.get_by_label("Remove and delete branch").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Worktree(WorktreeMsg::Remove))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktree_remove_force_light() {
    let (st, dir) = remove_dialog(ThemeChoice::Light, "worktree-remove-force", "feature-login", |f| {
        f.refusal = Some(Refusal::Changes(3));
    });
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("worktree_remove_force_light");
    assert!(h.query_by_label_contains("It has 3 changed or untracked files.").is_some());
    assert!(h.query_by_label("Force remove").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktree_remove_locked() {
    let (mut st, dir) = with_worktrees(ThemeChoice::Dark, "worktree-remove-locked");
    let hotfix = st.snap.as_mut().unwrap().worktrees.iter_mut().find(|w| w.folder_name() == "hotfix").unwrap();
    hotfix.locked = Some("on usb".into());
    let form = RemoveForm::new(hotfix);
    st.modal = Some(Modal::RemoveWorktree(form));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("worktree_remove_locked");
    assert!(h.query_by_label_contains("This worktree is locked (on usb).").is_some());
    assert!(h.query_by_label_contains("Also delete branch").is_none(), "a detached worktree has no branch to delete");
    assert!(!h.get_by_label("Force remove").is_disabled());
    // While a job runs the button waits (a spinner keeps repainting, so step instead of run).
    h.state_mut().0.job = Some(crate::state::RunningJob {
        label: "Fetch".into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        then_pr: None,
        then_link: false,
        keeps_undo: true,
        fetches: false,
    });
    h.run_steps(2);
    assert!(h.get_by_label("Force remove").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktree_lock_dialog() {
    let (mut st, dir) = with_worktrees(ThemeChoice::Dark, "worktree-lock");
    let wt = st.snap.as_ref().unwrap().worktrees[1].clone();
    st.modal = Some(Modal::LockWorktree(LockForm::new(&wt)));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("worktree_lock_dialog");
    assert!(h.query_by_label("Lock worktree feature-login").is_some());
    h.get_by_label("Lock").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Worktree(WorktreeMsg::Lock))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktree_menu_manage() {
    let (st, dir) = with_worktrees(ThemeChoice::Dark, "worktree-menu-manage");
    let mut h = harness(st, true);
    h.run();
    menu_at(&mut h, "feature-login");
    assert!(!h.get_by_label("Lock…").is_disabled());
    assert!(h.query_by_label("Unlock").is_none());
    assert!(h.query_by_label("Prune missing worktrees").is_none(), "only on a missing row");
    assert!(!h.get_by_label("Move…").is_disabled());
    h.get_by_label("Remove…").click();
    h.run();
    let login = p("C:/work/app-worktrees/feature-login");
    assert!(pushed(&h, &|m| matches!(m, Msg::Worktree(WorktreeMsg::RemoveDialog(x)) if *x == login)));

    menu_at(&mut h, "old");
    assert!(h.query_by_label("Lock…").is_none());
    h.get_by_label("Unlock").click();
    h.run();
    let old = p("C:/work/app-worktrees/old");
    assert!(pushed(&h, &|m| matches!(m, Msg::Worktree(WorktreeMsg::Unlock(x)) if *x == old)));

    menu_at(&mut h, "gone");
    h.check_snapshot("worktree_menu_manage");
    assert!(!h.get_by_label("Prune missing worktrees").is_disabled());
    assert!(!h.get_by_label("Remove…").is_disabled(), "a missing worktree can be removed");
    escape(&mut h);

    menu_at(&mut h, "✔  app");
    assert!(h.query_by_label("Remove…").is_none() && h.query_by_label("Lock…").is_none(), "nothing for the main worktree");
    escape(&mut h);

    // The header's menu, 12 + 15 px above the first row's center.
    let row = center(&h, "✔  app");
    let header = pos2(row.x, row.y - 27.0);
    click(&mut h, header, PointerButton::Secondary);
    h.check_snapshot("worktree_header_menu");
    h.get_by_label("Prune missing worktrees").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Worktree(WorktreeMsg::Prune))));
    drop(h);

    // Nothing prunable, and feature-login open in another tab.
    let (mut st, _) = with_worktrees(ThemeChoice::Dark, "worktree-menu-manage");
    st.snap.as_mut().unwrap().worktrees.retain(|w| !w.missing);
    st.tabs.push(crate::state::tabs::Tab::unloaded(p("C:/work/app-worktrees/feature-login")));
    let mut h = harness(st, true);
    h.run();
    let row = center(&h, "✔  app");
    click(&mut h, pos2(row.x, row.y - 27.0), PointerButton::Secondary);
    assert!(h.get_by_label("Prune missing worktrees").is_disabled());
    escape(&mut h);
    menu_at(&mut h, "feature-login");
    assert!(!h.get_by_label("Remove…").is_disabled(), "Remove closes the other tab first");
    assert!(!h.get_by_label("Lock…").is_disabled());
    assert!(!h.get_by_label("Move…").is_disabled());
    escape(&mut h);
    drop(h);
    remove_fixture(&dir);
}
