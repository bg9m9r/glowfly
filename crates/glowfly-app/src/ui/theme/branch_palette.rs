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
//! On paper a gold or olive dark enough for 3:1 reads brown (docs/COLOUR.md, "No brown"), so the
//! light tones take a deep Wine in Olive's slot and a Navy in Gold's, and their Orange is a clear
//! amber at hue 62, the one warm line that stays clean at 3:1 on every light theme.
//!
//! Colour-blind safe: 8 hues that stay apart under protan, deutan and tritan vision as well as
//! normal vision (Red, Aqua, Yellow, Blue, Plum, Lime, Purple, Periwinkle), out of the same
//! bands. Dark themes take the dark tones for lines and chips; every light theme takes the line
//! tones for lines and the text tones for chips. `applied` puts the chosen preset into a palette.
//!
//! These are the families of the green-accent themes (Garden, high contrast, Sand); Dusk and
//! Dawn's gold accent takes the families in `branch_palette_gold.rs`.

use super::branch_names::Hue;
use super::color::Palette;
use super::families::{Branch, LANE_COUNT};
use eframe::egui::Color32;
use glowfly_core::settings::BranchPalette;

const fn hex(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// The light tones' clear amber (OKLCH hue 62, chroma at the sRGB edge): 3:1 on Sand's paper
/// as a line; its text tone (4.5:1) is a deep amber, lettered in the ink (`contrast::clean_ink`).
pub const AMBER_LINE: Color32 = hex(0xc47100);
pub const AMBER_TEXT: Color32 = hex(0x7c4600);
/// Wine (hue 352) and Navy (hue 262), darker than their neighbours, in Olive's and Gold's
/// light slots; Forest (hue 142) where the accent leaves the green free (Dawn, Linen).
pub const WINE_LINE: Color32 = hex(0x952c63);
pub const WINE_TEXT: Color32 = hex(0x6f1647);
pub const NAVY_LINE: Color32 = hex(0x2750a2);
pub const NAVY_TEXT: Color32 = hex(0x17397b);
pub const FOREST_LINE: Color32 = hex(0x2c6b27);
pub const FOREST_TEXT: Color32 = hex(0x1d4f19);
/// The light slots that take another hue than the dark tones': Olive (Wine) and Gold (Navy).
pub const LIGHT_WINE_SLOT: usize = 6;
pub const LIGHT_NAVY_SLOT: usize = 11;
/// The Orange slot, whose light tones turn to hue 62.
pub const ORANGE_SLOT: usize = 3;

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
    AMBER_TEXT,    //  3 Orange
    hex(0x057080), //  4 Cyan
    hex(0x953d7c), //  5 Magenta
    WINE_TEXT,     //  6 Wine
    hex(0x2462b1), //  7 Azure
    hex(0xa43c33), //  8 Red
    hex(0x005957), //  9 Teal
    hex(0x6b2d7d), // 10 Orchid
    NAVY_TEXT,     // 11 Navy
    hex(0x005475), // 12 Sky
    hex(0x3b4097), // 13 Indigo
];

/// Lines, dots and bands on Light (3:1 on its background and panel).
pub const LIGHT_LINES: [Color32; LANE_COUNT] = [
    hex(0xc04d6e), //  0 Rose
    hex(0x719c27), //  1 Lime
    hex(0x9776db), //  2 Violet
    AMBER_LINE,    //  3 Orange
    hex(0x0b9db3), //  4 Cyan
    hex(0xc664a8), //  5 Magenta
    WINE_LINE,     //  6 Wine
    hex(0x498de5), //  7 Azure
    hex(0xd86357), //  8 Red
    hex(0x0a8c89), //  9 Teal
    hex(0x9e59b3), // 10 Orchid
    NAVY_LINE,     // 11 Navy
    hex(0x0685b6), // 12 Sky
    hex(0x646fd1), // 13 Indigo
];

/// The standard hues' names, slot by slot.
pub const NAMES: [Hue; LANE_COUNT] = [
    Hue::Rose,
    Hue::Lime,
    Hue::Violet,
    Hue::Orange,
    Hue::Cyan,
    Hue::Magenta,
    Hue::Olive,
    Hue::Azure,
    Hue::Red,
    Hue::Teal,
    Hue::Orchid,
    Hue::Gold,
    Hue::Sky,
    Hue::Indigo,
];

/// The light tones' names: Wine and Navy for Olive and Gold.
pub const LIGHT_NAMES: [Hue; LANE_COUNT] = {
    let mut n = NAMES;
    n[LIGHT_WINE_SLOT] = Hue::Wine;
    n[LIGHT_NAVY_SLOT] = Hue::Navy;
    n
};

/// Lines on High contrast light and Sand: the text tones, with the amber line for Orange (a
/// text-strength amber reads brown).
pub const LIGHT_STRONG_LINES: [Color32; LANE_COUNT] = {
    let mut t = LIGHT_TEXT;
    t[ORANGE_SLOT] = AMBER_LINE;
    t
};

/// Garden dark and High contrast dark.
pub const BRANCH_DARK: Branch = Branch { lanes: DARK_TONES, chips: DARK_TONES, preset: BranchPalette::Standard, names: NAMES };
/// Garden light.
pub const BRANCH_LIGHT: Branch = Branch { lanes: LIGHT_LINES, chips: LIGHT_TEXT, preset: BranchPalette::Standard, names: LIGHT_NAMES };
/// High contrast light and Sand, whose surfaces need the stronger text tones for lines too.
pub const BRANCH_LIGHT_STRONG: Branch =
    Branch { lanes: LIGHT_STRONG_LINES, chips: LIGHT_TEXT, preset: BranchPalette::Standard, names: LIGHT_NAMES };

/// How many hues the colour-blind safe preset has.
const CB: usize = glowfly_core::branch_colour::COLOUR_BLIND_SLOTS as usize;

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

/// Colour-blind safe lines, dots and bands on light themes (3:1 on the background and panel
/// of Light, High contrast light and Sand). A yellow dark enough reads olive on paper, so the
/// light set's Yellow is the clear amber, and its Lime a deeper green to stay apart from the
/// amber under protan and deutan vision.
pub const CB_LIGHT_LINES: [Color32; CB] = [
    hex(0xb17272), // 0 Red
    hex(0x0d9295), // 1 Aqua
    AMBER_LINE,    // 2 Orange
    hex(0x207aa0), // 3 Blue
    hex(0xb2397b), // 4 Plum
    hex(0x4d7002), // 5 Lime
    hex(0x9169d5), // 6 Purple
    hex(0x3a45cf), // 7 Periwinkle
];

/// Colour-blind safe chip text and tints on light themes (4.5:1 on their surfaces).
pub const CB_LIGHT_TEXT: [Color32; CB] = [
    hex(0x8b0422), // 0 Red
    hex(0x0a4b4b), // 1 Aqua
    AMBER_TEXT,    // 2 Orange
    hex(0x106082), // 3 Blue
    hex(0xac1c6f), // 4 Plum
    hex(0x426101), // 5 Lime
    hex(0x482b74), // 6 Purple
    hex(0x4158bd), // 7 Periwinkle
];

/// The colour-blind safe hues' names, slot by slot.
pub const CB_NAMES: [Hue; CB] = [Hue::Red, Hue::Aqua, Hue::Yellow, Hue::Blue, Hue::Plum, Hue::Lime, Hue::Purple, Hue::Periwinkle];
/// The light colour-blind set's names: Orange for Yellow.
pub const CB_LIGHT_NAMES: [Hue; CB] = [Hue::Red, Hue::Aqua, Hue::Orange, Hue::Blue, Hue::Plum, Hue::Lime, Hue::Purple, Hue::Periwinkle];

/// `t` repeated to fill every lane, so a slot past the preset's hues still reads a colour.
pub const fn pad<T: Copy>(t: [T; CB]) -> [T; LANE_COUNT] {
    let mut out = [t[0]; LANE_COUNT];
    let mut i = 0;
    while i < LANE_COUNT {
        out[i] = t[i % CB];
        i += 1;
    }
    out
}

/// Colour-blind safe, on Garden dark and High contrast dark.
pub const BRANCH_CB_DARK: Branch =
    Branch { lanes: pad(CB_DARK_TONES), chips: pad(CB_DARK_TONES), preset: BranchPalette::ColourBlind, names: pad(CB_NAMES) };
/// Colour-blind safe, on Garden light, High contrast light and Sand.
pub const BRANCH_CB_LIGHT: Branch =
    Branch { lanes: pad(CB_LIGHT_LINES), chips: pad(CB_LIGHT_TEXT), preset: BranchPalette::ColourBlind, names: pad(CB_LIGHT_NAMES) };

/// `p` with the branch preset `b`: Standard keeps its own family; the colour-blind safe preset
/// takes the palette's own colour-blind family (a theme file's from its base).
pub fn applied(mut p: Palette, b: BranchPalette) -> Palette {
    if b == BranchPalette::ColourBlind {
        p.branch = p.colour_blind;
    }
    p
}
