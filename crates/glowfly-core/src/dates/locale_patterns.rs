//! The date patterns a blank Preferences field stands for: each language's usual numeric
//! order and separators, keyed by the date locale, and ISO 8601 for anything else. The month
//! and weekday words come from `dates::locale`; this table only decides the shapes.

use std::sync::OnceLock;

/// Which of the four Preferences patterns: Date, Long date, Date and time, Full date and time.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DateKind {
    Date,
    LongDate,
    DateTime,
    Full,
}

struct Patterns {
    date: &'static str,
    long: &'static str,
    date_time: &'static str,
    full: &'static str,
}

impl Patterns {
    const fn new(date: &'static str, long: &'static str, date_time: &'static str, full: &'static str) -> Self {
        Self { date, long, date_time, full }
    }

    fn get(&self, kind: DateKind) -> &'static str {
        match kind {
            DateKind::Date => self.date,
            DateKind::LongDate => self.long,
            DateKind::DateTime => self.date_time,
            DateKind::Full => self.full,
        }
    }
}

/// The shapes of git's `--date=short` and `--date=iso`, for C, POSIX, no locale or a language
/// the table doesn't know.
const ISO: Patterns = Patterns::new("YYYY-MM-DD", "YYYY-MM-DD", "YYYY-MM-DD HH:mm", "YYYY-MM-DD HH:mm:ss");

/// Glowfly's own choice of each language's usual order and separators, keyed "lang_REGION" (a
/// regional override, looked up first) or "lang" (every other region of that language).
/// Japanese, Chinese and Korean use digits only: the bundled fonts have no CJK or Hangul glyphs.
const TABLE: &[(&str, Patterns)] = &[
    ("en_US", Patterns::new("MM/DD/YYYY", "MMMM D, YYYY", "MM/DD/YYYY h:mm A", "MMMM D, YYYY h:mm A")),
    ("en", Patterns::new("DD/MM/YYYY", "D MMMM YYYY", "DD/MM/YYYY HH:mm", "D MMMM YYYY HH:mm")),
    ("de", Patterns::new("DD.MM.YYYY", "D. MMMM YYYY", "DD.MM.YYYY HH:mm", "D. MMMM YYYY HH:mm")),
    ("fr", Patterns::new("DD/MM/YYYY", "D MMMM YYYY", "DD/MM/YYYY HH:mm", "D MMMM YYYY HH:mm")),
    ("es", Patterns::new("DD/MM/YYYY", "D [de] MMMM [de] YYYY", "DD/MM/YYYY H:mm", "D [de] MMMM [de] YYYY H:mm")),
    ("it", Patterns::new("DD/MM/YYYY", "D MMMM YYYY", "DD/MM/YYYY HH:mm", "D MMMM YYYY HH:mm")),
    ("pt", Patterns::new("DD/MM/YYYY", "D [de] MMMM [de] YYYY", "DD/MM/YYYY HH:mm", "D [de] MMMM [de] YYYY HH:mm")),
    ("nl", Patterns::new("DD-MM-YYYY", "D MMMM YYYY", "DD-MM-YYYY HH:mm", "D MMMM YYYY HH:mm")),
    ("sv", Patterns::new("YYYY-MM-DD", "D MMMM YYYY", "YYYY-MM-DD HH:mm", "D MMMM YYYY HH:mm")),
    ("pl", Patterns::new("DD.MM.YYYY", "D MMMM YYYY", "DD.MM.YYYY HH:mm", "D MMMM YYYY HH:mm")),
    ("ja", Patterns::new("YYYY/MM/DD", "YYYY/MM/DD", "YYYY/MM/DD HH:mm", "YYYY/MM/DD HH:mm:ss")),
    ("zh", Patterns::new("YYYY/MM/DD", "YYYY/MM/DD", "YYYY/MM/DD HH:mm", "YYYY/MM/DD HH:mm:ss")),
    ("ko", Patterns::new("YYYY. MM. DD.", "YYYY. MM. DD.", "YYYY. MM. DD. HH:mm", "YYYY. MM. DD. HH:mm:ss")),
];

/// The pattern a blank `kind` field follows for the "Date/time locale" setting ("system", or a
/// locale such as "de_DE"): the language's regional row, then its language row, then ISO 8601.
pub fn default_pattern(locale_setting: &str, kind: DateKind) -> &'static str {
    let found = locale_tag(locale_setting).and_then(|tag| {
        let lang = tag.split('_').next().unwrap_or_default();
        let row = |key: &str| TABLE.iter().find(|(k, _)| *k == key).map(|(_, p)| p);
        row(&tag).or_else(|| row(lang))
    });
    found.unwrap_or(&ISO).get(kind)
}

/// The locale a date setting names, as "lang_REGION" or "lang": "system" (or blank) is the OS
/// locale read once per run, anything else is normalised from BCP 47 or POSIX form. None for
/// C, POSIX or no OS locale.
pub(crate) fn locale_tag(setting: &str) -> Option<String> {
    if setting.trim().is_empty() || setting.eq_ignore_ascii_case("system") {
        return system_tag();
    }
    normalise(setting)
}

fn normalise(tag: &str) -> Option<String> {
    let (lang, region) = split(tag)?;
    Some(match region {
        Some(r) => format!("{lang}_{r}"),
        None => lang,
    })
}

/// The language (lowercased) and region (uppercased, or three digits) of a tag like "de-AT",
/// "es-419", "zh-Hans-CN" or "de_DE.UTF-8@euro". A four-letter script subtag is skipped.
fn split(tag: &str) -> Option<(String, Option<String>)> {
    let tag = tag.trim().split(['.', '@']).next().unwrap_or_default();
    let mut parts = tag.split(['-', '_']);
    let lang = parts.next().unwrap_or_default().to_ascii_lowercase();
    if lang.is_empty() || !lang.chars().all(|c| c.is_ascii_alphabetic()) || lang == "c" || lang == "posix" {
        return None;
    }
    let region = parts.find(|p| !(p.len() == 4 && p.chars().all(|c| c.is_ascii_alphabetic()))).and_then(|p| {
        let alpha = p.len() == 2 && p.chars().all(|c| c.is_ascii_alphabetic());
        let digits = p.len() == 3 && p.chars().all(|c| c.is_ascii_digit());
        (alpha || digits).then(|| p.to_ascii_uppercase())
    });
    Some((lang, region))
}

/// The OS locale, normalised; read on first use and kept for the rest of the run.
fn system_tag() -> Option<String> {
    static TAG: OnceLock<Option<String>> = OnceLock::new();
    TAG.get_or_init(|| sys_locale::get_locale().and_then(|l| normalise(&l))).clone()
}
