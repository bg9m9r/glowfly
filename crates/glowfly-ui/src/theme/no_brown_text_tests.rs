//! No built-in theme writes brown, tan, khaki or olive text (docs/COLOUR.md, "No brown"): every
//! coloured text colour (status, `warn`, tag and hunk text, syntax, the terminal's coloured
//! basics) as it sits on its surfaces stays out of `contrast::brownish_text`'s zone, and the
//! light themes' amber text (`warn`, `modified`, `syn_type`, terminal yellow) still reads at
//! 4.5:1 (7:1 in high contrast) there. Neutral inks (body text, comments, the terminal's
//! black, white and foreground) are judged as greys and left out. High contrast light's amber
//! text is the one exception: at 7:1 on white an amber is chocolate whatever its chroma.

use super::builtin;
use super::color::Palette;
use super::contrast::{brownish_text, max_chroma, oklch, ratio};
use eframe::egui::Color32;
use glowfly_core::settings::BuiltinTheme;

/// The terminal basics that are coloured text: red to cyan, normal and bright.
const ANSI_TEXT: [usize; 12] = [1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14];

/// The amber texts this rule retuned on paper, by theme-file key.
const AMBER: [&str; 5] = ["warn", "modified", "syn_type", "terminal_yellow", "terminal_bright_yellow"];

/// Named surfaces a text sits on.
type Surfaces = Vec<(&'static str, Color32)>;

/// Every coloured text `p` writes, with the surfaces it sits on: (key, colour, surfaces).
fn texts(p: &Palette) -> Vec<(String, Color32, Surfaces)> {
    let rows =
        vec![("bg", p.bg), ("panel", p.panel), ("raised", p.raised), ("input", p.input), ("hover", p.hover), ("selected", p.selected)];
    let code = vec![("panel", p.panel), ("diff_add", p.diff_add), ("diff_del", p.diff_del)];
    let mut v = Vec::new();
    for key in ["added", "removed", "modified", "info", "progress", "warn"] {
        v.push((key.to_owned(), p.get(key).unwrap(), rows.clone()));
    }
    v.push(("hunk_fg".to_owned(), p.hunk_fg, vec![("hunk_bg", p.hunk_bg)]));
    v.push(("tag_fg".to_owned(), p.tag_fg, vec![("tag_bg", p.tag_bg)]));
    for key in Palette::KEYS.iter().filter(|k| k.starts_with("syn_") && **k != "syn_comment") {
        v.push(((*key).to_owned(), p.get(key).unwrap(), code.clone()));
    }
    // The basics' keys run terminal_black (0) to terminal_bright_white (15).
    let black = Palette::KEYS.iter().position(|k| *k == "terminal_black").unwrap();
    for i in ANSI_TEXT {
        v.push((Palette::KEYS[black + i].to_owned(), p.ansi.colors[i], vec![("terminal_background", p.ansi.background)]));
    }
    v
}

/// Whether `key` in theme `t` may read brown: high contrast light's amber text.
fn allowed(t: BuiltinTheme, key: &str) -> bool {
    t == BuiltinTheme::HighContrastLight && AMBER.contains(&key)
}

fn describe(t: BuiltinTheme, what: &str, c: Color32) -> String {
    let (l, ch, h) = oklch(c);
    format!(
        "{t:?} {what}: #{:02x}{:02x}{:02x} (L {l:.2} C {ch:.3} h {h:.0}, {:.0}% of the gamut)",
        c.r(),
        c.g(),
        c.b(),
        100.0 * ch / max_chroma(l, h)
    )
}

#[test]
fn no_theme_writes_brown_text() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        for (key, c, _) in texts(builtin(t)) {
            if brownish_text(c) && !allowed(t, &key) {
                failures.push(describe(t, &key, c));
            }
        }
    }
    assert!(failures.is_empty(), "{} brown texts:\n{}", failures.len(), failures.join("\n"));
}

/// Floors the Themes v2 spec's values miss, awaiting the designer: Dawn's amber (#985700) is
/// 4.47:1 on its new selected row (#e6e1fa), under 4.5.
const PENDING: [&str; 2] = ["Light modified on selected", "Light warn on selected"];

#[test]
fn light_amber_text_reads_on_its_surfaces() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL.into_iter().filter(|t| !builtin(*t).dark) {
        let min = if t == BuiltinTheme::HighContrastLight { 7.0 } else { 4.5 };
        for (key, c, on) in texts(builtin(t)).into_iter().filter(|(k, _, _)| AMBER.contains(&k.as_str())) {
            for (sname, s) in on {
                let r = ratio(c, s);
                if r < min {
                    failures.push(format!("{t:?} {key} on {sname}: {r:.2}:1, below {min}:1"));
                }
            }
        }
    }
    let met: Vec<_> = PENDING.iter().filter(|p| !failures.iter().any(|f| f.starts_with(*p))).collect();
    assert!(met.is_empty(), "pending floors that now pass: drop them: {met:?}");
    failures.retain(|f| !PENDING.iter().any(|p| f.starts_with(p)));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn high_contrast_light_still_needs_its_allowance() {
    // If its amber text ever leaves the zone, drop the allowance.
    let p = builtin(BuiltinTheme::HighContrastLight);
    assert!(texts(p).iter().filter(|(k, _, _)| AMBER.contains(&k.as_str())).all(|(_, c, _)| brownish_text(*c)));
}

#[test]
fn the_text_zone_holds_ochre_and_spares_clean_orange() {
    let hex = |c: u32| Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
    // The old paper ambers and golds: ochre, mustard and chocolate.
    for c in [0xa86b06, 0x9a6700, 0x714900, 0x7a4b00, 0x8a5a00, 0x8a6400, 0x6e5000, 0xa86b00] {
        assert!(brownish_text(hex(c)), "#{c:06x} reads brown");
    }
    // Gamut-edge oranges and a clean amber, a vermilion, and the colours `brownish` already spares.
    for c in [0xae6704, 0xa96000, 0xc47100, 0xa34400, 0x985700, 0xc92d00, 0xf6c159, 0x1f5fbf] {
        assert!(!brownish_text(hex(c)), "#{c:06x} is not brown");
    }
}
