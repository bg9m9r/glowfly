//! Palettes: by-name access, the built-ins' contrast rules, palettes from theme files and the
//! per-thread selection.

use super::color::{self, Palette};
use super::*;
use crate::themes::ThemeState;
use eframe::egui::Color32;
use std::collections::BTreeMap;
use trellis_core::settings::themes::COLOR_KEYS;
use trellis_core::settings::{BuiltinTheme, ColorValue, ThemeCatalog, ThemeChoice, ThemeFile};

#[test]
fn keys_match_theme_files() {
    assert_eq!(Palette::KEYS, COLOR_KEYS);
    let mut p = DARK;
    for (i, k) in Palette::KEYS.iter().enumerate() {
        let c = Color32::from_rgb(i as u8, 1, 2);
        *p.get_mut(k).unwrap() = c;
        assert_eq!(p.get(k), Some(c), "{k}");
    }
    assert_eq!((p.get("acent"), p.get_mut("dark").is_none()), (None, true));
    assert_eq!(p.hex_entries().len(), COLOR_KEYS.len());
    assert_eq!(p.hex_entries()[3], ("input", [3, 1, 2]));
}

#[test]
fn builtins() {
    for t in BuiltinTheme::ALL {
        assert_eq!(builtin(t).dark, t.is_dark(), "{t:?}");
    }
    // The original palettes are unchanged.
    assert_eq!((DARK.bg, DARK.text, DARK.syn_attr, DARK.diff_del_word), (rgb(0x1e2229), rgb(0xd3d8e0), rgb(0x8ac6d0), rgb(0x76303a)));
    assert_eq!((LIGHT.bg, LIGHT.text, LIGHT.syn_attr, LIGHT.diff_del_word), (rgb(0xfafbfc), rgb(0x242931), rgb(0x006b75), rgb(0xf2b4b4)));
}

fn rgb(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// WCAG relative luminance.
fn luminance(c: Color32) -> f64 {
    let lin = |v: u8| {
        let s = f64::from(v) / 255.0;
        if s <= 0.03928 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

fn ratio(a: Color32, b: Color32) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

#[test]
fn builtins_meet_the_contrast_rules() {
    let text_bgs =
        ["bg", "panel", "input", "hunk_bg", "diff_add", "diff_del", "diff_add_sel", "diff_del_sel", "diff_add_word", "diff_del_word"];
    let syn_bgs = ["panel", "diff_add", "diff_del", "diff_add_word", "diff_del_word"];
    let syn = Palette::KEYS.iter().filter(|k| k.starts_with("syn_"));
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let high = matches!(t, BuiltinTheme::HighContrastDark | BuiltinTheme::HighContrastLight);
        let mut check = |fg: &str, bg: &str, min: f64| {
            let r = ratio(p.get(fg).unwrap(), p.get(bg).unwrap());
            if r < min {
                failures.push(format!("{t:?}: {fg} on {bg} is {r:.2}:1, below {min}:1"));
            }
        };
        for bg in text_bgs {
            check("text", bg, if high { 7.0 } else { 4.5 });
        }
        if high {
            for bg in ["bg", "panel", "input"] {
                check("weak", bg, 4.5);
            }
        }
        for s in syn.clone() {
            for bg in syn_bgs {
                check(s, bg, if high { 4.5 } else { 2.4 });
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn ocean(base: BuiltinTheme, colors: &[(&'static str, ColorValue)]) -> ThemeFile {
    let mut f = ThemeFile::failed("ocean", std::path::Path::new("ocean.json"), trellis_core::settings::ThemeProblem::TooLarge);
    f.problems.clear();
    f.base = base;
    f.colors = colors.iter().copied().collect::<BTreeMap<_, _>>();
    f
}

#[test]
fn build_applies_a_file() {
    let f = ocean(BuiltinTheme::Light, &[("accent", ColorValue::Rgb([1, 2, 3])), ("hover", ColorValue::Base("selected"))]);
    let p = custom::build(&f);
    assert_eq!((p.accent, p.hover, p.dark, p.bg), (Color32::from_rgb(1, 2, 3), LIGHT.selected, false, LIGHT.bg));
    assert_eq!(custom::starter(&p)[7], ("accent", [1, 2, 3]));
}

#[test]
fn resolve_each_choice() {
    let file = ocean(BuiltinTheme::Light, &[("bg", ColorValue::Rgb([9, 9, 9]))]);
    let themes = ThemeState { catalog: ThemeCatalog { files: vec![file], ..ThemeCatalog::default() }, ..ThemeState::default() };
    let r = |c: ThemeChoice, sys| custom::resolve(&c, &themes, sys);
    assert_eq!(r(ThemeChoice::Dark, false), DARK);
    assert_eq!(r(ThemeChoice::Light, true), LIGHT);
    assert_eq!((r(ThemeChoice::System, true), r(ThemeChoice::System, false)), (DARK, LIGHT));
    assert_eq!(r(ThemeChoice::HighContrastDark, false), HIGH_CONTRAST_DARK);
    assert_eq!(r(ThemeChoice::HighContrastLight, true), HIGH_CONTRAST_LIGHT);
    assert_eq!(r(ThemeChoice::Sand, true), SAND);
    let found = r(ThemeChoice::Custom("ocean".into()), true);
    assert_eq!((found.bg, found.text, found.dark), (Color32::from_rgb(9, 9, 9), LIGHT.text, false));
    assert_eq!(r(ThemeChoice::Custom("gone".into()), false), DARK);
}

#[test]
fn the_palette_is_per_thread() {
    color::set_palette(&DARK);
    let other = std::thread::spawn(|| {
        color::set_palette(&LIGHT);
        (color::current(), color::is_dark(), color::bg())
    })
    .join()
    .unwrap();
    assert_eq!(other, (LIGHT, false, LIGHT.bg));
    assert_eq!((color::current(), color::is_dark(), color::bg()), (DARK, true, DARK.bg));
    color::set_palette(&SAND);
    assert!(!color::is_dark());
    assert_eq!(color::accent(), SAND.accent);
    color::set_palette(&HIGH_CONTRAST_DARK);
    assert!(color::is_dark());
}
