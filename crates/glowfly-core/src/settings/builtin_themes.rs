//! The built-in themes: their ids (a theme file's `base`), names, kinds, the themes that took
//! the place of the retired ones, and how the settings' `ThemeChoice` maps onto them.

use super::ThemeChoice;

/// A palette that ships with Glowfly; theme files start from one. Eight, in four twin pairs:
/// Graphite and Porcelain (the defaults), Midnight and Mist, Dusk and Dawn (from the app icon),
/// and the high-contrast pair. `Dark` and `Light` are Dusk and Dawn: the settings' `Dark` and
/// `Light` and the ids `dark` and `light` stay theirs, so files written before the pair was
/// renamed open in Dusk and Dawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuiltinTheme {
    /// Near-neutral graphite with a cobalt accent.
    Graphite,
    /// Blue-black night with a sea-glass accent.
    Midnight,
    /// Dusk.
    Dark,
    HighContrastDark,
    /// Graphite's light twin: neutral white, grey-black ink and cobalt.
    Porcelain,
    /// Midnight's light twin: cool fog paper, deep sea-glass.
    Mist,
    /// Dawn.
    Light,
    HighContrastLight,
}

/// The themes no longer included, by the name settings saved them under: their name as it was
/// shown, and the theme that took their place.
const RETIRED: [(&str, &str, BuiltinTheme); 6] = [
    ("Pine", "Pine", BuiltinTheme::Dark),
    ("GardenDark", "Garden Dark", BuiltinTheme::Midnight),
    ("GardenLight", "Garden Light", BuiltinTheme::Mist),
    ("Ember", "Ember", BuiltinTheme::Graphite),
    ("Sand", "Sand", BuiltinTheme::Porcelain),
    ("Linen", "Linen", BuiltinTheme::Porcelain),
];

/// The retired themes' ids as theme files' `base` named them, and the theme each opens in now.
const RETIRED_IDS: [(&str, BuiltinTheme); 6] = [
    ("ember", BuiltinTheme::Graphite),
    ("pine", BuiltinTheme::Dark),
    ("garden-dark", BuiltinTheme::Midnight),
    ("garden-light", BuiltinTheme::Mist),
    ("sand", BuiltinTheme::Porcelain),
    ("linen", BuiltinTheme::Porcelain),
];

impl BuiltinTheme {
    /// The default dark theme: new settings, Match system on a dark desktop, older settings'
    /// `"Dark"` and a theme file that isn't in the folder show it.
    pub const DEFAULT_DARK: Self = Self::Graphite;
    /// The default light theme: Match system on a light desktop.
    pub const DEFAULT_LIGHT: Self = Self::Porcelain;

    /// The dark themes, then their light twins in the same order.
    pub const ALL: [Self; 8] = [
        Self::Graphite,
        Self::Midnight,
        Self::Dark,
        Self::HighContrastDark,
        Self::Porcelain,
        Self::Mist,
        Self::Light,
        Self::HighContrastLight,
    ];

    /// The name a theme file's `base` uses; Create from current writes it.
    pub fn id(self) -> &'static str {
        match self {
            Self::Graphite => "graphite",
            Self::Midnight => "midnight",
            Self::Dark => "dusk",
            Self::HighContrastDark => "high-contrast-dark",
            Self::Porcelain => "porcelain",
            Self::Mist => "mist",
            Self::Light => "dawn",
            Self::HighContrastLight => "high-contrast-light",
        }
    }

    /// The theme for a `base` value, spelled exactly as `id` gives it, or as an older file did:
    /// `dark` and `light` for Dusk and Dawn, and a retired theme's id for its successor.
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            _ => Self::ALL.into_iter().find(|t| t.id() == id).or_else(|| RETIRED_IDS.iter().find(|(old, _)| *old == id).map(|(_, t)| *t)),
        }
    }

    /// The theme that took the place of a retired one, by the name settings saved it under
    /// (`"GardenDark"`), with the name it was shown under (`"Garden Dark"`). None for any other
    /// name.
    pub fn successor(saved: &str) -> Option<(&'static str, Self)> {
        RETIRED.iter().find(|(name, _, _)| *name == saved).map(|(_, shown, t)| (*shown, *t))
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Graphite => "Graphite",
            Self::Midnight => "Midnight",
            Self::Dark => "Dusk",
            Self::HighContrastDark => "High Contrast Dark",
            Self::Porcelain => "Porcelain",
            Self::Mist => "Mist",
            Self::Light => "Dawn",
            Self::HighContrastLight => "High Contrast Light",
        }
    }

    /// Light text on a dark background.
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Graphite | Self::Midnight | Self::Dark | Self::HighContrastDark)
    }

    /// The choice that shows this theme.
    pub fn choice(self) -> ThemeChoice {
        match self {
            Self::Graphite => ThemeChoice::Graphite,
            Self::Midnight => ThemeChoice::Midnight,
            Self::Dark => ThemeChoice::Dark,
            Self::HighContrastDark => ThemeChoice::HighContrastDark,
            Self::Porcelain => ThemeChoice::Porcelain,
            Self::Mist => ThemeChoice::Mist,
            Self::Light => ThemeChoice::Light,
            Self::HighContrastLight => ThemeChoice::HighContrastLight,
        }
    }
}

impl ThemeChoice {
    /// The built-in choices, in the order the menus list them: the default pair (Graphite and
    /// Porcelain) and Match system, then the other dark themes, then their light twins.
    pub const BUILT_IN: [ThemeChoice; 9] = [
        ThemeChoice::Graphite,
        ThemeChoice::Porcelain,
        ThemeChoice::System,
        ThemeChoice::Midnight,
        ThemeChoice::Dark,
        ThemeChoice::HighContrastDark,
        ThemeChoice::Mist,
        ThemeChoice::Light,
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
            Self::HighContrastDark => BuiltinTheme::HighContrastDark,
            Self::HighContrastLight => BuiltinTheme::HighContrastLight,
            Self::Midnight => BuiltinTheme::Midnight,
            Self::Mist => BuiltinTheme::Mist,
            Self::Graphite => BuiltinTheme::Graphite,
            Self::Porcelain => BuiltinTheme::Porcelain,
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
            Self::HighContrastDark => "High contrast dark",
            Self::HighContrastLight => "High contrast light",
            Self::Custom(id) => return id.clone(),
            other => return other.builtin(true).map(BuiltinTheme::label).unwrap_or_default().to_owned(),
        }
        .to_owned()
    }
}
