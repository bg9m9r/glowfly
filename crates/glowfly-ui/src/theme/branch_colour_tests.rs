//! The branch hues (P1): out of the reserved bands, one hue per slot across the three tone sets,
//! alternating lightness tiers, and contrast on each built-in's surfaces.

use super::bands::{ATTENTION_BAND, GOLD_BAND, GREEN_BAND, IRIS_BAND};
use super::branch_palette::{
    BRANCH_DARK, BRANCH_LIGHT, BRANCH_LIGHT_STRONG, DARK_TONES, LIGHT_LINES, LIGHT_NAVY_SLOT, LIGHT_STRONG_LINES, LIGHT_TEXT,
    LIGHT_WINE_SLOT, ORANGE_SLOT,
};
use super::branch_palette_gold::{
    BRANCH_DAWN, BRANCH_DUSK, DAWN_FOREST_SLOT, DAWN_LIGHT_LINES, DAWN_LIGHT_TEXT, GOLD_DARK_TONES, JADE_SLOT,
};
use super::contrast::{brownish, oklch, ratio};
use super::tests::palettes;
use super::*;
use eframe::egui::Color32;

fn report(failures: &[String]) {
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The distance between two hues, in degrees around the circle.
fn hue_gap(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

/// A named tone set.
type ToneSet = (&'static str, [Color32; 14]);

fn tone_sets() -> [(&'static str, [Color32; 14]); 3] {
    [("dark", DARK_TONES), ("light text", LIGHT_TEXT), ("light line", LIGHT_LINES)]
}

#[test]
fn p1_outside_reserved_bands() {
    let mut failures = Vec::new();
    let sets: [(&[ToneSet], _); 3] = [
        (&tone_sets(), GREEN_BAND),
        (&[("gold dark", GOLD_DARK_TONES)], GOLD_BAND),
        (&[("dawn text", DAWN_LIGHT_TEXT), ("dawn line", DAWN_LIGHT_LINES)], IRIS_BAND),
    ];
    for (sets, band) in sets {
        for (set, tones) in sets {
            for (i, c) in tones.iter().enumerate() {
                let h = oklch(*c).2;
                if band.contains(&h) || ATTENTION_BAND.contains(&h) {
                    failures.push(format!("{set} slot {i}: hue {h:.1}"));
                }
            }
        }
    }
    report(&failures);
}

#[test]
fn tones_share_one_hue_per_slot() {
    // On paper Olive and Gold give way to Wine and Navy, and Orange turns to a clear amber.
    let mut failures = Vec::new();
    let sets = tone_sets();
    for i in (0..14).filter(|i| ![LIGHT_WINE_SLOT, LIGHT_NAVY_SLOT, ORANGE_SLOT].contains(i)) {
        let dark = oklch(sets[0].1[i]).2;
        for (set, tones) in &sets[1..] {
            let h = oklch(tones[i]).2;
            if hue_gap(dark, h) > 1.5 {
                failures.push(format!("slot {i}: dark {dark:.1} deg, {set} {h:.1} deg"));
            }
        }
    }
    for (slot, want) in [(ORANGE_SLOT, 62.5), (LIGHT_WINE_SLOT, 352.0), (LIGHT_NAVY_SLOT, 262.0)] {
        for (set, tones) in &sets[1..] {
            let h = oklch(tones[slot]).2;
            if hue_gap(want, h) > 1.5 {
                failures.push(format!("{set} slot {slot}: {h:.1} deg, wanted {want}"));
            }
        }
    }
    report(&failures);
}

#[test]
fn no_light_line_reads_brown() {
    let mut failures = Vec::new();
    for (set, tones) in [("light line", LIGHT_LINES), ("strong line", LIGHT_STRONG_LINES), ("dawn line", DAWN_LIGHT_LINES)] {
        for (i, c) in tones.iter().enumerate() {
            if brownish(*c) {
                failures.push(format!("{set} slot {i}: {c:?}"));
            }
        }
    }
    report(&failures);
}

#[test]
fn jade_takes_the_gold_slot_and_nothing_else_changes() {
    for i in (0..14).filter(|i| *i != JADE_SLOT) {
        assert_eq!(DARK_TONES[i], GOLD_DARK_TONES[i], "gold dark slot {i}");
    }
    let (l, c, h) = oklch(GOLD_DARK_TONES[JADE_SLOT]);
    assert!((160.0..=170.0).contains(&h) && c >= 0.08, "Jade at L {l:.2} C {c:.3} H {h:.1}");
    let (std_l, gold_l) = (oklch(DARK_TONES[JADE_SLOT]).0, oklch(GOLD_DARK_TONES[JADE_SLOT]).0);
    assert!((std_l - gold_l).abs() <= 0.03, "Jade keeps Gold's tier: {std_l:.3} and {gold_l:.3}");
    // Dawn's light tones: Jade for Navy, Forest for Indigo (beside the iris), the rest kept.
    for (std, dawn) in [(LIGHT_TEXT, DAWN_LIGHT_TEXT), (LIGHT_LINES, DAWN_LIGHT_LINES)] {
        for i in (0..14).filter(|i| ![JADE_SLOT, DAWN_FOREST_SLOT].contains(i)) {
            assert_eq!(std[i], dawn[i], "dawn slot {i}");
        }
        assert!((160.0..=170.0).contains(&oklch(dawn[JADE_SLOT]).2));
        assert!((137.0..=145.0).contains(&oklch(dawn[DAWN_FOREST_SLOT]).2));
    }
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
    for (t, p) in palettes() {
        let lines = p.branch.lanes.iter().copied().chain([p.accent]);
        for (i, c) in lines.enumerate() {
            for (name, bg) in [("bg", p.bg), ("panel", p.panel)] {
                let r = ratio(c, bg);
                if r < 3.0 {
                    failures.push(format!("{t} line {i} on {name}: {r:.2}:1"));
                }
            }
        }
    }
    report(&failures);
}

#[test]
fn light_chip_tones_reach_4_5() {
    let mut failures = Vec::new();
    for (theme, p) in [("Garden light", &GARDEN_LIGHT), ("Dawn", &DAWN)] {
        for (i, c) in p.branch.chips.iter().enumerate() {
            for (name, bg) in [
                ("bg", p.bg),
                ("panel", p.panel),
                ("toolbar", p.toolbar),
                ("hover", p.hover),
                ("selected", p.selected),
                ("raised", p.raised),
            ] {
                let r = ratio(*c, bg);
                if r < 4.5 {
                    failures.push(format!("{theme} chip {i} on {name}: {r:.2}:1"));
                }
            }
        }
    }
    report(&failures);
}

#[test]
fn builtins_use_their_family() {
    assert_eq!(DUSK.branch, BRANCH_DUSK);
    assert_eq!(DAWN.branch, BRANCH_DAWN);
    assert_eq!(GARDEN_DARK.branch, BRANCH_DARK);
    assert_eq!(HIGH_CONTRAST_DARK.branch, BRANCH_DARK);
    assert_eq!(GARDEN_LIGHT.branch, BRANCH_LIGHT);
    assert_eq!(HIGH_CONTRAST_LIGHT.branch, BRANCH_LIGHT_STRONG);
}

#[test]
fn main_is_the_accent_and_slots_read_their_tones() {
    use super::branch_colours::{chip_in, line_in};
    use glowfly_core::branch_colour::Swatch;
    assert_eq!((line_in(Swatch::Main, &LIGHT), chip_in(Swatch::Main, &LIGHT)), (LIGHT.accent, LIGHT.accent));
    assert_eq!((line_in(Swatch::Slot(4), &GARDEN_LIGHT), chip_in(Swatch::Slot(4), &GARDEN_LIGHT)), (LIGHT_LINES[4], LIGHT_TEXT[4]));
    assert_eq!(line_in(Swatch::Slot(13), &GARDEN_DARK), DARK_TONES[13]);
    assert_eq!(line_in(Swatch::Slot(11), &DUSK), GOLD_DARK_TONES[11]);
}
