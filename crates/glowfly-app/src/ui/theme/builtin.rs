//! The built-in palettes, by theme: Dusk and Dawn (the standard pair, `builtin_dusk.rs`), the
//! Garden pair (`builtin_garden.rs`), the high-contrast and Sand palettes (`builtin_extra.rs`),
//! Midnight and Mist (`builtin_sea.rs`), Ember and Pine (`builtin_warm.rs`), and Graphite and
//! Linen (`builtin_cobalt.rs`). The values and the rules they meet are in docs/COLOUR.md.

use super::color::Palette;
use glowfly_core::settings::BuiltinTheme;

pub use super::builtin_cobalt::{GRAPHITE, LINEN};
pub use super::builtin_dusk::{DAWN, DUSK};
pub use super::builtin_extra::{HIGH_CONTRAST_DARK, HIGH_CONTRAST_LIGHT, SAND};
pub use super::builtin_garden::{GARDEN_DARK, GARDEN_LIGHT};
pub use super::builtin_sea::{MIDNIGHT, MIST};
pub use super::builtin_warm::{EMBER, PINE};

/// Dusk, the dark half of the icon pair (the palette most dark view snapshots draw in).
#[cfg(test)]
pub const DARK: Palette = DUSK;
/// Dawn, the default light palette.
pub const LIGHT: Palette = DAWN;
/// The default dark palette: Graphite (`BuiltinTheme::DEFAULT_DARK`). It stands in wherever a
/// theme can't be found.
pub const DEFAULT_DARK: Palette = GRAPHITE;

/// The palette of a built-in theme.
pub fn builtin(t: BuiltinTheme) -> &'static Palette {
    match t {
        BuiltinTheme::Dark => &DUSK,
        BuiltinTheme::Light => &DAWN,
        BuiltinTheme::GardenDark => &GARDEN_DARK,
        BuiltinTheme::GardenLight => &GARDEN_LIGHT,
        BuiltinTheme::HighContrastDark => &HIGH_CONTRAST_DARK,
        BuiltinTheme::HighContrastLight => &HIGH_CONTRAST_LIGHT,
        BuiltinTheme::Sand => &SAND,
        BuiltinTheme::Midnight => &MIDNIGHT,
        BuiltinTheme::Mist => &MIST,
        BuiltinTheme::Ember => &EMBER,
        BuiltinTheme::Pine => &PINE,
        BuiltinTheme::Graphite => &GRAPHITE,
        BuiltinTheme::Linen => &LINEN,
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
