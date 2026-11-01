//! The built-in palettes, by theme: Graphite and Porcelain (the defaults, `builtin_cobalt.rs`),
//! Midnight and Mist (`builtin_sea.rs`), Dusk and Dawn (`builtin_dusk.rs`) and the high-contrast
//! pair (`builtin_extra.rs`). The Garden pair (`builtin_garden.rs`) is no longer a theme: the view
//! snapshots draw in it, and theme files on Dusk or Dawn take its branch families. The values and
//! the rules they meet are in docs/COLOUR.md.

use super::color::Palette;
use glowfly_core::settings::BuiltinTheme;

pub use super::builtin_cobalt::{GRAPHITE, PORCELAIN};
pub use super::builtin_dusk::{DAWN, DUSK};
pub use super::builtin_extra::{HIGH_CONTRAST_DARK, HIGH_CONTRAST_LIGHT};
pub use super::builtin_garden::{GARDEN_DARK, GARDEN_LIGHT};
pub use super::builtin_sea::{MIDNIGHT, MIST};

/// Dusk, the dark half of the icon pair (the palette most dark view snapshots draw in).
#[cfg(test)]
pub const DARK: Palette = DUSK;
/// Dawn, the light half of the icon pair (the light palette the design sheets draw in, and egui's
/// other style under a dark palette).
pub const LIGHT: Palette = DAWN;
/// The default dark palette: Graphite (`BuiltinTheme::DEFAULT_DARK`). It stands in wherever a
/// theme can't be found.
pub const DEFAULT_DARK: Palette = GRAPHITE;

/// The palette of a built-in theme.
pub fn builtin(t: BuiltinTheme) -> &'static Palette {
    match t {
        BuiltinTheme::Graphite => &GRAPHITE,
        BuiltinTheme::Midnight => &MIDNIGHT,
        BuiltinTheme::Dark => &DUSK,
        BuiltinTheme::HighContrastDark => &HIGH_CONTRAST_DARK,
        BuiltinTheme::Porcelain => &PORCELAIN,
        BuiltinTheme::Mist => &MIST,
        BuiltinTheme::Light => &DAWN,
        BuiltinTheme::HighContrastLight => &HIGH_CONTRAST_LIGHT,
    }
}

/// The Garden palette of the same kind as the standard theme `t` (None for the others): a theme
/// file on Dusk or Dawn that sets its own accent takes the Garden branch families, made for an
/// accent that isn't gold.
pub fn garden_twin(t: BuiltinTheme) -> Option<&'static Palette> {
    match t {
        BuiltinTheme::Dark => Some(&GARDEN_DARK),
        BuiltinTheme::Light => Some(&GARDEN_LIGHT),
        _ => None,
    }
}
