//! No built-in theme paints brown, tan, khaki or olive (docs/COLOUR.md, "No brown"): every
//! surface, row band, chip, tint and wash, as composited on the surface it is drawn over, stays
//! out of `contrast::brownish`'s zone. Dusk's selected row (its bronze wash) is the one exception.

use super::branch_colours::{chip_in, line_in, swatches};
use super::builtin;
use super::color::Palette;
use super::contrast::{self, ChipStyle, brownish, oklch};
use super::surfaces::Tone;
use eframe::egui::Color32;
use glowfly_core::settings::BuiltinTheme;

/// The alphas the views wash a colour over a surface at (`Palette::wash`).
const WASHES: [f32; 10] = [0.03, 0.08, 0.12, 0.16, 0.18, 0.25, 0.28, 0.35, 0.6, 0.8];

/// Every fill `p` paints, composited onto what it is drawn over: (what, the opaque colour).
pub(super) fn fills(p: &Palette) -> Vec<(String, Color32)> {
    let mut v: Vec<(String, Color32)> = Vec::new();
    let mut add = |what: &str, c: Color32| v.push((what.to_owned(), c));
    for (k, c) in [
        ("bg", p.bg),
        ("panel", p.panel),
        ("toolbar", p.toolbar),
        ("raised", p.raised),
        ("input", p.input),
        ("border", p.border),
        ("hover", p.hover),
        ("selected", p.selected),
        ("primary_off", p.primary_off),
        ("danger_off", p.danger_off),
        ("tag_bg", p.tag_bg),
        ("banner", p.attention.banner),
        ("hunk_bg", p.hunk_bg),
        ("diff_add", p.diff_add),
        ("diff_del", p.diff_del),
        ("diff_add_sel", p.diff_add_sel),
        ("diff_del_sel", p.diff_del_sel),
        ("diff_add_word", p.diff_add_word),
        ("diff_del_word", p.diff_del_word),
        ("diff_add selected line", contrast::diff_line_selected(p, true)),
        ("diff_del selected line", contrast::diff_line_selected(p, false)),
        ("field fill", p.field_fill()),
        ("guide fill", p.guide_fill()),
        ("tool backing", p.tool_backing()),
        ("tool hover", p.tool_hover()),
        ("tool press", p.tool_press()),
        ("tool pressed", p.tool_pressed()),
        ("overlay", p.overlay()),
        ("overlay edge", p.overlay_edge()),
        ("inset", p.inset()),
        ("tonal fill", p.tonal_fill()),
        ("tonal hover", p.tonal_state(false)),
        ("tonal pressed", p.tonal_state(true)),
        ("terminal selection", p.wash_on(p.ansi.background, p.ansi.selection, 0.35)),
        ("terminal cursor", p.wash_on(p.ansi.background, p.ansi.cursor, 0.6)),
    ] {
        add(k, c);
    }
    for t in Tone::ALL {
        add(&format!("{t:?} tint"), p.tone_tint(t).0);
    }
    for (k, c) in [("added", p.status.added), ("removed", p.status.removed), ("modified", p.status.modified)] {
        add(&format!("{k} change chip"), contrast::change_chip(c, p).0);
    }
    // Washes of the semantic colours and the repository hues over the surfaces they sit on.
    let mut washed: Vec<(String, Color32)> = vec![
        ("accent".into(), p.accent),
        ("primary".into(), p.primary),
        ("warn".into(), p.attention.warn),
        ("needs_you".into(), p.attention.needs_you),
        ("added".into(), p.status.added),
        ("removed".into(), p.status.removed),
        ("modified".into(), p.status.modified),
        ("info".into(), p.status.info),
        ("progress".into(), p.status.progress),
        ("weak".into(), p.weak),
    ];
    washed.extend(p.repository.hues.iter().enumerate().map(|(i, c)| (format!("repository hue {i}"), *c)));
    for (name, c) in &washed {
        for (sname, s) in [("bg", p.bg), ("panel", p.panel), ("toolbar", p.toolbar), ("raised", p.raised)] {
            for t in WASHES {
                add(&format!("{name} wash {t} on {sname}"), p.wash_on(s, *c, t));
            }
        }
    }
    // The graph: each lane's band, highlighted (its branch hovered), its initials and its chips.
    for s in swatches() {
        let lane = line_in(s, p);
        let band = crate::ui::graph::band_color(lane, p);
        add(&format!("{s:?} band"), band);
        add(&format!("{s:?} band highlighted"), crate::ui::graph::highlight_color(band, lane));
        add(&format!("{s:?} initials"), super::branch_colours::initials_fill(lane, p));
        for style in ChipStyle::ALL {
            let look = contrast::ref_chip(style, chip_in(s, p), p);
            add(&format!("{s:?} {style:?} chip"), look.fill);
            if let Some(edge) = look.edge {
                add(&format!("{s:?} {style:?} chip edge"), edge);
            }
        }
    }
    v
}

/// Whether `what` in theme `t` may sit in the zone: Dusk's bronze selected row (the owner's
/// call on #25).
fn allowed(t: BuiltinTheme, what: &str) -> bool {
    t == BuiltinTheme::Dark && what == "selected"
}

#[test]
fn no_theme_paints_brown() {
    let mut failures = Vec::new();
    for t in BuiltinTheme::ALL {
        for (what, c) in fills(builtin(t)) {
            if brownish(c) && !allowed(t, &what) {
                let (l, ch, h) = oklch(c);
                failures.push(format!("{t:?} {what}: #{:02x}{:02x}{:02x} (L {l:.2} C {ch:.3} h {h:.0})", c.r(), c.g(), c.b()));
            }
        }
    }
    assert!(failures.is_empty(), "{} brown fills:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn dusk_keeps_its_bronze_selection() {
    // The exception is real: if Dusk's selection ever leaves the zone, drop the allowance.
    assert!(brownish(builtin(BuiltinTheme::Dark).selected));
}

#[test]
fn the_zone_holds_brown_and_spares_clean_colours() {
    let hex = |c: u32| Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
    // Brown, tan, khaki, olive, rust and a gold or amber wash on a dark field.
    for c in [0x5b3d0a, 0x4a3812, 0x8b5a2b, 0xc8a878, 0x9a8f5c, 0x6b6b2a, 0x3b2f0a, 0x7a4a2a, 0x3d3020, 0x4d3a28] {
        assert!(brownish(hex(c)), "#{c:06x} reads brown");
    }
    // Clean colours: greys, cream paper, saturated orange, gold and red, cool fields, a deep red.
    for c in [0x191a1d, 0x2f3134, 0xf9f7f3, 0xfbecc8, 0xff9b2a, 0xf6c159, 0xea6a64, 0x1f3d62, 0x0a1e1f, 0x7bc85f, 0x8a1f2c] {
        assert!(!brownish(hex(c)), "#{c:06x} is not brown");
    }
}
