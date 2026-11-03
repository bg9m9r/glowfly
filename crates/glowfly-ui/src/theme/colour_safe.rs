//! The colour-blind safe diff and status colours (Preferences > Accessibility > Colours >
//! Colour-blind safe): added turns blue, removed orange and modified violet, so the three stay
//! apart under protan, deutan and tritan vision as well as normal (`colour_safe_tests.rs`
//! measures them through `vision`).
//!
//! Each colour is re-hued at the WCAG luminance it had, keeping its chroma as far as sRGB holds
//! it, so every contrast the theme tests hold the palette to (text on the diff fills, marks on
//! the surfaces) stays exactly as it was. Two exceptions keep the house "No brown" rule
//! (docs/COLOUR.md): an orange that would read brown turns to a rust red on dark themes (hue
//! 30, chroma at least 0.08) and a redder peach on light ones (hue 45, chroma 0.07), and on
//! light themes removed text and marks take a vermilion (hue 40, chroma at least 0.08), as a
//! dark orange on paper reads brown. Modified is not at added's lightness, where violet and
//! blue look alike to red-green colour blindness: it sits `violet_step` below added in OKLab
//! lightness.

use super::color::Palette;
use super::contrast::{brownish, from_oklch, luminance, max_chroma, oklch};
use eframe::egui::Color32;
use glowfly_core::settings::ColourPalette;

/// Added: blue.
const BLUE: f64 = 250.0;
/// Removed: orange.
const ORANGE: f64 = 55.0;
/// Removed where orange reads brown: a rust red.
const RUST: f64 = 30.0;
/// Removed fills on paper where orange would read brown: a redder peach, at least
/// `PEACH_CHROMA`.
const PEACH: f64 = 45.0;
const PEACH_CHROMA: f64 = 0.07;
/// Removed text and marks on light themes: vermilion.
const VERMILION: f64 = 40.0;
/// Modified: violet.
const VIOLET: f64 = 300.0;
/// The least chroma of the rust and vermilion: 0.08 is the brown zone's edge, and a little more
/// keeps them out of it once rounded to sRGB bytes.
const WARM_CHROMA: f64 = 0.085;

/// How far modified sits below added in OKLab lightness: enough that the two stay 0.10 apart
/// under every simulated vision, with the marks still at their contrast floor on `selected`
/// (3:1, 4.5:1 in high contrast, which allows only a smaller step on the dark one).
fn violet_step(p: &Palette) -> f64 {
    match (p.dark, p.high_contrast) {
        (true, false) => 0.11,
        (true, true) => 0.10,
        (false, _) => 0.15,
    }
}

/// `p` with the colour-safe diff and status colours when `choice` asks for them.
pub fn applied(mut p: Palette, choice: ColourPalette) -> Palette {
    if choice != ColourPalette::ColourBlind {
        return p;
    }
    // Rounded to bytes, a fill errs away from the text (lighter on paper) and a mark away from
    // the surface, so no contrast drops below the theme's own.
    let (fill, mark) = (Toward(!p.dark), Toward(p.dark));
    for c in [&mut p.diff_add, &mut p.diff_add_sel, &mut p.diff_add_word] {
        *c = rehue(*c, BLUE, 0.0, fill);
    }
    p.status.added = rehue(p.status.added, BLUE, 0.0, mark);
    for c in [&mut p.diff_del, &mut p.diff_del_sel, &mut p.diff_del_word] {
        *c = orange(*c, fill, p.dark);
    }
    p.status.removed = if p.dark { orange(p.status.removed, mark, true) } else { rehue(p.status.removed, VERMILION, WARM_CHROMA, mark) };
    let l = oklch(p.status.added).0 - violet_step(&p);
    p.status.modified = from_oklch(l, 0.9 * max_chroma(l, VIOLET), VIOLET);
    p
}

/// Which way a re-hued colour may round: lighter (`true`) or darker than the original.
#[derive(Clone, Copy)]
struct Toward(bool);

/// `c` in orange or, where the orange would read brown, a rust red on a `dark` palette and a
/// clearer, redder peach on paper (a pink rust there would drift toward the blue fills).
fn orange(c: Color32, round: Toward, dark: bool) -> Color32 {
    let o = rehue(c, ORANGE, 0.0, round);
    match brownish(o) {
        false => o,
        true if dark => rehue(c, RUST, WARM_CHROMA, round),
        true => rehue(c, PEACH, PEACH_CHROMA, round),
    }
}

/// The colour of hue `h` with `c`'s WCAG luminance and its chroma (at least `min_chroma`), the
/// chroma held just inside the sRGB gamut, rounded the way `round` says.
fn rehue(c: Color32, h: f64, min_chroma: f64, round: Toward) -> Color32 {
    let target = luminance(c);
    let chroma = oklch(c).1.max(min_chroma);
    let at = |l: f64| from_oklch(l, chroma.min(0.999 * max_chroma(l, h)), h);
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if luminance(at(mid)) < target { lo = mid } else { hi = mid }
    }
    // `lo` stays under the original's luminance and `hi` at or over it.
    if round.0 { at(hi) } else { at(lo) }
}
