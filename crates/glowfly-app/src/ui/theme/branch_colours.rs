//! A branch swatch's colours in a palette: the accent for the main branch, else its slot's line
//! tone (lines, dots, bands) or chip tone (ref chips). Also a person's colours: the initials in
//! the graph and the commit details take a slot from the author's email (never the accent).

use super::color::{self, Palette};
use super::contrast::{mix, readable_on};
use super::families::LANE_COUNT;
use eframe::egui::Color32;
use glowfly_core::branch_colour::{Swatch, person_slot};

/// How far the fill behind initials moves from the background toward the person's line tone.
pub const INITIALS_FILL: f32 = 0.30;

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

/// The swatch of the person with `email` (or, without one, `name`) in the active preset.
pub fn person(email: &str, name: &str) -> Swatch {
    Swatch::Slot(person_slot(email, name, color::branch_preset().slots()))
}

/// The (fill, ink) of a person's initials in the active palette: a muted tint of their line tone,
/// and their chip tone pushed until it reads at 4.5:1 on that fill.
pub fn initials_tones(email: &str, name: &str) -> (Color32, Color32) {
    let p = color::current();
    let s = person(email, name);
    let fill = mix(p.bg, line(s), INITIALS_FILL);
    (fill, readable_on(chip_in(s, &p), fill, p.dark, 4.5))
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
    std::iter::once(Swatch::Main).chain((0..glowfly_core::branch_colour::SLOTS).map(Swatch::Slot))
}
