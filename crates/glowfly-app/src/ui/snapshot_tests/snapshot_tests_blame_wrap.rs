//! The blame view's long lines: wrapped (the gutter text on a run's first visual rows, the
//! numbers on each line's first row, shading over every row), a find landing centred below the
//! wrapped lines, unwrapped and scrolled sideways under the fixed gutter, "Updating…" while it is
//! blamed again, and the Wrap lines toggle. Shares the fixture repository and renderer with
//! `snapshot_tests`.

use super::history_file::WRAP_LINES;
use super::snapshot_tests::{git, harness, remove_fixture, with_repo};
use crate::msg::{BlameMsg, Msg};
use crate::state::AppState;
use crate::state::blame::{BlameData, BlameView};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Event, Modifiers, MouseWheelUnit, pos2, vec2};
use egui_kittest::kittest::Queryable;
use glowfly_core::highlight::{self, SyntaxHint};
use glowfly_core::repo::{self, BlameOptions, LoadOptions};
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

const COMMENT: &str = "// Every reading is checked against the calibration table before it is stored, so a sensor that drifts \
                       out of range is flagged at once rather than at the end of the day when the report is built and sent out.";
const MESSAGE: &str = "    let note = \"The sensor reported a value outside the calibrated range; the reading was kept but marked for \
                       review, and the next three readings from the same sensor will be compared against it before use.\";";

/// The source: a short header, the long comment and the long string, then plenty of short lines.
fn source(comment: &str, message: &str) -> String {
    let mut s = format!(
        "use std::fmt;\n\n{comment}\nfn check(value: f64) -> bool {{\n{message}\n    println!(\"{{note}}\");\n    value < 10.0\n}}\n"
    );
    for i in 0..40 {
        s.push_str(&format!("\nfn reading_{i}() -> f64 {{\n    {i}.0\n}}\n"));
    }
    s
}

/// The fixture with src/sensor.rs added by Ada, its two long lines (3 and 5) rewritten by
/// Grace, blamed at HEAD into the view, with Wrap lines set to `wrap`.
fn with_long_lines(theme: ThemeChoice, name: &str, wrap: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    let write = |text: &str| std::fs::write(dir.join("src/sensor.rs"), text).unwrap();
    write(&source("// Readings are checked.", "    let note = \"out of range\";"));
    let d = "2026-01-11T10:00:00+00:00";
    git(&dir, &["add", "src/sensor.rs"], d);
    git(&dir, &["commit", "-q", "-m", "Add the sensor checks"], d);
    write(&source(COMMENT, MESSAGE));
    let by = "Grace Hopper <grace@example.com>";
    git(&dir, &["commit", "-q", "--author", by, "-m", "Explain the range check", "src/sensor.rs"], "2026-01-12T10:00:00+00:00");
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let blame = repo::blame_file(&dir, "src/sensor.rs", None, &BlameOptions::default()).unwrap();
    let spans = highlight::highlight_file(SyntaxHint { path: "src/sensor.rs", language: None }, &blame.lines);
    let mut view = BlameView::loading("src/sensor.rs".into(), None);
    view.loading = false;
    view.data = Some(BlameData::new(blame, spans));
    view.data_gen = 1;
    st.session.blame.view = Some(view);
    st.settings.blame.wrap_lines = wrap;
    (st, dir)
}

#[test]
fn blame_wrapped() {
    let (st, dir) = with_long_lines(ThemeChoice::Dark, "blame-wrapped", true);
    let b = &st.session.blame.view.as_ref().unwrap().data.as_ref().unwrap().blame;
    let runs: Vec<(usize, usize)> = b.runs.iter().take(4).map(|r| (r.start, r.len)).collect();
    assert_eq!(runs, [(0, 2), (2, 1), (3, 1), (4, 1)], "the long lines are Grace's");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("blame_wrapped");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn blame_wrapped_find() {
    let (mut st, dir) = with_long_lines(ThemeChoice::Dark, "blame-wrapped-find", true);
    let view = st.session.blame.view.as_mut().unwrap();
    view.find.query = "reading_20".into();
    view.find.hits = vec![89];
    view.land_on(89);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("blame_wrapped_find");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn blame_scrolled() {
    let (st, dir) = with_long_lines(ThemeChoice::Dark, "blame-scrolled", false);
    let mut h = harness(st, true);
    h.run();
    h.input_mut().events.push(Event::PointerMoved(pos2(900.0, 300.0)));
    h.input_mut().events.push(Event::MouseWheel { unit: MouseWheelUnit::Point, delta: vec2(-400.0, 0.0), modifiers: Modifiers::NONE });
    // The pointer stays over the code, which keeps asking to repaint for its hover.
    h.run_steps(6);
    h.check_snapshot("blame_scrolled");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn blame_updating() {
    let (mut st, dir) = with_long_lines(ThemeChoice::Dark, "blame-updating", false);
    st.session.blame.view.as_mut().unwrap().loading = true;
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Updating…").is_some());
    h.check_snapshot("blame_updating");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn wrap_lines_toggle_asks_to_wrap() {
    let (st, dir) = with_long_lines(ThemeChoice::Dark, "blame-wrap-toggle", false);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label(WRAP_LINES).click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Blame(BlameMsg::SetWrapLines(true)))));
    drop(h);
    remove_fixture(&dir);
}
