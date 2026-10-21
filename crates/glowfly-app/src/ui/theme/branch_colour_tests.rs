//! The branch hues (P1): out of the reserved bands, one hue per slot across the three tone sets,
//! alternating lightness tiers, and contrast on each built-in's surfaces.

use super::branch_palette::{BRANCH_DARK, BRANCH_LIGHT, BRANCH_LIGHT_STRONG, DARK_TONES, LIGHT_LINES, LIGHT_TEXT};
use super::contrast::{oklch, ratio};
use super::*;
use eframe::egui::Color32;
use glowfly_core::settings::BuiltinTheme;

const ACCENT_BAND: std::ops::RangeInclusive<f64> = 145.0..=175.0;
const ATTENTION_BAND: std::ops::RangeInclusive<f64> = 35.0..=60.0;

fn report(failures: &[String]) {
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The distance between two hues, in degrees around the circle.
fn hue_gap(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

fn tone_sets() -> [(&'static str, [Color32; 14]); 3] {
    [("dark", DARK_TONES), ("light text", LIGHT_TEXT), ("light line", LIGHT_LINES)]
}

#[test]
fn p1_outside_reserved_bands() {
    let mut failures = Vec::new();
    for (set, tones) in tone_sets() {
        for (i, c) in tones.iter().enumerate() {
            let h = oklch(*c).2;
            if ACCENT_BAND.contains(&h) || ATTENTION_BAND.contains(&h) {
                failures.push(format!("{set} slot {i}: hue {h:.1}"));
            }
        }
    }
    report(&failures);
}

#[test]
fn tones_share_one_hue_per_slot() {
    let mut failures = Vec::new();
    for i in 0..14 {
        let dark = oklch(DARK_TONES[i]).2;
        for (set, tones) in &tone_sets()[1..] {
            let h = oklch(tones[i]).2;
            if hue_gap(dark, h) > 1.5 {
                failures.push(format!("slot {i}: dark {dark:.1} deg, {set} {h:.1} deg"));
            }
        }
    }
    report(&failures);
}

#[test]
fn hues_alternate_tiers() {
    let mut tones: Vec<(f64, f64, f64)> = DARK_TONES.iter().map(|c| oklch(*c)).collect();
    tones.sort_by(|a, b| a.2.total_cmp(&b.2));
    let mut failures = Vec::new();
    for (k, (l, c, h)) in tones.iter().enumerate() {
        let want = if k % 2 == 0 { 0.70 } else { 0.80 };
        if (l - want).abs() > 0.015 || !(0.09..=0.15).contains(c) {
            failures.push(format!("hue {h:.1} (#{k} by hue): L {l:.3} (want {want}), C {c:.3}"));
        }
    }
    report(&failures);
}

#[test]
fn lines_reach_3_to_1() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let lines = p.branch.lanes.iter().copied().chain([p.accent]);
        for (i, c) in lines.enumerate() {
            for (name, bg) in [("bg", p.bg), ("panel", p.panel)] {
                let r = ratio(c, bg);
                if r < 3.0 {
                    failures.push(format!("{t:?} line {i} on {name}: {r:.2}:1"));
                }
            }
        }
    }
    report(&failures);
}

#[test]
fn light_chip_tones_reach_4_5() {
    let p = &LIGHT;
    let mut failures = Vec::new();
    for (i, c) in p.branch.chips.iter().enumerate() {
        for (name, bg) in
            [("bg", p.bg), ("panel", p.panel), ("toolbar", p.toolbar), ("hover", p.hover), ("selected", p.selected), ("raised", p.raised)]
        {
            let r = ratio(*c, bg);
            if r < 4.5 {
                failures.push(format!("chip {i} on {name}: {r:.2}:1"));
            }
        }
    }
    report(&failures);
}

#[test]
fn builtins_use_their_family() {
    assert_eq!(DARK.branch, BRANCH_DARK);
    assert_eq!(HIGH_CONTRAST_DARK.branch, BRANCH_DARK);
    assert_eq!(LIGHT.branch, BRANCH_LIGHT);
    assert_eq!(HIGH_CONTRAST_LIGHT.branch, BRANCH_LIGHT_STRONG);
    assert_eq!(SAND.branch, BRANCH_LIGHT_STRONG);
}

#[test]
fn main_is_the_accent_and_slots_read_their_tones() {
    use super::branch_colours::{chip_in, line_in};
    use glowfly_core::branch_colour::Swatch;
    assert_eq!((line_in(Swatch::Main, &LIGHT), chip_in(Swatch::Main, &LIGHT)), (LIGHT.accent, LIGHT.accent));
    assert_eq!((line_in(Swatch::Slot(4), &LIGHT), chip_in(Swatch::Slot(4), &LIGHT)), (LIGHT_LINES[4], LIGHT_TEXT[4]));
    assert_eq!(line_in(Swatch::Slot(13), &DARK), DARK_TONES[13]);
}
