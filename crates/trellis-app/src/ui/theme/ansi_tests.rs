//! The terminal's basic colours (ansi.rs): contrast on the terminal background, the hue bands,
//! the bright colours' direction, and theme files taking their base's table.

use super::contrast::{oklch, ratio};
use super::tests::ocean;
use super::*;
use eframe::egui::Color32;
use std::ops::RangeInclusive;
use trellis_core::settings::BuiltinTheme;

/// The OKLCH hue bands of colours 1-6 (red, green, yellow, blue, magenta, cyan); red wraps.
const BANDS: [&[RangeInclusive<f64>]; 6] =
    [&[0.0..=45.0, 345.0..=360.0], &[115.0..=165.0], &[60.0..=115.0], &[225.0..=285.0], &[290.0..=345.0], &[165.0..=225.0]];

fn high(t: BuiltinTheme) -> bool {
    matches!(t, BuiltinTheme::HighContrastDark | BuiltinTheme::HighContrastLight)
}

#[test]
fn basic_colours_read_on_the_terminal_background() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let c = p.ansi.colors;
        let (body, dim) = if high(t) { (7.0, 4.5) } else { (4.5, 3.0) };
        let mut need = |i: usize, on: Color32, min: f64, what: &str| {
            let r = ratio(c[i], on);
            if r < min {
                failures.push(format!("{t:?} {i} {what}: {r:.2} < {min}"));
            }
        };
        // Text colours on `input`; 8 is the dim one.
        let text: Vec<usize> = if p.dark { (1..=7).chain(9..=15).collect() } else { (0..=6).chain(9..=14).collect() };
        for i in text {
            need(i, p.input, body, "on input");
        }
        need(8, p.input, dim, "dim on input");
        // Background colours: off `input`, with `text` readable on them.
        let backs: &[usize] = if p.dark { &[0] } else { &[7, 15] };
        for &i in backs {
            need(i, p.input, 1.25, "as a background, off input");
        }
        for &i in backs {
            let r = ratio(p.text, c[i]);
            if r < body {
                failures.push(format!("{t:?} text on {i}: {r:.2} < {body}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn basic_colours_keep_their_hues_and_bright_ones_go_the_right_way() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        for (k, band) in BANDS.iter().enumerate() {
            let i = k + 1;
            for j in [i, i + 8] {
                let h = oklch(p.ansi.colors[j]).2;
                if !band.iter().any(|b| b.contains(&h)) {
                    failures.push(format!("{t:?} {j}: hue {h:.0} out of {band:?}"));
                }
            }
            let (normal, bright) = (oklch(p.ansi.colors[i]).0, oklch(p.ansi.colors[i + 8]).0);
            let step = if p.dark { bright - normal } else { normal - bright };
            if step < 0.04 {
                failures.push(format!("{t:?} {i}/{}: bright is {step:.3} L the wrong way or too close", i + 8));
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
