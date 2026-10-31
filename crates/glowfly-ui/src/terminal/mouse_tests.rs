//! The terminal mouse's pure parts: cells under the pointer, wheel steps, the arrow keys the
//! wheel sends a full-screen program in both cursor-keys modes, and scrolling past the edge.

use super::*;
use eframe::egui::{Modifiers, pos2, vec2};
use glowfly_core::terminal::{Modes, encode};

#[test]
fn cells_round_to_the_nearest_boundary() {
    let o = pos2(10.0, 20.0);
    let at = |x, y| cell_at(pos2(x, y), o, 8.0, 16.0, 24, 80);
    assert_eq!(at(10.0, 20.0), (0, 0));
    assert_eq!(at(13.9, 35.9), (0, 0), "left half of the first cell");
    assert_eq!(at(14.1, 36.1), (1, 1), "right half: the boundary after it");
    assert_eq!(at(0.0, 0.0), (0, 0), "above and left of the grid");
    assert_eq!(at(5000.0, 5000.0), (23, 80), "past the last cell");
}

#[test]
fn the_cell_under_the_pointer() {
    let g = Geometry { origin: pos2(10.0, 20.0), cw: 8.0, rh: 16.0, rows: 24, cols: 80 };
    let at = |x, y| cell_under(pos2(x, y), &g);
    assert_eq!(at(10.0, 20.0), (0, 0));
    assert_eq!(at(17.9, 35.9), (0, 0), "anywhere in the first cell");
    assert_eq!(at(18.0, 36.0), (1, 1));
    assert_eq!(at(0.0, 0.0), (0, 0), "above and left of the grid");
    assert_eq!(at(5000.0, 5000.0), (23, 79), "past the last cell is the last cell");
}

fn wheel(unit: MouseWheelUnit, y: f32, modifiers: Modifiers) -> Event {
    Event::MouseWheel { unit, delta: vec2(0.0, y), modifiers }
}

#[test]
fn wheel_steps() {
    let none = Modifiers::NONE;
    let mut rem = 0.0;
    assert_eq!(wheel_rows(&[wheel(MouseWheelUnit::Line, 1.0, none)], 16.0, 24, &mut rem), 3);
    assert_eq!(wheel_rows(&[wheel(MouseWheelUnit::Line, -2.0, none)], 16.0, 24, &mut rem), -6);
    assert_eq!(wheel_rows(&[wheel(MouseWheelUnit::Page, 1.0, none)], 16.0, 24, &mut rem), 24);
    // Points: 10 px of a 16 px row is carried until a whole row has come.
    assert_eq!(wheel_rows(&[wheel(MouseWheelUnit::Point, 10.0, none)], 16.0, 24, &mut rem), 0);
    assert_eq!(wheel_rows(&[wheel(MouseWheelUnit::Point, 10.0, none)], 16.0, 24, &mut rem), 1);
    assert!((rem - 0.25).abs() < 1e-4, "{rem}");
    let mut rem = 0.0;
    for zoom in [Modifiers::CTRL, Modifiers::COMMAND, Modifiers::MAC_CMD | Modifiers::COMMAND] {
        assert_eq!(wheel_rows(&[wheel(MouseWheelUnit::Line, 1.0, zoom)], 16.0, 24, &mut rem), 0, "{zoom:?}");
    }
    assert_eq!(rem, 0.0);
}

/// The bytes `rows` of wheel send in `modes`.
fn sent(rows: i32, application_cursor: bool) -> Vec<u8> {
    let modes = Modes { application_cursor, ..Modes::default() };
    wheel_keys(rows).iter().flat_map(|k| encode(k, modes)).collect()
}

#[test]
fn the_wheel_sends_arrows_in_the_programs_cursor_mode() {
    assert_eq!(sent(3, false), b"\x1b[A\x1b[A\x1b[A", "a notch up: three Up arrows");
    assert_eq!(sent(-2, false), b"\x1b[B\x1b[B", "down: Down arrows");
    assert_eq!(sent(3, true), b"\x1bOA\x1bOA\x1bOA", "application cursor keys");
    assert_eq!(sent(-1, true), b"\x1bOB");
    assert!(wheel_keys(0).is_empty());
    assert_eq!(wheel_keys(10_000).len(), MAX_WHEEL_KEYS as usize, "a huge page is capped");
}

#[test]
fn dragging_past_an_edge_scrolls_faster_further_out() {
    let (top, bottom, rh, dt) = (100.0, 484.0, 16.0, 0.1);
    let mut rem = 0.0;
    assert_eq!(edge_rows(300.0, top, bottom, rh, dt, &mut rem), 0, "inside the grid");
    // Just past the top: 8.5 rows a second, the part of a row carried to the next frame.
    assert_eq!(edge_rows(99.0, top, bottom, rh, dt, &mut rem), 0);
    assert_eq!(edge_rows(99.0, top, bottom, rh, dt, &mut rem), 1);
    let speed = |past: f32| edge_rows(top - past, top, bottom, rh, dt, &mut 0.0);
    assert_eq!((speed(64.0), speed(144.0)), (4, 8), "four rows out five times the speed, nine rows out ten times");
    assert_eq!(edge_rows(bottom + 64.0, top, bottom, rh, dt, &mut 0.0), -4, "below the grid scrolls forward");
    assert_eq!(speed(5000.0), 20, "capped at 200 rows a second");
    assert_eq!(edge_rows(top - 64.0, top, bottom, rh, 5.0, &mut 0.0), 4, "a long stall counts as a tenth of a second");
    // At 60 frames a second just past the edge, a row comes every few frames.
    let mut rem = 0.0;
    let rows: i32 = (0..60).map(|_| edge_rows(99.0, top, bottom, rh, 1.0 / 60.0, &mut rem)).sum();
    assert!((7..=8).contains(&rows), "{rows} rows in a second");
    assert!(rem > 0.0);
    assert_eq!(edge_rows(200.0, top, bottom, rh, dt, &mut rem), 0);
    assert_eq!(rem, 0.0, "back inside, the carried part is dropped");
}
