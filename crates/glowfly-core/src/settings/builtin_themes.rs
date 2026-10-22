//! The built-in themes: their ids (a theme file's `base`), names, kinds, and how the settings'
//! `ThemeChoice` maps onto them.

use super::ThemeChoice;

/// A palette that ships with Glowfly; theme files start from one. `Dark` and `Light` are Dusk and
/// Dawn (from the app icon); Graphite is the default dark theme (`DEFAULT_DARK`) and Dawn the
/// default light one. The Garden pair is the earlier green look.
/// The ids `dark` and `light` (and the settings' `Dark` and `Light`) stay the standard pair's, so
/// files written before the pair was renamed open in Dusk and Dawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuiltinTheme {
    /// Dusk.
    Dark,
    /// Dawn.
    Light,
    GardenDark,
    GardenLight,
    HighContrastDark,
    HighContrastLight,
    Sand,
    /// Blue-black night with a sea-glass accent.
    Midnight,
    /// Midnight's light twin: cool fog paper, deep sea-glass.
    Mist,
    /// Oxblood charcoal with a flame-orange accent.
    Ember,
    /// Deep pine green with the lantern gold.
    Pine,
    /// Near-neutral graphite with a cobalt accent.
    Graphite,
    /// Clean warm paper with deep cobalt.
    Linen,
}

impl BuiltinTheme {
    /// The default dark theme: new settings, Match system on a dark desktop, older settings'
    /// `"Dark"` and a theme file that isn't in the folder show it.
    pub const DEFAULT_DARK: Self = Self::Graphite;
    /// The default light theme.
    pub const DEFAULT_LIGHT: Self = Self::Light;

    pub const ALL: [Self; 13] = [
        Self::Dark,
        Self::Light,
        Self::GardenDark,
        Self::GardenLight,
        Self::HighContrastDark,
        Self::HighContrastLight,
        Self::Sand,
        Self::Midnight,
        Self::Mist,
        Self::Ember,
        Self::Pine,
        Self::Graphite,
        Self::Linen,
    ];

    /// The name a theme file's `base` uses.
    pub fn id(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::GardenDark => "garden-dark",
            Self::GardenLight => "garden-light",
            Self::HighContrastDark => "high-contrast-dark",
            Self::HighContrastLight => "high-contrast-light",
            Self::Sand => "sand",
            Self::Midnight => "midnight",
            Self::Mist => "mist",
            Self::Ember => "ember",
            Self::Pine => "pine",
            Self::Graphite => "graphite",
            Self::Linen => "linen",
        }
    }

    /// The theme for a `base` value, spelled exactly as `id` gives it.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dusk",
            Self::Light => "Dawn",
            Self::GardenDark => "Garden Dark",
            Self::GardenLight => "Garden Light",
            Self::HighContrastDark => "High Contrast Dark",
            Self::HighContrastLight => "High Contrast Light",
            Self::Sand => "Sand",
            Self::Midnight => "Midnight",
            Self::Mist => "Mist",
            Self::Ember => "Ember",
            Self::Pine => "Pine",
            Self::Graphite => "Graphite",
            Self::Linen => "Linen",
        }
    }

    /// Light text on a dark background.
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark | Self::GardenDark | Self::HighContrastDark | Self::Midnight | Self::Ember | Self::Pine | Self::Graphite)
    }

    /// The choice that shows this theme.
    pub fn choice(self) -> ThemeChoice {
        match self {
            Self::Dark => ThemeChoice::Dark,
            Self::Light => ThemeChoice::Light,
            Self::GardenDark => ThemeChoice::GardenDark,
            Self::GardenLight => ThemeChoice::GardenLight,
            Self::HighContrastDark => ThemeChoice::HighContrastDark,
            Self::HighContrastLight => ThemeChoice::HighContrastLight,
            Self::Sand => ThemeChoice::Sand,
            Self::Midnight => ThemeChoice::Midnight,
            Self::Mist => ThemeChoice::Mist,
            Self::Ember => ThemeChoice::Ember,
            Self::Pine => ThemeChoice::Pine,
            Self::Graphite => ThemeChoice::Graphite,
            Self::Linen => ThemeChoice::Linen,
        }
    }
}

impl ThemeChoice {
    /// The built-in choices, in the order the menus list them: the default pair (Graphite and
    /// Dawn) and Match system, then the other dark themes, then the other light ones.
    pub const BUILT_IN: [ThemeChoice; 14] = [
        ThemeChoice::Graphite,
        ThemeChoice::Light,
        ThemeChoice::System,
        ThemeChoice::Dark,
        ThemeChoice::Midnight,
        ThemeChoice::Ember,
        ThemeChoice::Pine,
        ThemeChoice::GardenDark,
        ThemeChoice::HighContrastDark,
        ThemeChoice::Mist,
        ThemeChoice::Linen,
        ThemeChoice::Sand,
        ThemeChoice::GardenLight,
        ThemeChoice::HighContrastLight,
    ];

    /// The built-in palette this choice shows (`System` follows `system_dark`); None for a
    /// theme file.
    pub fn builtin(&self, system_dark: bool) -> Option<BuiltinTheme> {
        Some(match self {
            Self::Dark => BuiltinTheme::Dark,
            Self::Light => BuiltinTheme::Light,
            Self::System if system_dark => BuiltinTheme::DEFAULT_DARK,
            Self::System => BuiltinTheme::DEFAULT_LIGHT,
            Self::GardenDark => BuiltinTheme::GardenDark,
            Self::GardenLight => BuiltinTheme::GardenLight,
            Self::HighContrastDark => BuiltinTheme::HighContrastDark,
            Self::HighContrastLight => BuiltinTheme::HighContrastLight,
            Self::Sand => BuiltinTheme::Sand,
            Self::Midnight => BuiltinTheme::Midnight,
            Self::Mist => BuiltinTheme::Mist,
            Self::Ember => BuiltinTheme::Ember,
            Self::Pine => BuiltinTheme::Pine,
            Self::Graphite => BuiltinTheme::Graphite,
            Self::Linen => BuiltinTheme::Linen,
            Self::Custom(_) => return None,
        })
    }

    /// The name Preferences shows; a theme file's id (its name is in the catalog).
    pub fn label(&self) -> String {
        match self {
            Self::System => "Match system".to_owned(),
            Self::Custom(id) => id.clone(),
            other => other.builtin(true).map(BuiltinTheme::label).unwrap_or_default().to_owned(),
        }
    }

    /// The short name the command palette shows after "Theme: ".
    pub fn palette_label(&self) -> String {
        match self {
            Self::Dark => "Dusk",
            Self::Light => "Dawn",
            Self::System => "Follow system",
            Self::GardenDark => "Garden dark",
            Self::GardenLight => "Garden light",
            Self::HighContrastDark => "High contrast dark",
            Self::HighContrastLight => "High contrast light",
            Self::Custom(id) => return id.clone(),
            other => return other.builtin(true).map(BuiltinTheme::label).unwrap_or_default().to_owned(),
        }
        .to_owned()
    }
}
