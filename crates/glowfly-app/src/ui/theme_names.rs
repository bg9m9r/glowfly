//! The themes' names as the View > Theme menu and Preferences' Theme row show them, in the
//! language. Theme files keep their own names; the starter file and the palette use the English
//! labels from glowfly-core.

use glowfly_core::i18n::Lang;
use glowfly_core::settings::{BuiltinTheme, ThemeChoice, ThemeFile};

/// A built-in theme's name.
pub(in crate::ui) fn builtin(t: Lang, b: BuiltinTheme) -> &'static str {
    match b {
        BuiltinTheme::Dark => t.tr("prefs.ui.theme.name.dark"),
        BuiltinTheme::Light => t.tr("prefs.ui.theme.name.light"),
        BuiltinTheme::HighContrastDark => t.tr("prefs.ui.theme.name.high_contrast_dark"),
        BuiltinTheme::HighContrastLight => t.tr("prefs.ui.theme.name.high_contrast_light"),
        BuiltinTheme::Sand => t.tr("prefs.ui.theme.name.sand"),
    }
}

/// A choice's name: Match system, a built-in's name or a theme file's id.
pub(in crate::ui) fn choice(t: Lang, c: &ThemeChoice) -> String {
    match c {
        ThemeChoice::System => t.tr("prefs.ui.theme.name.system").to_owned(),
        ThemeChoice::Custom(id) => id.clone(),
        other => other.builtin(true).map(|b| builtin(t, b)).unwrap_or_default().to_owned(),
    }
}

/// A theme file's entry: its name, marked when it has problems.
pub(in crate::ui) fn file_entry(t: Lang, f: &ThemeFile) -> String {
    if f.problems.is_empty() { f.name.clone() } else { t.trf("prefs.ui.theme.has_problems", &[("name", &f.name)]) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_names_match_the_core_labels() {
        for c in ThemeChoice::BUILT_IN {
            assert_eq!(choice(Lang::En, &c), c.label());
        }
        assert_ne!(choice(Lang::De, &ThemeChoice::Dark), ThemeChoice::Dark.label());
    }
}
