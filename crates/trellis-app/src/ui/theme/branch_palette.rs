//! The branch hues, in two presets (Preferences > UI Customization > Commit graph > Branch
//! colors).
//!
//! Standard (P1): 14 hues kept out of the accent band (about 145 to 175 deg OKLCH) and the
//! attention band (35 to 60 deg), neighbouring hues alternating between two lightness tiers. In
//! slot order: Rose, Lime, Violet, Orange, Cyan, Magenta, Olive, Azure, Red, Teal, Orchid, Gold,
//! Sky, Indigo. Each palette kind takes one family: dark tones on dark themes; on Light, bright
//! line tones (3:1 is enough for lines and dots) with darker text tones for chips; on the light
//! themes whose surfaces the line tones fall below 3:1 on, the text tones for both.
//!
//! Colour-blind safe: 8 hues that stay apart under protan, deutan and tritan vision as well as
//! normal vision (Red, Aqua, Yellow, Blue, Plum, Lime, Purple, Periwinkle), out of the same
//! bands. Dark themes take the dark tones for lines and chips; every light theme takes the line
//! tones for lines and the text tones for chips. `applied` puts the chosen preset into a palette.

use super::color::Palette;
use super::families::{Branch, LANE_COUNT};
use eframe::egui::Color32;
use trellis_core::settings::BranchPalette;

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
pub const BRANCH_DARK: Branch = Branch { lanes: DARK_TONES, chips: DARK_TONES, preset: BranchPalette::Standard };
/// Light.
pub const BRANCH_LIGHT: Branch = Branch { lanes: LIGHT_LINES, chips: LIGHT_TEXT, preset: BranchPalette::Standard };
/// High contrast light and Sand, whose surfaces need the stronger text tones for lines too.
pub const BRANCH_LIGHT_STRONG: Branch = Branch { lanes: LIGHT_TEXT, chips: LIGHT_TEXT, preset: BranchPalette::Standard };

/// How many hues the colour-blind safe preset has.
const CB: usize = trellis_core::branch_colour::COLOUR_BLIND_SLOTS as usize;

/// Colour-blind safe lines and chips on dark themes (4.2:1 on Dark's and High contrast dark's
/// background and panel).
pub const CB_DARK_TONES: [Color32; CB] = [
    hex(0xed485a), // 0 Red
    hex(0x45fafc), // 1 Aqua
    hex(0xd1c038), // 2 Yellow
    hex(0x2898c9), // 3 Blue
    hex(0xba7294), // 4 Plum
    hex(0xccee6e), // 5 Lime
    hex(0x9d62f4), // 6 Purple
    hex(0xa2b5f2), // 7 Periwinkle
];

/// Colour-blind safe lines, dots and bands on light themes (3.1:1 on the background and panel
/// of Light, High contrast light and Sand).
pub const CB_LIGHT_LINES: [Color32; CB] = [
    hex(0xb17272), // 0 Red
    hex(0x0d9295), // 1 Aqua
    hex(0x70640c), // 2 Yellow
    hex(0x207aa0), // 3 Blue
    hex(0xb2397b), // 4 Plum
    hex(0x668014), // 5 Lime
    hex(0x9169d5), // 6 Purple
    hex(0x3a45cf), // 7 Periwinkle
];

/// Colour-blind safe chip text and tints on light themes (4.5:1 on their surfaces).
pub const CB_LIGHT_TEXT: [Color32; CB] = [
    hex(0x8b0422), // 0 Red
    hex(0x0a4b4b), // 1 Aqua
    hex(0x474219), // 2 Yellow
    hex(0x106082), // 3 Blue
    hex(0xac1c6f), // 4 Plum
    hex(0x56680f), // 5 Lime
    hex(0x482b74), // 6 Purple
    hex(0x4158bd), // 7 Periwinkle
];

/// `t` repeated to fill every lane, so a slot past the preset's hues still reads a colour.
pub const fn pad(t: [Color32; CB]) -> [Color32; LANE_COUNT] {
    let mut out = [Color32::BLACK; LANE_COUNT];
    let mut i = 0;
    while i < LANE_COUNT {
        out[i] = t[i % CB];
        i += 1;
    }
    out
}

/// Colour-blind safe, on Dark and High contrast dark.
pub const BRANCH_CB_DARK: Branch = Branch { lanes: pad(CB_DARK_TONES), chips: pad(CB_DARK_TONES), preset: BranchPalette::ColourBlind };
/// Colour-blind safe, on Light, High contrast light and Sand.
pub const BRANCH_CB_LIGHT: Branch = Branch { lanes: pad(CB_LIGHT_LINES), chips: pad(CB_LIGHT_TEXT), preset: BranchPalette::ColourBlind };

/// `p` with the branch preset `b`: Standard keeps its own family; the colour-blind safe preset
/// takes the dark or light family by the palette's kind (a theme file's through its base).
pub fn applied(mut p: Palette, b: BranchPalette) -> Palette {
    if b == BranchPalette::ColourBlind {
        p.branch = if p.dark { BRANCH_CB_DARK } else { BRANCH_CB_LIGHT };
    }
    p
}
