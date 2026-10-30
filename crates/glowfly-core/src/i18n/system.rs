//! The OS's UI language: read once at startup from its ordered list of preferred languages, and
//! never again while Glowfly runs. Windows: the user's preferred UI languages; macOS: the
//! preferred languages, both through sys-locale. Linux and other Unix follow gettext
//! (`message_locales`): LANGUAGE's list, then the first non-empty of LC_ALL, LC_MESSAGES and
//! LANG, with LANGUAGE ignored when that locale is C or POSIX (or unset, which is C). Tests never
//! read it, so "system" is English there.

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
        let _ = SYSTEM_LANG.set(pick(os_locales()));
    }
}

/// Linux and other Unix: gettext's order, read from the environment.
#[cfg(all(unix, not(target_os = "macos")))]
fn os_locales() -> Vec<String> {
    env_message_locales()
}

/// Linux and other Unix: `message_locales` for the process environment (also the spell-check
/// language's order, `spell::system_languages`).
#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn env_message_locales() -> Vec<String> {
    let var = |name: &str| std::env::var(name).ok();
    let (lc_all, lc_messages, lang, language) = (var("LC_ALL"), var("LC_MESSAGES"), var("LANG"), var("LANGUAGE"));
    message_locales(LocaleEnv {
        lc_all: lc_all.as_deref(),
        lc_messages: lc_messages.as_deref(),
        lang: lang.as_deref(),
        language: language.as_deref(),
    })
}

/// Windows and macOS: the OS's preferred languages through sys-locale.
#[cfg(not(all(unix, not(target_os = "macos"))))]
fn os_locales() -> Vec<String> {
    sys_locale::get_locales().collect()
}

/// The locale variables gettext reads for messages, as found in the environment (None: unset).
#[derive(Clone, Copy, Debug, Default)]
pub struct LocaleEnv<'a> {
    pub lc_all: Option<&'a str>,
    pub lc_messages: Option<&'a str>,
    pub lang: Option<&'a str>,
    pub language: Option<&'a str>,
}

/// The ordered locale list gettext would try for messages: LANGUAGE's entries, then the
/// effective locale (the first non-empty of LC_ALL, LC_MESSAGES, LANG). When that locale is C or
/// POSIX (or none is set, which is C), LANGUAGE is ignored and the list is just the C locale.
pub fn message_locales(env: LocaleEnv<'_>) -> Vec<String> {
    let effective = [env.lc_all, env.lc_messages, env.lang].into_iter().flatten().find(|v| !v.is_empty()).unwrap_or("C");
    if is_c_locale(effective) {
        return vec![effective.to_owned()];
    }
    let mut out: Vec<String> = env.language.unwrap_or_default().split(':').filter(|p| !p.is_empty()).map(str::to_owned).collect();
    out.push(effective.to_owned());
    out
}

/// Whether a locale name is C or POSIX, with any `.codeset` or `@modifier` (`C.UTF-8`, `C.utf8`).
pub(crate) fn is_c_locale(locale: &str) -> bool {
    let base = locale.split(['.', '@']).next().unwrap_or_default();
    base.eq_ignore_ascii_case("c") || base.eq_ignore_ascii_case("posix")
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
