//! The command palette's Delete branch step over the main window: the local branches that can
//! be deleted first, the checked-out one last, dim, with the reason it can't be deleted as its
//! detail; a click on a deletable row; and a reason too long for its row, cut so the branch's
//! name stays, whole on hover; and its Delete remote branch step: the remote branches sorted by
//! remote and name, with the local branches tracking each as its detail. Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_palette_nav::palette_row;
use crate::snapshot_check::CheckSnapshot;
use eframe::egui::{Event, pos2};
use egui_kittest::kittest::Queryable;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Tracking;
use glowfly_core::settings::ThemeChoice;
use glowfly_model::msg::{Msg, PaletteMsg};
use glowfly_model::state::palette::{Mode, Palette, PaletteCommand, Pick};

#[test]
fn palette_delete_branch() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-delete-branch");
    let mut pk = Pick::branches_to_delete(Lang::En, st.snap.as_ref().unwrap());
    pk.refresh("");
    st.palette = Some(Palette { mode: Mode::Pick(pk), ..Default::default() });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("palette_delete_branch");
    assert!(h.query_by_label("main is checked out. Check out another branch before deleting it.").is_none(), "painted, not a label");
    palette_row(&h, "search").click();
    h.run();
    let chosen = |m: &Msg| matches!(m, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::DeleteBranch(b))) if b == "search");
    assert!(h.state().1.iter().any(chosen));
    drop(h);
    remove_fixture(&dir);
}

/// A reason wider than the row is cut at its end, so the branch's name still shows (its whole
/// name is the row's accessible name), and the whole reason shows on hover over it.
#[test]
fn a_long_reason_keeps_the_branch_name() {
    let long = "worktree-agent-a3f9c1d2-refresh-the-sidebar-rows-so-names-that-share-a-start-stay-apart-in-every-list-zq";
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-delete-branch-long");
    let snap = st.snap.as_mut().unwrap();
    snap.head_branch = Some(long.into());
    for b in snap.locals.iter_mut().filter(|b| b.is_head) {
        b.name = long.into();
    }
    let mut pk = Pick::branches_to_delete(Lang::En, snap);
    pk.refresh("");
    let reason = pk.blocked(2).expect("the checked-out branch is last, and blocked").to_owned();
    st.palette = Some(Palette { mode: Mode::Pick(pk), ..Default::default() });
    let mut h = harness(st, true);
    h.run();
    let row = palette_row(&h, long).raw_bounds().expect("the row is drawn");
    // The right part of the row, over the cut reason.
    let at = pos2((row.x1 - 60.0) as f32, ((row.y0 + row.y1) / 2.0) as f32);
    h.input_mut().events.push(Event::PointerMoved(at));
    for _ in 0..4 {
        h.step();
    }
    h.run_ok();
    assert!(h.query_by_label_contains("is checked out").is_some(), "the whole reason shows as a tooltip");
    assert!(h.get_all_by_label(&reason).count() >= 1);
    drop(h);
    remove_fixture(&dir);
}

/// The Delete remote branch step: every remote branch sorted by remote then name, the local
/// branches tracking one as its detail.
#[test]
fn palette_delete_remote_branch() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-delete-remote-branch");
    let snap = st.snap.as_mut().unwrap();
    let tip = |snap: &glowfly_core::repo::Snapshot, name: &str| snap.locals.iter().find(|b| b.name == name).and_then(|b| b.oid).unwrap();
    let (main, search) = (tip(snap, "main"), tip(snap, "search"));
    snap.remotes = vec![
        ("origin".into(), vec![("main".into(), main), ("search".into(), search), ("login-page".into(), main)]),
        ("fork".into(), vec![("search".into(), search)]),
    ];
    for name in ["search", "main"] {
        let b = snap.locals.iter_mut().find(|b| b.name == name).unwrap();
        b.tracking = Some(Tracking { remote: "origin".into(), merge: name.into() });
        b.upstream = Some(format!("origin/{name}"));
    }
    let mut pk = Pick::remote_branches_to_delete(snap);
    pk.refresh("");
    st.palette = Some(Palette { mode: Mode::Pick(pk), ..Default::default() });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("palette_delete_remote_branch");
    palette_row(&h, "origin/search").click();
    h.run();
    let chosen = |m: &Msg| {
        matches!(m, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::DeleteRemoteBranch { remote, branch }))
            if remote == "origin" && branch == "search")
    };
    assert!(h.state().1.iter().any(chosen));
    drop(h);
    remove_fixture(&dir);
}
