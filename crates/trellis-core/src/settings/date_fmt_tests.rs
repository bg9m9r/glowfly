//! The Date, Date word, Date/Time and verbose formats, relative ages and date hover text.

use super::{DEFAULT_DATE_FORMAT, DEFAULT_DATE_TIME_FORMAT, DEFAULT_DATE_VERBOSE_FORMAT, DEFAULT_DATE_WORD_FORMAT, Settings};

/// 2026-10-13 12:00 UTC.
const SECS: i64 = 1_791_892_800;

fn en() -> Settings {
    Settings { date_locale: "en_US".into(), ..Settings::default() }
}

#[test]
fn date_and_date_word_use_their_patterns() {
    let s = en();
    assert_eq!(s.format_date(SECS, 0), "10/13/2026");
    assert_eq!(s.format_date_word(SECS, 0), "October 13, 2026");
    let de = Settings { date_locale: "de_DE".into(), ..Settings::default() };
    assert_eq!(de.format_date_word(SECS, 0), "Oktober 13, 2026");
    let iso = Settings { date_format: "YYYY-MM-DD".into(), ..en() };
    assert_eq!(iso.format_date(SECS, 0), "2026-10-13");
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
fn blank_patterns_use_defaults() {
    let s = Settings {
        date_format: "  ".into(),
        date_word_format: String::new(),
        date_time_format: String::new(),
        date_verbose_format: " ".into(),
        ..en()
    };
    assert_eq!(s.format_date(SECS, 0), "10/13/2026");
    assert_eq!(s.format_date_word(SECS, 0), "October 13, 2026");
    assert_eq!(s.format_time(SECS, SECS, 0), "10/13/2026 @ 12:00 PM");
    assert_eq!(s.format_verbose(SECS, 0), "October 13, 2026 12:00 PM");
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

#[test]
fn defaults_match_constants() {
    let d = Settings::default();
    assert_eq!(d.date_format, DEFAULT_DATE_FORMAT);
    assert_eq!(d.date_word_format, DEFAULT_DATE_WORD_FORMAT);
    assert_eq!(d.date_time_format, DEFAULT_DATE_TIME_FORMAT);
    assert_eq!(d.date_verbose_format, DEFAULT_DATE_VERBOSE_FORMAT);
}
