//! Palettes: by-name access, the built-ins' contrast and shape rules, palettes from theme files,
//! the per-thread selection, and the spacing grid.

use super::color::{self, Palette};
use super::contrast::{self, ChipStyle, ratio};
use super::*;
use crate::themes::ThemeState;
use eframe::egui::Color32;
use glowfly_core::settings::themes::COLOR_KEYS;
use glowfly_core::settings::{BuiltinTheme, ColorValue, ThemeCatalog, ThemeChoice, ThemeFile};
use std::collections::BTreeMap;

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
    assert_eq!((p.hex_entries()[3].0, p.hex_entries()[4]), ("raised", ("input", [4, 1, 2])));
}

#[test]
fn builtins() {
    for t in BuiltinTheme::ALL {
        assert_eq!(builtin(t).dark, t.is_dark(), "{t:?}");
    }
    // A few values pinned, so a palette change is deliberate.
    assert_eq!((DUSK.bg, DUSK.panel, DUSK.accent, DUSK.text), (rgb(0x1b1829), rgb(0x221f31), rgb(0xf6c159), rgb(0xe6e5ef)));
    assert_eq!(
        (DAWN.bg, DAWN.panel, DAWN.accent, DAWN.primary, DAWN.text),
        (rgb(0xf7f6fc), rgb(0xfbfafe), rgb(0x5a47cf), rgb(0x5a47cf), rgb(0x211f38))
    );
    assert_eq!((DARK, LIGHT, DEFAULT_DARK), (DUSK, DAWN, GRAPHITE), "Graphite is the default dark theme");
    assert_eq!(builtin(BuiltinTheme::DEFAULT_LIGHT), &PORCELAIN, "and Porcelain the light");
    let g = (&GARDEN_DARK, &GARDEN_LIGHT);
    assert_eq!((g.0.bg, g.0.text, g.0.syn_attr, g.0.diff_del_word), (rgb(0x151b18), rgb(0xe0e6e3), rgb(0x8ac6d0), rgb(0x76303a)));
    assert_eq!((g.1.bg, g.1.text, g.1.syn_attr, g.1.diff_del_word), (rgb(0xf9f7f1), rgb(0x161c19), rgb(0x006b75), rgb(0xf2b4b4)));
}

/// The palettes the theme tests hold to the rules: the eight built-ins, then the Garden pair,
/// which is no longer a theme but is what the view snapshots draw in.
pub(super) fn palettes() -> Vec<(String, &'static Palette)> {
    let built_in = BuiltinTheme::ALL.into_iter().map(|t| (format!("{t:?}"), builtin(t)));
    built_in.chain([("GardenDark".to_owned(), &GARDEN_DARK), ("GardenLight".to_owned(), &GARDEN_LIGHT)]).collect()
}

/// Whether `p` is one of the high-contrast palettes, which meet the higher floors.
pub(super) fn high_contrast(p: &Palette) -> bool {
    *p == HIGH_CONTRAST_DARK || *p == HIGH_CONTRAST_LIGHT
}

pub(super) fn rgb(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

#[test]
fn builtins_meet_the_contrast_rules() {
    let text_bgs = [
        "bg",
        "panel",
        "toolbar",
        "raised",
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
    for (t, p) in palettes() {
        let high = high_contrast(p);
        let (text_min, border_min) = if high { (7.0, 4.5) } else { (4.5, 3.0) };
        let mut check = |fg: &str, bg: &str, min: f64| {
            let r = ratio(p.get(fg).unwrap(), p.get(bg).unwrap());
            if r < min {
                failures.push(format!("{t}: {fg} on {bg} is {r:.2}:1, below {min}:1"));
            }
        };
        for bg in text_bgs {
            check("text", bg, text_min);
        }
        check("strong", "selected", text_min);
        for bg in ["bg", "panel", "toolbar", "raised", "input", "hover", "selected"] {
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
        // Filled primary buttons (the commit button) carry `on_primary` text.
        check("on_primary", "primary", text_min);
        let (text, weak, faint) = (ratio(p.text, p.panel), ratio(p.weak, p.panel), ratio(p.faint, p.panel));
        if weak < faint + 1.0 || text <= weak {
            failures.push(format!("{t}: on panel text {text:.2}, weak {weak:.2} and faint {faint:.2} are too close"));
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
    for (t, p) in palettes() {
        for lane in super::branch_colours::swatches() {
            for style in ChipStyle::ALL {
                let contrast::ChipLook { fill, text: fg, .. } = contrast::ref_chip(style, super::branch_colours::chip_in(lane, p), p);
                let r = ratio(fg, fill);
                if fill.a() != 255 || r < 4.5 {
                    failures.push(format!("{t} {lane:?} {style:?}: {r:.2}:1, fill alpha {}", fill.a()));
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
fn change_colors_are_readable() {
    let mut failures = Vec::new();
    for (t, p) in palettes() {
        let min = if high_contrast(p) { 4.5 } else { 3.0 };
        let lanes = super::branch_colours::swatches()
            .map(|l| (format!("{l:?}"), crate::ui::graph::band_color(super::branch_colours::line_in(l, p), p)));
        let bgs: Vec<(String, Color32)> = [("bg", p.bg), ("panel", p.panel), ("hover", p.hover), ("selected", p.selected)]
            .map(|(n, c)| (n.to_owned(), c))
            .into_iter()
            .chain(lanes)
            .collect();
        for name in ["added", "removed", "modified", "info", "progress", "warn"] {
            let fg = p.get(name).unwrap();
            for (bg_name, bg) in &bgs {
                let r = ratio(fg, *bg);
                if r < min {
                    failures.push(format!("{t}: {name} on {bg_name} is {r:.2}:1, below {min}:1"));
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
fn ref_chips_are_soft() {
    let mut failures = Vec::new();
    for (t, p) in palettes() {
        for lane in super::branch_colours::swatches() {
            let lane_col = super::branch_colours::chip_in(lane, p);
            let fill = |style| contrast::ref_chip(style, lane_col, p).fill;
            // The checked-out branch's chip may stand out (a solid fill on dark palettes).
            for style in [ChipStyle::Branch, ChipStyle::Remote] {
                let r = ratio(fill(style), p.bg);
                if r >= 3.0 {
                    failures.push(format!("{t} {lane:?} {style:?}: {r:.2}:1 against the background"));
                }
            }
            if fill(ChipStyle::Branch) == fill(ChipStyle::CurrentBranch) {
                failures.push(format!("{t} {lane:?}: the checked-out branch looks like any other"));
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
    // The dark panels are near black too (L* about 12), so they drop by a little less.
    for (name, p) in [
        ("dusk", DUSK),
        ("dawn", DAWN),
        ("garden dark", GARDEN_DARK),
        ("garden light", GARDEN_LIGHT),
        ("high contrast light", HIGH_CONTRAST_LIGHT),
        ("midnight", MIDNIGHT),
        ("mist", MIST),
        ("graphite", GRAPHITE),
        ("porcelain", PORCELAIN),
    ] {
        let dimmed = contrast::dimmed(p.panel, backdrop(&p));
        let drop = contrast::lightness(p.panel) - contrast::lightness(dimmed);
        assert!((8.0..=25.0).contains(&drop), "{name}: panel L* drops by {drop:.1}");
        if !p.dark {
            assert!(contrast::lightness(dimmed) >= 65.0, "{name}: dimmed panel is too dark");
        }
    }
}

#[test]
fn panels_are_lifted() {
    for (t, p) in palettes() {
        let l = contrast::lightness;
        assert!(l(p.panel) > l(p.bg), "{t}: panel is lighter than the canvas");
        if !high_contrast(p) {
            assert!(l(p.toolbar) < l(p.bg), "{t}: the chrome is darker than the canvas");
        }
        let r = ratio(p.raised, p.panel);
        assert!(r >= 1.08, "{t}: raised is only {r:.3}:1 off the panel");
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
    for (t, p) in palettes() {
        let ctx = egui::Context::default();
        apply_style(&ctx, p);
        let style = ctx.style_of(egui_theme(p));
        let v = &style.visuals;
        assert_eq!(v.selection.stroke.color, p.strong, "{t}");
        assert_eq!(v.widgets.noninteractive.weak_bg_fill, p.panel, "{t}");
        assert_eq!(v.widgets.inactive.bg_stroke, plain.style_of(egui_theme(p)).visuals.widgets.inactive.bg_stroke, "{t}: buttons");
        let hint = ratio(v.weak_text_color(), p.input);
        assert!(hint >= 3.0, "{t}: hint text is {hint:.2}:1 on input");
        assert_eq!(style.spacing.item_spacing, spacing::ITEM_SPACING, "{t}");
        assert_eq!(style.spacing.button_padding, spacing::BUTTON_PADDING, "{t}");
    }
}

pub(super) fn ocean(base: BuiltinTheme, colors: &[(&'static str, ColorValue)]) -> ThemeFile {
    let mut f = ThemeFile::failed("ocean", std::path::Path::new("ocean.json"), glowfly_core::settings::ThemeProblem::TooLarge);
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
    assert_eq!(custom::starter(&p)[10], ("accent", [1, 2, 3]));
}

#[test]
fn custom_files_inherit_the_new_tokens() {
    let p = custom::build(&ocean(BuiltinTheme::Light, &[("panel", ColorValue::Rgb([1, 2, 3])), ("input", ColorValue::Rgb([4, 5, 6]))]));
    assert_eq!((p.input_border, p.dialog_border), (LIGHT.input_border, LIGHT.dialog_border));
    let p = custom::build(&ocean(BuiltinTheme::Light, &[("input_border", ColorValue::Base("accent"))]));
    assert_eq!(p.input_border, LIGHT.accent);
    let text = br##"{ "base": "dark", "colors": { "accent": "#070809", "input_border": "accent" } }"##;
    let p = custom::build(&glowfly_core::settings::themes::parse("ocean", std::path::Path::new("ocean.json"), text));
    assert_eq!((p.input_border, p.accent), (Color32::from_rgb(7, 8, 9), Color32::from_rgb(7, 8, 9)), "the file's own accent");
}

#[test]
fn custom_files_get_the_default_selected_code_fills() {
    let hex = |v: u32| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
    // A file on Dusk takes the dark defaults, not Dusk's indigo.
    let p = custom::build(&ocean(BuiltinTheme::Dark, &[("bg", ColorValue::Rgb([9, 9, 9]))]));
    assert_eq!((p.code_sel, p.code_sel_dim), (hex(0x1d4f82), hex(0x474e56)));
    assert_ne!(p.code_sel, DARK.code_sel);
    // A file on a light base takes the light ones.
    let p = custom::build(&ocean(BuiltinTheme::Light, &[]));
    assert_eq!((p.code_sel, p.code_sel_dim), (hex(0xb9d5fe), hex(0xd8dfe9)));
    // A file's own code_sel stays; the quiet one it leaves out still defaults.
    let p = custom::build(&ocean(BuiltinTheme::Dark, &[("code_sel", ColorValue::Rgb([1, 2, 3]))]));
    assert_eq!((p.code_sel, p.code_sel_dim), (Color32::from_rgb(1, 2, 3), hex(0x474e56)));
}

#[test]
fn resolve_each_choice() {
    let file = ocean(BuiltinTheme::Light, &[("bg", ColorValue::Rgb([9, 9, 9]))]);
    let themes = ThemeState { catalog: ThemeCatalog { files: vec![file], ..ThemeCatalog::default() }, ..ThemeState::default() };
    let r = |c: ThemeChoice, sys| custom::resolve(&c, &themes, sys);
    assert_eq!(r(ThemeChoice::Dark, false), DARK);
    assert_eq!(r(ThemeChoice::Light, true), LIGHT);
    assert_eq!((r(ThemeChoice::System, true), r(ThemeChoice::System, false)), (GRAPHITE, PORCELAIN));
    assert_eq!(r(ThemeChoice::HighContrastDark, false), HIGH_CONTRAST_DARK);
    assert_eq!(r(ThemeChoice::HighContrastLight, true), HIGH_CONTRAST_LIGHT);
    assert_eq!((r(ThemeChoice::Midnight, false), r(ThemeChoice::Mist, true)), (MIDNIGHT, MIST));
    assert_eq!((r(ThemeChoice::Graphite, false), r(ThemeChoice::Porcelain, true)), (GRAPHITE, PORCELAIN));
    let found = r(ThemeChoice::Custom("ocean".into()), true);
    assert_eq!((found.bg, found.text, found.dark), (Color32::from_rgb(9, 9, 9), LIGHT.text, false));
    assert_eq!(r(ThemeChoice::Custom("gone".into()), false), GRAPHITE, "a missing file shows the default dark theme");
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
    color::set_palette(&PORCELAIN);
    assert!(!color::is_dark());
    assert_eq!(color::accent(), PORCELAIN.accent);
    color::set_palette(&HIGH_CONTRAST_DARK);
    assert!(color::is_dark());
}

#[test]
fn field_and_guide_fills_stay_readable() {
    let mut failures = Vec::new();
    for (t, p) in palettes() {
        let high = high_contrast(p);
        let mut check = |what: &str, fg: Color32, bg: Color32, min: f64| {
            let r = ratio(fg, bg);
            if r < min {
                failures.push(format!("{t}: {what} is {r:.2}:1, below {min}:1"));
            }
        };
        let (field, guide) = (p.field_fill(), p.guide_fill());
        check("text on field_fill", p.text, field, if high { 7.0 } else { 4.5 });
        check("weak on field_fill", p.weak, field, 4.5);
        check("faint on field_fill", p.faint, field, 4.5);
        check("input_border on field_fill", p.input_border, field, 3.0);
        check("input_border on panel", p.input_border, p.panel, 3.0);
        check("text on guide_fill", p.text, guide, 4.5);
        check("weak on guide_fill", p.weak, guide, 4.5);
        check("accent on guide_fill", p.accent, guide, 3.0);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
