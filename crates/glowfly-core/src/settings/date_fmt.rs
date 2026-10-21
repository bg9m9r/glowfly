//! How dates are shown: the four patterns (Date, Long date, Date and time, Full date and
//! time), each following the date locale while blank, relative ages and hover text.

use super::Settings;
use crate::dates::DateKind;

/// `value`, or `fallback` (the locale's pattern) when the field is blank.
fn pattern<'a>(value: &'a str, fallback: &'static str) -> &'a str {
    if value.trim().is_empty() { fallback } else { value }
}

impl Settings {
    /// The pattern a blank `kind` field follows: the date locale's usual one, or ISO 8601.
    pub fn date_default(&self, kind: DateKind) -> &'static str {
        crate::dates::default_pattern(&self.date_locale, kind)
    }

    fn fmt(&self, secs: i64, value: &str, kind: DateKind, utc_offset: i32) -> String {
        crate::dates::format(secs, pattern(value, self.date_default(kind)), &self.date_locale, utc_offset)
    }

    /// Graph and list dates: the Date and time format, or relative ("3 hours ago") to `now`.
    pub fn format_time(&self, secs: i64, now: i64, utc_offset: i32) -> String {
        if self.relative_dates {
            return crate::dates::relative(self.lang(), now - secs);
        }
        self.fmt(secs, &self.date_time_format, DateKind::DateTime, utc_offset)
    }

    /// Commit details and tooltips: the Full date and time format.
    pub fn format_verbose(&self, secs: i64, utc_offset: i32) -> String {
        self.fmt(secs, &self.date_verbose_format, DateKind::Full, utc_offset)
    }

    /// The Date format, always absolute.
    pub fn format_date(&self, secs: i64, utc_offset: i32) -> String {
        self.fmt(secs, &self.date_format, DateKind::Date, utc_offset)
    }

    /// Created-on lines and sentences: the Long date format, always absolute.
    pub fn format_date_word(&self, secs: i64, utc_offset: i32) -> String {
        self.fmt(secs, &self.date_word_format, DateKind::LongDate, utc_offset)
    }

    /// List dates with no room for the time (file history, blame): relative, or the Date format.
    pub fn format_day(&self, secs: i64, now: i64, utc_offset: i32) -> String {
        if self.relative_dates {
            return crate::dates::relative(self.lang(), now - secs);
        }
        self.format_date(secs, utc_offset)
    }

    /// Hover text for a shown date: the full date and time, and its age unless dates are already relative.
    pub fn date_tooltip(&self, secs: i64, now: i64, utc_offset: i32) -> String {
        let verbose = self.format_verbose(secs, utc_offset);
        if self.relative_dates { verbose } else { format!("{verbose}\n{}", crate::dates::relative(self.lang(), now - secs)) }
    }
}
