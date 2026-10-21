//! The operation in progress: the strip under the toolbar, the merge banner with Commit merge and
//! Abort merge, and the danger confirmation Abort opens. Shares the fixtures of
//! `snapshot_tests` and `snapshot_tests_conflict_files`.

use super::super::snapshot_tests::{git, harness, remove_fixture, with_repo};
use super::super::snapshot_tests_conflict_files::{fill_lists, merging};
use crate::msg::{Msg, RepoMsg};
use crate::state::op_actions::state_actions;
use crate::state::op_confirm::banner_msg;
use crate::state::{AppState, Modal, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use git2::RepositoryState;
use glowfly_core::rebase::{RebaseProgress, Stop, StopKind};
use glowfly_core::repo::{self, LoadOptions};
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;
use std::time::Instant;

const DATE: &str = "2026-01-11T10:00:00+00:00";

/// The merge fixture (alpha.txt and beta.txt conflicted, gamma.txt resolved) with its lists read.
fn mid_merge(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = merging(name, false);
    fill_lists(&mut st, &dir);
    (st, dir)
}

#[test]
fn op_strip_merge() {
    let (mut st, dir) = mid_merge("op-strip-merge");
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    st.selection = Selection::Commit(head);
    st.settings.show_details_panel = false;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("op_strip_merge");
    for text in ["Merge in progress", "2 conflicted files left"] {
        assert!(h.query_by_label(text).is_some(), "{text}");
    }
    h.get_by_label("Show work in progress").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::ShowInProgress))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn op_strip_rebase_step() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "op-strip-rebase");
    let snap = st.snap.as_mut().unwrap();
    snap.state = RepositoryState::RebaseInteractive;
    let stop = Stop { kind: StopKind::Edit, oid: None };
    snap.rebase =
        Some(RebaseProgress { step: 2, total: 4, stop, summary: String::new(), autostash: false, interactive: true, command: None });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("op_strip_rebase_step");
    assert!(h.query_by_label("Interactive rebase in progress, step 2 of 4").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn wip_merge_banner() {
    let (st, dir) = mid_merge("wip-merge-banner");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("wip_merge_banner");
    assert!(h.get_all_by_label("Commit merge").all(|n| n.is_disabled()), "conflicts remain");
    assert!(!h.get_by_label("Abort merge").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn wip_merge_banner_ready() {
    let (mut st, dir) = merging("wip-merge-ready", true);
    std::fs::write(dir.join("alpha.txt"), (1..=10).map(|i| format!("alpha.txt {i}\n")).collect::<String>()).unwrap();
    git(&dir, &["add", "alpha.txt", "beta.txt"], DATE);
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    fill_lists(&mut st, &dir);
    let message = st.snap.as_ref().unwrap().merge_message.clone().expect("MERGE_MSG");
    assert_eq!(message, "Merge branch 'list-side'");
    st.draft.fill_merge(&message);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("wip_merge_banner_ready");
    assert_eq!(h.get_all_by_label("Commit merge").filter(|n| !n.is_disabled()).count(), 2, "the banner's and the commit button");
    assert!(h.query_by_label("All conflicts resolved").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn abort_merge_confirm() {
    let (mut st, dir) = mid_merge("abort-merge-confirm");
    let (label, args) = state_actions(RepositoryState::Merge).unwrap()[0];
    let ask = banner_msg(&st, label, args);
    crate::update::update(&mut st, ask, Instant::now());
    assert!(matches!(st.modal, Some(Modal::ConfirmDanger { .. })));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("abort_merge_confirm");
    // The dialog's button is drawn after the banner's of the same name.
    h.get_all_by_label("Abort merge").last().expect("the dialog's button").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::ConfirmAccepted)));
    drop(h);
    remove_fixture(&dir);
}
