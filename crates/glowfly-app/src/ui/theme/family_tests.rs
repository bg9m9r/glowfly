//! The colour families: the brand values, the reserved OKLCH hue bands, tinted neutrals, primary
//! ink, status chips, sand tags and theme files (docs/COLOUR.md has the rules).

use super::bands::{GOLD_BAND, accent_band, hue, in_reserved_band};
use super::color::Palette;
use super::contrast::{self, ChipStyle, oklch, ratio};
use super::tests::{ocean, rgb};
use super::*;
use eframe::egui::Color32;
use glowfly_core::settings::{BuiltinTheme, ColorValue};

fn high(t: BuiltinTheme) -> bool {
    matches!(t, BuiltinTheme::HighContrastDark | BuiltinTheme::HighContrastLight)
}

fn report(failures: &[String]) {
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The five status colours and `warn`, by theme-file key.
const STATUS_AND_WARN: [&str; 6] = ["added", "removed", "modified", "info", "progress", "warn"];

#[test]
fn the_hue_maths() {
    let (l, c, h) = oklch(rgb(0x4dc588));
    assert!((l - 0.74).abs() < 0.01 && (c - 0.14).abs() < 0.01 && (h - 158.0).abs() < 1.0, "{l} {c} {h}");
    let (l, c, h) = oklch(rgb(0x14764a));
    assert!((l - 0.50).abs() < 0.01 && (c - 0.11).abs() < 0.01 && (h - 158.0).abs() < 1.0, "{l} {c} {h}");
    let h = hue(rgb(0xff0000));
    assert!((h - 29.0).abs() < 1.0, "red's hue is {h}");
}

#[test]
fn garden_takes_the_brand_values() {
    let d = &GARDEN_DARK;
    assert_eq!((d.accent, d.primary, d.on_primary), (rgb(0x4dc588), rgb(0x4dc588), rgb(0x0f1a14)));
    assert_eq!((d.tag_bg, d.tag_fg), (rgb(0x4a283f), rgb(0xf4c9e4)));
    assert_eq!((d.toolbar, d.bg, d.panel, d.raised, d.border), (rgb(0x0f1612), rgb(0x151b18), rgb(0x1b221e), rgb(0x262e29), rgb(0x2d3530)));
    // faint is the review's #7f8983 nudged one step, so it reaches 4.5:1 on the panel.
    assert_eq!((d.selected, d.text, d.weak, d.faint), (rgb(0x244534), rgb(0xe0e6e3), rgb(0xa4aea8), rgb(0x808a84)));
    let s = d.status;
    assert_eq!(
        [s.added, s.removed, s.modified, s.info, s.progress],
        [rgb(0x7bc85f), rgb(0xea6a64), rgb(0xedb345), rgb(0x60aaf3), rgb(0xa689f1)]
    );
    let l = &GARDEN_LIGHT;
    assert_eq!((l.accent, l.primary, l.on_primary), (rgb(0x14764a), rgb(0x14764a), Color32::WHITE));
    assert_eq!((l.tag_bg, l.tag_fg), (rgb(0xf5d8ea), rgb(0x763463)));
    assert_eq!((l.toolbar, l.bg, l.panel, l.border), (rgb(0xece9e1), rgb(0xf9f7f1), rgb(0xfdfcf9), rgb(0xdcd9d0)));
    assert_eq!((l.selected, l.text, l.weak, l.faint), (rgb(0xcfeddb), rgb(0x161c19), rgb(0x4c5550), rgb(0x68716c)));
    let s = l.status;
    assert_eq!(
        [s.added, s.removed, s.modified, s.info, s.progress],
        [rgb(0x357518), rgb(0xb63132), rgb(0xa06604), rgb(0x1f68bc), rgb(0x6d47b8)]
    );
}

#[test]
fn accent_family_sits_in_the_accent_band() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        for (key, c) in [("accent", p.accent), ("primary", p.primary), ("primary_border", p.primary_border), ("selected", p.selected)] {
            // Ember's selected row is a deep red and Pine's a petrol blue, not a dark wash of
            // their orange or gold, which would read as brown (docs/COLOUR.md, "No brown").
            if matches!(t, BuiltinTheme::Ember | BuiltinTheme::Pine) && key == "selected" {
                continue;
            }
            // Dawn's accent is the iris (a gold line on paper reads brown); its buttons and
            // selected row keep the lantern gold.
            if t == BuiltinTheme::Light && key != "accent" {
                assert!(GOLD_BAND.contains(&hue(c)), "Dawn's {key} stays gold");
                continue;
            }
            let h = hue(c);
            if !accent_band(t).contains(&h) {
                failures.push(format!("{t:?}: {key}'s OKLCH hue is {h:.1}"));
            }
        }
    }
    report(&failures);
}

#[test]
fn status_stays_out_of_the_reserved_bands() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        for key in ["added", "removed", "modified", "info", "progress"] {
            let (_, c, h) = oklch(p.get(key).unwrap());
            if c < 0.08 || in_reserved_band(t, p.get(key).unwrap()) {
                failures.push(format!("{t:?}: {key} has chroma {c:.3} and hue {h:.1}"));
            }
        }
    }
    report(&failures);
}

#[test]
fn no_branch_tone_sits_in_a_reserved_band() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let (b, cb) = (p.branch, p.colour_blind);
        for (i, c) in b.lanes.iter().chain(&b.chips).chain(&cb.lanes).chain(&cb.chips).enumerate() {
            if in_reserved_band(t, *c) {
                failures.push(format!("{t:?} tone {i} at {:.1} deg", hue(*c)));
            }
        }
    }
    report(&failures);
}

#[test]
fn standard_neutrals_are_tinted_and_quiet() {
    let neutrals = [
        "bg",
        "panel",
        "toolbar",
        "raised",
        "input",
        "border",
        "hover",
        "text",
        "strong",
        "weak",
        "faint",
        "input_border",
        "dialog_border",
    ];
    let mut failures = Vec::new();
    for (name, p) in [("Garden dark", &GARDEN_DARK), ("Garden light", &GARDEN_LIGHT)] {
        for key in neutrals {
            let (_, c, h) = oklch(p.get(key).unwrap());
            if c > 0.0155 {
                failures.push(format!("{name}: {key} has chroma {c:.4}"));
            }
            if p.dark && c >= 0.005 && !(150.0..=170.0).contains(&h) {
                failures.push(format!("{name}: {key}'s hue is {h:.1}"));
            }
        }
    }
    for key in ["bg", "panel", "toolbar", "raised", "hover", "border"] {
        let h = hue(GARDEN_LIGHT.get(key).unwrap());
        if !(80.0..=100.0).contains(&h) {
            failures.push(format!("Garden light: {key}'s hue is {h:.1}"));
        }
    }
    report(&failures);
}

#[test]
fn dusk_and_dawn_neutrals_take_the_dusk() {
    // Surfaces and greys take the icon's indigo (OKLCH hue 280 to 300): Dusk's at up to 0.045
    // chroma, Dawn's paper at up to 0.018 under ink of up to 0.05.
    let surfaces = ["bg", "panel", "toolbar", "raised", "border", "hover"];
    let greys = ["text", "weak", "faint", "input_border", "dialog_border"];
    let mut failures = Vec::new();
    for (name, p, surface_max, grey_max) in [("Dusk", &DUSK, 0.045, 0.045), ("Dawn", &DAWN, 0.018, 0.05)] {
        for key in surfaces.iter().chain(&greys) {
            let (_, c, h) = oklch(p.get(key).unwrap());
            let max = if surfaces.contains(key) { surface_max } else { grey_max };
            if c > max || !(280.0..=300.0).contains(&h) {
                failures.push(format!("{name}: {key} has chroma {c:.4} at {h:.1} deg"));
            }
        }
    }
    report(&failures);
}

#[test]
fn primary_ink_reads() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let min = if high(t) { 7.0 } else { 4.5 };
        let pole = if p.dark { Color32::WHITE } else { Color32::BLACK };
        // As a primary button rests, hovers and is pressed.
        for shade in [0.0, 0.10, 0.18] {
            let r = ratio(p.on_primary, contrast::mix(p.primary, pole, shade));
            if r < min {
                failures.push(format!("{t:?}: on_primary at shade {shade} is {r:.2}:1"));
            }
        }
    }
    report(&failures);
}

#[test]
fn status_chips_are_readable() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        for key in STATUS_AND_WARN {
            let col = p.get(key).unwrap();
            let (fill, fg) = contrast::change_chip(col, p);
            let r = ratio(fg, fill);
            if fill.a() != 255 || r < 4.5 {
                failures.push(format!("{t:?} {key} chip: {r:.2}:1, fill alpha {}", fill.a()));
            }
            if !p.dark && fill != contrast::unbrown(p.bg, contrast::mix(p.bg, col, 0.10)) {
                failures.push(format!("{t:?} {key} chip: not a 10% tint"));
            }
        }
    }
    for key in ["added", "removed", "info", "progress"] {
        let col = GARDEN_LIGHT.get(key).unwrap();
        assert_eq!(contrast::change_chip(col, &GARDEN_LIGHT).1, col, "Garden light {key}: the chip text is the status colour itself");
    }
    report(&failures);
}

#[test]
fn tags_are_plum() {
    // The plum of the icon's lower edge (the Garden pair's sand tags read brown: docs/COLOUR.md,
    // "No brown").
    let plum = 330.0..=350.0;
    for (name, p, band) in
        [("Garden dark", &GARDEN_DARK, &plum), ("Garden light", &GARDEN_LIGHT, &plum), ("Dusk", &DUSK, &plum), ("Dawn", &DAWN, &plum)]
    {
        let tag = contrast::ref_chip(ChipStyle::Tag, p.branch.chips[0], p);
        assert_eq!((tag.fill, tag.text), (p.tag_bg, p.tag_fg), "{name}");
        let r = ratio(p.tag_fg, p.tag_bg);
        assert!(r >= 4.5, "{name}: tag text is {r:.2}:1");
        let h = hue(p.tag_bg);
        assert!(band.contains(&h), "{name}: tag_bg's hue is {h:.1}");
    }
}

#[test]
fn custom_files_inherit_the_families() {
    let p: Palette = custom::build(&ocean(BuiltinTheme::Light, &[("hover", ColorValue::Rgb([1, 2, 3]))]));
    assert_eq!((p.raised, p.on_primary, p.attention, p.status), (LIGHT.raised, LIGHT.on_primary, LIGHT.attention, LIGHT.status));
    assert_eq!((p.repository, p.branch, p.colour_blind, p.ansi), (LIGHT.repository, LIGHT.branch, LIGHT.colour_blind, LIGHT.ansi));
    // An accent of its own on Dusk or Dawn (a file from the green Dark, say) takes the Garden
    // branch families, which keep out of a green accent's way; the base's own accent keeps Dusk's.
    let p = custom::build(&ocean(BuiltinTheme::Dark, &[("accent", ColorValue::Rgb([0x4d, 0xc5, 0x88]))]));
    assert_eq!((p.branch, p.colour_blind, p.repository), (GARDEN_DARK.branch, GARDEN_DARK.colour_blind, DARK.repository));
    let own = DARK.accent;
    let p = custom::build(&ocean(BuiltinTheme::Dark, &[("accent", ColorValue::Rgb([own.r(), own.g(), own.b()]))]));
    assert_eq!(p.branch, DARK.branch);
    let p = custom::build(&ocean(BuiltinTheme::Sand, &[("accent", ColorValue::Rgb([1, 2, 3]))]));
    assert_eq!(p.branch, SAND.branch, "Sand keeps its own");
    let text = br##"{ "base": "dark", "colors": { "green": "#010203" } }"##;
    let p = custom::build(&glowfly_core::settings::themes::parse("ocean", std::path::Path::new("ocean.json"), text));
    assert_eq!(p.status.added, Color32::from_rgb(1, 2, 3), "the old name sets added");
    assert_eq!(p.attention.warn, DARK.attention.warn);
}

#[test]
fn custom_primary_without_ink_stays_readable() {
    let build = |base, colors: &[(&'static str, ColorValue)]| custom::build(&ocean(base, colors));
    // A file made from the old Dark starter: its own primary, no on_primary.
    let old_dark = build(BuiltinTheme::GardenDark, &[("primary", ColorValue::Rgb([0x0e, 0x7a, 0x85]))]);
    assert_eq!(old_dark.on_primary, Color32::WHITE);
    for (base, rgb) in [(BuiltinTheme::Dark, [0x1f, 0x3a, 0x8a]), (BuiltinTheme::Light, [0xf5, 0xd0, 0x4a])] {
        let p = build(base, &[("primary", ColorValue::Rgb(rgb))]);
        let r = ratio(p.on_primary, p.primary);
        assert!(r >= 4.5, "{base:?} with primary {rgb:?}: on_primary is {r:.2}:1");
    }
    // The base ink stays while it reads, and a file's own on_primary is kept as set.
    let near = build(BuiltinTheme::GardenDark, &[("primary", ColorValue::Rgb([0x55, 0xcc, 0x90]))]);
    assert_eq!(near.on_primary, GARDEN_DARK.on_primary);
    let own = build(BuiltinTheme::Dark, &[("primary", ColorValue::Rgb([0x0e, 0x7a, 0x85])), ("on_primary", ColorValue::Rgb([1, 2, 3]))]);
    assert_eq!(own.on_primary, Color32::from_rgb(1, 2, 3));
}
