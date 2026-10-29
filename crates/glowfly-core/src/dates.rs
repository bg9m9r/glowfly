//! Date formatting with moment.js-style patterns such as `YYYY-MM-DD HH:mm`, localized month
//! and weekday names, relative times, and each locale's usual patterns (`default_pattern`).

use crate::i18n::Lang;
use chrono::{DateTime, FixedOffset, Locale};

mod locale_patterns;
pub use locale_patterns::{DateKind, default_pattern};

/// Converts a moment.js pattern to a chrono format string. Text in `[brackets]` is literal.
pub fn to_chrono(pattern: &str) -> String {
    // Longest tokens first so "MMMM" wins over "MM".
    const TOKENS: [(&str, &str); 22] = [
        ("YYYY", "%Y"),
        ("YY", "%y"),
        ("MMMM", "%B"),
        ("MMM", "%b"),
        ("MM", "%m"),
        ("M", "%-m"),
        ("DD", "%d"),
        ("Do", "%-d"),
        ("D", "%-d"),
        ("dddd", "%A"),
        ("ddd", "%a"),
        ("HH", "%H"),
        ("H", "%-H"),
        ("hh", "%I"),
        ("h", "%-I"),
        ("mm", "%M"),
        ("m", "%-M"),
        ("ss", "%S"),
        ("s", "%-S"),
        ("A", "%p"),
        ("a", "%P"),
        ("Z", "%:z"),
    ];
    let mut out = String::new();
    let mut rest = pattern;
    'outer: while let Some(c) = rest.chars().next() {
        if c == '['
            && let Some(end) = rest.find(']')
        {
            out.push_str(&rest[1..end].replace('%', "%%"));
            rest = &rest[end + 1..];
            continue;
        }
        for (tok, fmt) in TOKENS {
            if rest.starts_with(tok) {
                out.push_str(fmt);
                rest = &rest[tok.len()..];
                continue 'outer;
            }
        }
        if c == '%' {
            out.push_str("%%");
        } else {
            out.push(c);
        }
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// The locale for month and weekday words: an explicit one like "de_DE", or the operating
/// system's (read once per run); en_US when chrono has no words for it.
pub fn locale(setting: &str) -> Locale {
    let name = locale_patterns::locale_tag(setting).unwrap_or_else(|| "en_US".into());
    Locale::try_from(name.as_str()).unwrap_or(Locale::en_US)
}

/// The machine's current UTC offset in seconds (pass it to `format`).
pub fn local_offset() -> i32 {
    use chrono::Offset;
    chrono::Local::now().offset().fix().local_minus_utc()
}

/// Formats Unix time `secs` with a moment.js `pattern`, in the time zone `utc_offset` seconds
/// from UTC (callers pass `local_offset()`; tests pass a fixed value).
pub fn format(secs: i64, pattern: &str, locale_setting: &str, utc_offset: i32) -> String {
    let Some(t) = DateTime::from_timestamp(secs, 0) else { return String::new() };
    let zone = FixedOffset::east_opt(utc_offset).unwrap_or_else(|| FixedOffset::east_opt(0).expect("zero offset is valid"));
    let local = t.with_timezone(&zone);
    let fmt = to_chrono(pattern);
    // An invalid pattern makes chrono's Display fail; fall back instead of panicking.
    let mut out = String::new();
    use std::fmt::Write;
    if write!(out, "{}", local.format_localized(&fmt, locale(locale_setting))).is_err() {
        return local.format("%Y-%m-%d %H:%M").to_string();
    }
    out
}

/// How long ago `delta` seconds is, in `t`: "just now", "3 hours ago", "yesterday". Negative
/// deltas (a clock ahead of ours) are "just now".
pub fn relative(t: Lang, delta: i64) -> String {
    let n = |d: i64| usize::try_from(d).unwrap_or(0);
    match delta {
        d if d < 60 => t.tr("dates.just_now").to_owned(),
        d if d < 3600 => t.trn("dates.minutes_ago", n(d / 60), &[]),
        d if d < 86_400 => t.trn("dates.hours_ago", n(d / 3600), &[]),
        d if d < 86_400 * 2 => t.tr("dates.yesterday").to_owned(),
        d if d < 86_400 * 30 => t.trn("dates.days_ago", n(d / 86_400), &[]),
        d if d < 86_400 * 365 => t.trn("dates.months_ago", n(d / (86_400 * 30)), &[]),
        d => t.trn("dates.years_ago", n(d / (86_400 * 365)), &[]),
    }
}

/// Common locales offered in Preferences.
pub const LOCALES: [(&str, &str); 14] = [
    ("system", "System"),
    ("en_US", "English (US)"),
    ("en_GB", "English (UK)"),
    ("de_DE", "Deutsch"),
    ("fr_FR", "Français"),
    ("es_ES", "Español"),
    ("it_IT", "Italiano"),
    ("pt_BR", "Português (Brasil)"),
    ("nl_NL", "Nederlands"),
    ("sv_SE", "Svenska"),
    ("pl_PL", "Polski"),
    // Named in English: CJK and Hangul glyphs come from the system's fonts, which may be missing.
    ("ja_JP", "Japanese"),
    ("zh_CN", "Chinese (Simplified)"),
    ("ko_KR", "Korean"),
];

#[cfg(test)]
mod locale_patterns_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_moment_patterns() {
        assert_eq!(to_chrono("MM/DD/YYYY"), "%m/%d/%Y");
        assert_eq!(to_chrono("MMMM D, YYYY"), "%B %-d, %Y");
        assert_eq!(to_chrono("MM/DD/YYYY @ h:mm A"), "%m/%d/%Y @ %-I:%M %p");
        assert_eq!(to_chrono("[Week of] YYYY 100%"), "Week of %Y 100%%");
        assert_eq!(to_chrono("dddd HH:mm:ss"), "%A %H:%M:%S");
    }

    #[test]
    fn formats_with_locale() {
        // 2026-10-13 12:00 UTC; the exact local time depends on the machine's zone, so only
        // check the locale-dependent words.
        let secs = 1_791_892_800;
        assert_eq!(format(secs, "MMMM D, YYYY HH:mm", "en_US", 0), "October 13, 2026 12:00");
        assert_eq!(format(secs, "MMMM YYYY", "de_DE", 0), "Oktober 2026");
        assert_eq!(format(secs, "HH:mm", "en_US", -5 * 3600), "07:00", "offset applied");
    }

    #[test]
    fn relative_keeps_the_english_texts() {
        let en = |d: i64| relative(Lang::En, d);
        assert_eq!(en(30), "just now");
        assert_eq!(en(60), "1 minute ago");
        assert_eq!(en(7200), "2 hours ago");
        assert_eq!(en(90_000), "yesterday");
        assert_eq!(en(3 * 86_400), "3 days ago");
        assert_eq!(en(45 * 86_400), "1 month ago");
        assert_eq!(en(400 * 86_400), "1 year ago");
        assert_eq!(en(-5), "just now");
    }

    #[test]
    fn relative_follows_the_language() {
        assert_eq!(relative(Lang::De, 7200), "vor 2 Stunden");
        assert_eq!(relative(Lang::De, 60), "vor 1 Minute");
        assert_eq!(relative(Lang::De, 90_000), "gestern");
        assert_eq!(relative(Lang::Es, 3 * 86_400), "hace 3 días");
        assert_eq!(relative(Lang::Es, 60), "hace 1 minuto");
    }
}
