//! The theme chosen in Preferences > UI Customization > Theme, as settings.json saves it.

use serde::{Deserialize, Serialize};

/// The theme in use (Preferences > UI Customization > Theme). A theme file is saved as
/// `{"Custom": "ocean"}`. `"Dark"` in older files was "the default dark theme", so it loads as
/// the default dark theme today (Graphite); Dusk saves as `"Dusk"`. `"Light"` and `"System"`
/// load unchanged.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ThemeChoice {
    /// Dusk, the dark theme from the app icon (saved as `Dusk`; see the enum's note on `Dark`).
    #[serde(rename = "Dusk")]
    Dark,
    /// Dawn, the default light theme (saved as `Light`).
    Light,
    /// Follow the operating system: the default dark or light theme, Graphite or Dawn.
    System,
    /// The earlier green pair.
    GardenDark,
    GardenLight,
    HighContrastDark,
    HighContrastLight,
    Sand,
    /// The themes added beside the standard pair: Midnight and Mist (sea glass), Ember (flame),
    /// Pine (lantern gold), Graphite and Linen (cobalt).
    Midnight,
    Mist,
    Ember,
    Pine,
    /// The default dark theme; older files' `"Dark"` loads as it.
    #[serde(alias = "Dark")]
    Graphite,
    Linen,
    /// A theme file in the themes folder, by file stem ("ocean" for themes/ocean.json).
    Custom(String),
}
