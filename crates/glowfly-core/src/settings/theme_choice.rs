//! The theme chosen in Preferences > Appearance > Theme, as settings.json saves it, and how a
//! retired theme's name loads: as the theme that took its place (`BuiltinTheme::successor`),
//! noted for the one-time toast (`take_retired`).

use super::BuiltinTheme;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

/// The theme in use (Preferences > Appearance > Theme). A theme file is saved as
/// `{"Custom": "ocean"}`. `"Dark"` in older files was "the default dark theme", so it loads as
/// the default dark theme today (Graphite); Dusk saves as `"Dusk"`. `"Light"` and `"System"`
/// load unchanged. A retired theme's name (`"Pine"`, `"GardenDark"`, …) loads as its successor.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(from = "Saved")]
pub enum ThemeChoice {
    /// Dusk, the dark theme from the app icon (saved as `Dusk`; see the enum's note on `Dark`).
    #[serde(rename = "Dusk")]
    Dark,
    /// Dawn (saved as `Light`).
    Light,
    /// Follow the operating system: the default dark or light theme, Graphite or Porcelain.
    System,
    HighContrastDark,
    HighContrastLight,
    /// Midnight and Mist, the sea-glass pair.
    Midnight,
    Mist,
    /// The default dark theme; older files' `"Dark"` loads as it.
    Graphite,
    /// The default light theme, Graphite's twin.
    Porcelain,
    /// A theme file in the themes folder, by file stem ("ocean" for themes/ocean.json).
    Custom(String),
}

/// What a settings file may hold for a theme: today's choices, and the retired themes' names.
#[derive(Deserialize)]
enum Saved {
    #[serde(rename = "Dusk")]
    Dark,
    Light,
    System,
    HighContrastDark,
    HighContrastLight,
    Midnight,
    Mist,
    #[serde(alias = "Dark")]
    Graphite,
    Porcelain,
    Custom(String),
    Ember,
    Pine,
    GardenDark,
    GardenLight,
    Sand,
    Linen,
}

thread_local! {
    /// The first retired theme read since `take_retired` last ran on this thread: the name it was
    /// shown under, and its successor.
    static RETIRED: RefCell<Option<(String, BuiltinTheme)>> = const { RefCell::new(None) };
}

/// The first retired theme a settings read on this thread met since the last call, which
/// clears it.
pub(super) fn take_retired() -> Option<(String, BuiltinTheme)> {
    RETIRED.with_borrow_mut(Option::take)
}

/// Its successor, noted as the retired theme met unless one already is.
fn retired(saved: &str) -> ThemeChoice {
    let (shown, next) = BuiltinTheme::successor(saved).unwrap_or(("", BuiltinTheme::DEFAULT_DARK));
    RETIRED.with_borrow_mut(|r| {
        r.get_or_insert_with(|| (shown.to_owned(), next));
    });
    next.choice()
}

impl From<Saved> for ThemeChoice {
    fn from(s: Saved) -> Self {
        match s {
            Saved::Dark => Self::Dark,
            Saved::Light => Self::Light,
            Saved::System => Self::System,
            Saved::HighContrastDark => Self::HighContrastDark,
            Saved::HighContrastLight => Self::HighContrastLight,
            Saved::Midnight => Self::Midnight,
            Saved::Mist => Self::Mist,
            Saved::Graphite => Self::Graphite,
            Saved::Porcelain => Self::Porcelain,
            Saved::Custom(id) => Self::Custom(id),
            Saved::Ember => retired("Ember"),
            Saved::Pine => retired("Pine"),
            Saved::GardenDark => retired("GardenDark"),
            Saved::GardenLight => retired("GardenLight"),
            Saved::Sand => retired("Sand"),
            Saved::Linen => retired("Linen"),
        }
    }
}
