//! The bundled catalogs: one flat JSON object of key → text per language, compiled in and
//! parsed on first use. A test checks that every file parses.

use super::Lang;
use std::collections::HashMap;
use std::sync::OnceLock;

const EN: &str = include_str!("../../locales/en.json");
const DE: &str = include_str!("../../locales/de.json");
const ES: &str = include_str!("../../locales/es.json");

static EN_TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
static DE_TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
static ES_TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();

/// `lang`'s texts by key.
pub(super) fn table(lang: Lang) -> &'static HashMap<String, String> {
    let (cell, src) = match lang {
        Lang::En => (&EN_TABLE, EN),
        Lang::De => (&DE_TABLE, DE),
        Lang::Es => (&ES_TABLE, ES),
    };
    cell.get_or_init(|| parse(src))
}

/// A catalog file's texts, or none when it doesn't parse (the tests make sure it does).
pub(super) fn parse(src: &str) -> HashMap<String, String> {
    serde_json::from_str::<HashMap<String, String>>(src).unwrap_or_default()
}

/// A language's file as it is compiled in, for the tests.
#[cfg(test)]
pub(super) fn source(lang: Lang) -> &'static str {
    match lang {
        Lang::En => EN,
        Lang::De => DE,
        Lang::Es => ES,
    }
}
