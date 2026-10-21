//! The OS's UI language: read once at startup from its ordered list of preferred languages
//! (Windows: the user's preferred UI languages; macOS: the preferred languages; Unix:
//! LANGUAGE, then LC_ALL, LC_MESSAGES and LANG, all through sys-locale), and never again while
//! Glowfly runs. Tests never read it, so "system" is English there.

use super::Lang;
use std::sync::OnceLock;

static SYSTEM_LANG: OnceLock<Lang> = OnceLock::new();

/// The OS language found at startup, or English before (or without) `detect_system_language`.
pub fn system() -> Lang {
    SYSTEM_LANG.get().copied().unwrap_or_default()
}

/// Reads the OS's preferred languages and keeps the first Glowfly has. Called once by the app
/// shell at startup; a second call changes nothing.
pub fn detect_system_language() {
    if SYSTEM_LANG.get().is_none() {
        let found = pick(sys_locale::get_locales());
        let _ = SYSTEM_LANG.set(found);
    }
}

/// The language for an ordered list of locale tags (`de-AT`, `es-419`, `de_DE.UTF-8@euro`): the
/// first whose language Glowfly has. `C` or `POSIX` before any of them, or none at all, is
/// English.
pub fn pick<I, S>(locales: I) -> Lang
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    for tag in locales {
        let tag = tag.as_ref();
        let tag = tag.split(['.', '@']).next().unwrap_or_default();
        let primary = tag.split(['-', '_']).next().unwrap_or_default().to_ascii_lowercase();
        if primary == "c" || primary == "posix" {
            return Lang::En;
        }
        if let Some(lang) = Lang::from_code(&primary) {
            return lang;
        }
    }
    Lang::En
}
