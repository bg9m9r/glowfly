//! The `lantern` token (docs/COLOUR.md, "lantern"): at least 3:1 on every fill a graph row can
//! have, `text` in high contrast, Dusk's gold, a quiet amber at chroma 0.10 or less in Graphite
//! and Midnight, apart from `modified` and `warn`; and the HEAD glow it tints is visible but
//! quiet, and quieter with colour-blind safe colours.

use super::branch_palette::applied;
use super::contrast::{oklch, ratio, tint};
use super::tests::{high_contrast, palettes};
use super::*;
use crate::ui::graph::lantern_peak;
use eframe::egui::Color32;
use glowfly_core::settings::ColourPalette;

/// `p` in each colour preset.
fn presets(p: &color::Palette) -> [(&'static str, color::Palette); 2] {
    let safe = ColourPalette::ColourBlind;
    [("standard", *p), ("colour-blind", colour_safe::applied(applied(*p, safe), safe))]
}

/// The OKLab distance between two colours.
fn oklab_distance(a: Color32, b: Color32) -> f64 {
    let lab = |c| {
        let (l, ch, h) = oklch(c);
        (l, ch * h.to_radians().cos(), ch * h.to_radians().sin())
    };
    let ((l1, a1, b1), (l2, a2, b2)) = (lab(a), lab(b));
    ((l1 - l2).powi(2) + (a1 - a2).powi(2) + (b1 - b2).powi(2)).sqrt()
}

#[test]
fn the_lantern_reads_on_every_row_fill() {
    let mut failures = Vec::new();
    for (name, p) in palettes() {
        if high_contrast(p) {
            continue;
        }
        for (preset, p) in presets(p) {
            for (k, fill) in
                [("bg", p.bg), ("hover", p.hover), ("selected", p.selected), ("selected inactive", tokens::selected_inactive(&p))]
            {
                let r = ratio(p.lantern, fill);
                if r < 3.0 {
                    failures.push(format!("{name} {preset}: lantern {r:.2}:1 on {k}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn high_contrast_lanterns_are_the_text_colour() {
    for p in [HIGH_CONTRAST_DARK, HIGH_CONTRAST_LIGHT] {
        assert_eq!(p.lantern, p.text);
    }
}

#[test]
fn dusk_lights_its_gold_and_the_dark_ambers_stay_quiet() {
    assert_eq!(DUSK.lantern, DUSK.accent);
    for (name, p) in [("Graphite", GRAPHITE), ("Midnight", MIDNIGHT)] {
        let (_, c, h) = oklch(p.lantern);
        assert!(c <= 0.10, "{name}: chroma {c:.3}");
        assert!((55.0..=90.0).contains(&h), "{name}: hue {h:.0} is an amber's");
    }
    // The light themes' lantern is a saturated orange: a 3:1 amber at chroma 0.10 on paper
    // reads brown (docs/COLOUR.md, "lantern").
    for p in [PORCELAIN, MIST, DAWN] {
        assert_eq!(p.lantern, PORCELAIN.lantern);
    }
}

#[test]
fn the_lantern_stands_apart_from_modified_and_warn() {
    let mut failures = Vec::new();
    for (name, p) in palettes() {
        if high_contrast(p) || *p == DUSK || p.lantern == p.accent {
            continue;
        }
        for (k, c) in [("modified", p.status.modified), ("warn", p.attention.warn)] {
            let d = oklab_distance(p.lantern, c);
            if d < 0.07 {
                failures.push(format!("{name}: lantern {d:.3} from {k}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_glow_is_visible_but_quiet() {
    let mut failures = Vec::new();
    for (name, p) in palettes() {
        if high_contrast(p) {
            continue;
        }
        let at = |safe| tint(p.bg, p.lantern, lantern_peak(p.dark, safe), p.dark);
        let r = ratio(at(false), p.bg);
        if !(1.15..=2.0).contains(&r) {
            failures.push(format!("{name}: the glow's peak is {r:.2}:1 on bg"));
        }
        if ratio(at(true), p.bg) >= r {
            failures.push(format!("{name}: colour-blind safe colours don't lower the glow"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
