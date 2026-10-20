use super::*;
use crate::state::window_native::Backdrop;
use crate::themes::ThemeState;
use crate::ui::theme::contrast::lightness;
use crate::ui::theme::{DARK, LIGHT, SAND, ThemeCache};
use eframe::egui;
use trellis_core::settings::ThemeChoice;

/// The built-in palettes that can sit on Mica, with the tints the ladder picks for them. Dark's
/// `faint` misses 4.5 over the lightest dark bands at every step, so its sidebar stays opaque.
const PINNED: [(&str, Palette, MicaTint); 3] = [
    ("Dark", DARK, MicaTint { chrome: 153, side: OPAQUE }),
    ("Light", LIGHT, MicaTint { chrome: 153, side: 191 }),
    ("Sand", SAND, MicaTint { chrome: 153, side: 153 }),
];

fn alpha(t: MicaTint, s: Surface) -> u8 {
    match s {
        Surface::Chrome => t.chrome,
        Surface::Side => t.side,
    }
}

#[test]
fn builtins_get_a_mica_tint() {
    for (name, p, pinned) in PINNED {
        let tint = mica_tint(&p).unwrap_or_else(|| panic!("{name} gets a tint"));
        assert_eq!(tint, pinned, "{name}: a palette change moved its tint; pin the new one deliberately");
        assert!(tint.chrome >= 153 && tint.side >= 153, "{name}");
        assert!(tint.chrome < OPAQUE, "{name}: the chrome sits on Mica");
    }
}

#[test]
fn tinted_surfaces_meet_the_floors_over_every_band() {
    for (name, p, tint) in PINNED {
        for s in [Surface::Chrome, Surface::Side] {
            let a = alpha(tint, s);
            for &b in bands(&p) {
                let c = tinted(&p, s, b, a);
                for (token, fg, min) in checks(&p, s) {
                    let r = ratio(fg, c);
                    assert!(r >= min, "{name} {s:?} at {a} over {b:?}: {token} is {r:.2}, under {min}");
                }
            }
        }
    }
    // The title bar never shows faint text, so it isn't checked there; the side surface checks it.
    assert!(checks(&DARK, Surface::Chrome).iter().all(|(t, _, _)| *t != "faint"));
    assert!(checks(&DARK, Surface::Side).iter().any(|(t, _, _)| *t == "faint"));
}

#[test]
fn one_step_lower_fails_somewhere() {
    for (name, p, tint) in PINNED {
        for s in [Surface::Chrome, Surface::Side] {
            let a = alpha(tint, s);
            if a == OPAQUE {
                assert!(STEPS.iter().all(|&step| !passes(&p, s, step)), "{name} {s:?}: opaque only when every step fails");
                continue;
            }
            let i = STEPS.iter().position(|&step| step == a).unwrap_or_else(|| panic!("{name} {s:?}: {a} is a step"));
            if i > 0 {
                assert!(!passes(&p, s, STEPS[i - 1]), "{name} {s:?}: {} would do", STEPS[i - 1]);
            }
        }
    }
}

/// The tint the shell's theme cache gives `choice`.
fn cached_tint(choice: ThemeChoice) -> Option<MicaTint> {
    let ctx = egui::Context::default();
    let mut cache = ThemeCache::default();
    cache.select(&ctx, &choice, Default::default(), &ThemeState::default(), true);
    cache.mica_tint()
}

#[test]
fn high_contrast_and_unreadable_palettes_stay_solid() {
    assert_eq!(cached_tint(ThemeChoice::HighContrastDark), None);
    assert_eq!(cached_tint(ThemeChoice::HighContrastLight), None);
    assert_eq!(cached_tint(ThemeChoice::Dark), mica_tint(&DARK));
    assert_eq!(cached_tint(ThemeChoice::Sand), mica_tint(&SAND));
    let unreadable = Palette { weak: DARK.toolbar, ..DARK };
    assert_eq!(mica_tint(&unreadable), None, "weak can't be read on the toolbar or the panel");
}

#[test]
fn fill_keeps_solid_and_tints_mica() {
    let c = Color32::from_rgb(10, 20, 30);
    for s in [Surface::Chrome, Surface::Side] {
        assert_eq!(fill(Backdrop::Solid, s, c), c);
    }
    let mica = Backdrop::Mica { chrome: 166, side: 204 };
    assert_eq!(fill(mica, Surface::Chrome, c), Color32::from_rgba_premultiplied(7, 13, 20, 166));
    assert_eq!(fill(mica, Surface::Side, c), Color32::from_rgba_premultiplied(8, 16, 24, 204));
    assert_eq!(fill(Backdrop::Mica { chrome: OPAQUE, side: OPAQUE }, Surface::Side, c), c, "an opaque surface");
    // The bands sit where documented: dark Mica is dark, light Mica is light.
    for b in MICA_DARK {
        assert!(lightness(b) <= 32.0, "{b:?}");
    }
    for b in MICA_LIGHT {
        assert!(lightness(b) >= 80.0, "{b:?}");
    }
}

/// `fg` (premultiplied) over the opaque `bg`, blended per channel in gamma space as the renderer
/// and the compositor do.
fn over(fg: Color32, bg: Color32) -> Color32 {
    let keep = 255 - u32::from(fg.a());
    let ch = |f: u8, b: u8| (u32::from(f) + (u32::from(b) * keep + 127) / 255).min(255) as u8;
    Color32::from_rgb(ch(fg.r(), bg.r()), ch(fg.g(), bg.g()), ch(fg.b(), bg.b()))
}

#[test]
fn what_fill_draws_is_what_the_ladder_checks() {
    for (name, p, tint) in PINNED {
        let mica = Backdrop::Mica { chrome: tint.chrome, side: tint.side };
        for s in [Surface::Chrome, Surface::Side] {
            let a = alpha(tint, s);
            for &b in bands(&p) {
                let drawn = over(fill(mica, s, base(&p, s)), b);
                let modelled = tinted(&p, s, b, a);
                let off = [drawn.r().abs_diff(modelled.r()), drawn.g().abs_diff(modelled.g()), drawn.b().abs_diff(modelled.b())];
                assert!(off.iter().all(|&d| d <= 1), "{name} {s:?} at {a} over {b:?}: draws {drawn:?}, the ladder checks {modelled:?}");
            }
        }
    }
}
