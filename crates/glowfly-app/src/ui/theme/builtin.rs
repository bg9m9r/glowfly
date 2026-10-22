//! The built-in palettes, by theme: Dusk and Dawn (the standard pair, `builtin_dusk.rs`), the
//! Garden pair (`builtin_garden.rs`), and the high-contrast and Sand palettes
//! (`builtin_extra.rs`). The values and the rules they meet are in docs/COLOUR.md.

use super::color::Palette;
use glowfly_core::settings::BuiltinTheme;

pub use super::builtin_dusk::{DAWN, DUSK};
pub use super::builtin_extra::{HIGH_CONTRAST_DARK, HIGH_CONTRAST_LIGHT, SAND};
pub use super::builtin_garden::{GARDEN_DARK, GARDEN_LIGHT};

/// The standard dark palette: Dusk. It stands in wherever a theme can't be found.
pub const DARK: Palette = DUSK;
/// The standard light palette: Dawn.
pub const LIGHT: Palette = DAWN;

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
