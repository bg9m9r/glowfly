//! How dates are shown: the four Date/Time patterns, relative ages and hover text.

use super::Settings;

/// Day-only list dates (file history, blame).
pub const DEFAULT_DATE_FORMAT: &str = "MM/DD/YYYY";
/// Dates read as part of a sentence or a "created on" field.
pub const DEFAULT_DATE_WORD_FORMAT: &str = "MMMM D, YYYY";
/// Graph and list dates with the time.
pub const DEFAULT_DATE_TIME_FORMAT: &str = "MM/DD/YYYY @ h:mm A";
/// Commit details and hovers.
pub const DEFAULT_DATE_VERBOSE_FORMAT: &str = "MMMM D, YYYY h:mm A";

/// `value`, or `default` when the field was cleared.
fn pattern<'a>(value: &'a str, default: &'a str) -> &'a str {
    if value.trim().is_empty() { default } else { value }
}

impl Settings {
    fn fmt(&self, secs: i64, value: &str, default: &str, utc_offset: i32) -> String {
        crate::dates::format(secs, pattern(value, default), &self.date_locale, utc_offset)
    }

    /// Graph and list dates: the Date/Time format, or relative ("3 hours ago") to `now`.
    pub fn format_time(&self, secs: i64, now: i64, utc_offset: i32) -> String {
        if self.relative_dates {
            return crate::dates::relative(now - secs);
        }
        self.fmt(secs, &self.date_time_format, DEFAULT_DATE_TIME_FORMAT, utc_offset)
    }

    /// Commit details and tooltips: the verbose format.
    pub fn format_verbose(&self, secs: i64, utc_offset: i32) -> String {
        self.fmt(secs, &self.date_verbose_format, DEFAULT_DATE_VERBOSE_FORMAT, utc_offset)
    }

    /// The Date format, always absolute.
    pub fn format_date(&self, secs: i64, utc_offset: i32) -> String {
        self.fmt(secs, &self.date_format, DEFAULT_DATE_FORMAT, utc_offset)
    }

    /// Created-on lines and sentences: the Date word format, always absolute.
    pub fn format_date_word(&self, secs: i64, utc_offset: i32) -> String {
        self.fmt(secs, &self.date_word_format, DEFAULT_DATE_WORD_FORMAT, utc_offset)
    }

    /// List dates with no room for the time (file history, blame): relative, or the Date format.
    pub fn format_day(&self, secs: i64, now: i64, utc_offset: i32) -> String {
        if self.relative_dates {
            return crate::dates::relative(now - secs);
        }
        self.format_date(secs, utc_offset)
    }

    /// Hover text for a shown date: the verbose date, and its age unless dates are already relative.
    pub fn date_tooltip(&self, secs: i64, now: i64, utc_offset: i32) -> String {
        let verbose = self.format_verbose(secs, utc_offset);
        if self.relative_dates { verbose } else { format!("{verbose}\n{}", crate::dates::relative(now - secs)) }
    }
}
