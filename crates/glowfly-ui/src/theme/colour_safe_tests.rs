//! The colour-blind safe diff and status colours (`colour_safe.rs`) in every built-in theme:
//! added, removed and modified stay apart through simulated protan, deutan and tritan vision
//! (docs/COLOUR.md; #426 §5.5), diff text and syntax stay readable on the new fills, the marks
//! keep their contrast on the surfaces and graph row bands they sit on, and the standard preset
//! changes nothing.

use super::builtin;
use super::color::Palette;
use super::colour_safe::applied;
use super::contrast::{diff_line_selected, oklch, ratio};
use super::vision::{VISIONS, distance};
use eframe::egui::Color32;
use glowfly_core::settings::{BuiltinTheme, ColourPalette};

/// The least OKLab distance between the marks, and between the stronger fills, in every vision.
const APART: f64 = 0.10;
/// Dark themes' plain fills: Graphite's and Midnight's land at 0.095, the gutter's + and - carry
/// the meaning with them.
const DARK_PLAIN_APART: f64 = 0.09;
/// Light themes' plain fills: at their lightness sRGB can't hold the chroma for 0.10 (about
/// 0.057 at most); the standard pair is 0.047 apart and the gutter's + and - carry the meaning.
const LIGHT_PLAIN_APART: f64 = 0.04;

fn safe(t: BuiltinTheme) -> Palette {
    applied(*builtin(t), ColourPalette::ColourBlind)
}

/// The least distance of `a` and `b` over the visions, and the vision it falls in.
fn least(a: Color32, b: Color32) -> (f64, &'static str) {
    VISIONS.iter().map(|(n, m)| (distance(a, b, m), *n)).fold((f64::MAX, ""), |x, y| if y.0 < x.0 { y } else { x })
}

#[test]
fn added_removed_and_modified_stay_apart_in_every_vision() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = safe(t);
        let s = p.status;
        let plain = if p.dark { DARK_PLAIN_APART } else { LIGHT_PLAIN_APART };
        for (what, a, b, min) in [
            ("added/removed", s.added, s.removed, APART),
            ("added/modified", s.added, s.modified, APART),
            ("removed/modified", s.removed, s.modified, APART),
            ("diff_add/diff_del", p.diff_add, p.diff_del, plain),
            ("diff_add_sel/diff_del_sel", p.diff_add_sel, p.diff_del_sel, APART),
            ("diff_add_word/diff_del_word", p.diff_add_word, p.diff_del_word, APART),
            ("selected lines", diff_line_selected(&p, true), diff_line_selected(&p, false), APART),
        ] {
            let (d, vision) = least(a, b);
            if d < min {
                failures.push(format!("{t:?} {what} ({a:?}, {b:?}): {d:.3} apart in {vision} vision, below {min}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_standard_pair_is_what_colour_blindness_loses() {
    // Without the preset, red and green fall together in deutan vision: the reason for it.
    let (deutan, p) = (&VISIONS[2], builtin(BuiltinTheme::Graphite));
    assert_eq!(deutan.0, "deutan");
    assert!(distance(p.status.added, p.status.removed, &deutan.1) < APART);
    let s = safe(BuiltinTheme::Graphite);
    assert!(distance(s.status.added, s.status.removed, &deutan.1) > 2.0 * APART);
}

#[test]
fn text_stays_readable_on_the_new_fills() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = safe(t);
        let (text_min, syn_min): (f64, f64) = if p.high_contrast { (7.0, 4.5) } else { (4.5, 2.4) };
        let fills = [
            ("diff_add", p.diff_add),
            ("diff_del", p.diff_del),
            ("diff_add_sel", p.diff_add_sel),
            ("diff_del_sel", p.diff_del_sel),
            ("diff_add_word", p.diff_add_word),
            ("diff_del_word", p.diff_del_word),
            ("selected + line", diff_line_selected(&p, true)),
            ("selected - line", diff_line_selected(&p, false)),
        ];
        for (name, fill) in fills {
            let r = ratio(p.text, fill);
            if r < text_min {
                failures.push(format!("{t:?}: text on {name} is {r:.2}:1"));
            }
            // The selected line's own floor is 2:1 for syntax (`diff_select_tests`).
            let syn_floor = if name.starts_with("selected") && !p.high_contrast { 2.0 } else { syn_min };
            for k in Palette::KEYS.iter().filter(|k| k.starts_with("syn_")) {
                let r = ratio(p.get(k).unwrap(), fill);
                if r < syn_floor {
                    failures.push(format!("{t:?}: {k} on {name} is {r:.2}:1"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn marks_keep_their_contrast() {
    use super::branch_colours::{line_in, swatches};
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        // As the shell shows it: the colour-blind branch hues too, whose row bands a mark sits on.
        let p = super::applied(*builtin(t), ColourPalette::ColourBlind);
        let min = if p.high_contrast { 4.5 } else { 3.0 };
        let surfaces = [("bg", p.bg), ("panel", p.panel), ("hover", p.hover), ("selected", p.selected)].map(|(n, c)| (n.to_owned(), c));
        let bands = swatches().map(|l| (format!("{l:?} band"), crate::ui::graph::band_color(line_in(l, &p), &p)));
        let on: Vec<(String, Color32)> = surfaces.into_iter().chain(bands).collect();
        for (name, c) in [("added", p.status.added), ("removed", p.status.removed), ("modified", p.status.modified)] {
            for (sname, s) in &on {
                let r = ratio(c, *s);
                if r < min {
                    failures.push(format!("{t:?}: {name} on {sname} is {r:.2}:1, below {min}:1"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_hues_are_blue_orange_and_violet() {
    let near = |c: Color32, h: f64| (oklch(c).2 - h).abs() <= 6.0;
    for t in BuiltinTheme::ALL {
        let p = safe(t);
        for c in [p.status.added, p.diff_add, p.diff_add_sel, p.diff_add_word] {
            assert!(near(c, 250.0), "{t:?}: added {c:?} is blue");
        }
        // Orange, a rust red where orange would read brown, a vermilion for text on paper.
        for c in [p.status.removed, p.diff_del, p.diff_del_sel, p.diff_del_word] {
            assert!([55.0, 40.0, 30.0].iter().any(|h| near(c, *h)), "{t:?}: removed {c:?} is orange to red");
        }
        assert!(near(p.status.modified, 300.0), "{t:?}: modified is violet");
    }
    // Pinned for the goldens' palettes, so a change is deliberate.
    let g = safe(BuiltinTheme::Graphite);
    let hex = |c: Color32| format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b());
    let pin = [g.status.added, g.status.removed, g.status.modified, g.diff_add, g.diff_del].map(hex);
    assert_eq!(pin, ["#77bbff", "#df7717", "#a772f5", "#1d3249", "#511912"].map(String::from));
}

#[test]
fn the_standard_preset_changes_nothing() {
    for t in BuiltinTheme::ALL {
        assert_eq!(applied(*builtin(t), ColourPalette::Standard), *builtin(t), "{t:?}");
    }
}

#[test]
fn the_shell_shows_the_preset_it_is_given() {
    use super::color;
    use crate::themes::ThemeState;
    use glowfly_core::settings::{SystemPair, ThemeChoice};
    let (ctx, themes) = (eframe::egui::Context::default(), ThemeState::default());
    let mut cache = super::ThemeCache::default();
    let pair = SystemPair::default();
    cache.select(&ctx, &ThemeChoice::Graphite, pair, ColourPalette::ColourBlind, &themes, true);
    assert_eq!(color::current().status, safe(BuiltinTheme::Graphite).status);
    assert!(color::colour_safe());
    cache.select(&ctx, &ThemeChoice::Graphite, pair, ColourPalette::Standard, &themes, true);
    assert_eq!(color::current().status, builtin(BuiltinTheme::Graphite).status);
    assert!(!color::colour_safe());
}
