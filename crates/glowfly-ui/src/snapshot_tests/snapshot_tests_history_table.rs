//! The history table: single-line rows (36 px, or 28 in Compact) banded in their lane's colour,
//! the AUTHOR and COMMIT (short SHA) columns, the right-aligned short ages, author and merge
//! nodes, soft ref chips with their marks and the "+N" chip with its list of every label, and
//! the selected and hovered rows. Shares the fixture repository with `snapshot_tests`.

use super::snapshot_tests::{harness, harness_palette, harness_sized, remove_fixture, render, with_repo};
use super::snapshot_tests_ref_sync::with_upstream;
use super::snapshot_tests_select::{click, row_y};
use crate::graph::row_h;
use crate::snapshot_check::CheckSnapshot;
use crate::snapshot_tests::snapshot_tests_painted::fills_at;
use crate::theme::{self, color};
use eframe::egui::{Event, PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::{RefKind, RefLabel};
use glowfly_core::settings::{ColumnWidths, Density, ThemeChoice};
use glowfly_model::state::AppState;
use std::time::Instant;

/// The fixture's search branch's tip (the WIP row is 0).
const SEARCH_ROW: usize = 1;
/// Where the "+N" chip sits in `history_labels_overflow`: after two chips in a 240 px column,
/// whose chips run from x 276 to about 460 (local chips have no mark).
const PLUS_X: f32 = 444.0;
/// The labels added to the search branch's tip in `history_labels_overflow`, as their tips read.
const EXTRA: [&str; 5] = ["feature-a", "feature-b", "origin/feature-c", "v0.2.0", "v0.3.0"];

#[test]
fn history_table_dark() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "history-table");
    assert_eq!((row_h(Density::Comfortable), row_h(Density::Compact)), (36.0, 28.0));
    render("history_table_dark", st, true);
    remove_fixture(&dir);
}

#[test]
fn history_table_light() {
    let (st, dir) = with_repo(ThemeChoice::Light, "history-table-light");
    render("history_table_light", st, false);
    remove_fixture(&dir);
}

#[test]
fn history_table_high_contrast_light() {
    let (st, dir) = with_repo(ThemeChoice::HighContrastLight, "history-table-hc");
    let mut h = harness_palette(st, theme::HIGH_CONTRAST_LIGHT);
    h.run();
    h.check_snapshot("history_table_high_contrast_light");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn history_table_compact_dark() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "history-compact");
    st.settings.graph_density = Density::Compact;
    render("history_table_compact_dark", st, true);
    remove_fixture(&dir);
}

/// How many rows fit a 1920 x 1080 window's graph at `density`: whole ones, and with the one
/// cut at the bottom.
fn rows_at_1080p(density: Density, name: &str) -> (usize, usize) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    st.settings.graph_density = density;
    let mut h = harness_sized(st, true, eframe::egui::vec2(1920.0, 1080.0));
    h.run();
    let (st, _): &mut (AppState, _) = h.state_mut();
    let whole = st.view.graph_fit_rows;
    let (_, height) = st.session.tab_view.graph_scroll;
    drop(h);
    remove_fixture(&dir);
    (whole, (height / row_h(density)).ceil() as usize)
}

#[test]
fn compact_rows_fill_a_1080p_window() {
    let (whole, shown) = rows_at_1080p(Density::Compact, "history-compact-1080");
    assert!(shown >= 34, "{whole} whole rows, {shown} shown");
    let (comfortable, _) = rows_at_1080p(Density::Comfortable, "history-comfortable-1080");
    assert!(whole > comfortable, "{whole} compact rows, {comfortable} comfortable");
}

fn label(full: &str, kind: RefKind) -> RefLabel {
    let name = full.splitn(3, '/').nth(2).unwrap_or(full).to_owned();
    RefLabel { name, full: full.into(), kind, is_head: false }
}

/// How many accessible labels read `text`.
fn named(h: &super::snapshot_tests_columns::H, text: &str) -> usize {
    h.query_all_by_label(text).count()
}

#[test]
fn history_labels_overflow() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "history-overflow");
    let snap = st.snap.as_mut().unwrap();
    let search = snap.locals.iter().find(|b| b.name == "search").and_then(|b| b.oid).unwrap();
    let labels = snap.refs.entry(search).or_default();
    labels.extend([
        label("refs/heads/feature-a", RefKind::Local),
        label("refs/heads/feature-b", RefKind::Local),
        label("refs/remotes/origin/feature-c", RefKind::Remote),
        label("refs/tags/v0.2.0", RefKind::Tag),
        label("refs/tags/v0.3.0", RefKind::Tag),
    ]);
    labels.sort_by(|a, b| (!a.is_head, a.kind, &a.name).cmp(&(!b.is_head, b.kind, &b.name)));
    st.settings.column_widths = ColumnWidths { refs: 240.0, ..ColumnWidths::default() };
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("history_labels_overflow");
    let before: Vec<usize> = EXTRA.iter().chain(&["search"]).map(|t| named(&h, t)).collect();
    h.input_mut().events.push(Event::PointerMoved(pos2(PLUS_X, row_y(SEARCH_ROW))));
    for _ in 0..4 {
        h.step();
    }
    h.check_snapshot("history_labels_overflow_tip");
    for (text, n) in EXTRA.iter().chain(&["search"]).zip(before) {
        assert_eq!(named(&h, text), n + 1, "the tooltip lists {text}");
    }
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn history_label_marks() {
    // In sync: one chip with the local and remote marks; the tag with its mark.
    let (st, dir, _) = with_upstream(ThemeChoice::Dark, "history-marks", true);
    render("history_label_marks", st, true);
    remove_fixture(&dir);
}

#[test]
fn history_row_selected_and_hovered() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "history-selected");
    let mut h = harness(st, true);
    h.run();
    click(&mut h, pos2(700.0, row_y(2)), PointerButton::Primary);
    let (st, msgs): &mut (AppState, _) = h.state_mut();
    for m in std::mem::take(msgs) {
        glowfly_model::update::update(st, m, Instant::now());
    }
    let merge = st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == "Merge branch 'login-page'").unwrap().oid;
    assert!(st.session.selection.includes(merge), "the click selected row 2");
    // Each row's fills, under the pointer's x.
    let (selected, hovered) = (pos2(700.0, row_y(2)), pos2(700.0, row_y(3)));
    assert!(!fills_at(&h, hovered).contains(&color::hover()), "row 3 is plain before the pointer comes");
    h.input_mut().events.push(Event::PointerMoved(hovered));
    for _ in 0..4 {
        h.step();
    }
    assert!(fills_at(&h, selected).contains(&color::selected()), "row 2 keeps the selection's fill");
    assert!(fills_at(&h, hovered).contains(&color::hover()), "row 3 takes the hover fill");
    assert!(!fills_at(&h, selected).contains(&color::hover()), "the selected row isn't lifted");
    drop(h);
    remove_fixture(&dir);
}
