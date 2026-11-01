//! The fills behind text selected in a diff's code, `code_sel` (the diff has the keyboard) and
//! `code_sel_dim` (it hasn't), in every built-in theme. Both are opaque, so they are checked
//! as they are: the text reads on them at 4.5:1 (7:1 in high contrast), every syntax colour at
//! 3:1 (4.5:1); `code_sel` stands at least 0.08 (OKLab ΔE) from each row colour it is painted
//! over, and `code_sel_dim` 0.05 from each and 0.055 from `code_sel`. The designer's table of
//! measured values is reproduced for the themes it names.

use super::builtin::builtin;
use super::color::Palette;
use super::contrast::{oklch, ratio};
use eframe::egui::Color32;
use glowfly_core::settings::BuiltinTheme;

/// OKLab distance between `a` and `b`.
fn delta_e(a: Color32, b: Color32) -> f64 {
    let lab = |c| {
        let (l, ch, h) = oklch(c);
        (l, ch * h.to_radians().cos(), ch * h.to_radians().sin())
    };
    let (x, y) = (lab(a), lab(b));
    ((x.0 - y.0).powi(2) + (x.1 - y.1).powi(2) + (x.2 - y.2).powi(2)).sqrt()
}

/// The row colours a selection is painted over.
fn rows(p: &Palette) -> [(&'static str, Color32); 7] {
    [
        ("bg", p.bg),
        ("diff_add", p.diff_add),
        ("diff_del", p.diff_del),
        ("diff_add_sel", p.diff_add_sel),
        ("diff_del_sel", p.diff_del_sel),
        ("diff_add_word", p.diff_add_word),
        ("diff_del_word", p.diff_del_word),
    ]
}

fn high_contrast(t: BuiltinTheme) -> bool {
    matches!(t, BuiltinTheme::HighContrastDark | BuiltinTheme::HighContrastLight)
}

/// The lowest contrast of the syntax colours on `fill`.
fn syntax_min(p: &Palette, fill: Color32) -> f64 {
    Palette::KEYS.iter().filter(|k| k.starts_with("syn_")).map(|k| ratio(p.get(k).unwrap(), fill)).fold(f64::MAX, f64::min)
}

/// The smallest distance from `fill` to the row colours.
fn row_min(p: &Palette, fill: Color32) -> f64 {
    rows(p).iter().map(|&(_, r)| delta_e(fill, r)).fold(f64::MAX, f64::min)
}

#[test]
fn selected_code_reads_and_stands_apart_from_the_rows() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let (text_min, syn_min) = if high_contrast(t) { (7.0, 4.5) } else { (4.5, 3.0) };
        for (name, fill, apart) in [("code_sel", p.code_sel, 0.08), ("code_sel_dim", p.code_sel_dim, 0.05)] {
            let r = ratio(p.text, fill);
            if r < text_min {
                failures.push(format!("{t:?} {name}: text is {r:.2}:1"));
            }
            for k in Palette::KEYS.iter().filter(|k| k.starts_with("syn_")) {
                let r = ratio(p.get(k).unwrap(), fill);
                if r < syn_min {
                    failures.push(format!("{t:?} {name}: {k} is {r:.2}:1"));
                }
            }
            for (row, c) in rows(p) {
                let d = delta_e(fill, c);
                if d < apart {
                    failures.push(format!("{t:?} {name}: {d:.3} from {row}"));
                }
            }
        }
        let d = delta_e(p.code_sel_dim, p.code_sel);
        if d < 0.055 {
            failures.push(format!("{t:?}: code_sel_dim is {d:.3} from code_sel"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
#[allow(clippy::approx_constant)] // 3.14 is a measured contrast, not π
fn the_measured_values_are_reproduced() {
    // Theme; code_sel's text, syntax min and row ΔE min; code_sel_dim's the same; their ΔE.
    let table = [
        (BuiltinTheme::Graphite, [6.81, 3.14, 0.138], [6.81, 3.14, 0.083], 0.084),
        (BuiltinTheme::Midnight, [6.86, 3.18, 0.139], [6.84, 3.16, 0.083], 0.083),
        (BuiltinTheme::Dark, [6.91, 3.33, 0.151], [6.76, 3.26, 0.087], 0.084),
        (BuiltinTheme::HighContrastDark, [8.46, 5.02, 0.161], [8.43, 5.00, 0.107], 0.083),
        (BuiltinTheme::Porcelain, [10.66, 3.43, 0.107], [11.91, 3.83, 0.053], 0.060),
        (BuiltinTheme::Mist, [10.27, 3.57, 0.112], [11.22, 3.90, 0.063], 0.061),
        (BuiltinTheme::Light, [10.67, 3.64, 0.093], [9.58, 3.27, 0.067], 0.061),
        (BuiltinTheme::HighContrastLight, [13.78, 5.69, 0.111], [15.06, 6.22, 0.063], 0.061),
    ];
    let mut off = Vec::new();
    for (t, sel, dim, between) in table {
        let p = builtin(t);
        for (name, fill, [text, syn, de]) in [("code_sel", p.code_sel, sel), ("code_sel_dim", p.code_sel_dim, dim)] {
            let got = [ratio(p.text, fill), syntax_min(p, fill), row_min(p, fill)];
            for (i, (g, want)) in got.into_iter().zip([text, syn, de]).enumerate() {
                let tol = if i == 2 { 0.005 } else { 0.02 };
                if (g - want).abs() > tol {
                    off.push(format!("{t:?} {name} [{i}]: {g:.3}, the table says {want}"));
                }
            }
        }
        let d = delta_e(p.code_sel_dim, p.code_sel);
        if (d - between).abs() > 0.005 {
            off.push(format!("{t:?}: code_sel_dim to code_sel {d:.3}, the table says {between}"));
        }
    }
    assert!(off.is_empty(), "{}", off.join("\n"));
}
