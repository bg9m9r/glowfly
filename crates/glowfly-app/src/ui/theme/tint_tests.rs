//! `contrast::tint`: faint surface tints stay in a coloured dark field's family (no grey-brown
//! rows or toolbar tiles in Dusk), and are the plain mix everywhere else (docs/COLOUR.md,
//! "Tinting a surface").

use super::branch_colours::{line_in, swatches};
use super::contrast::{mix, oklch, tint};
use super::*;
use eframe::egui::Color32;
use glowfly_core::settings::BuiltinTheme;

/// The angle between two hues (0 to 180).
fn apart(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

/// Every faint tint a palette paints: (what, the field it tints, the tint).
fn tints(p: &color::Palette) -> Vec<(String, Color32, Color32)> {
    let mut v = vec![
        ("tool backing".to_string(), p.toolbar, p.tool_backing()),
        ("guide fill".to_string(), p.panel, p.guide_fill()),
        ("field fill".to_string(), p.input, p.field_fill()),
    ];
    for s in swatches() {
        v.push((format!("{s:?} band"), p.bg, crate::ui::graph::band_color(line_in(s, p), p)));
    }
    v
}

#[test]
fn tints_on_a_coloured_dark_field_keep_its_family() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        for (what, field, c) in tints(p) {
            let ((_, fc, fh), (_, cc, ch)) = (oklch(field), oklch(c));
            if !p.dark || fc < 0.02 {
                continue;
            }
            checked += 1;
            // The byte rounding can lose a hair of chroma or turn the hue a little past the fence.
            if cc < fc - 0.002 || apart(ch, fh) > 22.0 {
                failures.push(format!("{t:?}: {what} is chroma {cc:.3} hue {ch:.0} on a field of {fc:.3} {fh:.0}"));
            }
        }
    }
    assert!(checked > 0, "Dusk's indigo is a coloured field");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn tint_is_the_plain_mix_on_light_and_neutral_fields() {
    let gold = Color32::from_rgb(0xf6, 0xc1, 0x59);
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        if !p.dark || oklch(p.bg).1 < 0.02 {
            assert_eq!(tint(p.bg, gold, 0.07, p.dark), mix(p.bg, gold, 0.07), "{t:?}");
        }
    }
    // Dusk: the gold on indigo would mix to a warm grey (chroma 0.017); the tint stays violet.
    let (_, c, h) = oklch(tint(DUSK.bg, gold, 0.07, true));
    assert!(c >= 0.031 && (290.0..=313.0).contains(&h), "chroma {c:.3}, hue {h:.0}");
}
