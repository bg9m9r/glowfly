//! The selection of a pane without the keyboard focus (`tokens::selected_inactive`): a neutral
//! in `selected`'s hue (OKLCH chroma at most 0.02) that stands apart from `selected` and
//! `hover` (OKLab ΔE at least 0.03) and keeps `weak` text at 4.5:1. The built-in themes use the
//! designer's measured values; any other palette gets one worked out by the same rules.
//! Checked in `inactive_tests.rs`.

use super::color::Palette;
use super::contrast::{brownish, from_oklch, oklch, ratio};
use eframe::egui::Color32;

/// The OKLCH chroma the worked-out value keeps: under the 0.02 most, so rounding to 8-bit
/// channels can't carry it over.
const AIM: f64 = 0.018;
/// How far the worked-out value moves in OKLCH lightness per step while it makes `weak` readable.
const STEP: f64 = 0.005;
/// How far it then moves per step while it stands apart from `selected` and `hover`.
const FINE: f64 = 0.0005;
/// The least OKLab distance from `selected` and from `hover`.
pub(super) const APART: f64 = 0.03;

const fn rgb(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// The designer's values for the built-in themes, found by each theme's `selected` and `hover`:
/// Graphite, Midnight, Dusk, High contrast dark, Porcelain, Mist, Dawn, High contrast light.
const PINNED: [(Color32, Color32, Color32); 8] = [
    (rgb(0x1f3d62), rgb(0x303236), rgb(0x363d46)),
    (rgb(0x173f3c), rgb(0x213044), rgb(0x313e3c)),
    (rgb(0x362f58), rgb(0x332d44), rgb(0x36353f)),
    (rgb(0x0e4430), rgb(0x26282d), rgb(0x313d37)),
    (rgb(0xdbe8fc), rgb(0xeceef2), rgb(0xd6deea)),
    (rgb(0xc7eddd), rgb(0xe2eef5), rgb(0xd4e2db)),
    (rgb(0xe6e1fa), rgb(0xece9f5), rgb(0xdcdae8)),
    (rgb(0xc2e8d0), rgb(0xe1e4e9), rgb(0xd4e0d7)),
];

/// The selected row's fill in a pane without the keyboard focus.
pub(super) fn selected_inactive(p: &Palette) -> Color32 {
    pinned(p).unwrap_or_else(|| worked_out(p))
}

/// The designer's value for a built-in palette (or one keeping its `selected` and `hover`).
pub(super) fn pinned(p: &Palette) -> Option<Color32> {
    PINNED.iter().find(|&&(s, h, _)| s == p.selected && h == p.hover).map(|&(_, _, v)| v)
}

/// `selected` with its chroma cut to `AIM` (a grey where that would read brown), moved in
/// lightness away from `weak` until `weak` reads on it at 4.5:1, then away from `hover` until it
/// stands `APART` from both `selected` and `hover`.
pub(super) fn worked_out(p: &Palette) -> Color32 {
    let (l0, c, h) = oklch(p.selected);
    let c = c.min(AIM);
    let at = |l: f64| {
        let fill = from_oklch(l, c, h);
        if brownish(fill) { from_oklch(l, 0.0, h) } else { fill }
    };
    let inside = |l: f64| (0.0..=1.0).contains(&l);
    // Weak text is lighter than the fill on a dark palette, darker on a light one.
    let away = if oklch(p.weak).0 > l0 { -STEP } else { STEP };
    let (mut l, mut fill) = (l0, at(l0));
    while ratio(p.weak, fill) < 4.5 && inside(l + away) {
        l += away;
        fill = at(l);
    }
    let apart = |f: Color32| delta_e(f, p.selected) >= APART && delta_e(f, p.hover) >= APART;
    let off = if l >= oklch(p.hover).0 { FINE } else { -FINE };
    while !apart(fill) && inside(l + off) {
        l += off;
        fill = at(l);
    }
    fill
}

/// The OKLab distance between `a` and `b`.
pub(super) fn delta_e(a: Color32, b: Color32) -> f64 {
    let lab = |c| {
        let (l, ch, h) = oklch(c);
        (l, ch * h.to_radians().cos(), ch * h.to_radians().sin())
    };
    let (x, y) = (lab(a), lab(b));
    ((x.0 - y.0).powi(2) + (x.1 - y.1).powi(2) + (x.2 - y.2).powi(2)).sqrt()
}
