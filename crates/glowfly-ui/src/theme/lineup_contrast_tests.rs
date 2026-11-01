//! The lineup's measured contrast (docs/COLOUR.md, section 6): the WCAG ratios the Themes v2
//! spec gives for each theme, reproduced within 0.02, so a palette change that moves one is
//! deliberate. The floors every built-in meets are in `tests.rs`.

use super::builtin;
use super::contrast::ratio;
use eframe::egui::Color32;
use glowfly_core::settings::BuiltinTheme;

/// How close a measured ratio must be to the spec's.
const WITHIN: f64 = 0.02;

/// (theme, text on bg / panel / chrome / raised / selected, weak on panel, faint on panel, accent
/// on panel, on_primary on primary). Faint is None where the spec gives none (high contrast).
type Row = (BuiltinTheme, [f64; 5], f64, Option<f64>, f64, f64);

/// The spec's table. Dusk's weak on panel is given as 7.9, one decimal: it measures 7.85.
const TABLE: [Row; 8] = [
    (BuiltinTheme::Graphite, [14.06, 12.89, 15.02, 10.54, 8.92], 8.11, Some(5.86), 7.93, 9.49),
    (BuiltinTheme::Midnight, [14.47, 13.34, 15.37, 11.00, 9.40], 8.35, Some(6.04), 9.90, 11.33),
    (BuiltinTheme::Dark, [13.91, 12.85, 14.82, 10.63, 9.87], 7.85, Some(5.63), 9.70, 11.03),
    (BuiltinTheme::HighContrastDark, [21.0, 19.66, 17.75, 17.75, 11.12], 12.33, None, 11.85, 12.39),
    (BuiltinTheme::Porcelain, [14.90, 15.45, 13.37, 13.99, 12.90], 6.93, Some(5.04), 6.33, 6.55),
    (BuiltinTheme::Mist, [14.61, 15.16, 12.68, 13.41, 12.36], 7.12, Some(5.03), 5.73, 5.92),
    (BuiltinTheme::Light, [14.83, 15.33, 12.79, 13.50, 12.53], 7.20, Some(5.14), 6.24, 6.49),
    (BuiltinTheme::HighContrastLight, [19.08, 21.0, 17.46, 17.46, 15.75], 10.44, None, 7.58, 8.13),
];

/// Pushes a line to `failures` unless `fg` on `bg` measures `want` within `WITHIN`.
fn expect(failures: &mut Vec<String>, what: String, fg: Color32, bg: Color32, want: f64) {
    let got = ratio(fg, bg);
    if (got - want).abs() > WITHIN {
        failures.push(format!("{what}: {got:.2}:1, the spec says {want:.2}"));
    }
}

#[test]
fn the_lineup_measures_as_the_spec_says() {
    let mut failures = Vec::new();
    for (t, text, weak, faint, accent, on_primary) in TABLE {
        let p = builtin(t);
        let surfaces = [("bg", p.bg), ("panel", p.panel), ("chrome", p.toolbar), ("raised", p.raised), ("selected", p.selected)];
        for ((name, bg), want) in surfaces.into_iter().zip(text) {
            expect(&mut failures, format!("{t:?} text on {name}"), p.text, bg, want);
        }
        expect(&mut failures, format!("{t:?} weak on panel"), p.weak, p.panel, weak);
        if let Some(faint) = faint {
            expect(&mut failures, format!("{t:?} faint on panel"), p.faint, p.panel, faint);
        }
        expect(&mut failures, format!("{t:?} accent on panel"), p.accent, p.panel, accent);
        expect(&mut failures, format!("{t:?} on_primary on primary"), p.on_primary, p.primary, on_primary);
    }
    // The accent on the selected row (and on Porcelain's input).
    for (t, want) in
        [(BuiltinTheme::Midnight, 6.98), (BuiltinTheme::Dark, 7.45), (BuiltinTheme::Porcelain, 5.29), (BuiltinTheme::Light, 5.10)]
    {
        let p = builtin(t);
        expect(&mut failures, format!("{t:?} accent on selected"), p.accent, p.selected, want);
    }
    let porcelain = builtin(BuiltinTheme::Porcelain);
    expect(&mut failures, "Porcelain accent on input".into(), porcelain.accent, porcelain.input, 6.55);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_other_measured_floors() {
    let mut failures = Vec::new();
    let porcelain = builtin(BuiltinTheme::Porcelain);
    expect(&mut failures, "Porcelain input_border on input".into(), porcelain.input_border, porcelain.input, 4.43);
    expect(&mut failures, "Porcelain input_border on chrome".into(), porcelain.input_border, porcelain.toolbar, 3.71);
    let dusk = builtin(BuiltinTheme::Dark);
    expect(&mut failures, "Dusk coral on selected".into(), dusk.attention.needs_you, dusk.selected, 4.66);
    // The least of a set on the selected row: the spec gives it as "at least".
    let least = |cs: &[Color32], bg: Color32| cs.iter().map(|c| ratio(*c, bg)).fold(f64::MAX, f64::min);
    for (t, want) in [(BuiltinTheme::Light, 4.54), (BuiltinTheme::Porcelain, 4.66)] {
        let p = builtin(t);
        let got = least(&p.branch.chips, p.selected);
        if (got - want).abs() > WITHIN {
            failures.push(format!("{t:?} chip tones on selected: the least is {got:.2}:1, the spec says {want:.2}"));
        }
    }
    let s = &porcelain.status;
    let got = least(&[s.added, s.removed, s.modified, s.info, s.progress, porcelain.attention.warn], porcelain.selected);
    if (got - 4.57).abs() > WITHIN {
        failures.push(format!("Porcelain status colours on selected: the least is {got:.2}:1, the spec says 4.57"));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
