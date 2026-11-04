//! A person's initials colours, in the graph's nodes and the commit details: one of eight muted
//! tones (`glowfly_core::branch_colour::person_tone`, from their email) behind `text` initials.
//! The tones are not branch hues, so the lane-coloured ring around the initials and the author
//! inside it read as two marks; they ignore the branch preset and colour-blind mode. Each is an
//! OKLCH colour a step lighter than the background (at least lightness 0.30; darker on a light
//! one) at low chroma, its
//! eight hues clear of the brown zone; a fill too close to `text` steps back toward the
//! background until the initials read.

use super::color::{self, Palette};
use super::contrast::{from_oklch, max_chroma, oklch, ratio};
use eframe::egui::Color32;
use glowfly_core::branch_colour::{PERSON_TONES, person_tone};

/// The tones' OKLCH hues in degrees, by tone: red-pink, then green round to magenta, skipping
/// the warm 20 to 125 deg that reads brown at this chroma.
const HUES: [f64; PERSON_TONES as usize] = [0.0, 135.0, 165.0, 195.0, 228.0, 260.0, 292.0, 325.0];
/// The tones' OKLCH chroma (at most what sRGB holds at their lightness and hue).
const CHROMA: f64 = 0.055;
/// How far a tone's OKLCH lightness sits from the background's: lighter on a dark palette,
/// darker on a light one.
const STEP_DARK: f64 = 0.14;
const STEP_LIGHT: f64 = -0.10;
/// The least OKLCH lightness of a dark palette's tones, so they show on a black background
/// (high contrast dark); stepping back for the ink never goes under it.
const FLOOR_DARK: f64 = 0.30;
/// How far a fill steps back toward the background while its initials don't read.
const BACK: f64 = 0.01;

/// The (fill, ink) of the initials of the person with `email` (or, without one, `name`) in the
/// active palette.
pub fn initials_tones(email: &str, name: &str) -> (Color32, Color32) {
    tone_in(person_tone(email, name), &color::current())
}

/// The initials' contrast target: 4.5:1, or 7:1 in the high-contrast themes.
pub(super) fn ink_target(p: &Palette) -> f64 {
    if p.high_contrast { 7.0 } else { 4.5 }
}

/// Tone `i`'s (fill, ink) in `p`.
pub(super) fn tone_in(i: u8, p: &Palette) -> (Color32, Color32) {
    let hue = HUES[usize::from(i) % HUES.len()];
    let (bg_l, _, _) = oklch(p.bg);
    let at = |l: f64| from_oklch(l, CHROMA.min(max_chroma(l, hue)), hue);
    if p.dark {
        // From the step above the background (or the floor) down toward it, never under the floor.
        let mut l = (bg_l + STEP_DARK).max(FLOOR_DARK);
        while ratio(p.text, at(l)) < ink_target(p) && l - BACK >= FLOOR_DARK.max(bg_l + BACK) {
            l -= BACK;
        }
        return (at(l.min(1.0)), p.text);
    }
    let mut step = STEP_LIGHT;
    loop {
        let l = (bg_l + step).clamp(0.0, 1.0);
        if ratio(p.text, at(l)) >= ink_target(p) || step.abs() <= BACK {
            return (at(l), p.text);
        }
        step += BACK;
    }
}

#[cfg(test)]
#[path = "initials_tests.rs"]
mod tests;
