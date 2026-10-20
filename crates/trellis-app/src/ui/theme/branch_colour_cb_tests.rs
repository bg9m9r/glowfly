//! The colour-blind safe branch hues: out of the reserved bands, one hue per slot across the three
//! tone sets, contrast on each built-in's surfaces once applied, and distance between every pair
//! (and from the accent) under normal, protan, deutan and tritan vision.
//!
//! Vision is simulated with Machado, Oliveira and Fernandes (2009) at severity 1.0, applied to
//! linear sRGB and clamped; distance is Euclidean in OKLab.

use super::branch_palette::{BRANCH_CB_DARK, BRANCH_CB_LIGHT, BRANCH_DARK, CB_DARK_TONES, CB_LIGHT_LINES, CB_LIGHT_TEXT, DARK_TONES, pad};
use super::contrast::{oklch, ratio};
use super::*;
use eframe::egui::Color32;
use trellis_core::settings::{BranchPalette, BuiltinTheme};

const ACCENT_BAND: std::ops::RangeInclusive<f64> = 145.0..=175.0;
const ATTENTION_BAND: std::ops::RangeInclusive<f64> = 35.0..=60.0;
/// The least OKLab distance between dark tones, and between light line tones, in every vision.
const DARK_FLOOR: f64 = 0.08;
const LIGHT_FLOOR: f64 = 0.05;

type Matrix = [[f64; 3]; 3];

const PROTAN: Matrix = [[0.152286, 1.052583, -0.204868], [0.114503, 0.786281, 0.099216], [-0.003882, -0.048116, 1.051998]];
const DEUTAN: Matrix = [[0.367322, 0.860646, -0.227968], [0.280085, 0.672501, 0.047413], [-0.011820, 0.042940, 0.968881]];
const TRITAN: Matrix = [[1.255528, -0.076749, -0.178779], [-0.078411, 0.930809, 0.147602], [0.004733, 0.691367, 0.303900]];
const NORMAL: Matrix = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
const VISIONS: [(&str, Matrix); 4] = [("normal", NORMAL), ("protan", PROTAN), ("deutan", DEUTAN), ("tritan", TRITAN)];

fn report(failures: &[String]) {
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn linear(c: Color32) -> [f64; 3] {
    let lin = |v: u8| {
        let s = f64::from(v) / 255.0;
        if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    [lin(c.r()), lin(c.g()), lin(c.b())]
}

/// OKLab of linear sRGB `rgb`.
fn oklab([r, g, b]: [f64; 3]) -> [f64; 3] {
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
    ]
}

/// `c` as seen through `vision`, in OKLab.
fn seen(c: Color32, vision: &Matrix) -> [f64; 3] {
    let v = linear(c);
    let row = |k: usize| (vision[k][0] * v[0] + vision[k][1] * v[1] + vision[k][2] * v[2]).clamp(0.0, 1.0);
    oklab([row(0), row(1), row(2)])
}

fn distance(a: Color32, b: Color32, vision: &Matrix) -> f64 {
    let (x, y) = (seen(a, vision), seen(b, vision));
    ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt()
}

/// Every pair of `tones` closer than `floor` in some vision, and every tone that close to an
/// accent in `accents`.
fn too_close(set: &str, tones: &[Color32], accents: &[Color32], floor: f64) -> Vec<String> {
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

fn dark_accents() -> Vec<Color32> {
    vec![DARK.accent, HIGH_CONTRAST_DARK.accent]
}

fn light_accents() -> Vec<Color32> {
    vec![LIGHT.accent, HIGH_CONTRAST_LIGHT.accent, SAND.accent]
}

fn hue_gap(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

#[test]
fn cb_tones_outside_reserved_bands() {
    let mut failures = Vec::new();
    for (set, tones) in [("dark", CB_DARK_TONES), ("light line", CB_LIGHT_LINES), ("light text", CB_LIGHT_TEXT)] {
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
fn cb_tones_share_one_hue_per_slot() {
    let mut failures = Vec::new();
    for i in 0..8 {
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
    for t in BuiltinTheme::ALL {
        let p = applied(*builtin(t), BranchPalette::ColourBlind);
        for (i, c) in p.branch.lanes.iter().copied().chain([p.accent]).enumerate() {
            for (name, bg) in [("bg", p.bg), ("panel", p.panel)] {
                let r = ratio(c, bg);
                if r < 3.0 {
                    failures.push(format!("{t:?} line {i} on {name}: {r:.2}:1"));
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
                    failures.push(format!("{t:?} chip {i} on {name}: {r:.2}:1"));
                }
            }
        }
    }
    report(&failures);
}

#[test]
fn cb_tones_stay_apart_in_every_vision() {
    let mut failures = too_close("dark", &CB_DARK_TONES, &dark_accents(), DARK_FLOOR);
    failures.extend(too_close("light line", &CB_LIGHT_LINES, &light_accents(), LIGHT_FLOOR));
    report(&failures);
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
        let want = if matches!(t, BuiltinTheme::Dark | BuiltinTheme::HighContrastDark) { BRANCH_CB_DARK } else { BRANCH_CB_LIGHT };
        assert_eq!(p.branch, want, "{t:?}");
        assert_eq!(p.branch.preset.slots(), 8);
        assert_eq!(Palette { branch: base.branch, ..p }, base, "only the branch family changes");
    }
    assert_eq!(DARK.branch, BRANCH_DARK);
    assert_eq!(BRANCH_DARK.preset, BranchPalette::Standard);
}
