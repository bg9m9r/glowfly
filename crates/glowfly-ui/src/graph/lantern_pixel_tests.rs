//! The HEAD lantern as rendered (issue #456, after review): HEAD's node, rim and glow drawn by
//! the graph's own code (`row_look::paint`, `head_glow`, `nodes`, `rim`) on every fill a HEAD
//! row can have, in every built-in theme and colour preset, both densities, a commit and a
//! merge node, from unsnapped centres at sub-pixel offsets, at 1 and 1.5 pixels per point.
//! Scanning right from the snapped centre along its pixel row:
//! - (a) the pixel wholly between the rim's antialiased edge and the glow is the plain fill;
//! - (b) the lantern reads at 3:1 on it (2.5:1 on a highlighted band, `theme::lantern_floor`);
//! - (c) the next pixel out is the glow, at 1.08:1 or more from the fill;
//! - (d) past the glow's outer edge every pixel is the fill;
//! - (e) on a hovered row the hover's shadow is the same as without the glow;
//! - (f) the high-contrast themes draw no rim or glow;
//! - (g) the snapped centre is within half a pixel of the unsnapped one;
//! - (h) the rim paints a pixel of solid `lantern`, so (b) describes what is painted.

use super::super::nodes::{self, NodeKind};
use super::super::row_look::{self, RowLook};
use super::{FEATHER, extent, head_glow, rim, rim_radius, rim_width};
use crate::ui::snapshot_check::{SKIP_VAR, skip_requested};
use crate::ui::theme::branch_colours::{line_in, swatches};
use crate::ui::theme::color::{self, Palette};
use crate::ui::theme::contrast::ratio;
use crate::ui::theme::{self, builtin};
use crate::ui::widgets::row_fill::HOVER_SHADOW_H;
use eframe::egui::{Color32, Pos2, Rect, emath::GuiRounding, pos2, vec2};
use glowfly_core::branch_colour::Swatch;
use glowfly_core::settings::{BuiltinTheme, ColourPalette, Density};
use std::cell::RefCell;

const CELL_W: f32 = 64.0;
const COLS: usize = 16;
/// Where the node's centre sits in its cell, before the sub-pixel offset.
const NODE_X: f32 = 24.0;
const OFFSETS: [f32; 4] = [0.0, 0.3, 0.5, 0.7];

/// One drawing: the fill's name and look, the node, the unsnapped centre's offset, and whether
/// the glow is left out (the hover shadow's twin).
#[derive(Clone)]
struct Cell {
    fill: String,
    look: RowLook,
    kind: NodeKind,
    offset: (f32, f32),
    bare: bool,
}

fn look(lane: Color32) -> RowLook {
    RowLook { lane, wip: false, highlighted: false, selected: false, active: true, hovered: false, marked: false }
}

/// Every cell for `p`: bg, hover, selected and selected-inactive at every offset, each lane's
/// band (plain and highlighted) at two, a hover twin without the glow for each hover cell.
fn cells(p: &Palette) -> Vec<Cell> {
    let trunk = line_in(Swatch::Trunk, p);
    let main = [
        ("bg", RowLook { wip: true, ..look(trunk) }),
        ("hover", RowLook { hovered: true, ..look(trunk) }),
        ("selected", RowLook { selected: true, ..look(trunk) }),
        ("selected inactive", RowLook { selected: true, active: false, ..look(trunk) }),
    ];
    let mut v = Vec::new();
    for kind in [NodeKind::Commit, NodeKind::Merge] {
        for (name, l) in main {
            for x in OFFSETS {
                for y in OFFSETS {
                    v.push(Cell { fill: name.into(), look: l, kind, offset: (x, y), bare: false });
                    if l.hovered {
                        v.push(Cell { fill: name.into(), look: l, kind, offset: (x, y), bare: true });
                    }
                }
            }
        }
        for s in swatches() {
            for highlighted in [false, true] {
                let l = RowLook { highlighted, ..look(line_in(s, p)) };
                for offset in [(0.0, 0.0), (0.7, 0.3)] {
                    v.push(Cell {
                        fill: format!("{s:?} band{}", if highlighted { " highlighted" } else { "" }),
                        look: l,
                        kind,
                        offset,
                        bare: false,
                    });
                }
            }
        }
    }
    v
}

fn cell_rect(i: usize, row_h: f32) -> Rect {
    Rect::from_min_size(pos2((i % COLS) as f32 * CELL_W, (i / COLS) as f32 * row_h), vec2(CELL_W, row_h))
}

/// Draws `cell` in `rect` as the graph draws HEAD's row; returns the centre it drew the node
/// from (snapped when the glow was drawn) and whether the glow was.
fn draw(ui: &eframe::egui::Ui, rect: Rect, cell: &Cell, (node_r, row_h): (f32, f32)) -> (Pos2, bool) {
    let p = ui.painter().with_clip_rect(rect);
    row_look::paint(&p, rect, cell.look);
    let c = pos2(rect.left() + NODE_X + cell.offset.0, rect.center().y + cell.offset.1);
    let glow = if cell.bare { None } else { head_glow(&p, c, cell.kind, (node_r, row_h), cell.look) };
    let at = match (glow, cell.bare) {
        (Some((at, _)), _) => at,
        (None, true) => c.round_to_pixels(p.pixels_per_point()),
        (None, false) => c,
    };
    match cell.kind {
        NodeKind::Merge => nodes::merge_dot(&p, at, cell.look.lane),
        NodeKind::Commit => nodes::commit(&p, (at, node_r), "Ada Lovelace", "ada@example.com", cell.look.lane, None),
    }
    if let Some((at, edge)) = glow {
        rim(&p, at, edge);
    }
    (at, glow.is_some())
}

fn px(img: &image::RgbaImage, x: f32, y: f32) -> Color32 {
    let p = img.get_pixel(x as u32, y as u32);
    Color32::from_rgb(p[0], p[1], p[2])
}

fn near(a: Color32, b: Color32) -> bool {
    [(a.r(), b.r()), (a.g(), b.g()), (a.b(), b.b())].iter().all(|(x, y)| x.abs_diff(*y) <= 1)
}

/// Renders `p`'s cells at `ppp` in `density` and checks them, adding what fails to `failures`.
fn check(name: &str, p: &Palette, ppp: f32, density: Density, failures: &mut Vec<String>) {
    let (node_r, row_h) = (nodes::node_r(density), super::super::row_h(density));
    let cells = cells(p);
    let rows = cells.len().div_ceil(COLS);
    let drawn = RefCell::new(Vec::new());
    let pal = *p;
    let mut h = crate::ui::snapshot_check::builder()
        .with_size(vec2(COLS as f32 * CELL_W, rows as f32 * row_h))
        .with_pixels_per_point(ppp)
        .build_ui(|ui| {
            color::set_palette(&pal);
            let mut d = drawn.borrow_mut();
            d.clear();
            for (i, cell) in cells.iter().enumerate() {
                d.push(draw(ui, cell_rect(i, row_h), cell, (node_r, row_h)));
            }
        });
    h.run();
    let img = h.render().expect("render");
    drop(h);
    let drawn = drawn.into_inner();
    for (i, (cell, (at, glowed))) in cells.iter().zip(drawn).enumerate() {
        let rect = cell_rect(i, row_h);
        let what = format!("{name} {density:?} @{ppp} {:?} on {} at {:?}", cell.kind, cell.fill, cell.offset);
        let c = pos2(rect.left() + NODE_X + cell.offset.0, rect.center().y + cell.offset.1);
        let (edge, _, outer) = extent(cell.kind, node_r, row_h);
        let row = at.y * ppp;
        let fill = px(&img, rect.right() * ppp - 2.0, row);
        let at_px = |d: f32| px(&img, at.x * ppp + d, row);
        if p.high_contrast {
            // (f) the node only.
            let from = ((c.x + edge + 1.0) * ppp).ceil();
            for x in from as u32..(rect.right() * ppp) as u32 - 1 {
                if !near(px(&img, x as f32, (c.y * ppp).floor()), fill) {
                    failures.push(format!("{what}: a mark outside the node at x {x}"));
                    break;
                }
            }
            continue;
        }
        if cell.bare {
            continue;
        }
        if !glowed {
            failures.push(format!("{what}: no glow"));
            continue;
        }
        // (g) the snap.
        if (at.x - c.x).abs() > 0.5 / ppp + 1e-4 || (at.y - c.y).abs() > 0.5 / ppp + 1e-4 {
            failures.push(format!("{what}: snapped {at:?} from {c:?}"));
        }
        // (h) the rim's own pixel.
        let rim_r = rim_radius(edge, ppp);
        let solid = at_px((rim_r * ppp).floor());
        if [(solid.r(), p.lantern.r()), (solid.g(), p.lantern.g()), (solid.b(), p.lantern.b())].iter().any(|(a, b)| a.abs_diff(*b) > 2) {
            failures.push(format!("{what}: the rim's pixel is {solid:?}, not the lantern {:?}", p.lantern));
        }
        // (a) and (b): the pixels wholly between the rim's antialiased edge and the glow.
        let (gap, glow_px) =
            (((rim_r * ppp) + rim_width(ppp) * ppp / 2.0 + FEATHER).ceil(), ((edge + super::GLOW_GAP) * ppp + 1e-3).floor());
        assert!(gap + 1.0 <= glow_px, "{what}: no whole pixel in the gap");
        let mut d = gap;
        while d + 1.0 <= glow_px {
            if !near(at_px(d), fill) {
                failures.push(format!("{what}: the gap pixel at +{d} px is {:?}, the fill {fill:?}", at_px(d)));
            }
            d += 1.0;
        }
        let plain = at_px(gap);
        let r = ratio(p.lantern, plain);
        if r < crate::ui::theme::lantern_floor(cell.fill.ends_with("highlighted")) {
            failures.push(format!("{what}: the rim {r:.2}:1 on the gap pixel"));
        }
        // (c) the glow begins.
        let first = at_px(((edge + super::GLOW_GAP) * ppp).ceil());
        if ratio(first, fill) < 1.08 {
            failures.push(format!("{what}: the first glow pixel {first:?} is {:.3}:1 from the fill", ratio(first, fill)));
        }
        // (d) the fill past the glow.
        let from = ((outer + 1.0) * ppp).ceil();
        let to = (rect.right() - at.x) * ppp - 2.0;
        let mut d = from;
        while d < to {
            if !near(at_px(d), fill) {
                failures.push(format!("{what}: {:?} past the glow at +{d} px", at_px(d)));
                break;
            }
            d += 1.0;
        }
        // (e) the hover shadow, against the twin drawn without the glow (the next cell).
        if cell.look.hovered && !cell.look.selected {
            let twin = cell_rect(i + 1, row_h);
            let shadow = (HOVER_SHADOW_H * ppp) as u32;
            let (w, bottom) = ((CELL_W * ppp) as u32, (rect.bottom() * ppp) as u32);
            'rows: for y in bottom - shadow..bottom {
                for x in 0..w {
                    let ours = px(&img, rect.left() * ppp + x as f32, y as f32);
                    let theirs = px(&img, twin.left() * ppp + x as f32, twin.bottom() * ppp - (bottom - y) as f32);
                    // Within the renderer's dithering.
                    if !near(ours, theirs) {
                        failures.push(format!(
                            "{what}: the glow paints the hover shadow at ({x}, {y}): {ours:?} vs {theirs:?}, cell {rect:?} twin {twin:?}"
                        ));
                        break 'rows;
                    }
                }
            }
        }
    }
}

#[test]
fn the_rendered_lantern_keeps_a_plain_pixel_by_the_rim() {
    if skip_requested(std::env::var(SKIP_VAR).ok().as_deref()) {
        eprintln!("note: {SKIP_VAR} is set; not rendering the lantern");
        return;
    }
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        for preset in [ColourPalette::Standard, ColourPalette::ColourBlind] {
            let p = theme::applied(*builtin(t), preset);
            for ppp in [1.0, 1.5] {
                for density in [Density::Comfortable, Density::Compact] {
                    check(&format!("{t:?} {preset:?}"), &p, ppp, density, &mut failures);
                }
            }
        }
    }
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.iter().take(40).cloned().collect::<Vec<_>>().join("\n"));
}
