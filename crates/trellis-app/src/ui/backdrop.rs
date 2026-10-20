//! The Mica backdrop behind the title bar, toolbar and sidebar on Windows 11: how translucent
//! each surface's colour may be while its text stays readable (`mica_tint`), and the fill those
//! surfaces paint with (`fill`). Everything else stays opaque.

use super::theme::color::Palette;
use super::theme::contrast::{mix, ratio};
use crate::state::window_native::{Backdrop, MicaTint};
use eframe::egui::Color32;

/// Stand-ins for dark Mica: they bracket the blurred, tinted wallpaper it shows, from neutral
/// to strongly hued and from black to busy. A check against the real backdrop is listed in
/// docs/PLATFORMS.md.
pub const MICA_DARK: [Color32; 9] = [
    Color32::from_rgb(0x00, 0x00, 0x00),
    Color32::from_rgb(0x20, 0x20, 0x20),
    Color32::from_rgb(0x2c, 0x2c, 0x2c),
    Color32::from_rgb(0x3a, 0x3a, 0x3a),
    Color32::from_rgb(0x3d, 0x33, 0x49),
    Color32::from_rgb(0x2f, 0x3f, 0x33),
    Color32::from_rgb(0x47, 0x39, 0x2c),
    Color32::from_rgb(0x2b, 0x3a, 0x4c),
    Color32::from_rgb(0x4a, 0x2e, 0x33),
];

/// Stand-ins for light Mica, like `MICA_DARK`: from white to strongly hued and busy.
pub const MICA_LIGHT: [Color32; 9] = [
    Color32::from_rgb(0xff, 0xff, 0xff),
    Color32::from_rgb(0xf3, 0xf3, 0xf3),
    Color32::from_rgb(0xe3, 0xe3, 0xe3),
    Color32::from_rgb(0xd4, 0xd4, 0xd4),
    Color32::from_rgb(0xdd, 0xd3, 0xea),
    Color32::from_rgb(0xd2, 0xe0, 0xd4),
    Color32::from_rgb(0xea, 0xdb, 0xc9),
    Color32::from_rgb(0xcf, 0xdb, 0xea),
    Color32::from_rgb(0xeb, 0xd2, 0xd5),
];

/// The tint alphas tried, most translucent first: 0.60 to 0.90.
pub(super) const STEPS: [u8; 7] = [153, 166, 179, 191, 204, 217, 230];
/// The alpha of a surface that fails at every step: it stays opaque.
pub const OPAQUE: u8 = 255;

/// A surface that can sit on Mica.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// The title bar and the toolbar, on `toolbar`.
    Chrome,
    /// The sidebar and its edge strip, on `panel`.
    Side,
}

/// The Mica stand-ins for `p`.
pub(super) fn bands(p: &Palette) -> &'static [Color32; 9] {
    if p.dark { &MICA_DARK } else { &MICA_LIGHT }
}

/// What must stay readable on `s`, as (token name, colour, minimum ratio): the floors of
/// docs/COLOUR.md §3. The toolbar never shows faint text.
pub(super) fn checks(p: &Palette, s: Surface) -> Vec<(&'static str, Color32, f64)> {
    let mut out = vec![("text", p.text, 4.5), ("weak", p.weak, 4.5), ("input_border", p.input_border, 3.0)];
    if s == Surface::Side {
        out.extend([
            ("faint", p.faint, 4.5),
            ("accent", p.accent, 3.0),
            ("added", p.status.added, 3.0),
            ("removed", p.status.removed, 3.0),
            ("modified", p.status.modified, 3.0),
            ("info", p.status.info, 3.0),
            ("progress", p.status.progress, 3.0),
            ("warn", p.attention.warn, 3.0),
        ]);
    }
    out
}

/// The surface's own colour in `p`.
pub(super) fn base(p: &Palette, s: Surface) -> Color32 {
    match s {
        Surface::Chrome => p.toolbar,
        Surface::Side => p.panel,
    }
}

/// What `s` looks like at `alpha` over the backdrop colour `b`.
pub(super) fn tinted(p: &Palette, s: Surface, b: Color32, alpha: u8) -> Color32 {
    mix(b, base(p, s), f32::from(alpha) / 255.0)
}

/// Whether every check passes on `s` at `alpha` over every stand-in.
pub(super) fn passes(p: &Palette, s: Surface, alpha: u8) -> bool {
    let checks = checks(p, s);
    bands(p).iter().all(|&b| {
        let c = tinted(p, s, b, alpha);
        checks.iter().all(|&(_, fg, min)| ratio(fg, c) >= min)
    })
}

/// The lowest step at which `s` stays readable, if any.
fn lowest(p: &Palette, s: Surface) -> Option<u8> {
    STEPS.into_iter().find(|&a| passes(p, s, a))
}

/// The tints that keep `p`'s text readable over Mica. A surface that fails even at the highest
/// step stays opaque (`OPAQUE`); None (all solid) when both do.
pub fn mica_tint(p: &Palette) -> Option<MicaTint> {
    match (lowest(p, Surface::Chrome), lowest(p, Surface::Side)) {
        (None, None) => None,
        (chrome, side) => Some(MicaTint { chrome: chrome.unwrap_or(OPAQUE), side: side.unwrap_or(OPAQUE) }),
    }
}

/// The colour surface `s` paints instead of `base`: `base` itself on a solid backdrop, else
/// `base` at the surface's tint.
pub fn fill(b: Backdrop, s: Surface, base: Color32) -> Color32 {
    match b {
        Backdrop::Solid => base,
        Backdrop::Mica { chrome, side } => {
            let alpha = match s {
                Surface::Chrome => chrome,
                Surface::Side => side,
            };
            Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), alpha)
        }
    }
}

#[cfg(test)]
#[path = "backdrop_tests.rs"]
mod tests;
