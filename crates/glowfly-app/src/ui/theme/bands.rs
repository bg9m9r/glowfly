//! The reserved OKLCH hue bands the colour tests share (docs/COLOUR.md, "Rules"): each theme's
//! accent band, which only its accent family sits in, and the coral's attention band.

use super::contrast::oklch;
use eframe::egui::Color32;
use glowfly_core::settings::BuiltinTheme;
use std::ops::RangeInclusive;

/// The green accent's band: the Garden pair, high contrast, Sand, and the sea glass of Midnight
/// and Mist.
pub const GREEN_BAND: RangeInclusive<f64> = 145.0..=175.0;
/// The lantern gold's band: Dusk, Dawn and Pine. Narrower than the green's, as the warm side of
/// the wheel is crowded (the coral, `modified` and `warn`, Orange); there the gold also keeps
/// its distance by lightness (`branch_colour_cb_tests.rs`, `the_gold_needs_its_own_dark_set`).
pub const GOLD_BAND: RangeInclusive<f64> = 72.0..=92.0;
/// Ember's flame orange, just above the attention band and below the gold: Orange makes room for Jade, and
/// `modified` and `warn` move up to the amber side (hue about 90).
pub const FLAME_BAND: RangeInclusive<f64> = 61.0..=75.0;
/// The cobalt of Graphite and Linen, between the Sky and Slate repository hues: Azure makes
/// room for Jade, and `info` moves to hue about 228.
pub const COBALT_BAND: RangeInclusive<f64> = 243.0..=259.0;
/// The attention band: the coral "needs you" colour's alone.
pub const ATTENTION_BAND: RangeInclusive<f64> = 35.0..=60.0;

/// `t`'s accent band.
pub fn accent_band(t: BuiltinTheme) -> RangeInclusive<f64> {
    match t {
        BuiltinTheme::Dark | BuiltinTheme::Light | BuiltinTheme::Pine => GOLD_BAND,
        BuiltinTheme::Ember => FLAME_BAND,
        BuiltinTheme::Graphite | BuiltinTheme::Linen => COBALT_BAND,
        _ => GREEN_BAND,
    }
}

pub fn hue(c: Color32) -> f64 {
    oklch(c).2
}

/// Whether `c` sits in `t`'s accent band or the attention band.
pub fn in_reserved_band(t: BuiltinTheme, c: Color32) -> bool {
    accent_band(t).contains(&hue(c)) || ATTENTION_BAND.contains(&hue(c))
}
