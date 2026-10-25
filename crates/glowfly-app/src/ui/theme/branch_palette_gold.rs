//! The branch families beside Dusk's gold accent (and Pine's), and beside Dawn's iris. The
//! main branch's lane is the accent, so the hues that would pass for it make room:
//!
//! - Standard, dark: Jade (in the green the gold accent leaves free) takes the Gold slot, at the
//!   tier Gold had.
//! - Colour-blind safe, dark: Yellow and Lime fall within 0.08 OKLab of the gold under protan or
//!   deutan vision, so Orange and a paler Lime take their slots.
//! - Dawn: a gold line dark enough for paper reads brown, so Dawn's accent is an iris violet
//!   (hue 283) beside its gold buttons. Its standard light tones take Jade in the slot Navy
//!   has elsewhere and Forest in Indigo's (the iris's neighbour); its colour-blind set takes
//!   Linen's Wine for Periwinkle.
//!
//! See `branch_palette.rs` for the families these start from and docs/COLOUR.md for the rules.

use super::branch_names::Hue;
use super::branch_palette::{
    CB_DARK_TONES, CB_NAMES, DARK_TONES, FOREST_LINE, FOREST_TEXT, LIGHT_LINES, LIGHT_NAMES, LIGHT_TEXT, NAMES, pad,
};
use super::branch_palette_more::{COBALT_CB_LIGHT_LINES, COBALT_CB_LIGHT_NAMES, COBALT_CB_LIGHT_TEXT};
use super::families::{Branch, LANE_COUNT};
use eframe::egui::Color32;
use glowfly_core::settings::BranchPalette;

const fn hex(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// `t` with slot `i` set to `c`.
pub(super) const fn with<T: Copy, const N: usize>(mut t: [T; N], i: usize, c: T) -> [T; N] {
    t[i] = c;
    t
}

/// The slot Jade takes.
pub const JADE_SLOT: usize = 11;
/// The colour-blind slots that make room: Yellow and Lime.
pub const CB_YELLOW_SLOT: usize = 2;
pub const CB_LIME_SLOT: usize = 5;

/// Jade's dark tone (lines and chips on Dusk).
pub const JADE_DARK: Color32 = hex(0x56d1a3);
/// Jade's light text tone (chips on Dawn).
pub const JADE_LIGHT_TEXT: Color32 = hex(0x176449);
/// Jade's light line tone (lines, dots and bands on Dawn).
pub const JADE_LIGHT_LINE: Color32 = hex(0x2d9570);

/// The slot Forest takes on Dawn: Indigo, beside the iris.
pub const DAWN_FOREST_SLOT: usize = 13;

/// The standard hues with Jade.
pub const GOLD_DARK_TONES: [Color32; LANE_COUNT] = with(DARK_TONES, JADE_SLOT, JADE_DARK);
pub const GOLD_NAMES: [Hue; LANE_COUNT] = with(NAMES, JADE_SLOT, Hue::Jade);
/// Dawn's light tones: Jade and Forest.
pub const DAWN_LIGHT_TEXT: [Color32; LANE_COUNT] = with(with(LIGHT_TEXT, JADE_SLOT, JADE_LIGHT_TEXT), DAWN_FOREST_SLOT, FOREST_TEXT);
pub const DAWN_LIGHT_LINES: [Color32; LANE_COUNT] = with(with(LIGHT_LINES, JADE_SLOT, JADE_LIGHT_LINE), DAWN_FOREST_SLOT, FOREST_LINE);
pub const DAWN_NAMES: [Hue; LANE_COUNT] = with(with(LIGHT_NAMES, JADE_SLOT, Hue::Jade), DAWN_FOREST_SLOT, Hue::Forest);

const CB: usize = glowfly_core::branch_colour::COLOUR_BLIND_SLOTS as usize;

/// The colour-blind safe dark tones beside the gold: Orange and a paler Lime.
pub const GOLD_CB_DARK_TONES: [Color32; CB] = with(with(CB_DARK_TONES, CB_YELLOW_SLOT, hex(0xeb8f13)), CB_LIME_SLOT, hex(0xc8feac));
pub const GOLD_CB_DARK_NAMES: [Hue; CB] = with(CB_NAMES, CB_YELLOW_SLOT, Hue::Orange);

/// Dusk.
pub const BRANCH_DUSK: Branch =
    Branch { lanes: GOLD_DARK_TONES, chips: GOLD_DARK_TONES, preset: BranchPalette::Standard, names: GOLD_NAMES };
/// Dawn.
pub const BRANCH_DAWN: Branch =
    Branch { lanes: DAWN_LIGHT_LINES, chips: DAWN_LIGHT_TEXT, preset: BranchPalette::Standard, names: DAWN_NAMES };
/// Colour-blind safe, on Dusk.
pub const BRANCH_CB_DUSK: Branch = Branch {
    lanes: pad(GOLD_CB_DARK_TONES),
    chips: pad(GOLD_CB_DARK_TONES),
    preset: BranchPalette::ColourBlind,
    names: pad(GOLD_CB_DARK_NAMES),
};
/// Colour-blind safe, on Dawn.
pub const BRANCH_CB_DAWN: Branch = Branch {
    lanes: pad(COBALT_CB_LIGHT_LINES),
    chips: pad(COBALT_CB_LIGHT_TEXT),
    preset: BranchPalette::ColourBlind,
    names: pad(COBALT_CB_LIGHT_NAMES),
};
