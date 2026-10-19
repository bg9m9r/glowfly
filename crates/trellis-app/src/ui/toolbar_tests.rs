//! The restyled toolbar in the fixture window: its spacing (4 px within a group, none before a
//! caret, 16 px between groups), tooltips for a focused button and near the window's edge, and
//! snapshots of the bar and of the rich tooltips. Motion is reduced so a tooltip settles at once.

use crate::state::AppState;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{NOW, harness, harness_sized, remove_fixture, with_repo};
use crate::ui::snapshot_tests_remotes::with_remotes;
use crate::ui::snapshot_tests_worktrees::H;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::repo::{StashInfo, Tracking};
use trellis_core::settings::ThemeChoice;
use trellis_core::undo::Action;

fn settle(h: &mut H) {
    for _ in 0..4 {
        h.step();
    }
    h.run();
}

/// The node labelled `label`'s left and right edges.
fn span(h: &H, label: &str) -> (f64, f64) {
    let b = h.get_by_label(label).bounding_box().expect("bounds");
    (b.x0, b.x1)
}

fn hover(h: &mut H, label: &str) {
    h.get_by_label(label).hover();
    settle(h);
}

fn plain(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    st.settings.reduce_motion = true;
    (st, dir)
}

/// The fixture with origin/main at the commit `upstream` (a summary) as main's upstream, main
/// being `ahead` and `behind` it.
fn tracking(theme: ThemeChoice, name: &str, upstream: &str, ahead: usize, behind: usize) -> (AppState, PathBuf) {
    let (mut st, dir) = with_remotes(theme, name);
    st.settings.reduce_motion = true;
    let snap = st.snap.as_mut().unwrap();
    let at = snap.commits.iter().find(|c| c.summary == upstream).expect("the commit").oid;
    snap.remotes[0].1[0].1 = at;
    let main = snap.locals.iter_mut().find(|b| b.name == "main").unwrap();
    main.upstream = Some("origin/main".into());
    main.tracking = Some(Tracking { remote: "origin".into(), merge: "main".into() });
    (main.ahead, main.behind) = (ahead, behind);
    (st, dir)
}

#[test]
fn groups_are_spaced_evenly() {
    let (st, dir) = plain(ThemeChoice::Dark, "toolbar-gaps");
    let mut h = harness(st, true);
    h.run();
    let gap = |h: &H, a: &str, b: &str| span(h, b).0 - span(h, a).1;
    assert_eq!(gap(&h, "Undo the last action", "Redo the undone action"), 4.0, "within a group");
    assert_eq!(gap(&h, "Pull the current branch", "More pull actions"), 0.0, "a caret touches its button");
    assert_eq!(gap(&h, "Redo the undone action", "Pull the current branch"), 16.0, "between groups");
    assert_eq!(gap(&h, "More push actions", "Create a branch"), 16.0, "between groups");
    assert_eq!(gap(&h, "Create a branch", "Stash all changes"), 4.0);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn a_focused_button_shows_its_tooltip() {
    let (st, dir) = plain(ThemeChoice::Dark, "toolbar-focus");
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Nothing to undo").is_none());
    // Disabled buttons still take keyboard focus, so their reason can be read.
    h.get_by_label("Undo the last action").focus();
    settle(&mut h);
    assert!(h.query_by_label("Nothing to undo").is_some(), "Undo's tooltip");
    h.get_by_label("Pop the latest stash").focus();
    settle(&mut h);
    assert!(h.query_by_label("No stashes to pop").is_some(), "Pop's tooltip");
    assert!(h.query_by_label("Nothing to undo").is_none(), "one tooltip at a time");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn a_tooltip_at_the_edge_stays_in_the_window() {
    let (st, dir) = plain(ThemeChoice::Dark, "toolbar-edge");
    let mut h = harness_sized(st, true, egui::vec2(900.0, 600.0));
    h.run();
    hover(&mut h, "Open settings");
    let body = h.get_by_label("Themes, accounts, keys and git options.").bounding_box().expect("the tooltip");
    assert!(body.x0 >= 0.0 && body.x1 <= 900.0 && body.y1 <= 600.0, "{body:?}");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn toolbar_restyled() {
    for (theme, dark, name) in [(ThemeChoice::Dark, true, "toolbar_restyled_dark"), (ThemeChoice::Light, false, "toolbar_restyled_light")] {
        let (st, dir) = plain(theme, name);
        let mut h = harness(st, dark);
        settle(&mut h);
        h.check_snapshot(name);
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn toolbar_labels_hidden() {
    let (mut st, dir) = plain(ThemeChoice::Dark, "toolbar-labels-hidden");
    st.settings.toolbar_labels = false;
    let mut h = harness(st, true);
    settle(&mut h);
    h.check_snapshot("toolbar_labels_hidden");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tip_push_ahead() {
    let (st, dir) = tracking(ThemeChoice::Dark, "tip-push-ahead", "Add app skeleton", 3, 0);
    let mut h = harness(st, true);
    h.run();
    hover(&mut h, "Push the current branch");
    assert!(h.query_by_label("Sends 3 commits").is_some());
    h.check_snapshot("tip_push_ahead");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tip_pull_behind() {
    let (mut st, dir) = tracking(ThemeChoice::Dark, "tip-pull-behind", "Search results page", 0, 1);
    st.snap.as_mut().unwrap().fetched_at = Some(NOW - 120);
    let mut h = harness(st, true);
    h.run();
    hover(&mut h, "Pull the current branch");
    assert!(h.query_by_label("Last fetched 2 minutes ago").is_some());
    h.check_snapshot("tip_pull_behind");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tip_pop() {
    for (stashed, name) in [(true, "tip_pop"), (false, "tip_pop_disabled")] {
        let (mut st, dir) = plain(ThemeChoice::Dark, name);
        if stashed {
            let oid = st.snap.as_ref().unwrap().head_oid.unwrap();
            st.snap.as_mut().unwrap().stashes = vec![StashInfo { index: 0, message: "On main: half-done README".into(), oid }];
        }
        let mut h = harness(st, true);
        h.run();
        hover(&mut h, "Pop the latest stash");
        h.check_snapshot(name);
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn tip_undo_commit() {
    let (mut st, dir) = plain(ThemeChoice::Dark, "tip-undo-commit");
    let snap = st.snap.as_ref().unwrap();
    let to = snap.head_oid.unwrap();
    let from = snap.commits[snap.row_of[&to]].parents[0];
    let message = snap.commits[snap.row_of[&to]].summary.clone();
    st.undo.record(Action::Commit { branch: Some("main".into()), from, to, message, amend: false });
    let mut h = harness(st, true);
    h.run();
    hover(&mut h, "Undo the last action");
    assert!(h.query_by_label("Reverts").is_some());
    h.check_snapshot("tip_undo_commit");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tip_settings_keycaps() {
    let (st, dir) = plain(ThemeChoice::Light, "tip-settings-keycaps");
    let mut h = harness(st, false);
    h.run();
    hover(&mut h, "Open settings");
    h.check_snapshot("tip_settings_keycaps");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn tip_push_german() {
    let (mut st, dir) = tracking(ThemeChoice::Dark, "tip-push-german", "Add app skeleton", 3, 0);
    st.settings.language = "de".into();
    let name = trellis_core::i18n::Lang::De.tr("toolbar.push.name");
    let mut h = harness(st, true);
    h.run();
    hover(&mut h, name);
    h.check_snapshot("tip_push_german");
    drop(h);
    remove_fixture(&dir);
}
