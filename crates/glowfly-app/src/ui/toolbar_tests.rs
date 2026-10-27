//! The restyled toolbar in the fixture window: its spacing (2 px within a group, none before a
//! caret, 18 px from a button to the next group's), the pickers' long-name snapshots, tooltips for a
//! focused button and near the window's edge, the reasons of a busy toolbar's dropdown items,
//! and snapshots of the bar and of the rich tooltips. Motion is reduced so a tooltip settles at
//! once.

use crate::state::AppState;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::snapshot_tests_remotes::with_remotes;
use crate::ui::snapshot_tests::snapshot_tests_worktrees::H;
use crate::ui::snapshot_tests::{NOW, harness, harness_sized, remove_fixture, with_repo};
use eframe::egui;
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::{StashInfo, Tracking};
use glowfly_core::settings::ThemeChoice;
use glowfly_core::undo::Action;
use std::path::PathBuf;

fn settle(h: &mut H) {
    for _ in 0..4 {
        h.step();
    }
    h.run();
}

/// The node labelled `label`'s left and right edges.
pub(super) fn span(h: &H, label: &str) -> (f64, f64) {
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
    assert_eq!(gap(&h, "Undo the last action", "Redo the undone action"), 2.0, "within a group");
    assert_eq!(gap(&h, "Pull the current branch", "More pull actions"), 0.0, "a caret touches its button");
    assert_eq!(gap(&h, "Redo the undone action", "Pull the current branch"), 18.0, "between groups");
    assert_eq!(gap(&h, "More push actions", "Create a branch"), 18.0, "between groups");
    assert_eq!(gap(&h, "Create a branch", "Stash all changes"), 2.0);
    assert_eq!(gap(&h, "Pop the latest stash", "Show the terminal panel"), 18.0, "between groups");
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
fn busy_dropdown_items_say_why() {
    let (mut st, dir) = plain(ThemeChoice::Dark, "toolbar-busy-menus");
    crate::state::toolbar_tips::tests::running(&mut st);
    let busy = glowfly_core::i18n::Lang::En.tr("commit.blocked.job_running");
    // A running job shows a spinner, which keeps repainting.
    let mut h = harness(st, true);
    h.run_steps(4);
    h.get_by_label("More pull actions").click();
    h.run_steps(4);
    let fetch = h.get_by_label("Fetch all");
    assert!(fetch.is_disabled());
    assert_eq!(fetch.description().as_deref(), Some(busy));
    h.get_by_label("More push actions").click();
    h.run_steps(4);
    for label in ["Force push (with lease)", "Publish to GitHub…"] {
        let item = h.get_by_label(label);
        assert!(item.is_disabled(), "{label}");
        assert_eq!(item.description().as_deref(), Some(busy), "{label}");
    }
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn a_tooltip_at_the_edge_stays_in_the_window() {
    // About the narrowest window the toolbar's groups fit in with captions.
    let (st, dir) = plain(ThemeChoice::Dark, "toolbar-edge");
    let mut h = harness_sized(st, true, egui::vec2(1240.0, 600.0));
    h.run();
    hover(&mut h, "Open settings");
    let body = h.get_by_label("Themes, accounts, keys and git options.").bounding_box().expect("the tooltip");
    assert!(body.x0 >= 0.0 && body.x1 <= 1240.0 && body.y1 <= 600.0, "{body:?}");
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
    let message = snap.commits[snap.row_of[&to]].summary.to_string();
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
    let name = glowfly_core::i18n::Lang::De.tr("toolbar.push.name");
    let mut h = harness(st, true);
    h.run();
    hover(&mut h, name);
    h.check_snapshot("tip_push_german");
    drop(h);
    remove_fixture(&dir);
}

/// The fixture as `plain`, changed by `case` (a name, the head, the chip, the hosting service).
pub(super) fn variant(case: usize, labels: bool, name: &str) -> (AppState, PathBuf) {
    use crate::state::ForgeState;
    use crate::state::conflict_check::{ConflictReport, current_request};
    use glowfly_core::conflict_check::Prediction;
    use glowfly_core::forge::{Provider, Target};
    let (mut st, dir) = plain(ThemeChoice::Dark, name);
    st.settings.toolbar_labels = labels;
    if case == 4 {
        // On search, which the chip compares with main.
        crate::ui::snapshot_tests::git(&dir, &["checkout", "-q", "search"], "2026-01-10T10:00:00+00:00");
        crate::ui::snapshot_tests::reload(&mut st, &dir, &glowfly_core::repo::LoadOptions::all(500));
    }
    let long_branch = "feature/".to_owned() + &"a-very-long-branch-name-".repeat(3) + "that-goes-on";
    let snap = st.snap.as_mut().unwrap();
    match case {
        0 => {}
        1 | 4 => {
            // An 80-character head branch (with the chip in case 4).
            let long = format!("{long_branch:-<80}");
            let head = snap.locals.iter_mut().find(|b| b.is_head).unwrap();
            head.name.clone_from(&long);
            snap.head_branch = Some(long);
            if case == 4 {
                let request = current_request(snap).expect("the long branch compares with main");
                let generation = st.session.reads.conflict_checks.start().unwrap();
                let result = Ok(Prediction::Conflicts(vec!["a.txt".into(); 12]));
                st.session.reads.conflict_checks.finish(generation, ConflictReport { request, result });
            }
        }
        2 => snap.head_branch = None,
        3 => snap.name = "r".repeat(60),
        _ => {
            let github = Target {
                provider: Provider::GitHub,
                host: "github.com".into(),
                owner: "o".into(),
                project: String::new(),
                repo: "r".into(),
            };
            st.forge = Some(ForgeState::new(Some(github), "origin".into(), None));
        }
    }
    (st, dir)
}

#[test]
fn toolbar_right_group_stays_put() {
    const REFRESH: &str = "Refresh the repository";
    for labels in [true, false] {
        let mut first = None;
        for case in 0..6 {
            let (st, dir) = variant(case, labels, &format!("toolbar-put-{case}-{labels}"));
            let mut h = harness(st, true);
            h.run();
            let at = span(&h, REFRESH).0;
            assert_eq!(*first.get_or_insert(at), at, "case {case}, labels {labels}");
            drop(h);
            remove_fixture(&dir);
        }
    }
}

#[test]
fn toolbar_long_branch_name() {
    let (st, dir) = variant(1, true, "toolbar-long-branch");
    let mut h = harness(st, true);
    settle(&mut h);
    h.check_snapshot("toolbar_long_branch_name");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn toolbar_pickers_short() {
    let (mut st, dir) = plain(ThemeChoice::Dark, "toolbar-pickers-short");
    st.snap.as_mut().unwrap().name = "ui".into();
    let mut h = harness(st, true);
    settle(&mut h);
    h.check_snapshot("toolbar_pickers_short");
    drop(h);
    remove_fixture(&dir);
}

/// Folder's right edge and Refresh's left edge (the end of the middle group and the start of
/// the right-hand one), in a window `width` wide with captions, in `lang`, for `variant`'s
/// `case`.
fn group_edges(case: usize, lang: &str, width: f32) -> (f64, f64) {
    let (mut st, dir) = variant(case, true, &format!("toolbar-narrow-{case}-{lang}-{width}"));
    st.settings.language = lang.into();
    let t = st.settings.lang();
    let names = ["toolbar.folder.name", "toolbar.refresh.name"].map(|k| t.tr(k));
    let mut h = harness_sized(st, true, egui::vec2(width, 600.0));
    h.run();
    let edges = (span(&h, names[0]).1, span(&h, names[1]).0);
    drop(h);
    remove_fixture(&dir);
    edges
}

#[test]
fn a_narrow_window_narrows_the_pickers_before_the_groups_meet() {
    // 1280 px: a maximized window on a 1920 px screen at 150%; German captions need 1330 px.
    for (lang, width) in [("en", 1280.0), ("en", 1250.0), ("de", 1330.0)] {
        for case in 0..6 {
            let (folder, refresh) = group_edges(case, lang, width);
            assert!(refresh - folder >= 18.0, "{lang} {width}, case {case}: Folder ends at {folder}, Refresh starts at {refresh}");
        }
    }
}
