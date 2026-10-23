//! The graph's row bands and selected row in Dusk and Dawn: branches pinned to warm hues (Orange,
//! Red) beside cool ones (Cyan, Lime), main in the gold, and a selected commit on main
//! (docs/COLOUR.md, Rules, "Tinting a surface").

use super::snapshot_tests::{git, harness_palette_sized, reload, remove_fixture, with_repo};
use super::theme;
use crate::state::{AppState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::vec2;
use glowfly_core::repo::LoadOptions;
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

#[path = "snapshot_tests_colour_sheet.rs"]
mod colour_sheet;

/// The fixture with the four pinned branches, a commit on main selected.
pub(super) fn with_bands(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    // (branch, pinned slot, commits): Orange, Red, Lime and Cyan in the standard palette.
    let branches = [("tidy-me", 3u8, 3), ("stepwise", 8, 2), ("lime-work", 1, 2), ("cyan-work", 4, 2)];
    for (i, (b, _, n)) in branches.iter().enumerate() {
        for k in 0..*n {
            let date = format!("2026-01-{:02}T{:02}:00:00+00:00", 11 + i, 10 + k);
            if k == 0 {
                git(&dir, &["checkout", "-q", "-b", b, "main"], &date);
            }
            let file = format!("{b}-{k}.txt");
            std::fs::write(dir.join(&file), b).unwrap();
            git(&dir, &["add", &file], &date);
            git(&dir, &["commit", "-q", "-m", &format!("Work on {b}, step {}", k + 1)], &date);
        }
    }
    git(&dir, &["checkout", "-q", "main"], "2026-01-20T10:00:00+00:00");
    reload(&mut st, &dir, &LoadOptions::all(500));
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    for (b, slot, _) in branches {
        st.settings.set_branch_pin(&real, b, Some(slot));
    }
    let snap = st.snap.as_mut().unwrap();
    snap.set_colouring(st.settings.colouring_for(&real));
    let head = snap.head_oid.expect("HEAD");
    // The commit under main's tip, so main's own row shows its band.
    let below = snap.commits[snap.row_of[&head]].parents[0];
    st.selection = Selection::Commit(below);
    (st, dir)
}

#[test]
fn graph_bands_dusk_and_dawn() {
    for (name, p) in [("graph_bands_dusk", theme::DUSK), ("graph_bands_dawn", theme::DAWN)] {
        let (st, dir) = with_bands(&name.replace('_', "-"));
        // Wide enough for the messages beside the graph.
        let mut h = harness_palette_sized(st, p, vec2(1600.0, 820.0));
        // The commit's details load in the background (a spinner): a fixed number of frames.
        h.run_steps(4);
        h.check_snapshot(name);
        drop(h);
        remove_fixture(&dir);
    }
}
