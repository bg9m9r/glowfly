//! History edits from the graph: the multi-selection menu's squash and drop, the Reword and
//! Squash dialogs, and the rebase banner on the WIP row. Shares the fixture repository with
//! `snapshot_tests`.

use super::snapshot_tests::{git, harness, reload, remove_fixture, with_repo};
use super::snapshot_tests_select::{click, row_y};
use crate::msg::{Msg, RebaseMsg};
use crate::state::rebase_quick::RewordForm;
use crate::state::selection::MultiSelection;
use crate::state::{AppState, Modal, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use git2::{Oid, RepositoryState};
use std::path::PathBuf;
use trellis_core::rebase::{self, QuickEdit, RebaseProgress, Resume, Stop, StopKind};
use trellis_core::repo::{FileEntry, LoadOptions};
use trellis_core::settings::ThemeChoice;

/// The fixture with three more commits on main after the merge, newest first in the result, so
/// main's tip is a linear run. `clean` restores the modified README first.
fn with_linear(theme: ThemeChoice, name: &str, clean: bool) -> (AppState, PathBuf, [Oid; 3]) {
    let (mut st, dir) = with_repo(theme, name);
    let d = "2026-01-05T10:00:00+00:00";
    if clean {
        git(&dir, &["checkout", "--", "README.md"], d);
    }
    for (i, (file, message)) in
        [("notes.txt", "Add release notes"), ("todo.txt", "Track open tasks"), ("style.txt", "Tidy styles")].into_iter().enumerate()
    {
        std::fs::write(dir.join(file), format!("{message}\n")).unwrap();
        let date = format!("2026-01-{:02}T10:00:00+00:00", 11 + i);
        git(&dir, &["add", file], &date);
        git(&dir, &["commit", "-q", "-m", message], &date);
    }
    reload(&mut st, &dir, &LoadOptions::all(500));
    let snap = st.snap.as_ref().unwrap();
    let oids = [snap.commits[0].oid, snap.commits[1].oid, snap.commits[2].oid];
    (st, dir, oids)
}

fn select(st: &mut AppState, oids: &[Oid]) {
    st.selection = MultiSelection::build(oids.iter().copied(), oids[0], st.snap.as_ref().unwrap());
}

#[test]
fn multi_commit_menu_edit() {
    let (mut st, dir, [c3, c2, c1]) = with_linear(ThemeChoice::Dark, "multi-edit", false);
    select(&mut st, &[c3, c2]);
    let mut h = harness(st, true);
    h.run();
    // Row 0 is the WIP row (the README is modified).
    click(&mut h, pos2(700.0, row_y(1)), PointerButton::Secondary);
    h.check_snapshot("multi_commit_menu_edit");
    assert!(!h.get_by_label("Squash 2 commits").is_disabled());
    assert!(!h.get_by_label("Drop 2 commits…").is_disabled());
    h.state_mut().1.clear();
    h.get_by_label("Squash 2 commits").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Rebase(RebaseMsg::Edit(QuickEdit::Squash(o))) if *o == [c3, c2])));

    // Not one after another: only Drop.
    select(&mut h.state_mut().0, &[c3, c1]);
    h.run();
    click(&mut h, pos2(700.0, row_y(1)), PointerButton::Secondary);
    assert!(h.get_by_label("Squash 2 commits").is_disabled());
    assert!(!h.get_by_label("Drop 2 commits…").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

/// The Reword dialog for the middle of the three new commits.
fn reword(theme: ThemeChoice, name: &str, clean: bool) -> (AppState, PathBuf) {
    let (mut st, dir, [_, c2, _]) = with_linear(theme, name, clean);
    let plan = rebase::prepare_edit(&dir, &QuickEdit::Reword(c2)).unwrap();
    st.modal = Some(Modal::RewordCommit(Box::new(RewordForm::new(dir.clone(), plan))));
    (st, dir)
}

#[test]
fn reword_dialog() {
    let (st, dir) = reword(ThemeChoice::Dark, "reword-dialog", true);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("reword_dialog");
    assert!(h.get_by_label("Reword").is_disabled(), "the message is unchanged");
    h.get_by_label("Reword").click();
    h.run();
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Rebase(RebaseMsg::SubmitReword))));
    if let Some(Modal::RewordCommit(f)) = h.state_mut().0.modal.as_mut() {
        f.draft = "Track open tasks and owners".into();
    }
    h.run();
    h.get_by_label("Reword").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Rebase(RebaseMsg::SubmitReword))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn reword_dialog_dirty_light() {
    let (st, dir) = reword(ThemeChoice::Light, "reword-dirty", false);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("reword_dialog_dirty_light");
    assert!(h.query_by_label("Stash and reword").is_some());
    drop(h);
    remove_fixture(&dir);
}

/// The Squash dialog for the two newest of the three new commits.
fn squash(theme: ThemeChoice, name: &str, clean: bool) -> (AppState, PathBuf) {
    let (mut st, dir, [c3, c2, _]) = with_linear(theme, name, clean);
    let plan = rebase::prepare_edit(&dir, &QuickEdit::Squash(vec![c3, c2])).unwrap();
    st.modal = Some(Modal::RewordCommit(Box::new(RewordForm::new(dir.clone(), plan))));
    (st, dir)
}

#[test]
fn squash_dialog() {
    let (st, dir) = squash(ThemeChoice::Dark, "squash-dialog", true);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("squash_dialog");
    assert!(!h.get_by_label("Squash").is_disabled(), "the joined messages may be kept as they are");
    h.get_by_label("Squash").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Rebase(RebaseMsg::SubmitReword))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn squash_dialog_dirty_light() {
    let (st, dir) = squash(ThemeChoice::Light, "squash-dirty", false);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("squash_dialog_dirty_light");
    assert!(h.query_by_label("Stash and squash").is_some());
    drop(h);
    remove_fixture(&dir);
}

/// The fixture with an interactive rebase stopped at "Add app skeleton" (step 2 of 4).
fn rebasing(theme: ThemeChoice, name: &str, kind: StopKind, conflicted: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    st.selection = Selection::Wip;
    let snap = st.snap.as_mut().unwrap();
    let skeleton = snap.commits.iter().find(|c| c.summary == "Add app skeleton").unwrap().oid;
    snap.state = RepositoryState::RebaseInteractive;
    let stop = Stop { kind, oid: Some(skeleton) };
    let autostash = kind == StopKind::Conflict;
    snap.rebase =
        Some(RebaseProgress { step: 2, total: 4, stop, summary: "Add app skeleton".into(), autostash, interactive: true, command: None });
    if conflicted {
        snap.unstaged.push(FileEntry::new("app.txt", '!'));
    }
    (st, dir)
}

#[test]
fn rebase_banner_conflict() {
    let (st, dir) = rebasing(ThemeChoice::Dark, "banner-conflict", StopKind::Conflict, true);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("rebase_banner_conflict");
    assert!(h.get_by_label("Continue").is_disabled(), "conflicted files remain");
    assert!(!h.get_by_label("Skip commit").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn rebase_banner_edit_light() {
    let (st, dir) = rebasing(ThemeChoice::Light, "banner-edit", StopKind::Edit, false);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("rebase_banner_edit_light");
    h.get_by_label("Continue").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Rebase(RebaseMsg::Resume(Resume::Continue)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn rebase_banner_command() {
    let (mut st, dir) = rebasing(ThemeChoice::Dark, "banner-command", StopKind::Command, false);
    st.snap.as_mut().unwrap().rebase.as_mut().unwrap().command = Some("cargo test --workspace".into());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("rebase_banner_command");
    h.get_by_label(crate::state::rebase_banner::EDIT_STEPS).click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Rebase(RebaseMsg::OpenRemaining))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_stopped_sha_selects_its_commit() {
    let (st, dir) = rebasing(ThemeChoice::Dark, "banner-link", StopKind::Edit, false);
    let skeleton = st.snap.as_ref().unwrap().rebase.as_ref().unwrap().stop.oid.unwrap();
    let mut h = harness(st, true);
    h.run();
    h.get_by_label(&skeleton.to_string()[..7]).click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Repo(crate::msg::RepoMsg::JumpTo(o)) if *o == skeleton)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn no_step_editing_for_a_plain_rebase() {
    let (mut st, dir) = rebasing(ThemeChoice::Dark, "banner-plain", StopKind::Conflict, true);
    st.snap.as_mut().unwrap().state = RepositoryState::RebaseMerge;
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(crate::state::rebase_banner::EDIT_STEPS).is_none());
    drop(h);
    remove_fixture(&dir);
}
