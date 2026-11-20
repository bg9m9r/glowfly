//! `long_list`: drawing only the visible rows equals drawing every row (ids, rects, where the
//! list ends), costs the visible rows only, and keeps the filter's matches per snapshot and text.

use super::*;
use crate::theme::fonts;
use eframe::egui::{self, Pos2, Rect, ScrollArea, Sense};
use std::cell::Cell;

type Seen = Vec<(usize, Rect, Id)>;

/// What one pass of a list drew: its rows, where its cursor and auto ids ended and its content's height.
struct Pass {
    seen: Seen,
    cursor_y: f32,
    next_id: Id,
    content_h: f32,
}

/// Runs a list in a 200 px scroll area at `offset` with `spacing` between rows, in a fresh context
/// (two frames; the second is recorded). `list` draws it.
fn pass(spacing: f32, offset: f32, list: impl Fn(&mut Ui, &mut Seen)) -> Pass {
    let ctx = egui::Context::default();
    fonts::use_test_cjk();
    fonts::install(&ctx);
    let input = || egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(300.0, 400.0))), ..Default::default() };
    let mut out = None;
    for _ in 0..3 {
        let mut seen = Seen::new();
        let _ = ctx.run(input(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = spacing;
                let area = ScrollArea::vertical().max_height(200.0).vertical_scroll_offset(offset).show(ui, |ui| {
                    list(ui, &mut seen);
                    (ui.cursor().min.y, ui.next_auto_id())
                });
                let (cursor_y, next_id) = area.inner;
                out = Some(Pass { seen: std::mem::take(&mut seen), cursor_y, next_id, content_h: area.content_size.y });
            });
        });
    }
    out.unwrap()
}

fn record(ui: &mut Ui, seen: &mut Seen, i: usize) {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), LIST_ROW_H), Sense::click());
    seen.push((i, r, resp.id));
}

#[test]
fn visible_rows_match_drawing_every_row() {
    let n = 1000;
    for spacing in [0.0, 4.0] {
        let end = n as f32 * (LIST_ROW_H + spacing);
        for offset in [0.0, 17.5, 1234.0, 5000.0, end] {
            let old = pass(spacing, offset, |ui, seen| {
                for i in 0..n {
                    if list_row_hidden(ui) {
                        continue;
                    }
                    record(ui, seen, i);
                }
            });
            let new = pass(spacing, offset, |ui, seen| rows(ui, n, |ui, i| record(ui, seen, i)));
            let ctx = format!("spacing {spacing}, offset {offset}");
            let idx = |p: &Pass| p.seen.iter().map(|s| s.0).collect::<Vec<_>>();
            assert_eq!(idx(&old), idx(&new), "the rows drawn, {ctx}");
            assert!(!old.seen.is_empty() || offset >= end, "something is drawn, {ctx}");
            for (a, b) in old.seen.iter().zip(&new.seen) {
                assert_eq!(a.2, b.2, "the widget id of row {}, {ctx}", a.0);
                for (x, y) in [(a.1.min.x, b.1.min.x), (a.1.min.y, b.1.min.y), (a.1.max.x, b.1.max.x), (a.1.max.y, b.1.max.y)] {
                    assert!((x - y).abs() < 1e-3, "row {} rect {:?} vs {:?}, {ctx}", a.0, a.1, b.1);
                }
            }
            assert!((old.cursor_y - new.cursor_y).abs() < 1e-3, "where the list ends, {ctx}");
            assert_eq!(old.next_id, new.next_id, "the next auto id, {ctx}");
            assert!((old.content_h - new.content_h).abs() < 1e-3, "the scroll extent, {ctx}");
        }
    }
}

#[test]
fn a_long_list_visits_only_the_rows_in_view() {
    let n = 100_000;
    let calls = Cell::new(0_usize);
    let p = pass(0.0, n as f32 * LIST_ROW_H / 2.0, |ui, _| {
        rows(ui, n, |ui, _| {
            calls.set(calls.get() + 1);
            ui.allocate_exact_size(vec2(ui.available_width(), LIST_ROW_H), Sense::click());
        });
    });
    // The last of the passes counted; the three together stay within three views.
    let per_view = (200.0 / LIST_ROW_H).ceil() as usize + 2;
    assert!(calls.get() > 0 && calls.get() <= 3 * per_view, "{} rows visited", calls.get());
    assert!((p.content_h - n as f32 * LIST_ROW_H).abs() < 1.0, "content is {} tall", p.content_h);
}

#[test]
fn no_rows_take_no_space_or_id() {
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let (cursor, id) = (ui.cursor(), ui.next_auto_id());
            rows(ui, 0, |_, _| panic!("no rows to draw"));
            assert_eq!((ui.cursor(), ui.next_auto_id()), (cursor, id));
        });
    });
}

#[test]
fn matching_rows_are_kept_per_snapshot_and_filter() {
    let ctx = egui::Context::default();
    let id = Id::new("rows");
    let calls = Cell::new(0_usize);
    let names = ["alpha", "beta", "gamma", "alpine"];
    let get = |stamp: u64, filter: &str| {
        shown(&ctx, id, (stamp, filter), names.len(), |i| {
            calls.set(calls.get() + 1);
            names[i].contains(filter)
        })
    };
    let all = get(1, "");
    assert_eq!((all.len(), all.at(2), calls.get()), (4, 2, 0), "no filter lists nothing and matches nothing");
    let v = get(1, "al");
    assert_eq!(calls.get(), 4);
    assert_eq!((v.len(), v.at(0), v.at(1)), (2, 0, 3), "the matches keep the list's order");
    get(1, "al");
    assert_eq!(calls.get(), 4, "the same snapshot and text match once");
    get(1, "alp");
    assert_eq!(calls.get(), 8, "a new text matches again");
    get(2, "alp");
    assert_eq!(calls.get(), 12, "a new snapshot matches again");
    assert_eq!(get(2, "zzz").len(), 0);
}
