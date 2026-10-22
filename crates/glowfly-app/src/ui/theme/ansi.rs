//! The terminal's colours per built-in theme: its background and text (the theme's `input` and
//! `text`), cursor and selection (the accent), and its 16 basic colours.
//!
//! The rules (ui/theme/ansi_tests.rs checks them; docs/COLOUR.md has the tables): body text
//! reaches 4.5:1 (7:1 in the high-contrast themes) on the background, and dim text 3:1 (4.5:1).
//! - Dark themes: 1-7 and 9-15 are body text; 8 (bright black, dim hints) is dim text; 0
//!   (black, a background) stays at least 1.25:1 off the background, with `text` readable on it.
//! - Light themes: 0-6 and 9-14 are body text; 8 is dim text; 7 and 15 (white, backgrounds)
//!   stay at least 1.25:1 off the background, with `text` readable on them.
//! - 1-6 and 9-14 keep their hue's OKLCH band (red, green, yellow, blue, magenta, cyan), and
//!   each bright colour is at least 0.04 OKLCH lightness off its normal one: lighter in dark
//!   themes, darker in light ones, so bold text (drawn in 8-15) gets stronger.
//!
//! [`hard_to_read`] applies the text rules (at the standard 4.5:1 and 3:1) to any scheme, for
//! Preferences' warning under an imported one.

use super::contrast::ratio;
use super::families::Ansi;
use eframe::egui::Color32;

const fn hex(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// A built-in's terminal colours: background, text and accent, then the 16 basic colours.
const fn table([bg, fg, accent]: [u32; 3], c: [u32; 16]) -> Ansi {
    let mut colors = [Color32::BLACK; 16];
    let mut i = 0;
    while i < 16 {
        colors[i] = hex(c[i]);
        i += 1;
    }
    Ansi { background: hex(bg), foreground: hex(fg), cursor: hex(accent), selection: hex(accent), colors }
}

/// Body text's least contrast on the background, and dim text's (colour 8).
pub const BODY_MIN: f64 = 4.5;
pub const DIM_MIN: f64 = 3.0;

/// Whether a scheme is a dark one: light text on a dark background.
pub fn is_dark(t: &Ansi) -> bool {
    ratio(t.background, Color32::BLACK) < ratio(t.background, Color32::WHITE)
}

/// The basic colours that are body text on a scheme of this kind (see the rules above).
pub fn body_indices(dark: bool) -> Vec<usize> {
    if dark { (1..=7).chain(9..=15).collect() } else { (0..=6).chain(9..=14).collect() }
}

/// What falls below the text rules on `t`'s background, in order: `None` for the foreground,
/// then the basic colours by index (8 against the dim floor).
pub fn hard_to_read(t: &Ansi) -> Vec<Option<usize>> {
    let mut out = Vec::new();
    if ratio(t.foreground, t.background) < BODY_MIN {
        out.push(None);
    }
    let mut body = body_indices(is_dark(t));
    body.push(8);
    body.sort_unstable();
    for i in body {
        let min = if i == 8 { DIM_MIN } else { BODY_MIN };
        if ratio(t.colors[i], t.background) < min {
            out.push(Some(i));
        }
    }
    out
}

pub const GARDEN_DARK: Ansi = table(
    [0x111814, 0xe0e6e3, 0x4dc588],
    [
        0x2a322d, 0xf0726b, 0x7fcc63, 0xe3bd4f, 0x6aa8f5, 0xc28af2, 0x4fc4c4, 0xc9d1cc, //
        0x86918a, 0xff9a92, 0xa3e388, 0xf5d77a, 0x9cc6ff, 0xdcb0ff, 0x80dede, 0xf4f7f5,
    ],
);

pub const GARDEN_LIGHT: Ansi = table(
    [0xffffff, 0x161c19, 0x14764a],
    [
        0x1c2420, 0xc0362f, 0x2f7d1c, 0x8a6400, 0x2a5fc4, 0x8a3fb8, 0x087c80, 0xd3d9d5, //
        0x6b746f, 0xa3241e, 0x226614, 0x6e5000, 0x1d4ba3, 0x70299b, 0x006468, 0xe2e7e4,
    ],
);

/// Dusk: Garden dark's colours over indigo greys.
pub const DUSK: Ansi = table(
    [0x161423, 0xe6e5ef, 0xf6c159],
    [
        0x2b2939, 0xf0726b, 0x7fcc63, 0xe3bd4f, 0x6aa8f5, 0xc28af2, 0x4fc4c4, 0xceccda, //
        0x8c899d, 0xff9a92, 0xa3e388, 0xf5d77a, 0x9cc6ff, 0xdcb0ff, 0x80dede, 0xf5f4fa,
    ],
);

/// Dawn: Garden light's colours over lavender greys.
pub const DAWN: Ansi = table(
    [0xffffff, 0x211f38, 0x714900],
    [
        0x1d1c31, 0xc0362f, 0x2f7d1c, 0x8a6400, 0x2a5fc4, 0x8a3fb8, 0x087c80, 0xd8d6e1, //
        0x686679, 0xa3241e, 0x226614, 0x6e5000, 0x1d4ba3, 0x70299b, 0x006468, 0xe3e2ec,
    ],
);

pub const HIGH_CONTRAST_DARK: Ansi = table(
    [0x000000, 0xffffff, 0x5fe0a0],
    [
        0x303030, 0xff6b6b, 0x5fe36a, 0xffd84a, 0x7ab4ff, 0xe08aff, 0x4fe0e0, 0xe6e6e6, //
        0xa6a6a6, 0xff9c9c, 0x9cf0a2, 0xffec96, 0xa8ccff, 0xeeb5ff, 0x96f0f0, 0xffffff,
    ],
);

pub const HIGH_CONTRAST_LIGHT: Ansi = table(
    [0xffffff, 0x000000, 0x00613a],
    [
        0x000000, 0xa31515, 0x08621a, 0x6b4c00, 0x0646a8, 0x6a1fa0, 0x005f63, 0xd0d0d0, //
        0x55595f, 0x7f0d0d, 0x084f15, 0x523900, 0x043682, 0x52157d, 0x004a4d, 0xe5e5e5,
    ],
);

pub const SAND: Ansi = table(
    [0xfffcf5, 0x2e2519, 0x2a7550],
    [
        0x2e2519, 0xb23a2b, 0x4a7a1a, 0x8a6200, 0x2d65a8, 0x84429e, 0x1b7570, 0xe3d9c6, //
        0x76695a, 0x93291c, 0x3a6312, 0x6d4c00, 0x1f4f8a, 0x6a2f82, 0x0f5d59, 0xeae2d2,
    ],
);
