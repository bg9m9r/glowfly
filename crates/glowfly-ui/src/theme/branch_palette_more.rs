//! The branch families beside the flame accent (Ember) and the cobalt one (Graphite and Linen).
//! As beside Dusk's gold, the hue that would pass for the accent makes room for Jade, in the
//! green these accents leave free, at that hue's own tiers:
//!
//! - Flame: Jade takes the Orange slot. In the colour-blind set, Yellow falls within 0.04
//!   OKLab of the flame under deutan vision, so a light Jade takes its slot.
//! - Cobalt: Jade takes the Azure slot. In the colour-blind sets Periwinkle falls within 0.05
//!   of the cobalt under protan and deutan vision, so a light Rose (dark) and a deep Wine
//!   (light) take its slot. On Linen, Forest takes the light tones' Navy slot (Navy would pass
//!   for the cobalt).
//!
//! See `branch_palette.rs` for the families these start from and docs/COLOUR.md for the rules.

use super::branch_names::Hue;
use super::branch_palette::{
    CB_DARK_TONES, CB_LIGHT_LINES, CB_LIGHT_NAMES, CB_LIGHT_TEXT, CB_NAMES, DARK_TONES, FOREST_LINE, FOREST_TEXT, LIGHT_LINES, LIGHT_NAMES,
    LIGHT_NAVY_SLOT, LIGHT_TEXT, NAMES, pad,
};
use super::branch_palette_gold::{JADE_DARK, JADE_LIGHT_LINE, JADE_LIGHT_TEXT, with};
use super::families::{Branch, LANE_COUNT};
use eframe::egui::Color32;
use glowfly_core::settings::BranchPalette;

const fn hex(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

const CB: usize = glowfly_core::branch_colour::COLOUR_BLIND_SLOTS as usize;

/// The standard slot Jade takes beside the flame: Orange.
pub const FLAME_JADE_SLOT: usize = 3;
/// The colour-blind slot that makes room beside the flame: Yellow.
pub const FLAME_CB_SLOT: usize = 2;
/// The standard slot Jade takes beside the cobalt: Azure.
pub const COBALT_JADE_SLOT: usize = 7;
/// The colour-blind slot that makes room beside the cobalt: Periwinkle.
pub const COBALT_CB_SLOT: usize = 7;

pub const FLAME_DARK_TONES: [Color32; LANE_COUNT] = with(DARK_TONES, FLAME_JADE_SLOT, JADE_DARK);
const FLAME_NAMES: [Hue; LANE_COUNT] = with(NAMES, FLAME_JADE_SLOT, Hue::Jade);
/// A light Jade (OKLCH hue 170) for the colour-blind set's Yellow.
pub const FLAME_CB_DARK_TONES: [Color32; CB] = with(CB_DARK_TONES, FLAME_CB_SLOT, hex(0x57dcb4));
const FLAME_CB_NAMES: [Hue; CB] = with(CB_NAMES, FLAME_CB_SLOT, Hue::Jade);

pub const COBALT_DARK_TONES: [Color32; LANE_COUNT] = with(DARK_TONES, COBALT_JADE_SLOT, JADE_DARK);
pub const COBALT_LIGHT_TEXT: [Color32; LANE_COUNT] =
    with(with(LIGHT_TEXT, COBALT_JADE_SLOT, JADE_LIGHT_TEXT), LIGHT_NAVY_SLOT, FOREST_TEXT);
pub const COBALT_LIGHT_LINES: [Color32; LANE_COUNT] =
    with(with(LIGHT_LINES, COBALT_JADE_SLOT, JADE_LIGHT_LINE), LIGHT_NAVY_SLOT, FOREST_LINE);
const COBALT_NAMES: [Hue; LANE_COUNT] = with(NAMES, COBALT_JADE_SLOT, Hue::Jade);
const LINEN_NAMES: [Hue; LANE_COUNT] = with(with(LIGHT_NAMES, COBALT_JADE_SLOT, Hue::Jade), LIGHT_NAVY_SLOT, Hue::Forest);
/// A light Rose (hue 18) for Periwinkle on Graphite; a deep Wine (hue 350) on Linen, as line and
/// chip tone both.
pub const COBALT_CB_DARK_TONES: [Color32; CB] = with(CB_DARK_TONES, COBALT_CB_SLOT, hex(0xf49194));
const COBALT_CB_DARK_NAMES: [Hue; CB] = with(CB_NAMES, COBALT_CB_SLOT, Hue::Rose);
const WINE: Color32 = hex(0x800953);
pub const COBALT_CB_LIGHT_LINES: [Color32; CB] = with(CB_LIGHT_LINES, COBALT_CB_SLOT, WINE);
pub const COBALT_CB_LIGHT_TEXT: [Color32; CB] = with(CB_LIGHT_TEXT, COBALT_CB_SLOT, WINE);
pub const COBALT_CB_LIGHT_NAMES: [Hue; CB] = with(CB_LIGHT_NAMES, COBALT_CB_SLOT, Hue::Wine);

/// Ember.
pub const BRANCH_EMBER: Branch =
    Branch { lanes: FLAME_DARK_TONES, chips: FLAME_DARK_TONES, preset: BranchPalette::Standard, names: FLAME_NAMES };
/// Colour-blind friendly, on Ember.
pub const BRANCH_CB_EMBER: Branch = Branch {
    lanes: pad(FLAME_CB_DARK_TONES),
    chips: pad(FLAME_CB_DARK_TONES),
    preset: BranchPalette::ColourBlind,
    names: pad(FLAME_CB_NAMES),
};
/// Graphite.
pub const BRANCH_GRAPHITE: Branch =
    Branch { lanes: COBALT_DARK_TONES, chips: COBALT_DARK_TONES, preset: BranchPalette::Standard, names: COBALT_NAMES };
/// Colour-blind friendly, on Graphite.
pub const BRANCH_CB_GRAPHITE: Branch = Branch {
    lanes: pad(COBALT_CB_DARK_TONES),
    chips: pad(COBALT_CB_DARK_TONES),
    preset: BranchPalette::ColourBlind,
    names: pad(COBALT_CB_DARK_NAMES),
};
/// Linen.
pub const BRANCH_LINEN: Branch =
    Branch { lanes: COBALT_LIGHT_LINES, chips: COBALT_LIGHT_TEXT, preset: BranchPalette::Standard, names: LINEN_NAMES };
/// Colour-blind friendly, on Linen.
pub const BRANCH_CB_LINEN: Branch = Branch {
    lanes: pad(COBALT_CB_LIGHT_LINES),
    chips: pad(COBALT_CB_LIGHT_TEXT),
    preset: BranchPalette::ColourBlind,
    names: pad(COBALT_CB_LIGHT_NAMES),
};
