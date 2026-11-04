//! A branch swatch's colours in a palette: the trunk tone for the main branch without a pin (the
//! `weak` ink, `text` in high contrast; never the accent), else its slot's line tone (lines,
//! dots, bands) or chip tone (ref chips). A person's initials colours are in `initials.rs`.

use super::color::{self, Palette};
use super::families::LANE_COUNT;
use super::vision::{self, VISIONS};
use eframe::egui::Color32;
use glowfly_core::branch_colour::Swatch;
use glowfly_core::settings::ColourPalette;

/// `s`'s line tone in the active palette.
pub fn line(s: Swatch) -> Color32 {
    match s {
        Swatch::Trunk => trunk(&color::current()),
        Swatch::Slot(i) => color::lane(usize::from(i)),
    }
}

/// The trunk tone in `p`, for the main branch's line and its chip's edge: `weak`, or `text` in
/// the high-contrast themes.
pub fn trunk(p: &Palette) -> Color32 {
    if p.high_contrast { p.text } else { p.weak }
}

/// The slots whose line tone passes for the trunk in `p` (bit `i` for slot `i`): under the
/// line floor (`vision::floor`) from the trunk tone in a vision the preset is built for, normal
/// vision for the standard hues, every vision for the colour-blind ones. An automatic branch
/// beside the trunk skips them (`Colouring::trunk_near`).
pub fn trunk_near(p: &Palette) -> u16 {
    let trunk = trunk(p);
    let visions = if p.branch.preset == ColourPalette::ColourBlind { &VISIONS[..] } else { &VISIONS[..1] };
    let floor = vision::floor(p.dark);
    let near = |i: usize| visions.iter().any(|(_, m)| vision::distance(p.branch.lanes[i], trunk, m) < floor);
    (0..usize::from(p.branch.preset.slots())).filter(|&i| near(i)).fold(0, |mask, i| mask | 1 << i)
}

/// `s`'s chip tone in the active palette.
pub fn chip(s: Swatch) -> Color32 {
    chip_in(s, &color::current())
}

/// `s`'s line tone in `p` (`line` reads the active palette's).
pub fn line_in(s: Swatch, p: &Palette) -> Color32 {
    match s {
        Swatch::Trunk => trunk(p),
        Swatch::Slot(i) => p.branch.lanes[usize::from(i) % LANE_COUNT],
    }
}

/// `s`'s chip tone in `p`.
pub fn chip_in(s: Swatch, p: &Palette) -> Color32 {
    match s {
        Swatch::Trunk => trunk(p),
        Swatch::Slot(i) => p.branch.chips[usize::from(i) % LANE_COUNT],
    }
}

/// Every swatch: Trunk, then the slots in palette order (for tests).
#[cfg(test)]
pub fn swatches() -> impl Iterator<Item = Swatch> {
    std::iter::once(Swatch::Trunk).chain((0..glowfly_core::branch_colour::SLOTS).map(Swatch::Slot))
}
