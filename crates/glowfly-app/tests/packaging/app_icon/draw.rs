//! Glowfly's icon, "Glass wings", drawn in the 1024-unit design space (y down) and rendered with
//! supersampling: a dusk tile (indigo to plum, on the macOS icon grid, with a faint white rim)
//! holding a small commit graph drawn the way the app draws its own. The gold lane rises into an
//! oval glowing lantern with two faint glass wings, its head; a magenta lane branches off a gold
//! commit and an azure lane off a magenta commit, each ending in a glowing companion head. The
//! lanes take the app's branch hues (`ui/theme/branch_palette.rs`) and its S-shaped elbows, and
//! the commits its ringed dots.
//!
//! 16 to 32 px use a cut of their own: a near-full-bleed tile, the lantern, and the magenta and
//! azure lanes only, 64 units wide (exactly 1 px at 16 and 2 px at 32) and on the pixel grid, so
//! the heads stay round blobs.

use super::paint::{Ellipse, Frame, Point, Polyline, Rgb, Stop, at, cubic, dist2, hex, in_round_rect, over};

const GOLD: Rgb = hex(0xdfba3b);
const MAGENTA: Rgb = hex(0xfb97db);
const AZURE: Rgb = hex(0x93c1fe);
const ORANGE: Rgb = hex(0xd78c1a);
const WHITE: Rgb = hex(0xffffff);
/// The ring that sets a commit dot off from its lane.
const RING: Rgb = hex(0x241c4e);

const SKY: [Stop; 3] = [(0.0, hex(0x1a1a4c), 1.0), (0.55, hex(0x36275f), 1.0), (1.0, hex(0x6e3560), 1.0)];
const SKY_SMALL: [Stop; 2] = [(0.0, hex(0x1d1c52), 1.0), (1.0, hex(0x5e2f5e), 1.0)];
/// The lantern's wide warm halo.
const HALO_WARM: [Stop; 4] = [(0.0, hex(0xf6b453), 0.9), (0.3, hex(0xe59a2c), 0.42), (0.65, ORANGE, 0.14), (1.0, ORANGE, 0.0)];
/// The glow just around the lantern.
const GLOW_WARM: [Stop; 3] = [(0.0, hex(0xffd27a), 0.7), (0.6, hex(0xf0a33a), 0.3), (1.0, ORANGE, 0.0)];
const LANTERN: [Stop; 5] =
    [(0.0, hex(0xfffbf1), 1.0), (0.34, hex(0xffe6b0), 1.0), (0.7, hex(0xf4b14d), 1.0), (0.9, hex(0xe0961f), 0.75), (1.0, ORANGE, 0.25)];

/// The lantern's group: `translate(340 664) rotate(28) scale(1.1)` (cos and sin of 28 degrees).
const LIGHT: Frame = Frame { tx: 340.0, ty: 664.0, cos: 0.882_947_592_858_927, sin: 0.469_471_562_785_891, scale: 1.1 };
/// The small cut's: `translate(300 740) rotate(26) scale(1.5)`.
const LIGHT_SMALL: Frame = Frame { tx: 300.0, ty: 740.0, cos: 0.898_794_046_299_167, sin: 0.438_371_146_789_077, scale: 1.5 };

/// A companion head: a glow and a bright core over it.
struct Head {
    c: Point,
    halo: f64,
    core: f64,
    hue: Rgb,
    centre: Rgb,
    mid: Rgb,
}

const HEADS: [Head; 2] = [
    Head { c: (792.0, 250.0), halo: 150.0, core: 44.0, hue: AZURE, centre: hex(0xf4f9ff), mid: hex(0xc3dcff) },
    Head { c: (612.0, 452.0), halo: 132.0, core: 40.0, hue: MAGENTA, centre: hex(0xfff3fb), mid: hex(0xfdc4ea) },
];

/// The commits: where, the lane's hue, the radius and the pale core's tint.
const COMMITS: [(Point, Rgb, f64, Rgb); 5] = [
    ((340.0, 858.0), GOLD, 22.0, hex(0xfff1c4)),
    ((612.0, 616.0), MAGENTA, 22.0, hex(0xfff0fa)),
    ((612.0, 534.0), MAGENTA, 18.0, hex(0xfff0fa)),
    ((792.0, 458.0), AZURE, 22.0, hex(0xeef5ff)),
    ((792.0, 356.0), AZURE, 18.0, hex(0xeef5ff)),
];

const STARS: [(Point, f64, f64); 3] = [((248.0, 250.0), 6.0, 0.5), ((400.0, 196.0), 4.0, 0.35), ((520.0, 300.0), 3.0, 0.3)];

/// One wing, in the lantern's units (its path is drawn at `translate(0 -40) scale(0.68)` there),
/// mirrored for the right one.
fn wing(mirror: bool) -> Polyline {
    let s = if mirror { -1.0 } else { 1.0 };
    let q = |x: f64, y: f64| (x * s * 0.68, y * 0.68 - 40.0);
    let mut points = vec![q(-24.0, -104.0)];
    let curves = [
        [(-90.0, -190.0), (-200.0, -236.0), (-236.0, -180.0)],
        [(-258.0, -144.0), (-214.0, -96.0), (-150.0, -74.0)],
        [(-100.0, -58.0), (-56.0, -62.0), (-24.0, -74.0)],
    ];
    for [c1, c2, end] in curves {
        let start = points[points.len() - 1];
        points.extend(cubic(start, q(c1.0, c1.1), q(c2.0, c2.1), q(end.0, end.1), 24));
    }
    // The path closes with a straight line back to its start.
    points.push(points[0]);
    Polyline::new(points)
}

/// A lane: an elbow from `from` over to `turn`, then straight up to `top`. The elbow is the
/// app's graph edge (`ui/graph/paint.rs`): a cubic with vertical tangents at both ends and both
/// control points at height `my` (mid height, except the small cut's magenta lane), so it is a
/// smooth S.
fn lane(from: Point, turn: Point, my: f64, top: f64) -> Polyline {
    let mut points = vec![from];
    points.extend(cubic(from, (from.0, my), (turn.0, my), turn, 32));
    points.push((turn.0, top));
    Polyline::new(points)
}

/// The shapes a cut builds once per render.
struct Scene {
    lanes: Vec<(Polyline, Rgb)>,
    wings: [Polyline; 2],
}

fn scene(small: bool) -> Scene {
    let lanes = if small {
        vec![(lane((300.0, 740.0), (768.0, 600.0), 690.0, 448.0), MAGENTA), (lane((300.0, 740.0), (544.0, 500.0), 620.0, 224.0), AZURE)]
    } else {
        vec![
            (Polyline::new(vec![(340.0, 1000.0), (340.0, 664.0)]), GOLD),
            (lane((340.0, 858.0), (612.0, 690.0), 774.0, 452.0), MAGENTA),
            (lane((612.0, 616.0), (792.0, 458.0), 537.0, 250.0), AZURE),
        ]
    };
    Scene { lanes, wings: [wing(false), wing(true)] }
}

fn fill_gradient(c: &mut Rgb, l: Point, shape: Ellipse, stops: &[Stop], centre: (f64, f64), r: f64) {
    if shape.inside(l) {
        let (rgb, a) = at(stops, shape.gradient_t(l, centre.0, centre.1, r));
        over(c, rgb, a);
    }
}

/// The halo (`halo`: the small cut draws it in the lantern's group), the glass wings, the glow
/// and the lantern, at `l` in the lantern's units.
fn light(c: &mut Rgb, l: Point, s: &Scene, small: bool) {
    let halo = Ellipse { c: (0.0, -10.0), r: (290.0, 340.0) };
    if small {
        fill_gradient(c, l, halo, &HALO_WARM, (0.5, 0.5), 0.5);
    }
    for w in &s.wings {
        if w.contains(l) {
            if small { over(c, hex(0xffe3bd), 0.34) } else { over(c, hex(0xffe9c8), 0.13) }
        }
        // The full cut's warm edge: a 4-unit stroke at the wing's scale.
        if !small && w.dist2_within(l, 2.0 * 0.68).is_some() {
            over(c, hex(0xffd99a), 0.32);
        }
    }
    fill_gradient(c, l, Ellipse { c: (0.0, 4.0), r: (150.0, 186.0) }, &GLOW_WARM, (0.5, 0.55), 0.5);
    fill_gradient(c, l, Ellipse { c: (0.0, 0.0), r: (100.0, 132.0) }, &LANTERN, (0.5, 0.6), 0.6);
}

/// The full icon's colour at `p`; None outside the tile.
fn full(p: Point, s: &Scene) -> Option<Rgb> {
    if !in_round_rect(p, 100.0, 924.0, 185.0) {
        return None;
    }
    let mut c = at(&SKY, (p.1 - 100.0) / 824.0).0;
    for (centre, r, a) in STARS {
        if dist2(p, centre) <= r * r {
            over(&mut c, hex(0xeef0ff), a);
        }
    }
    let l = LIGHT.local(p);
    fill_gradient(&mut c, l, Ellipse { c: (0.0, -10.0), r: (290.0, 340.0) }, &HALO_WARM, (0.5, 0.5), 0.5);
    // Each lane: a soft under-glow 3.2 times its width at 18 %, then the 16-unit lane.
    for (line, hue) in &s.lanes {
        if let Some(d2) = line.dist2_within(p, 25.6) {
            over(&mut c, *hue, 0.18);
            if d2 <= 8.0 * 8.0 {
                c = *hue;
            }
        }
    }
    for (centre, hue, r, tint) in COMMITS {
        let d2 = dist2(p, centre);
        if d2 <= (r * 1.32) * (r * 1.32) {
            over(&mut c, RING, 0.75);
        }
        if d2 <= r * r {
            c = hue;
        }
        if d2 <= (r * 0.46) * (r * 0.46) {
            c = tint;
        }
    }
    light(&mut c, l, s, false);
    for h in &HEADS {
        let d2 = dist2(p, h.c);
        if d2 <= h.halo * h.halo {
            let (rgb, a) = at(&[(0.0, h.hue, 0.7), (0.35, h.hue, 0.2), (1.0, h.hue, 0.0)], d2.sqrt() / h.halo);
            over(&mut c, rgb, a);
        }
        if d2 <= h.core * h.core {
            c = at(&[(0.0, h.centre, 1.0), (0.45, h.mid, 1.0), (1.0, h.hue, 1.0)], d2.sqrt() / h.core).0;
        }
    }
    // The rim: a 4-unit line just inside the tile's edge.
    if !in_round_rect(p, 104.0, 920.0, 181.0) {
        over(&mut c, WHITE, 0.10);
    }
    Some(c)
}

/// The 16 to 32 px cut's colour at `p`; None outside the tile.
fn small(p: Point, s: &Scene) -> Option<Rgb> {
    if !in_round_rect(p, 64.0, 960.0, 200.0) {
        return None;
    }
    let mut c = at(&SKY_SMALL, (p.1 - 64.0) / 896.0).0;
    for (line, hue) in &s.lanes {
        if line.dist2_within(p, 32.0).is_some() {
            c = *hue;
        }
    }
    light(&mut c, LIGHT_SMALL.local(p), s, true);
    for (centre, r, hue) in [((544.0, 224.0), 116.0, hex(0xb4d3ff)), ((768.0, 448.0), 112.0, hex(0xfcb2e5))] {
        if dist2(p, centre) <= r * r {
            c = hue;
        }
    }
    if !in_round_rect(p, 70.0, 954.0, 194.0) {
        over(&mut c, WHITE, 0.12);
    }
    Some(c)
}

/// The icon at `size` x `size` pixels, as straight-alpha RGBA bytes: the small cut up to 32 px,
/// with more samples per pixel the smaller it is.
pub fn render(size: u32) -> Vec<u8> {
    let cut = size <= 32;
    let s = scene(cut);
    let n: u32 = match size {
        0..=64 => 8,
        65..=256 => 4,
        _ => 2,
    };
    let unit = 1024.0 / size as f64;
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let (mut sum, mut hits) = ([0.0_f64; 3], 0u32);
            for sy in 0..n {
                for sx in 0..n {
                    let x = (px as f64 + (sx as f64 + 0.5) / n as f64) * unit;
                    let y = (py as f64 + (sy as f64 + 0.5) / n as f64) * unit;
                    if let Some(c) = if cut { small((x, y), &s) } else { full((x, y), &s) } {
                        for i in 0..3 {
                            sum[i] += c[i];
                        }
                        hits += 1;
                    }
                }
            }
            let alpha = 255.0 * hits as f64 / (n * n) as f64;
            let rgb = |i: usize| if hits > 0 { sum[i] / hits as f64 } else { 0.0 };
            for v in [rgb(0), rgb(1), rgb(2), alpha] {
                out.push((v + 0.5).floor().clamp(0.0, 255.0) as u8);
            }
        }
    }
    out
}
