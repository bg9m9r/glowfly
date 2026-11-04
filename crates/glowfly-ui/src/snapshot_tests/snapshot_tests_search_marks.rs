//! How a highlighting search marks the graph (design pass D14): matching rows get the accent
//! bar at their left edge and the find fill behind "login", in the row's full ink; the other
//! rows keep their message in opaque `weak`. Rendered in Light and High contrast light too,
//! beside `snapshot_tests_search`'s dark `graph_search_dim`.

use super::snapshot_tests::{harness, harness_palette, remove_fixture};
use super::snapshot_tests_painted::fills_at;
use super::snapshot_tests_search::{searching, with_two_authors};
use crate::snapshot_check::CheckSnapshot;
use crate::theme::{self, color};
use eframe::egui::epaint::TextShape;
use eframe::egui::{Shape, pos2};
use egui_kittest::Harness;
use glowfly_core::settings::ThemeChoice;
use glowfly_model::msg::Msg;
use glowfly_model::state::{AppState, Selection};

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// The text shapes `h`'s last frame painted, as (text, shape).
fn texts(h: &H) -> Vec<TextShape> {
    fn walk(s: &Shape, out: &mut Vec<TextShape>) {
        match s {
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            Shape::Text(t) => out.push(t.clone()),
            _ => {}
        }
    }
    let mut out = Vec::new();
    h.output().shapes.iter().for_each(|c| walk(&c.shape, &mut out));
    out
}

/// A state searching "login" with the merge of `login-page` selected.
fn scene(theme: ThemeChoice, name: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = with_two_authors(theme, name);
    searching(&mut st, "login");
    let oid = st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == "Merge branch 'login-page'").unwrap().oid;
    st.session.selection = Selection::Commit(oid);
    (st, dir)
}

/// Checks the marks: every summary with "login" (two rows) has the find fill under the match
/// and an accent bar at its row's left edge; "Write release notes" is in opaque `weak`.
fn check_marks(h: &H) {
    let pal = color::current();
    let shapes = texts(h);
    let marked: Vec<&TextShape> =
        shapes.iter().filter(|t| t.galley.text().to_lowercase().contains("login")).filter(|t| t.pos.x > 300.0).collect();
    let rows: Vec<_> = marked.iter().filter(|t| t.galley.job.sections.len() > 1).collect();
    assert!(rows.len() >= 2, "both matching summaries ink their match: {}", marked.len());
    for t in rows {
        let row = &t.galley.rows[0];
        let start = t.galley.text().to_lowercase().find("login").unwrap();
        let glyph = &row.glyphs[t.galley.text()[..start].chars().count() + 2];
        let y = t.pos.y + t.galley.size().y / 2.0;
        let under = *fills_at(h, pos2(t.pos.x + glyph.pos.x, y)).last().unwrap();
        let beside = *fills_at(h, pos2(t.pos.x - 6.0, y)).last().unwrap();
        assert_ne!(under, beside, "{}: the find fill shows", t.galley.text());
        assert_eq!(under.a(), 255, "the find fill is opaque");
        assert!((0..300).any(|x| fills_at(h, pos2(x as f32, y)).contains(&pal.accent)), "{}: the accent bar", t.galley.text());
    }
    let quiet = shapes.iter().find(|t| t.galley.text() == "Write release notes").expect("the dimmed row's summary");
    let ink = quiet.galley.job.sections[0].format.color;
    assert_eq!(ink, pal.weak, "a dimmed row's message is opaque weak");
}

#[test]
fn graph_search_dim_marks() {
    let (st, dir) = scene(ThemeChoice::Dark, "search-marks");
    let mut h = harness(st, true);
    h.run_steps(4);
    check_marks(&h);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_search_dim_light() {
    let (st, dir) = scene(ThemeChoice::Light, "search-dim-light");
    let mut h = harness(st, false);
    h.run_steps(4);
    check_marks(&h);
    h.check_snapshot("graph_search_dim_light");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_search_dim_high_contrast_light() {
    let (st, dir) = scene(ThemeChoice::HighContrastLight, "search-dim-hc");
    let mut h = harness_palette(st, theme::HIGH_CONTRAST_LIGHT);
    h.run_steps(4);
    check_marks(&h);
    h.check_snapshot("graph_search_dim_high_contrast_light");
    drop(h);
    remove_fixture(&dir);
}
