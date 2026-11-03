//! A branch swatch's colours in a palette: the trunk tone for the main branch without a pin (the
//! `weak` ink, `text` in high contrast; never the accent), else its slot's line tone (lines,
//! dots, bands) or chip tone (ref chips). Also a person's colours: the initials in the graph and
//! the commit details take a slot from the author's email.

use super::color::{self, Palette};
use super::contrast::{clean_ink, mix, unbrown};
use super::families::LANE_COUNT;
use super::vision::{self, VISIONS};
use eframe::egui::Color32;
use glowfly_core::branch_colour::{Swatch, person_slot};
use glowfly_core::settings::ColourPalette;

/// How far the fill behind initials moves from the background toward the person's line tone.
const INITIALS_FILL: f32 = 0.30;

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

/// The swatch of the person with `email` (or, without one, `name`) in the active preset.
pub fn person(email: &str, name: &str) -> Swatch {
    Swatch::Slot(person_slot(email, name, color::branch_preset().slots()))
}

/// The (fill, ink) of a person's initials in the active palette: a muted tint of their line tone,
/// and their chip tone pushed until it reads at 4.5:1 on that fill (the ink, if that is brown).
pub fn initials_tones(email: &str, name: &str) -> (Color32, Color32) {
    let p = color::current();
    let s = person(email, name);
    let fill = initials_fill(line(s), &p);
    (fill, clean_ink(chip_in(s, &p), fill, &p))
}

/// The fill behind the initials of a person whose line tone is `lane`.
pub fn initials_fill(lane: Color32, p: &Palette) -> Color32 {
    unbrown(p.bg, mix(p.bg, lane, INITIALS_FILL))
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
