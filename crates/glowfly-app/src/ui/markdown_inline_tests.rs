//! Markdown text runs laid out (`markdown_inline.rs`): links open only over https, a space or
//! punctuation after any styled run keeps its gap (an italic run's slant used to cover it), text
//! keeps off a code run's background, and a run that starts with a space and wraps starts its
//! new row at the left edge.

use super::super::markdown::body;
use super::super::markdown_cache::MarkdownCache;
use super::super::theme::fonts;
use crate::msg::Msg;
use eframe::egui::{self, Pos2, Rect, Shape, vec2};
use egui_kittest::kittest::Queryable;

#[test]
fn an_https_link_opens_in_the_browser_and_others_do_nothing() {
    let mut cache = MarkdownCache::default();
    let src = "[docs](https://example.com/docs) and [local](a.md)";
    let mut h = crate::ui::snapshot_check::builder()
        .build_ui_state(move |ui, msgs: &mut Vec<Msg>| body(ui, "t", src, &mut cache, msgs), Vec::new());
    h.run();
    h.get_by_label("docs").click();
    h.run();
    assert!(matches!(h.state().as_slice(), [Msg::OpenUrl(u)] if u == "https://example.com/docs"));
    h.state_mut().clear();
    h.get_by_label("local").click();
    h.run();
    assert!(h.state().is_empty());
}

/// One painted row of text: its text, where it is drawn (y), and where its ink (glyphs, and a
/// code run's background) starts and ends.
#[derive(Debug)]
struct Ink {
    text: String,
    y: f32,
    left: f32,
    right: f32,
}

/// The rows of text `src` paints as a Markdown body `width` wide, in paint order (after a pass
/// that installs the bundled fonts).
fn ink(src: &str, width: f32) -> Vec<Ink> {
    let ctx = egui::Context::default();
    let input = || egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 400.0))), ..Default::default() };
    fonts::install(&ctx);
    let _ = ctx.run(input(), |_| {});
    let mut cache = MarkdownCache::default();
    let out = ctx.run(input(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| body(ui, "t", src, &mut cache, &mut Vec::new()));
    });
    fn walk(s: &Shape, out: &mut Vec<Ink>) {
        match s {
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            Shape::Text(t) => {
                for row in &t.galley.rows {
                    let bounds = row.visuals.mesh_bounds;
                    let text = row.glyphs.iter().map(|g| g.chr).collect();
                    let (left, right) = if bounds.is_positive() { (bounds.min.x, bounds.max.x) } else { (f32::NAN, f32::NAN) };
                    out.push(Ink { text, y: t.pos.y + row.rect.min.y, left: t.pos.x + left, right: t.pos.x + right });
                }
            }
            _ => {}
        }
    }
    let mut rows = Vec::new();
    out.shapes.iter().for_each(|s| walk(&s.shape, &mut rows));
    rows
}

/// The space between the ink of the run "try" and of the text after it, in `src`.
fn gap_after_try(src: &str) -> f32 {
    let rows = ink(src, 400.0);
    let at = rows.iter().position(|r| r.text == "try").unwrap_or_else(|| panic!("{src:?}: no run \"try\" in {rows:?}"));
    let next = rows.get(at + 1).unwrap_or_else(|| panic!("{src:?}: nothing after \"try\" in {rows:?}"));
    next.left - rows[at].right
}

#[test]
fn a_space_after_any_styled_run_keeps_its_gap() {
    // Plain "try the" in one label leaves 3 points between the ink of "y" and "t"; before the
    // fix the slant of an italic "y" reached the "t" (0 points).
    for src in [
        "*try* the",
        "_try_ the",
        "**try** the",
        "***try*** the",
        "`try` the",
        "~~try~~ the",
        "[try](https://x.test) the",
        "[*try*](https://x.test) the",
        "[try](a.md) the",
        "![try](a.png) the",
        "# *try* the",
    ] {
        let gap = gap_after_try(src);
        assert!(gap >= 2.0, "{src:?}: the space after the run is {gap} wide");
    }
}

#[test]
fn punctuation_after_an_italic_run_is_not_under_its_slant() {
    for src in ["*try*.", "*try*, then", "*try*)", "*try*!", "*try*'s", "**try**.", "`try`.", "`try`s", "[try](https://x.test)."] {
        let gap = gap_after_try(src);
        assert!(gap >= 0.0, "{src:?}: the run overlaps what follows by {}", -gap);
    }
}

#[test]
fn punctuation_before_a_code_run_keeps_off_its_background() {
    let rows = ink("(`try`)", 400.0);
    let at = rows.iter().position(|r| r.text == "try").expect("the code run");
    assert!(rows[at].left - rows[at - 1].right >= 1.0, "{rows:?}");
}

#[test]
fn a_run_that_starts_with_a_space_and_wraps_starts_at_the_left_edge() {
    for src in [
        "one two three *four* five six",
        "one two **three** four five six",
        "one two thre **x** fourfivesix seven",
        "a `code` b c d e f g h i j k l",
    ] {
        let rows = ink(src, 120.0);
        let left = rows[0].left;
        let mut lines: Vec<f32> = rows.iter().map(|r| r.y).collect();
        lines.dedup();
        assert!(lines.len() > 1, "{src:?} wraps: {rows:?}");
        for y in lines {
            let first = rows.iter().find(|r| r.y == y && !r.left.is_nan()).expect("ink on every line");
            assert_eq!(first.left, left, "{src:?}: the line at {y} starts at {} ({rows:?})", first.left);
        }
    }
}
