//! Palettes: by-name access, the built-ins' contrast and shape rules, palettes from theme files,
//! the per-thread selection, and the spacing grid.

use super::color::{self, Palette};
use super::contrast::{self, ChipStyle, ratio};
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
    // A few values pinned, so a palette change is deliberate.
    assert_eq!((DARK.bg, DARK.text, DARK.syn_attr, DARK.diff_del_word), (rgb(0x1a1d22), rgb(0xdde1e7), rgb(0x8ac6d0), rgb(0x76303a)));
    assert_eq!((LIGHT.bg, LIGHT.text, LIGHT.syn_attr, LIGHT.diff_del_word), (rgb(0xf3f5f8), rgb(0x1f242b), rgb(0x006b75), rgb(0xf2b4b4)));
}

fn rgb(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

#[test]
fn builtins_meet_the_contrast_rules() {
    let text_bgs = [
        "bg",
        "panel",
        "toolbar",
        "input",
        "hover",
        "selected",
        "hunk_bg",
        "diff_add",
        "diff_del",
        "diff_add_sel",
        "diff_del_sel",
        "diff_add_word",
        "diff_del_word",
    ];
    let syn_bgs = ["panel", "diff_add", "diff_del", "diff_add_word", "diff_del_word"];
    let syn = Palette::KEYS.iter().filter(|k| k.starts_with("syn_"));
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let high = matches!(t, BuiltinTheme::HighContrastDark | BuiltinTheme::HighContrastLight);
        let (text_min, border_min) = if high { (7.0, 4.5) } else { (4.5, 3.0) };
        let mut check = |fg: &str, bg: &str, min: f64| {
            let r = ratio(p.get(fg).unwrap(), p.get(bg).unwrap());
            if r < min {
                failures.push(format!("{t:?}: {fg} on {bg} is {r:.2}:1, below {min}:1"));
            }
        };
        for bg in text_bgs {
            check("text", bg, text_min);
        }
        check("strong", "selected", text_min);
        for bg in ["bg", "panel", "toolbar", "input", "hover", "selected"] {
            check("weak", bg, 4.5);
        }
        for bg in ["bg", "panel", "input"] {
            check("faint", bg, 4.5);
            check("accent", bg, 3.0);
        }
        for bg in ["bg", "panel", "input", "toolbar"] {
            check("input_border", bg, border_min);
        }
        check("dialog_border", "panel", border_min);
        check("tag_fg", "tag_bg", text_min);
        for s in syn.clone() {
            for bg in syn_bgs {
                check(s, bg, if high { 4.5 } else { 2.4 });
            }
        }
        // Filled primary buttons (the commit button) carry white text.
        let white = ratio(Color32::WHITE, p.primary);
        if white < text_min {
            failures.push(format!("{t:?}: white on primary is {white:.2}:1, below {text_min}:1"));
        }
        let (text, weak, faint) = (ratio(p.text, p.panel), ratio(p.weak, p.panel), ratio(p.faint, p.panel));
        if weak < faint + 1.0 || text <= weak {
            failures.push(format!("{t:?}: on panel text {text:.2}, weak {weak:.2} and faint {faint:.2} are too close"));
        }
    }
    assert!(
        failures.is_empty(),
        "{}",
        failures.join(
            "
"
        )
    );
}

#[test]
fn ref_chips_are_readable() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        for lane in 0..crate::ui::common::LANE_COUNT {
            for style in ChipStyle::ALL {
                let (fill, fg) = contrast::ref_chip(style, crate::ui::common::lane_color(lane), p);
                let r = ratio(fg, fill);
                if fill.a() != 255 || r < 4.5 {
                    failures.push(format!("{t:?} lane {lane} {style:?}: {r:.2}:1, fill alpha {}", fill.a()));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{}",
        failures.join(
            "
"
        )
    );
}

#[test]
fn backdrop_dims_light_and_dark_alike() {
    assert!(backdrop(&DARK).a() > backdrop(&LIGHT).a());
    // High contrast dark is left out: its surfaces are already black, so there is little to dim.
    for (name, p) in [("dark", DARK), ("light", LIGHT), ("sand", SAND), ("high contrast light", HIGH_CONTRAST_LIGHT)] {
        let dimmed = contrast::dimmed(p.panel, backdrop(&p));
        let drop = contrast::lightness(p.panel) - contrast::lightness(dimmed);
        assert!((9.0..=25.0).contains(&drop), "{name}: panel L* drops by {drop:.1}");
        if !p.dark {
            assert!(contrast::lightness(dimmed) >= 65.0, "{name}: dimmed panel is too dark");
        }
    }
}

/// The HSL hue of `c`, in degrees (0 for a grey).
fn hue(c: Color32) -> f32 {
    let [r, g, b] = [c.r(), c.g(), c.b()].map(|v| f32::from(v) / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let d = max - min;
    if d == 0.0 {
        return 0.0;
    }
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    h * 60.0
}

#[test]
fn one_teal_accent_and_amber_for_progress() {
    assert!((hue(rgb(0x2ec4d1)) - 184.9).abs() < 0.5 && hue(rgb(0xff0000)) == 0.0 && hue(rgb(0x0000ff)) == 240.0);
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        for (key, c) in [("accent", p.accent), ("primary", p.primary), ("primary_border", p.primary_border), ("selected", p.selected)] {
            let h = hue(c);
            assert!((170.0..=200.0).contains(&h), "{t:?}: {key}'s hue is {h:.0}");
        }
        let h = hue(p.amber);
        assert!((30.0..=50.0).contains(&h), "{t:?}: amber's hue is {h:.0}");
    }
}

#[test]
fn panels_are_lifted() {
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let l = contrast::lightness;
        assert!(l(p.panel) > l(p.bg), "{t:?}: panel is lighter than the canvas");
        if p.dark {
            assert!(l(p.toolbar) > l(p.panel), "{t:?}: toolbar is lighter than the panel");
        }
    }
}

#[test]
fn spacing_is_on_the_grid() {
    use super::spacing::*;
    let all = [GRID, ITEM_SPACING.x, ITEM_SPACING.y, BUTTON_PADDING.x, BUTTON_PADDING.y, PANEL_PAD_F];
    for v in all {
        assert_eq!(v % 4.0, 0.0, "{v} is on the 4 px grid");
    }
    assert!((16..=20).contains(&PANEL_PAD));
    assert_eq!(PANEL_PAD_F, f32::from(PANEL_PAD));
}

#[test]
fn egui_style_follows_the_palette() {
    let plain = egui::Context::default();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let ctx = egui::Context::default();
        apply_style(&ctx, p);
        let style = ctx.style_of(egui_theme(p));
        let v = &style.visuals;
        assert_eq!(v.selection.stroke.color, p.strong, "{t:?}");
        assert_eq!(v.widgets.noninteractive.weak_bg_fill, p.panel, "{t:?}");
        assert_eq!(v.widgets.inactive.bg_stroke, plain.style_of(egui_theme(p)).visuals.widgets.inactive.bg_stroke, "{t:?}: buttons");
        let hint = ratio(v.weak_text_color(), p.input);
        assert!(hint >= 3.0, "{t:?}: hint text is {hint:.2}:1 on input");
        assert_eq!(style.spacing.item_spacing, spacing::ITEM_SPACING, "{t:?}");
        assert_eq!(style.spacing.button_padding, spacing::BUTTON_PADDING, "{t:?}");
    }
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
    assert_eq!(custom::starter(&p)[9], ("accent", [1, 2, 3]));
}

#[test]
fn custom_files_inherit_the_new_tokens() {
    let p = custom::build(&ocean(BuiltinTheme::Light, &[("panel", ColorValue::Rgb([1, 2, 3])), ("input", ColorValue::Rgb([4, 5, 6]))]));
    assert_eq!((p.input_border, p.dialog_border), (LIGHT.input_border, LIGHT.dialog_border));
    let p = custom::build(&ocean(BuiltinTheme::Light, &[("input_border", ColorValue::Base("accent"))]));
    assert_eq!(p.input_border, LIGHT.accent);
    let text = br##"{ "base": "dark", "colors": { "accent": "#070809", "input_border": "accent" } }"##;
    let p = custom::build(&trellis_core::settings::themes::parse("ocean", std::path::Path::new("ocean.json"), text));
    assert_eq!((p.input_border, p.accent), (Color32::from_rgb(7, 8, 9), Color32::from_rgb(7, 8, 9)), "the file's own accent");
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
