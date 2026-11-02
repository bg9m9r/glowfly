//! The pair Match the system switches between (Preferences > Appearance > Theme): a dark and a
//! light built-in theme, saved as their ids, and what clicking a card or the check does to it
//! and to the theme in use.

use super::{BuiltinTheme, Settings, ThemeChoice};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Value, json};

/// The themes Match the system shows (`Settings::system_pair`, per profile): `dark` on a dark
/// desktop, `light` on a light one. Saved as `{"dark": "midnight", "light": "mist"}`. Loading
/// never fails: a member that isn't a theme id of its kind (`BuiltinTheme::from_id`, so older
/// and retired ids map silently) falls back to its default, Graphite or Porcelain.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SystemPair {
    pub dark: BuiltinTheme,
    pub light: BuiltinTheme,
}

impl Default for SystemPair {
    fn default() -> Self {
        Self { dark: BuiltinTheme::DEFAULT_DARK, light: BuiltinTheme::DEFAULT_LIGHT }
    }
}

impl SystemPair {
    /// The member Match the system shows on a dark (`os_dark`) or light desktop.
    pub fn pick(self, os_dark: bool) -> BuiltinTheme {
        if os_dark { self.dark } else { self.light }
    }

    /// `b` and its twin, each in its own slot.
    pub fn of_twins(b: BuiltinTheme) -> Self {
        let (dark, light) = if b.is_dark() { (b, b.twin()) } else { (b.twin(), b) };
        Self { dark, light }
    }
}

impl BuiltinTheme {
    /// The theme in the same place in the other kind: Graphite and Porcelain, Midnight and Mist,
    /// Dusk and Dawn, and the high-contrast pair.
    pub fn twin(self) -> Self {
        let i = Self::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Self::ALL[(i + 4) % Self::ALL.len()]
    }
}

impl Settings {
    /// A click on theme card `b`: with Match the system on, `b` becomes the pair's member of its
    /// kind and the theme stays System; off, `b` becomes the theme.
    pub fn pick_card(&mut self, b: BuiltinTheme) {
        match (&self.theme, b.is_dark()) {
            (ThemeChoice::System, true) => self.system_pair.dark = b,
            (ThemeChoice::System, false) => self.system_pair.light = b,
            _ => self.theme = b.choice(),
        }
    }

    /// The Match the system check turned on or off on a dark (`os_dark`) or light desktop.
    /// On, a built-in theme on screen and its twin become the pair (a theme file leaves the pair
    /// as saved). Off, the member on screen becomes the theme, so nothing on screen changes.
    pub fn set_match_system(&mut self, on: bool, os_dark: bool) {
        if on {
            if let Some(b) = self.theme.builtin(os_dark, self.system_pair) {
                self.system_pair = SystemPair::of_twins(b);
            }
            self.theme = ThemeChoice::System;
        } else if self.theme == ThemeChoice::System {
            self.theme = self.system_pair.pick(os_dark).choice();
        }
    }
}

impl Serialize for SystemPair {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        json!({ "dark": self.dark.id(), "light": self.light.id() }).serialize(s)
    }
}

impl<'de> Deserialize<'de> for SystemPair {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        let member = |key: &str, dark: bool, fallback: BuiltinTheme| {
            v.get(key).and_then(Value::as_str).and_then(BuiltinTheme::from_id).filter(|b| b.is_dark() == dark).unwrap_or(fallback)
        };
        let def = Self::default();
        Ok(Self { dark: member("dark", true, def.dark), light: member("light", false, def.light) })
    }
}

#[cfg(test)]
#[path = "theme_pair_tests.rs"]
mod tests;
