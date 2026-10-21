//! The Date, Long date, Date and time and Full date and time formats, their locale defaults,
//! relative ages and date hover text. Every test names its date locale; none reads the OS.

use super::{DateKind, Settings};

/// 2026-10-13 12:00 UTC.
const SECS: i64 = 1_791_892_800;

fn with_locale(locale: &str) -> Settings {
    Settings { date_locale: locale.into(), ..Settings::default() }
}

fn en() -> Settings {
    with_locale("en_US")
}

#[test]
fn defaults_are_blank() {
    let d = Settings::default();
    for p in [&d.date_format, &d.date_word_format, &d.date_time_format, &d.date_verbose_format] {
        assert_eq!(p, "");
    }
}

#[test]
fn us_english_blank_fields() {
    let s = en();
    assert_eq!(s.format_date(SECS, 0), "10/13/2026");
    assert_eq!(s.format_date_word(SECS, 0), "October 13, 2026");
    assert_eq!(s.format_time(SECS, SECS, 0), "10/13/2026 12:00 PM");
    assert_eq!(s.format_verbose(SECS, 0), "October 13, 2026 12:00 PM");
}

#[test]
fn german_blank_fields() {
    let s = with_locale("de_DE");
    assert_eq!(s.format_date(SECS, 0), "13.10.2026");
    assert_eq!(s.format_date_word(SECS, 0), "13. Oktober 2026");
    assert_eq!(s.format_time(SECS, SECS, 0), "13.10.2026 12:00");
    assert_eq!(s.format_verbose(SECS, 0), "13. Oktober 2026 12:00");
}

#[test]
fn unknown_locale_uses_iso() {
    let s = with_locale("xx");
    assert_eq!(s.format_date(SECS, 0), "2026-10-13");
    assert_eq!(s.format_time(SECS, SECS, 0), "2026-10-13 12:00");
    assert_eq!(s.format_verbose(SECS, 0), "2026-10-13 12:00:00");
}

#[test]
fn a_typed_pattern_wins() {
    let s = Settings { date_format: "YYYY-MM-DD".into(), date_time_format: "  ".into(), ..with_locale("de_DE") };
    assert_eq!(s.format_date(SECS, 0), "2026-10-13");
    assert_eq!(s.format_time(SECS, SECS, 0), "13.10.2026 12:00", "whitespace is blank");
}

#[test]
fn date_default_follows_the_locale() {
    assert_eq!(en().date_default(DateKind::DateTime), "MM/DD/YYYY h:mm A");
    assert_eq!(with_locale("de_DE").date_default(DateKind::DateTime), "DD.MM.YYYY HH:mm");
    assert_eq!(with_locale("C").date_default(DateKind::DateTime), "YYYY-MM-DD HH:mm");
}

#[test]
fn date_formats_ignore_relative() {
    let s = Settings { relative_dates: true, ..en() };
    assert_eq!(s.format_date(SECS, 0), "10/13/2026");
    assert_eq!(s.format_date_word(SECS, 0), "October 13, 2026");
    assert_eq!(s.format_time(SECS, SECS + 7200, 0), "2 hours ago");
}

#[test]
fn format_day_is_relative_or_date() {
    assert_eq!(en().format_day(SECS, SECS + 90_000, 0), "10/13/2026");
    let s = Settings { relative_dates: true, ..en() };
    assert_eq!(s.format_day(SECS, SECS + 90_000, 0), "yesterday");
}

#[test]
fn offset_moves_the_day() {
    // 23:30 UTC is already the next day an hour east.
    assert_eq!(en().format_date(1_791_934_200, 3600), "10/14/2026");
    assert_eq!(en().format_date(1_791_934_200, 0), "10/13/2026");
}

#[test]
fn tooltip_adds_age_unless_relative() {
    assert_eq!(en().date_tooltip(SECS, SECS + 7200, 0), "October 13, 2026 12:00 PM\n2 hours ago");
    let rel = Settings { relative_dates: true, ..en() };
    assert_eq!(rel.date_tooltip(SECS, SECS + 7200, 0), "October 13, 2026 12:00 PM");
    let future = en().date_tooltip(SECS + 600, SECS, 0);
    assert_eq!(future.lines().nth(1), Some("just now"));
}

#[test]
fn relative_dates_follow_the_language() {
    let de = Settings { relative_dates: true, language: "de".into(), ..en() };
    assert_eq!(de.format_time(SECS, SECS + 7200, 0), "vor 2 Stunden");
    assert_eq!(de.format_day(SECS, SECS + 90_000, 0), "gestern");
    let absolute = Settings { language: "de".into(), ..en() };
    assert_eq!(absolute.date_tooltip(SECS, SECS + 7200, 0).lines().nth(1), Some("vor 2 Stunden"));
}
