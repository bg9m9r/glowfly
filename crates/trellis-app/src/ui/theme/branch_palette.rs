//! The branch hues (P1): 14 hues kept out of the accent band (about 145 to 175 deg OKLCH) and the
//! attention band (35 to 60 deg), neighbouring hues alternating between two lightness tiers. In
//! slot order: Rose, Lime, Violet, Orange, Cyan, Magenta, Olive, Azure, Red, Teal, Orchid, Gold,
//! Sky, Indigo. Each palette kind takes one family: dark tones on dark themes; on Light, bright
//! line tones (3:1 is enough for lines and dots) with darker text tones for chips; on the light
//! themes whose surfaces the line tones fall below 3:1 on, the text tones for both.

use super::families::{Branch, LANE_COUNT};
use eframe::egui::Color32;

const fn hex(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// Lines and chips on dark themes.
pub const DARK_TONES: [Color32; LANE_COUNT] = [
    hex(0xe77492), //  0 Rose
    hex(0xa3cf64), //  1 Lime
    hex(0xc5aeff), //  2 Violet
    hex(0xd78c1a), //  3 Orange
    hex(0x04d4f1), //  4 Cyan
    hex(0xfb97db), //  5 Magenta
    hex(0x9ea827), //  6 Olive
    hex(0x93c1fe), //  7 Azure
    hex(0xfea195), //  8 Red
    hex(0x0ab5b1), //  9 Teal
    hex(0xc37fd8), // 10 Orchid
    hex(0xdfba3b), // 11 Gold
    hex(0x0aabea), // 12 Sky
    hex(0x8794f7), // 13 Indigo
];

/// Chip text and tints on light themes (4.5:1 on Light's surfaces).
pub const LIGHT_TEXT: [Color32; LANE_COUNT] = [
    hex(0x861f43), //  0 Rose
    hex(0x4d7002), //  1 Lime
    hex(0x6d4ea8), //  2 Violet
    hex(0x6c4302), //  3 Orange
    hex(0x057080), //  4 Cyan
    hex(0x953d7c), //  5 Magenta
    hex(0x4d5201), //  6 Olive
    hex(0x2462b1), //  7 Azure
    hex(0xa43c33), //  8 Red
    hex(0x005957), //  9 Teal
    hex(0x6b2d7d), // 10 Orchid
    hex(0x786101), // 11 Gold
    hex(0x005475), // 12 Sky
    hex(0x3b4097), // 13 Indigo
];

/// Lines, dots and bands on Light (3:1 on its background and panel).
pub const LIGHT_LINES: [Color32; LANE_COUNT] = [
    hex(0xc04d6e), //  0 Rose
    hex(0x719c27), //  1 Lime
    hex(0x9776db), //  2 Violet
    hex(0xa86b06), //  3 Orange
    hex(0x0b9db3), //  4 Cyan
    hex(0xc664a8), //  5 Magenta
    hex(0x7a8201), //  6 Olive
    hex(0x498de5), //  7 Azure
    hex(0xd86357), //  8 Red
    hex(0x0a8c89), //  9 Teal
    hex(0x9e59b3), // 10 Orchid
    hex(0xa98904), // 11 Gold
    hex(0x0685b6), // 12 Sky
    hex(0x646fd1), // 13 Indigo
];

/// Dark and High contrast dark.
pub const BRANCH_DARK: Branch = Branch { lanes: DARK_TONES, chips: DARK_TONES };
/// Light.
pub const BRANCH_LIGHT: Branch = Branch { lanes: LIGHT_LINES, chips: LIGHT_TEXT };
/// High contrast light and Sand, whose surfaces need the stronger text tones for lines too.
pub const BRANCH_LIGHT_STRONG: Branch = Branch { lanes: LIGHT_TEXT, chips: LIGHT_TEXT };
