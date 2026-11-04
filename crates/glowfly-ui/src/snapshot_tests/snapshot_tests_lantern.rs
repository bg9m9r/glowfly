//! The HEAD lantern (issue #456): the one glow, round the checked-out commit's node with its
//! `lantern` rim, and this tab's WIP row's lit lantern, beside another worktree's WIP row (a
//! dashed ring, no glow) and a selected commit to judge the glow against. Dusk in Comfortable
//! rows, Dusk in Compact ones, and Porcelain with colour-blind safe colours.

use super::snapshot_tests::{git, harness_palette_sized, reload, remove_fixture, with_repo};
use super::snapshot_tests_worktrees::entry;
use super::theme;
use crate::state::graph_rows::{DisplayRow, GraphRows};
use crate::state::{AppState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::theme::color::Palette;
use eframe::egui::vec2;
use glowfly_core::repo::{ChangeCounts, LoadOptions};
use glowfly_core::settings::{ColourPalette, Density, ThemeChoice};
use std::path::PathBuf;

/// The fixture with HEAD on an ordinary commit (so the glow rings a 20 px disc) and the README
/// still changed, another worktree's changes at an older commit, and that commit selected.
fn with_lantern(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let d = "2026-01-11T10:00:00+00:00";
    std::fs::write(dir.join("notes.txt"), "notes\n").unwrap();
    git(&dir, &["add", "notes.txt"], d);
    git(&dir, &["commit", "-q", "-m", "Write the release notes"], d);
    reload(&mut st, &dir, &LoadOptions::all(500));
    let snap = st.snap.as_mut().unwrap();
    let (head, older) = (snap.head_oid, snap.commits[3].oid);
    let mut main = entry("C:/work/app", None, snap.head_branch.as_deref(), head);
    main.current = true;
    let mut login = entry("C:/work/app-worktrees/feature-login", Some("feature-login"), Some("feature/login"), Some(older));
    login.changes = ChangeCounts { modified: 2, added: 1, deleted: 0 };
    snap.worktrees = vec![main, login];
    snap.lay_out_graph();
    st.session.selection = Selection::Commit(older);
    (st, dir)
}

fn show(name: &str, st: AppState, p: Palette) {
    let mut h = harness_palette_sized(st, p, vec2(1200.0, 520.0));
    // The commit's details load in the background (a spinner): a fixed number of frames.
    h.run_steps(4);
    h.check_snapshot(name);
}

/// Only one display row is HEAD's, so a frame has one glow, with this tab's and another
/// worktree's WIP rows shown.
#[test]
fn one_row_holds_the_head_lantern() {
    let (st, dir) = with_lantern(ThemeChoice::Dark, "lantern-one");
    let snap = st.snap.as_ref().unwrap();
    let rows = GraphRows::new(snap, &st.session.search, false);
    let kinds: Vec<_> = (0..rows.len()).filter_map(|r| rows.at(r)).collect();
    assert!(kinds.contains(&DisplayRow::Wip) && kinds.iter().any(|k| matches!(k, DisplayRow::Worktree(_))));
    let head = kinds.iter().filter(|k| matches!(k, DisplayRow::Commit(i) if Some(snap.commits[*i].oid) == snap.head_oid)).count();
    assert_eq!(head, 1);
    remove_fixture(&dir);
}

#[test]
fn graph_lantern() {
    let (st, dir) = with_lantern(ThemeChoice::Dark, "lantern");
    assert!(st.snap.as_ref().unwrap().is_dirty(), "the WIP lantern is lit");
    show("graph_lantern_dusk", st, theme::DUSK);
    let (mut st, _) = with_lantern(ThemeChoice::Dark, "lantern");
    st.settings.graph_density = Density::Compact;
    show("graph_lantern_compact_dusk", st, theme::DUSK);
    let (mut st, _) = with_lantern(ThemeChoice::Porcelain, "lantern");
    st.settings.colour_palette = ColourPalette::ColourBlind;
    show("graph_lantern_colour_safe_porcelain", st, theme::applied(theme::PORCELAIN, ColourPalette::ColourBlind));
    remove_fixture(&dir);
}
