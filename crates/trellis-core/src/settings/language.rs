//! The UI language setting (Preferences > General): "system" or a language code.

use super::Settings;
use crate::i18n::Lang;

impl Settings {
    /// The language the UI is drawn in: the saved code, or the OS language for "system" and
    /// for a code this version doesn't know (kept as saved until the user picks another).
    pub fn lang(&self) -> Lang {
        Lang::from_setting(&self.language)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{SYSTEM, system};

    #[test]
    fn language_defaults_to_system_and_old_files_load() {
        assert_eq!(Settings::default().language, SYSTEM);
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(old.language, SYSTEM);
        assert_eq!(old.lang(), system());
        let de: Settings = serde_json::from_str(r#"{"language":"de"}"#).unwrap();
        assert_eq!(de.lang(), Lang::De);
        let newer: Settings = serde_json::from_str(r#"{"language":"tlh"}"#).unwrap();
        assert_eq!((newer.language.as_str(), newer.lang()), ("tlh", system()));
    }
}
