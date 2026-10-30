//! Spell checking for commit messages through the Windows Spell Checking API (the same
//! dictionaries as the rest of Windows, in the user's language). Other platforms get no-ops.
//! The language order (`system_languages`) is sys-locale's locale on Windows and macOS, and
//! gettext's message order on Linux and other Unix (`gettext_languages`).

use crate::i18n::{LocaleEnv, is_c_locale, message_locales};
use std::collections::HashMap;

/// The OS's spell-check languages, best first; the checker falls back to US English after them.
/// Linux and other Unix: gettext's order for the process environment (`gettext_languages`).
pub fn system_languages() -> Vec<String> {
    #[cfg(all(unix, not(target_os = "macos")))]
    return without_c(crate::i18n::env_message_locales());
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    sys_locale::get_locale().into_iter().collect()
}

/// The spell-check languages gettext's message order gives: LANGUAGE's entries (only when the
/// locale isn't C or POSIX), then the first non-empty of LC_ALL, LC_MESSAGES and LANG, each
/// without its `.codeset` or `@modifier` ("de_DE"). A C or POSIX locale gives none.
pub fn gettext_languages(env: LocaleEnv<'_>) -> Vec<String> {
    without_c(message_locales(env))
}

fn without_c(locales: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in locales.iter().filter(|l| !is_c_locale(l)) {
        let base = l.split(['.', '@']).next().unwrap_or_default();
        if !base.is_empty() && !out.iter().any(|o| o == base) {
            out.push(base.to_owned());
        }
    }
    out
}

/// A misspelling: byte range in the checked text.
#[derive(Clone, Debug, PartialEq)]
pub struct Miss {
    pub start: usize,
    pub len: usize,
}

pub struct Speller {
    #[cfg(windows)]
    checker: Option<windows::Win32::Globalization::ISpellChecker>,
    cache: HashMap<String, Vec<Miss>>,
}

impl Speller {
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            checker: win::create(),
            cache: HashMap::new(),
        }
    }

    pub fn available(&self) -> bool {
        #[cfg(windows)]
        return self.checker.is_some();
        #[cfg(not(windows))]
        false
    }

    /// Misspellings in `text`, cached per distinct text.
    pub fn check(&mut self, text: &str) -> Vec<Miss> {
        if text.trim().is_empty() {
            return Vec::new();
        }
        if let Some(m) = self.cache.get(text) {
            return m.clone();
        }
        #[cfg(windows)]
        let misses = self.checker.as_ref().map(|c| win::check(c, text)).unwrap_or_default();
        #[cfg(not(windows))]
        let misses = Vec::new();
        if self.cache.len() > 256 {
            self.cache.clear();
        }
        self.cache.insert(text.to_owned(), misses.clone());
        misses
    }

    pub fn suggest(&self, word: &str) -> Vec<String> {
        #[cfg(windows)]
        return self.checker.as_ref().map(|c| win::suggest(c, word)).unwrap_or_default();
        #[cfg(not(windows))]
        {
            let _ = word;
            Vec::new()
        }
    }

    pub fn add_to_dictionary(&mut self, word: &str) {
        #[cfg(windows)]
        if let Some(c) = &self.checker {
            win::add(c, word);
        }
        #[cfg(not(windows))]
        let _ = word;
        self.cache.clear();
    }
}

impl Default for Speller {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
mod win {
    use super::Miss;
    use windows::Win32::Globalization::{ISpellChecker, ISpellCheckerFactory, SpellCheckerFactory};
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree};
    use windows::core::{HSTRING, PWSTR};

    pub fn create() -> Option<ISpellChecker> {
        unsafe {
            // The UI thread may already be initialized for OLE; that's fine.
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let factory: ISpellCheckerFactory = CoCreateInstance(&SpellCheckerFactory, None, CLSCTX_INPROC_SERVER).ok()?;
            let langs = super::system_languages();
            for lang in langs.iter().map(String::as_str).chain(["en-US"]) {
                let tag = HSTRING::from(lang);
                if factory.IsSupported(&tag).map(|b| b.as_bool()).unwrap_or(false)
                    && let Ok(c) = factory.CreateSpellChecker(&tag)
                {
                    return Some(c);
                }
            }
            None
        }
    }

    /// Maps a UTF-16 offset to a byte offset in `text`.
    fn byte_offset(text: &str, utf16: usize) -> usize {
        let mut units = 0;
        for (i, ch) in text.char_indices() {
            if units >= utf16 {
                return i;
            }
            units += ch.len_utf16();
        }
        text.len()
    }

    pub fn check(c: &ISpellChecker, text: &str) -> Vec<Miss> {
        let mut out = Vec::new();
        unsafe {
            let Ok(errors) = c.Check(&HSTRING::from(text)) else { return out };
            loop {
                let mut next = None;
                // S_FALSE (1) marks the end of the list.
                if errors.Next(&mut next).0 != 0 {
                    break;
                }
                let Some(e) = next else { break };
                let (Ok(start), Ok(len)) = (e.StartIndex(), e.Length()) else { break };
                let s = byte_offset(text, start as usize);
                let end = byte_offset(text, (start + len) as usize);
                out.push(Miss { start: s, len: end - s });
            }
        }
        out
    }

    pub fn suggest(c: &ISpellChecker, word: &str) -> Vec<String> {
        let mut out = Vec::new();
        unsafe {
            let Ok(list) = c.Suggest(&HSTRING::from(word)) else { return out };
            loop {
                let mut item = [PWSTR::null()];
                let mut fetched = 0u32;
                if list.Next(&mut item, Some(&mut fetched)).is_err() || fetched == 0 {
                    break;
                }
                if let Ok(s) = item[0].to_string() {
                    out.push(s);
                }
                CoTaskMemFree(Some(item[0].0 as *const _));
                if out.len() >= 8 {
                    break;
                }
            }
        }
        out
    }

    pub fn add(c: &ISpellChecker, word: &str) {
        unsafe {
            let _ = c.Add(&HSTRING::from(word));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::gettext_languages;
    use crate::i18n::LocaleEnv;

    fn env<'a>(lc_all: Option<&'a str>, lc_messages: Option<&'a str>, lang: Option<&'a str>, language: Option<&'a str>) -> LocaleEnv<'a> {
        LocaleEnv { lc_all, lc_messages, lang, language }
    }

    #[test]
    fn spell_languages_follow_gettext_order() {
        assert_eq!(gettext_languages(env(None, None, Some("de_DE.UTF-8"), Some("es:fr"))), ["es", "fr", "de_DE"]);
        assert_eq!(gettext_languages(env(Some("de_AT.UTF-8"), Some("fr_FR"), Some("es_ES"), None)), ["de_AT"]);
        assert_eq!(gettext_languages(env(Some(""), Some("fr_FR@euro"), Some("es_ES"), None)), ["fr_FR"]);
        assert_eq!(gettext_languages(env(None, None, Some("en_GB.UTF-8"), Some("en_GB:de"))), ["en_GB", "de"]);
    }

    #[test]
    fn a_c_locale_ignores_language_and_gives_no_spell_language() {
        assert!(gettext_languages(env(None, None, Some("C.UTF-8"), Some("de"))).is_empty());
        assert!(gettext_languages(env(Some("POSIX"), None, Some("de_DE"), Some("de"))).is_empty());
        assert!(gettext_languages(env(None, None, None, Some("de"))).is_empty());
        assert!(gettext_languages(LocaleEnv::default()).is_empty());
        // A C entry inside LANGUAGE is skipped, not a stop.
        assert_eq!(gettext_languages(env(None, None, Some("de_DE"), Some("C:es"))), ["es", "de_DE"]);
    }
}
