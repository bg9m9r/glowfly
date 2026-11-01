//! The lineup's palettes: their pinned values (Porcelain's, and the Midnight, Dusk and Dawn
//! retunes), their neutrals on one hue, no brown or olive surface anywhere dark, and the cobalt
//! branch families, and the six sleekness rules (docs/COLOUR.md). The measured contrast is in
//! `lineup_contrast_tests.rs`; the rules every built-in meets are checked over all of them elsewhere.

use super::bands::{ATTENTION_BAND, COBALT_BAND, GREEN_BAND, accent_band};
use super::branch_palette::{
    BRANCH_CB_DARK, BRANCH_CB_LIGHT, BRANCH_DARK, BRANCH_LIGHT, CB_DARK_TONES, CB_LIGHT_LINES, CB_LIGHT_TEXT, DARK_TONES, LIGHT_LINES,
    LIGHT_NAVY_SLOT, LIGHT_TEXT,
};
use super::branch_palette_more::*;
use super::color::Palette;
use super::contrast::{oklch, ratio};
use super::tests::rgb;
use super::*;
use eframe::egui::Color32;
use glowfly_core::settings::{BuiltinTheme, ThemeChoice};

fn report(failures: &[String]) {
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn apart(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

#[test]
fn the_new_themes_take_their_values() {
    // (theme, bg, panel, accent, text, selected), pinned so a palette change is deliberate.
    let want = [
        (&MIDNIGHT, 0x0d1827, 0x14202f, 0x6eddb2, 0xe2e8f0, 0x173f3c),
        (&MIST, 0xf2f8fc, 0xf8fcfe, 0x007258, 0x142533, 0xc7eddd),
        (&GRAPHITE, 0x191a1d, 0x202225, 0x7ebbff, 0xe6e7e9, 0x1f3d62),
        (&PORCELAIN, 0xf6f7f9, 0xfbfbfc, 0x1a5bb5, 0x1e2228, 0xdbe8fc),
        (&DUSK, 0x1b1829, 0x221f31, 0xf6c159, 0xe6e5ef, 0x362f58),
        (&DAWN, 0xf7f6fc, 0xfbfafe, 0x5a47cf, 0x211f38, 0xe6e1fa),
    ];
    for (p, bg, panel, accent, text, selected) in want {
        assert_eq!((p.bg, p.panel, p.accent, p.text, p.selected), (rgb(bg), rgb(panel), rgb(accent), rgb(text), rgb(selected)));
    }
    // Dawn is iris only: its buttons take the accent, with white ink.
    assert_eq!((DAWN.primary, DAWN.on_primary, DAWN.primary_border), (rgb(0x5a47cf), rgb(0xffffff), rgb(0x4535b5)));
}

#[test]
fn porcelain_keeps_linens_colours_but_its_neutrals() {
    // Porcelain takes the spec's neutrals and cobalt; everything else is Linen's, the theme it
    // replaced (status, attention, diff, hunk, the other syntax colours, branch and repository).
    let p = &PORCELAIN;
    let neutrals = [
        ("toolbar", 0xe9ebee),
        ("raised", 0xeef0f3),
        ("input", 0xffffff),
        ("border", 0xdadde2),
        ("input_border", 0x737882),
        ("dialog_border", 0x737882),
        ("hover", 0xeceef2),
        ("strong", 0x0c0f13),
        ("weak", 0x525862),
        ("faint", 0x676d77),
        ("primary", 0x1a5bb5),
        ("on_primary", 0xffffff),
        ("primary_border", 0x134894),
        ("primary_off", 0xe8eaed),
        ("syn_comment", 0x686e78),
    ];
    for (key, want) in neutrals {
        assert_eq!(p.get(key), Some(rgb(want)), "{key}");
    }
    let linen = [
        ("danger", 0xc0332a),
        ("danger_off", 0xfbdcdc),
        ("tag_bg", 0xf8dae2),
        ("tag_fg", 0x813450),
        ("added", 0x357518),
        ("removed", 0xb63132),
        ("modified", 0x985700),
        ("info", 0x006e91),
        ("progress", 0x6d47b8),
        ("warn", 0x985700),
        ("banner", 0xfbecc8),
        ("diff_add", 0xe4f6e7),
        ("diff_del", 0xfbe6e6),
        ("diff_add_sel", 0xb8e6c1),
        ("diff_del_sel", 0xf5c2c2),
        ("diff_add_word", 0xa8dfb4),
        ("diff_del_word", 0xf2b4b4),
        ("hunk_bg", 0xe6edfb),
        ("hunk_fg", 0x2d4d8b),
        ("syn_string", 0x1a6f30),
        ("syn_number", 0xc92d00),
        ("syn_constant", 0x006f7e),
        ("syn_keyword", 0x8a2bb0),
        ("syn_type", 0x995a00),
        ("syn_function", 0x1f5fbf),
        ("syn_attr", 0x006b75),
    ];
    for (key, want) in linen {
        assert_eq!(p.get(key), Some(rgb(want)), "{key}");
    }
    assert_eq!((p.attention.needs_you, p.attention.on_needs_you), (rgb(0xbf4306), rgb(0xffffff)));
    assert_eq!((p.repository, p.branch, p.colour_blind), (MIST.repository, BRANCH_LINEN, BRANCH_CB_LINEN));
    // Its terminal: Linen's colours with neutral greys.
    for (i, want) in [(0, 0x1e2228), (7, 0xdadde2), (8, 0x6b717b), (15, 0xe2e5e9), (1, 0xc0362f), (12, 0x1d4ba3)] {
        assert_eq!(p.ansi.colors[i], rgb(want), "terminal colour {i}");
    }
}

#[test]
fn every_choice_reaches_its_theme() {
    for t in BuiltinTheme::ALL {
        assert_eq!(t.choice().builtin(true), Some(t), "{t:?}");
        assert!(ThemeChoice::BUILT_IN.contains(&t.choice()), "{t:?} is listed");
    }
}

#[test]
fn the_new_neutrals_sit_on_one_hue() {
    // Surfaces and greys on the theme's hue (within 12 deg; graphite's chroma is too low for a
    // hue to matter), surfaces quiet and the greys quieter than the field.
    let surfaces = ["bg", "panel", "toolbar", "raised", "input", "border", "hover"];
    let greys = ["text", "weak", "faint", "input_border", "dialog_border"];
    let mut failures = Vec::new();
    // (name, palette, hue of the surfaces, hue of the greys, most surface chroma, most grey chroma)
    let themes: [(&str, &Palette, f64, f64, f64, f64); 4] = [
        ("Midnight", &MIDNIGHT, 255.0, 255.0, 0.045, 0.045),
        ("Mist", &MIST, 235.0, 245.0, 0.02, 0.05),
        ("Graphite", &GRAPHITE, 265.0, 265.0, 0.008, 0.012),
        ("Porcelain", &PORCELAIN, 260.0, 260.0, 0.008, 0.02),
    ];
    for (name, p, surface_hue, grey_hue, surface_max, grey_max) in themes {
        for key in surfaces.iter().chain(&greys) {
            let c = p.get(key).unwrap();
            if c == Color32::WHITE {
                continue;
            }
            let (_, chroma, h) = oklch(c);
            let (want, max) = if surfaces.contains(key) { (surface_hue, surface_max) } else { (grey_hue, grey_max) };
            if chroma > max || (chroma >= 0.01 && apart(h, want) > 12.0) {
                failures.push(format!("{name}: {key} has chroma {chroma:.4} at {h:.1} deg"));
            }
        }
    }
    report(&failures);
}

#[test]
fn no_dark_surface_is_brown_or_olive() {
    // A dark surface with any colour in it stays out of the browns and olives (OKLCH hue 40 to
    // 125): a warm accent (Dusk's gold) sits over a cool field, never a muddy one.
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL.into_iter().filter(|t| t.is_dark()) {
        let p = builtin(t);
        for key in ["bg", "panel", "toolbar", "raised", "input", "border", "hover", "primary_off"] {
            let (_, c, h) = oklch(p.get(key).unwrap());
            if c >= 0.008 && (40.0..=125.0).contains(&h) {
                failures.push(format!("{t:?}: {key} is chroma {c:.3} at {h:.0} deg"));
            }
        }
    }
    report(&failures);
}

#[test]
fn porcelain_is_neutral_white_not_cream() {
    // Porcelain's paper and greys lean a hair cool (hue about 260), never warm: neutral white
    // under grey-black ink.
    for key in ["bg", "panel", "raised", "hover", "toolbar", "text", "weak", "faint"] {
        let (_, c, h) = oklch(PORCELAIN.get(key).unwrap());
        assert!(c <= 0.02 && (c < 0.003 || apart(h, 260.0) <= 12.0), "{key}: chroma {c:.4} at {h:.0}");
    }
}

#[test]
fn the_new_themes_use_their_families() {
    let want = [
        (&MIDNIGHT, BRANCH_DARK, BRANCH_CB_DARK),
        (&MIST, BRANCH_LIGHT, BRANCH_CB_LIGHT),
        (&GRAPHITE, BRANCH_GRAPHITE, BRANCH_CB_GRAPHITE),
        (&PORCELAIN, BRANCH_LINEN, BRANCH_CB_LINEN),
    ];
    for (p, branch, cb) in want {
        assert_eq!((p.branch, p.colour_blind), (branch, cb));
    }
}

#[test]
fn cobalt_families_keep_out_of_their_band() {
    let sets: [(&str, &[Color32]); 5] = [
        ("cobalt dark", &COBALT_DARK_TONES),
        ("cobalt light text", &COBALT_LIGHT_TEXT),
        ("cobalt light lines", &COBALT_LIGHT_LINES),
        ("cobalt cb dark", &COBALT_CB_DARK_TONES),
        ("cobalt cb light", &COBALT_CB_LIGHT_LINES),
    ];
    let mut failures = Vec::new();
    for (set, tones) in sets {
        for (i, c) in tones.iter().enumerate() {
            let h = oklch(*c).2;
            if COBALT_BAND.contains(&h) || ATTENTION_BAND.contains(&h) {
                failures.push(format!("{set} slot {i}: hue {h:.1}"));
            }
        }
    }
    // The band itself stays clear of the green that Jade sits in.
    assert!(!COBALT_BAND.contains(&165.0) && GREEN_BAND.contains(&165.0));
    assert_eq!((accent_band(BuiltinTheme::Graphite), accent_band(BuiltinTheme::Porcelain)), (COBALT_BAND, COBALT_BAND));
    report(&failures);
}

#[test]
fn only_the_making_room_slots_change() {
    let std_sets: [(&[Color32], &[Color32], usize); 4] = [
        (&DARK_TONES, &COBALT_DARK_TONES, COBALT_JADE_SLOT),
        (&LIGHT_TEXT, &COBALT_LIGHT_TEXT, COBALT_JADE_SLOT),
        (&LIGHT_LINES, &COBALT_LIGHT_LINES, COBALT_JADE_SLOT),
        (&CB_DARK_TONES, &COBALT_CB_DARK_TONES, COBALT_CB_SLOT),
    ];
    for (n, (from, to, slot)) in std_sets.iter().enumerate() {
        // The light tones also take Forest for Navy, which would pass for the cobalt.
        let forest = if (1..3).contains(&n) { LIGHT_NAVY_SLOT } else { *slot };
        for i in (0..from.len()).filter(|i| i != slot && *i != forest) {
            assert_eq!(from[i], to[i], "set {n} slot {i}");
        }
        assert_ne!(from[*slot], to[*slot], "set {n} changes slot {slot}");
    }
    for (from, to) in [(CB_LIGHT_LINES, COBALT_CB_LIGHT_LINES), (CB_LIGHT_TEXT, COBALT_CB_LIGHT_TEXT)] {
        for i in (0..8).filter(|i| *i != COBALT_CB_SLOT) {
            assert_eq!(from[i], to[i], "cb light slot {i}");
        }
    }
    // Jade keeps one hue across its tones, in the green the cobalt leaves free.
    for c in [COBALT_DARK_TONES[COBALT_JADE_SLOT], COBALT_LIGHT_TEXT[COBALT_JADE_SLOT], COBALT_LIGHT_LINES[COBALT_JADE_SLOT]] {
        let (_, chroma, h) = oklch(c);
        assert!((160.0..=172.0).contains(&h) && chroma >= 0.08, "Jade at {h:.1}, chroma {chroma:.3}");
    }
}

// The sleekness rules (docs/COLOUR.md, "Rules"): the six non-high-contrast themes.

/// The lineup but the high-contrast pair, which the sleekness rules exempt.
const SLEEK: [BuiltinTheme; 6] =
    [BuiltinTheme::Graphite, BuiltinTheme::Midnight, BuiltinTheme::Dark, BuiltinTheme::Porcelain, BuiltinTheme::Mist, BuiltinTheme::Light];

/// How far (OKLCH hue, deg) a selected row may sit from the hue it follows: within 25 deg of the
/// accent's hue (the field's when the accent is warm).
const SELECTED_HUE_SLACK: f64 = 25.0;

/// The limits are given to two decimals; a measured value that rounds to the limit meets it.
const ROUNDING: f64 = 0.0005;

/// A hue between amber and yellow-green: a warm accent, or a warm wash.
fn warm(h: f64) -> bool {
    (40.0..=125.0).contains(&h)
}

#[test]
fn rule_1_one_accent_per_theme() {
    for t in SLEEK {
        let p = builtin(t);
        assert_eq!(p.primary, p.accent, "{t:?}: primary is the accent");
    }
}

#[test]
fn rule_2_selection_is_quiet() {
    let mut failures = Vec::new();
    for t in SLEEK {
        let p = builtin(t);
        let (_, c, h) = oklch(p.selected);
        let (_, _, accent_h) = oklch(p.accent);
        let max = if p.dark { 0.08 } else { 0.05 };
        if c > max + ROUNDING {
            failures.push(format!("{t:?}: selected chroma {c:.4} is over {max}"));
        }
        // Within 25 deg of the accent's hue, or of the field's when the accent is warm.
        let follows = if warm(accent_h) { oklch(p.bg).2 } else { accent_h };
        if apart(h, follows) > SELECTED_HUE_SLACK {
            failures.push(format!("{t:?}: selected hue {h:.1} is {:.1} deg off {follows:.1}", apart(h, follows)));
        }
        if c >= 0.01 && warm(h) {
            failures.push(format!("{t:?}: selected is a warm wash (hue {h:.1})"));
        }
    }
    report(&failures);
}

#[test]
fn rule_3_accent_chroma() {
    let mut failures = Vec::new();
    for t in SLEEK {
        let p = builtin(t);
        let (l, c, _) = oklch(p.accent);
        // Each limit to two decimals: a value that rounds to it meets it.
        let (lightness, max) = if p.dark { (0.76..=0.86, 0.14) } else { (0.44..=0.52, 0.20) };
        let within = (lightness.start() - ROUNDING..=lightness.end() + ROUNDING).contains(&l);
        if !within || c > max + ROUNDING {
            failures.push(format!("{t:?}: accent at L {l:.3}, chroma {c:.4} (L {lightness:?}, chroma up to {max})"));
        }
    }
    report(&failures);
}

#[test]
fn rule_4_border_on_panel() {
    for t in SLEEK {
        let p = builtin(t);
        let r = ratio(p.border, p.panel);
        assert!((1.25..=1.45).contains(&r), "{t:?}: border on panel is {r:.3}:1");
    }
}

#[test]
fn rule_5_surface_ladder() {
    let lum = super::contrast::luminance;
    for t in SLEEK {
        let p = builtin(t);
        let ladder: &[(&str, Color32)] = if p.dark {
            &[("chrome", p.toolbar), ("bg", p.bg), ("panel", p.panel), ("raised", p.raised)]
        } else {
            &[("chrome", p.toolbar), ("raised", p.raised), ("bg", p.bg), ("panel", p.panel)]
        };
        for w in ladder.windows(2) {
            assert!(lum(w[0].1) < lum(w[1].1), "{t:?}: {} is not below {}", w[0].0, w[1].0);
        }
        if !p.dark {
            assert!(lum(p.panel) <= lum(p.input), "{t:?}: panel is above input");
        }
        let r = ratio(p.raised, p.panel);
        assert!(r >= 1.08, "{t:?}: raised is only {r:.3}:1 off the panel");
    }
}

#[test]
fn rule_6_themes_of_a_kind_are_distinct() {
    let mut failures = Vec::new();
    for (i, a) in SLEEK.iter().enumerate() {
        for b in SLEEK[i + 1..].iter().filter(|b| b.is_dark() == a.is_dark()) {
            let (pa, pb) = (builtin(*a), builtin(*b));
            let (accent, bg, selected) =
                (oklab_distance(pa.accent, pb.accent), oklab_distance(pa.bg, pb.bg), oklab_distance(pa.selected, pb.selected));
            if accent < 0.08 && bg < 0.02 {
                failures.push(format!("{a:?} and {b:?}: accents {accent:.3} and backgrounds {bg:.3} apart"));
            }
            if selected < 0.02 {
                failures.push(format!("{a:?} and {b:?}: selected rows {selected:.3} apart"));
            }
        }
    }
    report(&failures);
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
