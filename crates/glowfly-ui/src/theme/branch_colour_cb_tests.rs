//! The colour-blind friendly branch hues: out of the reserved bands, one hue per slot across the three
//! tone sets, contrast on each built-in's surfaces once applied, and distance between every pair
//! (and from the accent, with the trunk's near slots) under normal, protan, deutan and tritan vision.
//! The vision simulation and the floors are `vision.rs`'s.

use super::bands::{ATTENTION_BAND, GOLD_BAND, GREEN_BAND};
use super::branch_palette::{BRANCH_CB_DARK, BRANCH_CB_LIGHT, BRANCH_DARK, CB_DARK_TONES, CB_LIGHT_LINES, CB_LIGHT_TEXT, DARK_TONES, pad};
use super::branch_palette_gold::{BRANCH_CB_DAWN, BRANCH_CB_DUSK, GOLD_CB_DARK_TONES};
use super::branch_palette_more::{BRANCH_CB_GRAPHITE, BRANCH_CB_LINEN};
use super::contrast::{oklch, ratio};
use super::tests::palettes;
use super::vision::{DARK_FLOOR, DEUTAN, LIGHT_FLOOR, VISIONS, distance};
use super::*;
use eframe::egui::Color32;
use glowfly_core::settings::{BranchPalette, BuiltinTheme};

fn report(failures: &[String]) {
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Every pair of `tones` closer than `floor` in some vision, and every tone that close to an
/// accent in `accents`.
pub(super) fn too_close(set: &str, tones: &[Color32], accents: &[Color32], floor: f64) -> Vec<String> {
    let mut out = Vec::new();
    for (vision, m) in &VISIONS {
        for i in 0..tones.len() {
            for j in i + 1..tones.len() {
                let d = distance(tones[i], tones[j], m);
                if d < floor {
                    out.push(format!("{set} {vision}: slots {i} and {j} at {d:.3}"));
                }
            }
            for a in accents {
                let d = distance(tones[i], *a, m);
                if d < floor {
                    out.push(format!("{set} {vision}: slot {i} and accent {a:?} at {d:.3}"));
                }
            }
        }
    }
    out
}

fn hue_gap(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

#[test]
fn cb_tones_outside_reserved_bands() {
    let mut failures = Vec::new();
    // The green-accent sets keep out of the green band, the gold one (Dusk's) out of the gold;
    // Dawn's light tones are the green-accent ones and keep out of both.
    let sets = [
        ("dark", CB_DARK_TONES, &[GREEN_BAND][..]),
        ("light line", CB_LIGHT_LINES, &[GREEN_BAND, GOLD_BAND][..]),
        ("light text", CB_LIGHT_TEXT, &[GREEN_BAND, GOLD_BAND][..]),
        ("gold dark", GOLD_CB_DARK_TONES, &[GOLD_BAND][..]),
    ];
    for (set, tones, bands) in sets {
        for (i, c) in tones.iter().enumerate() {
            let h = oklch(*c).2;
            if bands.iter().any(|b| b.contains(&h)) || ATTENTION_BAND.contains(&h) {
                failures.push(format!("{set} slot {i}: hue {h:.1}"));
            }
        }
    }
    report(&failures);
}

#[test]
fn cb_tones_share_one_hue_per_slot() {
    let mut failures = Vec::new();
    for i in (0..8).filter(|i| ![2, 5].contains(i)) {
        assert_eq!(CB_DARK_TONES[i], GOLD_CB_DARK_TONES[i], "Dusk's set changes only Yellow and Lime");
    }
    // On paper Yellow is the clear amber (a dark yellow reads olive) and Lime a greener one.
    for i in (0..8).filter(|i| ![2, 5].contains(i)) {
        let dark = oklch(CB_DARK_TONES[i]).2;
        for (set, tones) in [("light line", CB_LIGHT_LINES), ("light text", CB_LIGHT_TEXT)] {
            let h = oklch(tones[i]).2;
            if hue_gap(dark, h) > 1.5 {
                failures.push(format!("slot {i}: dark {dark:.1} deg, {set} {h:.1} deg"));
            }
        }
    }
    report(&failures);
}

#[test]
fn applied_cb_lines_reach_3_to_1_and_light_chips_4_5() {
    let mut failures = Vec::new();
    for (t, p) in palettes() {
        let p = applied(*p, BranchPalette::ColourBlind);
        for (i, c) in p.branch.lanes.iter().copied().chain([super::branch_colours::trunk(&p)]).enumerate() {
            for (name, bg) in [("bg", p.bg), ("panel", p.panel)] {
                let r = ratio(c, bg);
                if r < 3.0 {
                    failures.push(format!("{t} line {i} on {name}: {r:.2}:1"));
                }
            }
        }
        if p.dark {
            continue;
        }
        let surfaces =
            [("bg", p.bg), ("panel", p.panel), ("toolbar", p.toolbar), ("hover", p.hover), ("selected", p.selected), ("raised", p.raised)];
        for (i, c) in p.branch.chips.iter().enumerate() {
            for (name, bg) in surfaces {
                let r = ratio(*c, bg);
                if r < 4.5 {
                    failures.push(format!("{t} chip {i} on {name}: {r:.2}:1"));
                }
            }
        }
    }
    report(&failures);
}

#[test]
fn cb_tones_stay_apart_in_every_vision() {
    // Each built-in's colour-blind lines against each other and its own accent (the selection
    // bar). Against main's trunk tone some hues fall under the floor, and no gray or hue retune
    // fixes that (the #470 ruling); instead an automatic branch beside the trunk skips them
    // (`trunk_near`), so every hue under the floor from the trunk must be in the mask.
    let mut failures = Vec::new();
    for (t, p) in palettes() {
        let floor = if p.dark { DARK_FLOOR } else { LIGHT_FLOOR };
        failures.extend(too_close(&t, &p.colour_blind.lanes[..8], &[p.accent], floor));
        let cb = applied(*p, BranchPalette::ColourBlind);
        let (trunk, mask) = (super::branch_colours::trunk(&cb), super::branch_colours::trunk_near(&cb));
        for (vision, m) in &VISIONS {
            for (i, lane) in cb.branch.lanes[..8].iter().enumerate() {
                let d = distance(*lane, trunk, m);
                if d < floor && mask & (1 << i) == 0 {
                    failures.push(format!("{t} {vision}: slot {i} passes for the trunk at {d:.3} but isn't in trunk_near"));
                }
            }
        }
    }
    report(&failures);
}

/// The designer's table (#470): which slots pass for the trunk in each built-in, with the
/// standard hues (normal vision only) and the colour-blind ones (every vision).
#[test]
fn trunk_near_slots_per_theme() {
    use super::branch_colours::trunk_near;
    let bits = |slots: &[u16]| slots.iter().fold(0u16, |m, s| m | 1 << s);
    let want: [(&str, u16, u16); 10] = [
        ("Dark", 0, bits(&[7])),
        ("Light", 0, bits(&[4])),
        ("Graphite", 0, bits(&[2, 7])),
        ("Porcelain", 0, bits(&[4])),
        ("Midnight", bits(&[7]), bits(&[7])),
        ("Mist", 0, bits(&[4])),
        ("HighContrastDark", 0, bits(&[1])),
        ("HighContrastLight", 0, 0),
        ("GardenDark", 0, bits(&[7])),
        ("GardenLight", 0, 0),
    ];
    let all = palettes();
    assert_eq!(all.len(), want.len(), "every palette in the table");
    for (name, standard, colour_blind) in want {
        let p = all.iter().find(|(t, _)| t == name).map(|(_, p)| **p).unwrap_or_else(|| panic!("{name}"));
        assert_eq!(trunk_near(&applied(p, BranchPalette::Standard)), standard, "{name} standard");
        assert_eq!(trunk_near(&applied(p, BranchPalette::ColourBlind)), colour_blind, "{name} colour-blind");
    }
}

#[test]
fn the_gold_needs_its_own_dark_set() {
    // Yellow and Lime pass for Dusk's gold under protan or deutan vision, which is why Dusk
    // takes Orange and a paler Lime in their slots.
    let yellow_and_lime = too_close("standard", &CB_DARK_TONES, &[DARK.accent], DARK_FLOOR);
    assert!(
        yellow_and_lime.iter().any(|f| f.contains("slot 2")) && yellow_and_lime.iter().any(|f| f.contains("slot 5")),
        "{yellow_and_lime:?}"
    );
}

#[test]
fn the_check_tells_the_standard_hues_apart() {
    // The standard dark tones fall under the deutan floor (Violet against Azure), which is why the
    // colour-blind preset exists; the check must see it.
    assert!(!too_close("standard dark", &DARK_TONES, &[], DARK_FLOOR).is_empty());
    assert!(distance(DARK_TONES[2], DARK_TONES[7], &DEUTAN) < 0.01, "Violet and Azure");
}

#[test]
fn pad_repeats_every_eight() {
    let lanes = pad(CB_DARK_TONES);
    for (i, c) in lanes.iter().enumerate() {
        assert_eq!(*c, CB_DARK_TONES[i % 8], "lane {i}");
    }
}

#[test]
fn applied_picks_the_family_by_kind() {
    for t in BuiltinTheme::ALL {
        let base = *builtin(t);
        assert_eq!(applied(base, BranchPalette::Standard), base, "{t:?} keeps its own");
        let p = applied(base, BranchPalette::ColourBlind);
        let want = match t {
            BuiltinTheme::Dark => BRANCH_CB_DUSK,
            BuiltinTheme::Light => BRANCH_CB_DAWN,
            BuiltinTheme::HighContrastDark | BuiltinTheme::Midnight => BRANCH_CB_DARK,
            BuiltinTheme::Graphite => BRANCH_CB_GRAPHITE,
            BuiltinTheme::Porcelain => BRANCH_CB_LINEN,
            BuiltinTheme::Mist | BuiltinTheme::HighContrastLight => BRANCH_CB_LIGHT,
        };
        assert_eq!(p.branch, want, "{t:?}");
        assert_eq!(p.branch.preset.slots(), 8);
        assert_eq!(Palette { branch: base.branch, ..p }, base, "only the branch family changes");
    }
    assert_eq!(GARDEN_DARK.branch, BRANCH_DARK);
    assert_eq!(applied(GARDEN_DARK, BranchPalette::ColourBlind).branch, BRANCH_CB_DARK);
    assert_eq!(applied(GARDEN_LIGHT, BranchPalette::ColourBlind).branch, BRANCH_CB_LIGHT);
    assert_eq!(BRANCH_DARK.preset, BranchPalette::Standard);
}
