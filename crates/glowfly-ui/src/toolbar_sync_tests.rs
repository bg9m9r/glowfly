//! Pull and Push against the head's upstream (`toolbar_tests::tracking`) at 0/0, 2/0, 0/3, 2/3
//! and without an upstream: the accessible names that say the counts (Pull's, Push's and the
//! branch picker's), the push menu's first item naming where Push goes with the "Push to"
//! submenu for the fixture's two remotes, and the toolbar with its badges in both default themes.

use super::tests::{settle, tracking};
use crate::msg::{JobMsg, Msg};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{harness, remove_fixture};
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;

const UPSTREAM: &str = "Add app skeleton";

#[test]
fn the_names_say_the_counts() {
    let cases = [
        (0, 0, "Pull the current branch", "Push the current branch", "main"),
        (2, 0, "Pull the current branch", "Push, 2 ahead of origin/main", "main, 2 ahead"),
        (0, 3, "Pull, 3 behind origin/main", "Push the current branch", "main, 3 behind"),
        (2, 3, "Pull, 3 behind origin/main", "Push, 2 ahead of origin/main", "main, 2 ahead, 3 behind"),
    ];
    for (ahead, behind, pull, push, branch) in cases {
        let (st, dir) = tracking(ThemeChoice::Dark, &format!("toolbar-sync-{ahead}-{behind}"), UPSTREAM, ahead, behind);
        let mut h = harness(st, true);
        h.run();
        for label in [pull, push, branch] {
            assert!(h.query_all_by_label(label).next().is_some(), "{ahead}/{behind}: {label}");
        }
        if (ahead, behind) == (0, 0) {
            assert!(h.query_by_label("main, 0 ahead").is_none() && h.query_by_label("Pull, 0 behind origin/main").is_none());
        }
        drop(h);
        remove_fixture(&dir);
    }
    // Without an upstream there is nothing to count against.
    let (mut st, dir) = tracking(ThemeChoice::Dark, "toolbar-sync-none", UPSTREAM, 2, 3);
    st.snap.as_mut().unwrap().locals.iter_mut().find(|b| b.name == "main").unwrap().upstream = None;
    let mut h = harness(st, true);
    h.run();
    for label in ["Pull the current branch", "Push the current branch", "main"] {
        assert!(h.query_all_by_label(label).next().is_some(), "no upstream: {label}");
    }
    for label in ["Pull, 3 behind origin/main", "Push, 2 ahead of origin/main", "main, 2 ahead, 3 behind"] {
        assert!(h.query_by_label(label).is_none(), "no upstream: {label}");
    }
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_push_menu_starts_with_where_push_goes() {
    let (st, dir) = tracking(ThemeChoice::Dark, "toolbar-sync-menu", UPSTREAM, 2, 0);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("More push actions").click();
    h.run();
    assert!(h.query_by_label("Push to").is_some(), "two remotes: the submenu");
    h.get_by_label("Push main to origin/main").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Job(JobMsg::Push { force: false }))), "it pushes as the button does");
    drop(h);
    remove_fixture(&dir);

    let (mut st, dir) = tracking(ThemeChoice::Dark, "toolbar-sync-menu-publish", UPSTREAM, 0, 0);
    let snap = st.snap.as_mut().unwrap();
    snap.locals.iter_mut().find(|b| b.name == "main").unwrap().upstream = None;
    snap.locals.iter_mut().find(|b| b.name == "main").unwrap().tracking = None;
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("More push actions").click();
    h.run();
    assert!(h.query_by_label("Publish main to origin (creates origin/main)").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn toolbar_ahead_behind() {
    for (theme, dark, name) in
        [(ThemeChoice::Dark, true, "toolbar_ahead_behind_dark"), (ThemeChoice::Light, false, "toolbar_ahead_behind_light")]
    {
        let (st, dir) = tracking(theme, name, UPSTREAM, 2, 3);
        let mut h = harness(st, dark);
        settle(&mut h);
        h.check_snapshot(name);
        drop(h);
        remove_fixture(&dir);
    }
}
