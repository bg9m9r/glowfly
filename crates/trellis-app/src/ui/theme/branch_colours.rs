//! A branch swatch's colours in a palette: the accent for the main branch, else its slot's line
//! tone (lines, dots, bands) or chip tone (ref chips).

use super::color::{self, Palette};
use super::families::LANE_COUNT;
use eframe::egui::Color32;
use trellis_core::branch_colour::Swatch;

/// `s`'s line tone in the active palette.
pub fn line(s: Swatch) -> Color32 {
    match s {
        Swatch::Main => color::accent(),
        Swatch::Slot(i) => color::lane(usize::from(i)),
    }
}

/// `s`'s chip tone in the active palette.
pub fn chip(s: Swatch) -> Color32 {
    chip_in(s, &color::current())
}

/// `s`'s line tone in `p` (`line` reads the active palette's).
#[cfg(test)]
pub fn line_in(s: Swatch, p: &Palette) -> Color32 {
    match s {
        Swatch::Main => p.accent,
        Swatch::Slot(i) => p.branch.lanes[usize::from(i) % LANE_COUNT],
    }
}

/// `s`'s chip tone in `p`.
pub fn chip_in(s: Swatch, p: &Palette) -> Color32 {
    match s {
        Swatch::Main => p.accent,
        Swatch::Slot(i) => p.branch.chips[usize::from(i) % LANE_COUNT],
    }
}

/// Every swatch: Main, then the slots in palette order (for tests).
#[cfg(test)]
pub fn swatches() -> impl Iterator<Item = Swatch> {
    std::iter::once(Swatch::Main).chain((0..trellis_core::branch_colour::SLOTS).map(Swatch::Slot))
}
