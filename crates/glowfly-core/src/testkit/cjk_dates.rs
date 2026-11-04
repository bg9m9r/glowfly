//! The characters dates show in the ja_JP, zh_CN and ko_KR locales, for the glyph checks: the
//! bundled fonts' (glowfly-ui's `glyph_check/cjk_dates.rs`) and the machine's own CJK fonts'
//! (the app's `platform/system_fonts/tests.rs`).

use crate::dates::{self, DateKind};

/// The CJK locales whose dates are checked.
pub const LOCALES: [&str; 3] = ["ja_JP", "zh_CN", "ko_KR"];

/// The words of a date: weekday, month and half of the day, long and short.
const WORDS: &str = "dddd ddd MMMM MMM A a";

/// Every non-ASCII character a date in `locale` can show: a year of days, at 06:00 and 18:00
/// UTC, in `WORDS` and the locale's own patterns.
pub fn date_chars(locale: &str) -> Vec<char> {
    // 2026-01-01 00:00 UTC.
    let start = 1_767_225_600;
    let patterns = [DateKind::Date, DateKind::LongDate, DateKind::DateTime, DateKind::Full].map(|k| dates::default_pattern(locale, k));
    let mut out: Vec<char> = Vec::new();
    for day in 0..366 {
        for hour in [6, 18] {
            let secs = start + day * 86_400 + hour * 3_600;
            for p in patterns.iter().copied().chain([WORDS]) {
                out.extend(dates::format(secs, p, locale, 0).chars().filter(|c| !c.is_ascii()));
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}
