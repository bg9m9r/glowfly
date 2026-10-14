//! Date formatting with moment.js-style patterns such as `MM/DD/YYYY @ h:mm A`,
//! localized month and weekday names, and relative times.

use chrono::{DateTime, FixedOffset, Locale};

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

/// The locale to use: an explicit one like "de_DE", or the operating system's.
pub fn locale(setting: &str) -> Locale {
    let name = if setting.is_empty() || setting.eq_ignore_ascii_case("system") {
        sys_locale::get_locale().unwrap_or_else(|| "en-US".into())
    } else {
        setting.to_owned()
    };
    // OS locales look like "en-US"; chrono wants "en_US".
    Locale::try_from(name.replace('-', "_").as_str()).unwrap_or(Locale::en_US)
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

pub fn relative(delta: i64) -> String {
    let plural = |n: i64, unit: &str| format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" });
    match delta {
        d if d < 60 => "just now".into(),
        d if d < 3600 => plural(d / 60, "minute"),
        d if d < 86_400 => plural(d / 3600, "hour"),
        d if d < 86_400 * 2 => "yesterday".into(),
        d if d < 86_400 * 30 => plural(d / 86_400, "day"),
        d if d < 86_400 * 365 => plural(d / (86_400 * 30), "month"),
        d => plural(d / (86_400 * 365), "year"),
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
    ("ja_JP", "日本語"),
    ("zh_CN", "中文"),
    ("ko_KR", "한국어"),
];

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
        assert_eq!(relative(30), "just now");
        assert_eq!(relative(7200), "2 hours ago");
        assert_eq!(relative(90_000), "yesterday");
    }
}
