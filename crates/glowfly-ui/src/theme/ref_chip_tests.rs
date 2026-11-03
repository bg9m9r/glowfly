//! The ref chips' guards in every built-in theme and both branch presets, on every swatch: the
//! text reads on the fill (or, outlined, on every row surface), the fill or edge stands 3:1 from
//! every row surface, and only the checked-out branch and a detached HEAD wear a halo, one that
//! stands apart too (docs/COLOUR.md, "Ref chips").

use super::branch_colours::swatches;
use super::color::Palette;
use super::ref_chip::{BranchTone, ChipLook, ChipStyle, ref_chip, text_target, worst};
use super::surfaces::row_surfaces;
use super::{applied, builtin};
use eframe::egui::Color32;
use glowfly_core::branch_colour::Swatch;
use glowfly_core::settings::{BranchPalette, BuiltinTheme};

/// Every built-in in both presets, and every chip it draws: (what, the palette, the look).
fn chips() -> Vec<(String, Palette, ChipLook)> {
    let mut v = Vec::new();
    for t in BuiltinTheme::ALL {
        for preset in [BranchPalette::Standard, BranchPalette::ColourBlind] {
            let p = applied(*builtin(t), preset);
            for s in swatches() {
                for style in ChipStyle::ALL {
                    v.push((format!("{t:?} {preset:?} {s:?} {style:?}"), p, ref_chip(style, BranchTone::of(s, &p), &p)));
                }
            }
        }
    }
    v
}

fn report(failures: &[String]) {
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn chip_text_reads_on_its_fill_or_on_every_row() {
    let mut failures = Vec::new();
    for (what, p, look) in chips() {
        let target = text_target(&p);
        let (r, on) = match look.fill {
            Some(fill) if fill.a() != 255 => {
                failures.push(format!("{what}: the fill is not opaque"));
                continue;
            }
            Some(fill) => (super::contrast::ratio(look.text, fill), "its fill"),
            None => (worst(look.text, &row_surfaces(&p)), "the worst row surface"),
        };
        if r < target {
            failures.push(format!("{what}: text {r:.2}:1 on {on}, under {target}:1"));
        }
    }
    report(&failures);
}

#[test]
fn every_chip_stands_apart_from_every_row() {
    let mut failures = Vec::new();
    for (what, p, look) in chips() {
        let rows = row_surfaces(&p);
        let apart = |c: Option<Color32>| c.map_or(0.0, |c| worst(c, &rows));
        let best = apart(look.fill).max(apart(look.edge));
        if best < 3.0 {
            failures.push(format!("{what}: fill or edge {best:.2}:1 against its worst row surface"));
        }
    }
    report(&failures);
}

#[test]
fn the_checked_out_branch_and_head_wear_a_halo() {
    let mut failures = Vec::new();
    for (what, p, look) in chips() {
        let wants = what.ends_with("CurrentBranch") || what.ends_with("Head");
        match look.halo {
            Some(halo) if !wants => failures.push(format!("{what}: a halo {halo:?} on a chip that isn't where you are")),
            None if wants => failures.push(format!("{what}: no halo")),
            Some(halo) if worst(halo, &row_surfaces(&p)) < 3.0 => {
                failures.push(format!("{what}: the halo is {:.2}:1 against its worst row surface", worst(halo, &row_surfaces(&p))));
            }
            _ => {}
        }
    }
    report(&failures);
}

#[test]
fn local_branches_are_filled_and_remotes_and_tags_outlined() {
    for (what, p, look) in chips() {
        let filled = !(what.ends_with("Remote") || what.ends_with("Tag"));
        assert_eq!(look.fill.is_some(), filled, "{what}");
        assert_eq!(look.edge.is_some(), !p.dark || !filled, "{what}: an edge on light palettes and on outlined chips");
    }
}

#[test]
fn row_surfaces_hold_the_selection_hover_and_every_band() {
    let p = *builtin(BuiltinTheme::Dark);
    let rows = row_surfaces(&p);
    for c in [p.bg, p.hover, p.selected, super::tokens::selected_inactive(&p)] {
        assert!(rows.contains(&c), "{c:?}");
    }
    let band = super::surfaces::band_color(super::branch_colours::line_in(Swatch::Slot(3), &p), &p);
    assert!(rows.contains(&band), "slot 3's band");
}
