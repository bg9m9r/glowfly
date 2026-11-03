//! The settings Preferences > Accessibility owns besides the ones it moved there (reduce motion,
//! the branch palette): the text cursor's blink, how long notifications stay, the tooltip
//! delay, and whether the system's text size, high contrast and reduced motion are followed
//! besides the choices made here. They sit flat in the settings file (`Settings::a11y` is flattened) and stay the same
//! whatever profile is current. The High contrast check's rules are here too; the theme it
//! brings back (`Settings::theme_before_contrast`) belongs to the profile, as `theme` does.

use super::{BuiltinTheme, Settings, ThemeChoice};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Preferences > Accessibility's own fields, flattened into `Settings`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct A11yPrefs {
    /// The text cursor in text fields blinks (the terminal's never does).
    pub cursor_blink: bool,
    /// How long a notification stays up.
    pub toast_time: ToastTime,
    /// How long the pointer rests before a tooltip shows.
    pub tooltip_delay: TooltipDelay,
    /// The system's text size scales the interface size too.
    pub follow_system_text_size: bool,
    /// A high-contrast theme shows while the system uses high contrast.
    pub follow_system_contrast: bool,
    /// Motion is reduced while the system asks for less.
    pub follow_system_motion: bool,
}

impl Default for A11yPrefs {
    fn default() -> Self {
        Self {
            cursor_blink: true,
            toast_time: ToastTime::default(),
            tooltip_delay: TooltipDelay::default(),
            follow_system_text_size: true,
            follow_system_contrast: true,
            follow_system_motion: true,
        }
    }
}

/// How long a notification stays up (Preferences > Accessibility > Notifications stay).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToastTime {
    /// 5 s, or 10 s with a button.
    #[default]
    Standard,
    /// 15 s, or 30 s with a button.
    Longer,
    /// Until it is closed.
    UntilClosed,
}

impl ToastTime {
    /// How long a toast stays, with a button (`action`) or without; None never expires.
    pub fn lifetime(self, action: bool) -> Option<Duration> {
        let secs = match (self, action) {
            (Self::Standard, false) => 5,
            (Self::Standard, true) => 10,
            (Self::Longer, false) => 15,
            (Self::Longer, true) => 30,
            (Self::UntilClosed, _) => return None,
        };
        Some(Duration::from_secs(secs))
    }
}

/// How long the pointer rests before a tooltip shows (Preferences > Accessibility > Tooltip delay).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TooltipDelay {
    /// 0.4 s.
    #[default]
    Quick,
    /// 1 s.
    Slow,
    /// At once.
    Instant,
}

impl TooltipDelay {
    /// The delay in seconds.
    pub fn seconds(self) -> f32 {
        match self {
            Self::Quick => 0.4,
            Self::Slow => 1.0,
            Self::Instant => 0.0,
        }
    }
}

impl ThemeChoice {
    /// One of the high-contrast themes.
    pub fn is_high_contrast(&self) -> bool {
        matches!(self, Self::HighContrastDark | Self::HighContrastLight)
    }
}

impl Settings {
    /// Turns High contrast on (saving the theme in use and showing the high-contrast theme as
    /// dark as the one in effect, `dark`) or off (bringing back the saved theme or, with none
    /// saved, the default theme as dark as the high-contrast one). Returns whether the theme
    /// changed.
    pub fn set_high_contrast(&mut self, on: bool, dark: bool) -> bool {
        if on == self.theme.is_high_contrast() {
            return false;
        }
        if on {
            let hc = if dark { BuiltinTheme::HighContrastDark } else { BuiltinTheme::HighContrastLight };
            let before = std::mem::replace(&mut self.theme, hc.choice());
            self.theme_before_contrast = Some(before);
        } else {
            let house = if self.theme == ThemeChoice::HighContrastDark { BuiltinTheme::DEFAULT_DARK } else { BuiltinTheme::DEFAULT_LIGHT };
            self.theme = self.theme_before_contrast.take().unwrap_or(house.choice());
        }
        true
    }

    /// Forgets the theme High contrast would bring back once another theme than a
    /// high-contrast one is picked (only a pick calls this: a profile switch or any other save
    /// keeps it). Returns whether it forgot one.
    pub fn settle_contrast(&mut self) -> bool {
        !self.theme.is_high_contrast() && self.theme_before_contrast.take().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_files_load_the_defaults_and_new_ones_round_trip() {
        let old: Settings = serde_json::from_str(r#"{"theme": "Mist"}"#).unwrap();
        assert_eq!(old.a11y, A11yPrefs::default());
        assert!(old.a11y.cursor_blink);
        assert!(old.a11y.follow_system_text_size && old.a11y.follow_system_contrast && old.a11y.follow_system_motion);
        let a11y = A11yPrefs {
            cursor_blink: false,
            toast_time: ToastTime::Longer,
            tooltip_delay: TooltipDelay::Instant,
            follow_system_text_size: false,
            follow_system_contrast: false,
            follow_system_motion: false,
        };
        let s = Settings { a11y, theme_before_contrast: Some(ThemeChoice::Custom("ocean".into())), ..Settings::default() };
        let text = serde_json::to_string(&s).unwrap();
        // Flat in the file, beside the other settings.
        assert!(text.contains(r#""toast_time":"Longer""#), "{text}");
        let back: Settings = serde_json::from_str(&text).unwrap();
        assert_eq!((back.a11y, back.theme_before_contrast), (s.a11y, s.theme_before_contrast));
    }

    #[test]
    fn lifetimes_and_delays() {
        let secs = |s| Some(Duration::from_secs(s));
        assert_eq!((ToastTime::Standard.lifetime(false), ToastTime::Standard.lifetime(true)), (secs(5), secs(10)));
        assert_eq!((ToastTime::Longer.lifetime(false), ToastTime::Longer.lifetime(true)), (secs(15), secs(30)));
        assert_eq!(ToastTime::UntilClosed.lifetime(true), None);
        assert_eq!([TooltipDelay::Quick, TooltipDelay::Slow, TooltipDelay::Instant].map(TooltipDelay::seconds), [0.4, 1.0, 0.0]);
    }

    #[test]
    fn high_contrast_saves_and_brings_back_the_theme() {
        let mut s = Settings { theme: ThemeChoice::Mist, ..Settings::default() };
        assert!(s.set_high_contrast(true, false));
        assert_eq!((s.theme.clone(), s.theme_before_contrast.clone()), (ThemeChoice::HighContrastLight, Some(ThemeChoice::Mist)));
        assert!(!s.set_high_contrast(true, true), "already on");
        assert!(s.set_high_contrast(false, false));
        assert_eq!((s.theme.clone(), s.theme_before_contrast.clone()), (ThemeChoice::Mist, None));
        // Nothing saved: the default theme as dark as the high-contrast one.
        for (hc, house) in
            [(ThemeChoice::HighContrastDark, ThemeChoice::Graphite), (ThemeChoice::HighContrastLight, ThemeChoice::Porcelain)]
        {
            let mut s = Settings { theme: hc, ..Settings::default() };
            assert!(s.set_high_contrast(false, true));
            assert_eq!(s.theme, house);
        }
    }

    #[test]
    fn picking_another_theme_forgets_the_saved_one() {
        let mut s = Settings { theme: ThemeChoice::Custom("ocean".into()), ..Settings::default() };
        s.set_high_contrast(true, true);
        assert!(!s.settle_contrast(), "still high contrast");
        s.theme = ThemeChoice::Porcelain;
        assert!(s.settle_contrast());
        assert_eq!(s.theme_before_contrast, None);
        assert!(!s.settle_contrast());
    }
}
