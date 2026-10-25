//! Midnight, Mist, Ember, Pine, Graphite and Linen: their pinned values, their neutrals on one
//! hue, no brown or olive surface anywhere dark, and the flame and cobalt branch families
//! (docs/COLOUR.md). The rules every built-in meets are checked over all of them elsewhere.

use super::bands::{ATTENTION_BAND, COBALT_BAND, FLAME_BAND, GREEN_BAND, accent_band};
use super::branch_palette::{
    BRANCH_CB_DARK, BRANCH_CB_LIGHT, BRANCH_DARK, BRANCH_LIGHT, CB_DARK_TONES, CB_LIGHT_LINES, CB_LIGHT_TEXT, DARK_TONES, LIGHT_LINES,
    LIGHT_NAVY_SLOT, LIGHT_TEXT,
};
use super::branch_palette_gold::{BRANCH_CB_DUSK, BRANCH_DUSK};
use super::branch_palette_more::*;
use super::color::Palette;
use super::contrast::oklch;
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
    // (theme, bg, panel, accent, text), pinned so a palette change is deliberate.
    let want = [
        (&MIDNIGHT, 0x0d1827, 0x14202f, 0x6eddb2, 0xe2e8f0),
        (&MIST, 0xf2f8fc, 0xf8fcfe, 0x007258, 0x142533),
        (&EMBER, 0x211819, 0x281f20, 0xff9b2a, 0xebe6e6),
        (&PINE, 0x111d1f, 0x182426, 0xf6c159, 0xe3e8ea),
        (&GRAPHITE, 0x191a1d, 0x202225, 0x7ebbff, 0xe6e7e9),
        (&LINEN, 0xf9f7f3, 0xfcfbf9, 0x004da8, 0x28211c),
    ];
    for (p, bg, panel, accent, text) in want {
        assert_eq!((p.bg, p.panel, p.accent, p.text), (rgb(bg), rgb(panel), rgb(accent), rgb(text)));
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
    let themes: [(&str, &Palette, f64, f64, f64, f64); 6] = [
        ("Midnight", &MIDNIGHT, 255.0, 255.0, 0.045, 0.045),
        ("Mist", &MIST, 235.0, 245.0, 0.02, 0.05),
        ("Ember", &EMBER, 10.0, 10.0, 0.025, 0.025),
        ("Pine", &PINE, 212.0, 212.0, 0.025, 0.025),
        ("Graphite", &GRAPHITE, 265.0, 265.0, 0.008, 0.012),
        ("Linen", &LINEN, 80.0, 55.0, 0.012, 0.016),
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
    // 125): a warm theme gets its warmth from its accent over an intentional hue (Ember's
    // oxblood), never from a muddy field.
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
fn linen_is_clean_not_cream() {
    // Linen's paper is warm by a hair (chroma at most 0.012), so it reads white, not sepia.
    for key in ["bg", "panel", "raised", "hover"] {
        let (_, c, _) = oklch(LINEN.get(key).unwrap());
        assert!(c <= 0.012, "{key}: chroma {c:.4}");
    }
}

#[test]
fn the_new_themes_use_their_families() {
    let want = [
        (&MIDNIGHT, BRANCH_DARK, BRANCH_CB_DARK),
        (&MIST, BRANCH_LIGHT, BRANCH_CB_LIGHT),
        (&EMBER, BRANCH_EMBER, BRANCH_CB_EMBER),
        (&PINE, BRANCH_DUSK, BRANCH_CB_DUSK),
        (&GRAPHITE, BRANCH_GRAPHITE, BRANCH_CB_GRAPHITE),
        (&LINEN, BRANCH_LINEN, BRANCH_CB_LINEN),
    ];
    for (p, branch, cb) in want {
        assert_eq!((p.branch, p.colour_blind), (branch, cb));
    }
}

#[test]
fn flame_and_cobalt_families_keep_out_of_their_bands() {
    let sets: [(&str, &[Color32], _); 7] = [
        ("flame dark", &FLAME_DARK_TONES, FLAME_BAND),
        ("flame cb dark", &FLAME_CB_DARK_TONES, FLAME_BAND),
        ("cobalt dark", &COBALT_DARK_TONES, COBALT_BAND),
        ("cobalt light text", &COBALT_LIGHT_TEXT, COBALT_BAND),
        ("cobalt light lines", &COBALT_LIGHT_LINES, COBALT_BAND),
        ("cobalt cb dark", &COBALT_CB_DARK_TONES, COBALT_BAND),
        ("cobalt cb light", &COBALT_CB_LIGHT_LINES, COBALT_BAND),
    ];
    let mut failures = Vec::new();
    for (set, tones, band) in sets {
        for (i, c) in tones.iter().enumerate() {
            let h = oklch(*c).2;
            if band.contains(&h) || ATTENTION_BAND.contains(&h) {
                failures.push(format!("{set} slot {i}: hue {h:.1}"));
            }
        }
    }
    // The bands themselves stay clear of the green that Jade sits in.
    assert!(!FLAME_BAND.contains(&165.0) && !COBALT_BAND.contains(&165.0) && GREEN_BAND.contains(&165.0));
    assert_eq!(accent_band(BuiltinTheme::Ember), FLAME_BAND);
    assert_eq!((accent_band(BuiltinTheme::Graphite), accent_band(BuiltinTheme::Linen)), (COBALT_BAND, COBALT_BAND));
    report(&failures);
}

#[test]
fn only_the_making_room_slots_change() {
    let std_sets: [(&[Color32], &[Color32], usize); 6] = [
        (&DARK_TONES, &FLAME_DARK_TONES, FLAME_JADE_SLOT),
        (&DARK_TONES, &COBALT_DARK_TONES, COBALT_JADE_SLOT),
        (&LIGHT_TEXT, &COBALT_LIGHT_TEXT, COBALT_JADE_SLOT),
        (&LIGHT_LINES, &COBALT_LIGHT_LINES, COBALT_JADE_SLOT),
        (&CB_DARK_TONES, &FLAME_CB_DARK_TONES, FLAME_CB_SLOT),
        (&CB_DARK_TONES, &COBALT_CB_DARK_TONES, COBALT_CB_SLOT),
    ];
    for (n, (from, to, slot)) in std_sets.iter().enumerate() {
        // Linen's light tones also take Forest for Navy, which would pass for the cobalt.
        let forest = if (2..4).contains(&n) { LIGHT_NAVY_SLOT } else { *slot };
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
    // Jade keeps one hue across its tones, in the green the flame and cobalt leave free.
    for c in [FLAME_DARK_TONES[FLAME_JADE_SLOT], COBALT_LIGHT_TEXT[COBALT_JADE_SLOT], COBALT_LIGHT_LINES[COBALT_JADE_SLOT]] {
        let (_, chroma, h) = oklch(c);
        assert!((160.0..=172.0).contains(&h) && chroma >= 0.08, "Jade at {h:.1}, chroma {chroma:.3}");
    }
}

#[test]
fn the_flame_sits_apart_from_the_coral() {
    // The flame is the nearest accent to the coral "needs you": at least 20 deg of hue and a
    // step of lightness apart.
    let ((cl, _, ch), (nl, _, nh)) = (oklch(EMBER.accent), oklch(EMBER.attention.needs_you));
    assert!(apart(ch, nh) >= 20.0 && cl - nl >= 0.05, "flame {cl:.2} {ch:.0}, coral {nl:.2} {nh:.0}");
}

#[test]
fn ember_is_red_not_brown_and_pine_is_a_hint_of_green() {
    // Ember's field is a charcoal leaning red (hue under 30, chroma under 0.02, so it never reads
    // chocolate), its selected row a clean deep red, not a dark orange or a brick, and its accent
    // a vivid flame. Pine's field is a quiet spruce slate, less coloured than Dusk's, so the gold
    // is the one vivid thing, and its selected row a petrol blue, not the gold's brown wash.
    for key in ["bg", "panel", "toolbar", "raised", "hover"] {
        let (_, c, h) = oklch(EMBER.get(key).unwrap());
        assert!((0.0..=30.0).contains(&h) && c <= 0.02, "Ember {key}: {c:.3} at {h:.0}");
        let (_, c, h) = oklch(PINE.get(key).unwrap());
        assert!((200.0..=225.0).contains(&h) && c <= 0.025, "Pine {key}: {c:.3} at {h:.0}");
    }
    let (_, sc, sh) = oklch(EMBER.selected);
    assert!(sh < 35.0 && sc >= 0.12, "Ember's selected row at {sh:.0}, chroma {sc:.3}");
    let (_, pc, ph) = oklch(PINE.selected);
    assert!((200.0..=240.0).contains(&ph) && pc >= 0.05, "Pine's selected row at {ph:.0}, chroma {pc:.3}");
    let (_, ac, _) = oklch(EMBER.accent);
    assert!(ac >= 0.15, "Ember's accent chroma {ac:.3}");
    // A tint toward the flame (toolbar groups, the main lane's band) stays red, not brown.
    let (_, _, th) = oklch(EMBER.tool_backing());
    assert!(th < 38.0, "Ember's toolbar backing turns to {th:.0}");
    let field = oklch(PINE.bg).1;
    assert!(field <= oklch(DUSK.bg).1 && oklch(PINE.accent).1 > 4.0 * field, "Pine's field chroma {field:.3}");
}
