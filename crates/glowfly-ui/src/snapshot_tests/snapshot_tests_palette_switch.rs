//! The command palette's Switch branch step over the main window: two local branches, a
//! remote-only branch and a tag, labelled without "Checkout ", and the click it turns into the
//! main list's checkout message. Shares the fixture repository and renderer with
//! `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;
use glowfly_model::msg::Msg;
use glowfly_model::msg::PaletteMsg;
use glowfly_model::state::palette::{Mode, Palette, PaletteCommand, Pick};

#[test]
fn palette_switch_branch() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-switch-branch");
    let snap = st.snap.as_mut().unwrap();
    let oid = snap.head_oid.unwrap();
    snap.remotes = vec![("origin".into(), vec![("feature/sidebar".into(), oid)])];
    let pk = Pick::checkouts(snap);
    st.palette = Some(Palette { mode: Mode::Pick(pk), ..Default::default() });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("palette_switch_branch");
    // The local rows share their names with the sidebar's branch rows.
    for row in ["origin/feature/sidebar", "tag v0.1.0"] {
        assert!(h.query_by_label(row).is_some(), "{row}");
    }
    assert!(h.query_by_label("Checkout origin/feature/sidebar").is_none(), "no leading Checkout");
    h.get_by_label("origin/feature/sidebar").click();
    h.run();
    let remote = |m: &Msg| {
        matches!(m, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::CheckoutRemote { remote, branch }))
            if remote == "origin" && branch == "feature/sidebar")
    };
    assert!(h.state().1.iter().any(remote));
    drop(h);
    remove_fixture(&dir);
}

/// D16: a remote branch too long for its row is cut in its middle, with the query's matches in
/// both the part kept at its start and the part kept at its end still lit; its whole name is
/// its accessible name and shows on hover (a pixel golden).
#[test]
fn palette_switch_long_name() {
    let long = "worktree-agent-a3f9c1d2-refresh-the-sidebar-rows-so-names-that-share-a-start-stay-apart-in-every-list-zq";
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-switch-long");
    let snap = st.snap.as_mut().unwrap();
    let oid = snap.head_oid.unwrap();
    snap.remotes = vec![("origin".into(), vec![(long.into(), oid)])];
    let mut pk = Pick::checkouts(snap);
    let query = "origzq";
    pk.refresh(query);
    let full = format!("origin/{long}");
    let hit = pk.ranked().and_then(|r| r.hits.first()).expect("the long branch matches");
    assert_eq!(pk.label(hit.index), full);
    let n = full.chars().count();
    assert!(hit.matched.iter().any(|&i| i < 4) && hit.matched.iter().any(|&i| i >= n - 2), "{:?}", hit.matched);
    st.palette = Some(Palette { query: query.into(), mode: Mode::Pick(pk), ..Default::default() });
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(&full).is_some(), "named in full");
    h.get_by_label(&full).hover();
    for _ in 0..4 {
        h.step();
    }
    h.run_ok();
    assert!(h.get_all_by_label(&full).count() >= 2, "the whole name shows in its tooltip");
    h.check_snapshot("palette_switch_long_name");
    drop(h);
    remove_fixture(&dir);
}
