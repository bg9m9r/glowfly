//! The selection of a pane without the keyboard focus (`tokens::selected_inactive`): in every
//! built-in theme it is the designer's measured value, and it is a neutral (OKLCH chroma at most
//! 0.02) in `selected`'s hue, at least 0.03 (OKLab ΔE) from `selected` and from `hover`, on
//! which `weak` text reads at 4.5:1. The value worked out for other palettes keeps the same
//! rules on every built-in.

use super::builtin;
use super::color::Palette;
use super::contrast::{brownish, oklch, ratio};
use super::inactive::{APART, delta_e, pinned, worked_out};
use super::tokens::selected_inactive;
use eframe::egui::Color32;
use glowfly_core::settings::BuiltinTheme;

/// The most OKLCH chroma the inactive selection may have (the design spec's neutral).
const INACTIVE_CHROMA: f64 = 0.02;
/// How far its hue may stray from `selected`'s (8-bit rounding at so little chroma).
const HUE_SLACK: f64 = 6.0;

/// The designer's measured values (PR #464's design sign-off).
const DESIGNER: [(BuiltinTheme, u32); 8] = [
    (BuiltinTheme::Dark, 0x36353f),
    (BuiltinTheme::Light, 0xdcdae8),
    (BuiltinTheme::Midnight, 0x313e3c),
    (BuiltinTheme::Graphite, 0x363d46),
    (BuiltinTheme::HighContrastDark, 0x313d37),
    (BuiltinTheme::Porcelain, 0xd6deea),
    (BuiltinTheme::Mist, 0xd4e2db),
    (BuiltinTheme::HighContrastLight, 0xd4e0d7),
];

fn hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// What `fill` breaks of the rules on `p`, if anything.
fn broken(p: &Palette, fill: Color32) -> Option<String> {
    let (_, c, h) = oklch(fill);
    let sel_h = oklch(p.selected).2;
    let hue_off = (h - sel_h).rem_euclid(360.0).min((sel_h - h).rem_euclid(360.0));
    let (weak, to_sel, to_hover) = (ratio(p.weak, fill), delta_e(fill, p.selected), delta_e(fill, p.hover));
    let ok = c <= INACTIVE_CHROMA
        && weak >= 4.5
        && to_sel >= APART
        && to_hover >= APART
        && !brownish(fill)
        && fill.a() == 255
        && (c < 0.005 || hue_off <= HUE_SLACK);
    let line =
        format!("{} C {c:.3} hue off {hue_off:.1}, weak {weak:.2}:1, ΔE {to_sel:.3} from selected, {to_hover:.3} from hover", hex(fill));
    eprintln!("{line}");
    (!ok).then_some(line)
}

#[test]
fn built_ins_use_the_designer_values_which_keep_the_rules() {
    let mut failures = Vec::new();
    for (t, want) in DESIGNER {
        let p = builtin(t);
        let fill = selected_inactive(p);
        assert_eq!(hex(fill), format!("#{want:06x}"), "{t:?}");
        assert_eq!(pinned(p), Some(fill), "{t:?} is found by its selected and hover");
        if let Some(why) = broken(p, fill) {
            failures.push(format!("{t:?}: {why}"));
        }
    }
    assert_eq!(DESIGNER.len(), BuiltinTheme::ALL.len(), "every built-in has a designer value");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_worked_out_value_keeps_the_rules() {
    // A palette that isn't a built-in (a theme file's) gets the worked-out value.
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let mut p = builtin(t).clone();
        p.hover = Color32::from_rgb(p.hover.r() ^ 1, p.hover.g(), p.hover.b());
        assert_eq!(pinned(&p), None);
        let fill = selected_inactive(&p);
        assert_eq!(fill, worked_out(&p));
        if let Some(why) = broken(&p, fill) {
            failures.push(format!("{t:?}: {why}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
