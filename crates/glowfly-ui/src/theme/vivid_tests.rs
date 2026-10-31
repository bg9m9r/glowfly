//! Vivid colour: the repository hues, the coral "needs you", the toolbar backing and the rule
//! that the window chrome draws no status, attention or branch-lane colour (docs/COLOUR.md).

use super::bands::{ATTENTION_BAND, accent_band};
use super::color::{self, Palette};
use super::contrast::{self, mix, oklch, ratio};
use super::tests::rgb;
use super::*;
use eframe::egui::Color32;
use glowfly_core::settings::{BuiltinTheme, RepoColour};

/// Each repository hue's OKLCH hue target, in `RepoColour` order.
const TARGETS: [f64; 8] = [10.0, 128.0, 176.5, 207.0, 237.5, 267.5, 298.0, 335.0];

fn hue(c: Color32) -> f64 {
    oklch(c).2
}

/// The angle between two hues (0 to 180).
fn apart(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

fn high(t: BuiltinTheme) -> bool {
    matches!(t, BuiltinTheme::HighContrastDark | BuiltinTheme::HighContrastLight)
}

fn report(failures: &[String]) {
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn hexes(p: &Palette) -> Vec<u32> {
    p.repository.hues.iter().map(|c| u32::from_be_bytes([0, c.r(), c.g(), c.b()])).collect()
}

#[test]
fn repository_hues_take_the_values() {
    let want: [(BuiltinTheme, [u32; 8]); 13] = [
        (BuiltinTheme::Dark, [0xcb8a93, 0x90a872, 0x65af9d, 0x5dadb8, 0x6ea6ca, 0x899ed2, 0xa494ca, 0xbe8cb2]),
        (BuiltinTheme::Light, [0xa0636d, 0x6a814c, 0x3a8776, 0x2e8590, 0x477ea0, 0x6376a7, 0x7d6da1, 0x95658a]),
        (BuiltinTheme::GardenDark, [0xcb8a93, 0x90a872, 0x65af9d, 0x5dadb8, 0x6ea6ca, 0x899ed2, 0xa494ca, 0xbe8cb2]),
        (BuiltinTheme::GardenLight, [0xa0636d, 0x6a814c, 0x3a8776, 0x2e8590, 0x477ea0, 0x6376a7, 0x7d6da1, 0x95658a]),
        (BuiltinTheme::HighContrastDark, [0xe5a2ac, 0xa9c28a, 0x7cc8b5, 0x76c6d1, 0x86bfe4, 0xa1b6ea, 0xbdade5, 0xd8a4cc]),
        (BuiltinTheme::HighContrastLight, [0x814750, 0x4e6330, 0x156a5a, 0x0a6771, 0x286182, 0x475988, 0x605182, 0x76496d]),
        (BuiltinTheme::Sand, [0x9d606a, 0x677e49, 0x368473, 0x2a828d, 0x447b9d, 0x6073a4, 0x7a6a9e, 0x926287]),
        (BuiltinTheme::Midnight, [0xcb8a93, 0x90a872, 0x65af9d, 0x5dadb8, 0x6ea6ca, 0x899ed2, 0xa494ca, 0xbe8cb2]),
        (BuiltinTheme::Mist, [0xa0636d, 0x6a814c, 0x3a8776, 0x2e8590, 0x477ea0, 0x6376a7, 0x7d6da1, 0x95658a]),
        (BuiltinTheme::Ember, [0xcb8a93, 0x90a872, 0x65af9d, 0x5dadb8, 0x6ea6ca, 0x899ed2, 0xa494ca, 0xbe8cb2]),
        (BuiltinTheme::Pine, [0xcb8a93, 0x90a872, 0x65af9d, 0x5dadb8, 0x6ea6ca, 0x899ed2, 0xa494ca, 0xbe8cb2]),
        (BuiltinTheme::Graphite, [0xcb8a93, 0x90a872, 0x65af9d, 0x5dadb8, 0x6ea6ca, 0x899ed2, 0xa494ca, 0xbe8cb2]),
        (BuiltinTheme::Linen, [0xa0636d, 0x6a814c, 0x3a8776, 0x2e8590, 0x477ea0, 0x6376a7, 0x7d6da1, 0x95658a]),
    ];
    for (t, hues) in want {
        assert_eq!(hexes(builtin(t)), hues, "{t:?}");
    }
}

#[test]
fn repository_hues_are_quiet() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        for (i, c) in builtin(t).repository.hues.iter().enumerate() {
            let chroma = oklch(*c).1;
            if !(0.075..=0.085).contains(&chroma) {
                failures.push(format!("{t:?} hue {i}: chroma {chroma:.3}"));
            }
        }
    }
    report(&failures);
}

#[test]
fn repository_hues_keep_their_hue() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        for (c, target) in builtin(t).repository.hues.iter().zip(TARGETS) {
            let h = hue(*c);
            if apart(h, target) > 4.0 {
                failures.push(format!("{t:?}: {h:.1} deg, wanted {target}"));
            }
        }
    }
    report(&failures);
}

#[test]
fn repository_hues_keep_out_of_the_reserved_bands_and_apart() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let hues = builtin(t).repository.hues.map(hue);
        for (i, h) in hues.iter().enumerate() {
            if accent_band(t).contains(h) || ATTENTION_BAND.contains(h) {
                failures.push(format!("{t:?} hue {i} sits in a reserved band at {h:.1}"));
            }
            for (j, o) in hues.iter().enumerate().skip(i + 1) {
                // 30 deg apart by design (Mint to Slate sit packed between the green and iris bands), less up to a
                // degree lost rounding to 8-bit sRGB.
                if apart(*h, *o) < 29.0 {
                    failures.push(format!("{t:?} hues {i} and {j} are only {:.1} deg apart", apart(*h, *o)));
                }
            }
        }
    }
    report(&failures);
}

#[test]
fn repository_hues_read_on_the_chrome() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let min = if high(t) { 4.5 } else { 3.0 };
        for (i, c) in p.repository.hues.iter().enumerate() {
            for (on, bg) in [("toolbar", p.toolbar), ("bg", p.bg), ("panel", p.panel), ("raised", p.raised), ("hover", p.hover)] {
                let r = ratio(*c, bg);
                if r < min {
                    failures.push(format!("{t:?} hue {i} on {on}: {r:.2}:1"));
                }
            }
        }
    }
    report(&failures);
}

#[test]
fn neighbouring_groups_differ() {
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        color::set_palette(p);
        let groups: Vec<Color32> = (0..8).map(color::repo_group).collect();
        for c in p.repository.hues {
            assert!(groups.contains(&c), "{t:?}: the groups visit every hue");
        }
        for (n, pair) in groups.iter().chain(groups.first()).collect::<Vec<_>>().windows(2).enumerate() {
            let d = apart(hue(*pair[0]), hue(*pair[1]));
            assert!(d >= 90.0, "{t:?}: groups {n} and {} are {d:.1} deg apart", n + 1);
        }
        assert_eq!(color::repo_group(8), color::repo_group(0), "they repeat after 8");
    }
    color::set_palette(&DARK);
}

#[test]
fn needs_you_takes_the_given_values() {
    let want = [
        (BuiltinTheme::Dark, 0xfb794a, 0x22110b),
        (BuiltinTheme::Light, 0xbf4306, 0xffffff),
        (BuiltinTheme::GardenDark, 0xfb794a, 0x22110b),
        (BuiltinTheme::GardenLight, 0xbf4306, 0xffffff),
        (BuiltinTheme::HighContrastDark, 0xff8a5c, 0x22110b),
        (BuiltinTheme::HighContrastLight, 0x9e3301, 0xffffff),
        (BuiltinTheme::Sand, 0xbf4306, 0xffffff),
        (BuiltinTheme::Midnight, 0xfb794a, 0x22110b),
        (BuiltinTheme::Mist, 0xbf4306, 0xffffff),
        (BuiltinTheme::Ember, 0xfb794a, 0x22110b),
        (BuiltinTheme::Pine, 0xfb794a, 0x22110b),
        (BuiltinTheme::Graphite, 0xfb794a, 0x22110b),
        (BuiltinTheme::Linen, 0xbf4306, 0xffffff),
    ];
    for (t, fill, ink) in want {
        let a = builtin(t).attention;
        assert_eq!((a.needs_you, a.on_needs_you), (rgb(fill), rgb(ink)), "{t:?}");
    }
}

#[test]
fn needs_you_alone_in_the_attention_band() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let h = hue(p.attention.needs_you);
        if !ATTENTION_BAND.contains(&h) {
            failures.push(format!("{t:?}: the coral's hue is {h:.1}"));
        }
        let others = [("warn", p.attention.warn), ("accent", p.accent), ("primary", p.primary), ("selected", p.selected)];
        let hues = p.repository.hues.map(|c| ("repository", c));
        for (key, c) in others.into_iter().chain(hues) {
            if ATTENTION_BAND.contains(&hue(c)) {
                failures.push(format!("{t:?}: {key} sits in the attention band at {:.1}", hue(c)));
            }
        }
    }
    report(&failures);
}

#[test]
fn needs_you_ink_reads() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let a = p.attention;
        let min = if high(t) { 7.0 } else { 4.5 };
        let pole = if p.dark { Color32::WHITE } else { Color32::BLACK };
        for shade in [0.0, 0.10] {
            let r = ratio(a.on_needs_you, mix(a.needs_you, pole, shade));
            if r < min {
                failures.push(format!("{t:?}: the coral's ink at shade {shade} is {r:.2}:1"));
            }
        }
    }
    report(&failures);
}

#[test]
fn needs_you_stands_off_its_surroundings() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let on = [
            ("bg", p.bg),
            ("panel", p.panel),
            ("toolbar", p.toolbar),
            ("selected", p.selected),
            ("banner", p.attention.banner),
            ("hover", p.hover),
        ];
        for (key, bg) in on {
            let r = ratio(p.attention.needs_you, bg);
            if r < 3.0 {
                failures.push(format!("{t:?}: the coral on {key} is {r:.2}:1"));
            }
        }
    }
    report(&failures);
}

#[test]
fn toolbar_backing_is_a_quiet_accent_tint() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        let p = builtin(t);
        let (back, hover, pressed) = (p.tool_backing(), p.tool_hover(), p.tool_pressed());
        assert_eq!(back, contrast::tint(p.toolbar, p.accent, 0.09, p.dark), "{t:?}");
        // Quiet: no more colourful than Dusk's own neutrals (its hover is 0.041).
        let chroma = oklch(back).1;
        if chroma > 0.04 {
            failures.push(format!("{t:?}: the backing's chroma is {chroma:.3}"));
        }
        let min = if high(t) { 7.0 } else { 4.5 };
        for (key, fill) in [("backing", back), ("hover", hover), ("pressed", pressed)] {
            let r = ratio(p.text, fill);
            if r < min {
                failures.push(format!("{t:?}: text on the tool {key} is {r:.2}:1"));
            }
        }
        let weak = ratio(p.weak, back);
        if weak < 4.5 {
            failures.push(format!("{t:?}: weak on the backing is {weak:.2}:1"));
        }
        let (off, step) = (ratio(hover, back), ratio(pressed, hover));
        if off < 1.1 || step < 1.1 {
            failures.push(format!("{t:?}: hover is {off:.3}:1 off the backing, pressed {step:.3}:1 off hover"));
        }
    }
    report(&failures);
}

/// The chrome's own files, which may only use the accent, the repository hues and the coral.
const CHROME: [(&str, &str); 12] = [
    ("title_bar.rs", include_str!("../title_bar.rs")),
    ("tabs.rs", include_str!("../tabs.rs")),
    ("tab_paint.rs", include_str!("../tab_paint.rs")),
    ("tab_list.rs", include_str!("../tab_list.rs")),
    ("toolbar.rs", include_str!("../toolbar.rs")),
    ("toolbar_pickers.rs", include_str!("../toolbar_pickers.rs")),
    ("toolbar_menus.rs", include_str!("../toolbar_menus.rs")),
    ("status.rs", include_str!("../status.rs")),
    ("update_notice.rs", include_str!("../update_notice.rs")),
    ("trust_chip.rs", include_str!("../trust_chip.rs")),
    ("repo_line.rs", include_str!("../repo_line.rs")),
    ("window_buttons.rs", include_str!("../window_buttons.rs")),
];

#[test]
fn chrome_draws_no_status_or_attention_colour() {
    let banned = [
        "color::added(",
        "color::removed(",
        "color::modified(",
        "color::info(",
        "color::progress(",
        "color::warn(",
        "color::banner(",
        "status_color(",
        "color::lane(",
    ];
    let mut failures = Vec::new();
    for (file, src) in CHROME {
        for (n, line) in src.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for b in banned.iter().filter(|b| line.contains(**b)) {
                failures.push(format!("{file}:{}: {b}", n + 1));
            }
        }
    }
    report(&failures);
}

#[test]
fn repo_hue_reads_the_active_palette() {
    color::set_palette(&LIGHT);
    assert_eq!(color::repo_hue(RepoColour::Teal), LIGHT.repository.hues[3]);
    color::set_palette(&DARK);
    assert_eq!(color::repo_hue(RepoColour::Plum), DARK.repository.hues[7]);
}
