//! Date tooltips in the graph: hovering a DATE cell shows the full date (and its age while dates
//! aren't relative); hovering elsewhere in the row doesn't. Shares the fixture repository and
//! renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_select::row_y;
use crate::msg::Msg;
use crate::state::AppState;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Event, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// The full date and time of the first commit (the row below WIP): the tooltip's first line.
fn first_commit_verbose(st: &AppState) -> String {
    let c = &st.snap.as_ref().unwrap().commits[0];
    st.settings.date_tooltip(c.time, st.view.now, 0).lines().next().unwrap().to_owned()
}

/// Rests the pointer on the first commit row, `dx` px right of the date column's grip.
fn hover_first_commit(h: &mut H, dx: f32) {
    let b = h.get_by_label("Resize date column").bounding_box().expect("the grip has bounds");
    let at = pos2(b.x0 as f32 + dx, row_y(1));
    h.input_mut().events.push(Event::PointerMoved(at));
    for _ in 0..4 {
        h.step();
    }
}

/// The tooltip label holding `verbose`, as its text.
fn tip_text(h: &H, verbose: &str) -> Option<String> {
    h.query_by_label_contains(verbose).map(|n| n.value().or_else(|| n.label()).unwrap_or_default())
}

#[test]
fn graph_date_tooltip() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "date-tip");
    let expected = first_commit_verbose(&st);
    let mut h = harness(st, true);
    h.run();
    hover_first_commit(&mut h, 30.0);
    let tip = tip_text(&h, &expected).expect("the date cell shows its full date on hover");
    assert!(tip.ends_with(" ago") || tip.ends_with("yesterday"), "and its age: {tip:?}");
    h.check_snapshot("graph_date_tooltip");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_date_tooltip_relative_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "date-tip-relative");
    st.settings.relative_dates = true;
    let expected = first_commit_verbose(&st);
    let mut h = harness(st, false);
    h.run();
    hover_first_commit(&mut h, 30.0);
    let tip = tip_text(&h, &expected).expect("the date cell shows its full date on hover");
    assert_eq!(tip, expected, "the cell already shows the age, so the tip doesn't repeat it");
    h.check_snapshot("graph_date_tooltip_relative_light");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_row_without_date_tip() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "date-tip-message");
    let expected = first_commit_verbose(&st);
    let mut h = harness(st, true);
    h.run();
    hover_first_commit(&mut h, -200.0);
    assert!(tip_text(&h, &expected).is_none(), "only the date cell has the date tooltip");
    drop(h);
    remove_fixture(&dir);
}
