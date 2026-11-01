//! The terminal's colours (ansi.rs): the built-ins' body colours, contrast on the terminal
//! background, the hue bands, the bright colours' direction, Preferences' readability check,
//! theme files' terminal keys, and the Colours setting (terminal_scheme.rs).

use super::contrast::{oklch, ratio};
use super::tests::ocean;
use super::tests::{high_contrast, palettes};
use super::*;
use eframe::egui::Color32;
use glowfly_core::settings::terminal_schemes::{SchemeCatalog, parse_stored};
use glowfly_core::settings::{BuiltinTheme, ColorValue, TerminalColours};
use std::ops::RangeInclusive;

/// The OKLCH hue bands of colours 1-6 (red, green, yellow, blue, magenta, cyan); red wraps.
const BANDS: [&[RangeInclusive<f64>]; 6] =
    [&[0.0..=45.0, 345.0..=360.0], &[115.0..=165.0], &[60.0..=115.0], &[225.0..=285.0], &[290.0..=345.0], &[165.0..=225.0]];

#[test]
fn basic_colours_read_on_the_terminal_background() {
    let mut failures = Vec::new();
    for (t, p) in palettes() {
        let c = p.ansi.colors;
        let bg = p.ansi.background;
        let (body, dim) = if high_contrast(p) { (7.0, 4.5) } else { (4.5, 3.0) };
        let r = ratio(p.ansi.foreground, bg);
        if r < body {
            failures.push(format!("{t} foreground: {r:.2} < {body}"));
        }
        let mut need = |i: usize, on: Color32, min: f64, what: &str| {
            let r = ratio(c[i], on);
            if r < min {
                failures.push(format!("{t} {i} {what}: {r:.2} < {min}"));
            }
        };
        // Text colours on the background; 8 is the dim one.
        assert_eq!(ansi::is_dark(&p.ansi), p.dark, "{t}");
        for i in ansi::body_indices(p.dark) {
            need(i, bg, body, "on the background");
        }
        need(8, bg, dim, "dim on the background");
        // Background colours: off the background, with the text readable on them.
        let backs: &[usize] = if p.dark { &[0] } else { &[7, 15] };
        for &i in backs {
            need(i, bg, 1.25, "as a background, off the background");
        }
        for &i in backs {
            let r = ratio(p.ansi.foreground, c[i]);
            if r < body {
                failures.push(format!("{t} text on {i}: {r:.2} < {body}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn basic_colours_keep_their_hues_and_bright_ones_go_the_right_way() {
    let mut failures = Vec::new();
    for (t, p) in palettes() {
        for (k, band) in BANDS.iter().enumerate() {
            let i = k + 1;
            for j in [i, i + 8] {
                let h = oklch(p.ansi.colors[j]).2;
                if !band.iter().any(|b| b.contains(&h)) {
                    failures.push(format!("{t} {j}: hue {h:.0} out of {band:?}"));
                }
            }
            let (normal, bright) = (oklch(p.ansi.colors[i]).0, oklch(p.ansi.colors[i + 8]).0);
            let step = if p.dark { bright - normal } else { normal - bright };
            if step < 0.04 {
                failures.push(format!("{t} {i}/{}: bright is {step:.3} L the wrong way or too close", i + 8));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn theme_files_take_their_base_table() {
    assert_eq!(custom::build(&ocean(BuiltinTheme::Light, &[])).ansi, LIGHT.ansi);
    assert_eq!(custom::build(&ocean(BuiltinTheme::Dark, &[])).ansi, DARK.ansi);
    assert_ne!(DARK.ansi, LIGHT.ansi);
}

#[test]
fn builtins_draw_on_input_in_text_with_the_accent() {
    for (t, p) in palettes() {
        let a = p.ansi;
        assert_eq!((a.background, a.foreground, a.cursor, a.selection), (p.input, p.text, p.accent, p.accent), "{t}");
        assert!(ansi::hard_to_read(&a).is_empty(), "{t}: {:?}", ansi::hard_to_read(&a));
        assert_eq!(p.get("terminal_background"), Some(p.input));
        assert_eq!(p.get("terminal_bright_white"), Some(a.colors[15]));
    }
}

#[test]
fn hard_to_read_names_the_foreground_and_body_colours() {
    let mut a = DARK.ansi;
    a.foreground = a.background;
    a.colors[1] = a.background;
    a.colors[8] = Color32::from_rgb(0x30, 0x2e, 0x40);
    a.colors[0] = a.background;
    assert_eq!(ansi::hard_to_read(&a), [None, Some(1), Some(8)], "black is a background in a dark scheme");
    let mut l = LIGHT.ansi;
    l.colors[15] = l.background;
    l.colors[14] = Color32::from_rgb(0xee, 0xff, 0xff);
    assert!(!ansi::is_dark(&l));
    assert_eq!(ansi::hard_to_read(&l), [Some(14)], "white is a background in a light scheme");
}

#[test]
fn theme_files_set_terminal_keys_and_the_body_follows_their_surfaces() {
    let rgb = |r, g, b| ColorValue::Rgb([r, g, b]);
    let p = custom::build(&ocean(BuiltinTheme::Dark, &[("input", rgb(1, 2, 3)), ("text", rgb(4, 5, 6)), ("accent", rgb(7, 8, 9))]));
    let a = p.ansi;
    assert_eq!(a.background, Color32::from_rgb(1, 2, 3), "unset, the background follows the file's input");
    assert_eq!((a.foreground, a.cursor, a.selection), (Color32::from_rgb(4, 5, 6), Color32::from_rgb(7, 8, 9), Color32::from_rgb(7, 8, 9)));
    assert_eq!(a.colors, DARK.ansi.colors, "the basic colours come from the base");
    let f = ocean(
        BuiltinTheme::Dark,
        &[
            ("input", rgb(1, 2, 3)),
            ("terminal_background", rgb(10, 11, 12)),
            ("terminal_red", rgb(13, 14, 15)),
            ("terminal_cursor", ColorValue::Base("warn")),
        ],
    );
    let a = custom::build(&f).ansi;
    assert_eq!((a.background, a.colors[1], a.cursor), (Color32::from_rgb(10, 11, 12), Color32::from_rgb(13, 14, 15), DARK.attention.warn));
    assert_eq!(a.colors[2], DARK.ansi.colors[2]);
}

#[test]
fn starters_include_the_terminal_keys() {
    let entries = custom::starter(&PORCELAIN);
    let keys: Vec<&str> = entries.iter().map(|(k, _)| *k).collect();
    assert!(keys.ends_with(&glowfly_core::settings::terminal_schemes::TERMINAL_KEYS));
    let red = entries.iter().find(|(k, _)| *k == "terminal_red").unwrap().1;
    assert_eq!(Color32::from_rgb(red[0], red[1], red[2]), PORCELAIN.ansi.colors[1]);
}

#[test]
fn the_colours_setting_picks_a_theme_or_a_scheme_over_the_app_theme() {
    let file = parse_stored(
        "night",
        std::path::Path::new("night.json"),
        br##"{"name": "Night", "colors": {"terminal_background": "#000000", "terminal_red": "#ff0000"}}"##,
    );
    let cat = SchemeCatalog { files: vec![file], error: None };
    let resolve = |c: TerminalColours| terminal_scheme::resolve(&c, &DUSK, &cat);
    assert_eq!(resolve(TerminalColours::MatchTheme), DUSK.ansi);
    assert_eq!(resolve(TerminalColours::Theme("porcelain".into())), PORCELAIN.ansi);
    assert_eq!(resolve(TerminalColours::Theme("sand".into())), PORCELAIN.ansi, "a retired theme's id picks its successor's");
    assert_eq!(resolve(TerminalColours::Theme("nope".into())), DUSK.ansi, "an unknown theme matches the app's");
    let night = resolve(TerminalColours::Scheme("night".into()));
    assert_eq!((night.background, night.colors[1]), (Color32::BLACK, Color32::from_rgb(255, 0, 0)));
    assert_eq!((night.foreground, night.colors[2]), (DUSK.ansi.foreground, DUSK.ansi.colors[2]), "unset keys from the app theme");
    assert_eq!(resolve(TerminalColours::Scheme("gone".into())), DUSK.ansi);
    assert_eq!(terminal_scheme::override_for(&TerminalColours::MatchTheme, &DUSK, &cat), None);
    assert_eq!(terminal_scheme::override_for(&TerminalColours::Theme("dark".into()), &DUSK, &cat), None, "the same colours");
    assert!(terminal_scheme::override_for(&TerminalColours::Theme("light".into()), &DUSK, &cat).is_some());
}
