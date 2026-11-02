//! The command palette's Switch branch step over the main window: two local branches, a
//! remote-only branch and a tag, labelled without "Checkout ", and the click it turns into the
//! main list's checkout message. Shares the fixture repository and renderer with
//! `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::Msg;
use crate::msg::PaletteMsg;
use crate::state::palette::{Mode, Palette, PaletteCommand, Pick};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;

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
