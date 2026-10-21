//! Stash and carry on: the Uncommitted changes prompt (a rebase in dark; a hard reset in light,
//! with Discard and reset in place of Always stash), the STASHES rows of an auto-stash and of
//! git's own "autostash" with their auto marker, and the merge banner's line while the
//! auto-stash waits.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_conflict_files::{fill_lists, merging};
use super::snapshot_tests_worktrees::pushed;
use crate::msg::{AutoStashMsg, Msg};
use crate::state::auto_stash::{AutoStashAsk, AutoStashJob, PendingStash, Phase};
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::StashInfo;
use glowfly_core::settings::ThemeChoice;
use glowfly_core::stash_ops::AutoStashFor;

fn asking(st: &mut AppState, op: AutoStashFor, files: usize) {
    st.modal = Some(Modal::AutoStash(AutoStashAsk { op, files, always: false, retry: Box::new(Msg::CloseModal) }));
}

#[test]
fn auto_stash_prompt() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "auto-stash-prompt");
    asking(&mut st, AutoStashFor::Rebase { branch: "login-page".into(), onto: "main".into() }, 3);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("auto_stash_prompt");
    // The title shares its words with the graph's WIP row.
    assert!(h.query_all_by_label("Uncommitted changes").count() >= 2);
    for text in ["Always stash", "Cancel", "You can change this in Preferences > General."] {
        assert!(h.query_by_label(text).is_some(), "{text}");
    }
    assert!(h.query_by_label("Discard and reset").is_none());
    h.get_by_label("Always stash").click();
    h.run();
    assert!(matches!(&h.state().0.modal, Some(Modal::AutoStash(a)) if a.always), "the box ticks");
    h.get_by_label("Stash and rebase").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::AutoStash(AutoStashMsg::Stash))));
    assert!(!pushed(&h, &|m| matches!(m, Msg::CloseModal)), "update takes the dialog with the answer");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn auto_stash_prompt_hard_reset() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "auto-stash-prompt-hard");
    asking(&mut st, AutoStashFor::HardReset { branch: "main".into(), to: "1a2b3c4".into() }, 1);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("auto_stash_prompt_hard_reset");
    assert!(h.query_by_label("Always stash").is_none(), "a hard reset always asks");
    h.get_by_label("Discard and reset").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::AutoStash(AutoStashMsg::Discard))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn stashes_auto_marker() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "auto-stash-marker");
    let snap = st.snap.as_mut().unwrap();
    let oid = |i: usize| snap.commits[i].oid;
    snap.stashes = vec![
        StashInfo { index: 0, message: "On main: Auto-stash before checking out login-page".into(), oid: oid(0) },
        StashInfo { index: 1, message: "On main: login form tweaks".into(), oid: oid(1) },
        StashInfo { index: 2, message: "autostash".into(), oid: oid(2) },
    ];
    st.view.sections.local = false;
    st.view.sections.remote = false;
    st.view.sections.stashes = true;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("stashes_auto_marker");
    // The marker is painted beside the row, which keeps the stash's whole message as its name.
    assert!(h.query_by_label("On main: Auto-stash before checking out login-page").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn merge_banner_auto_stash_note() {
    let (mut st, dir) = merging("merge-banner-auto-stash", false);
    fill_lists(&mut st, &dir);
    let op = AutoStashFor::Merge { from: "list-side".into(), into: "main".into() };
    let message = op.message();
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    let oid = st.snap.as_ref().unwrap().head_oid;
    let job = AutoStashJob { op, message, before: Vec::new() };
    st.auto_stash.pending.push(PendingStash { workdir, job, oid, phase: Phase::Ended { from: 0 }, told: true });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("merge_banner_auto_stash_note");
    assert!(h.query_by_label_contains("Auto-stash before merging list-side into main").is_some(), "the banner names the stash");
    drop(h);
    remove_fixture(&dir);
}
