//! Tests only: dates in the ja_JP, zh_CN and ko_KR locales draw with real glyphs. Every month
//! and weekday word, long and short, both halves of the day and each locale's own patterns are
//! formatted, and each character must be in the fonts egui draws with (here the bundled fonts
//! and the tests' CJK subset). The machine's own CJK fonts are asked too, when it has some, so
//! the fonts the app would pick are checked on every system that has them.

use super::{drawable, drawable_in, fonts};
use crate::platform::system_fonts;
use eframe::egui::FontId;
use glowfly_core::dates::{self, DateKind};

const LOCALES: [&str; 3] = ["ja_JP", "zh_CN", "ko_KR"];

/// The words of a date: weekday, month and half of the day, long and short.
const WORDS: &str = "dddd ddd MMMM MMM A a";

/// Every non-ASCII character a date in `locale` can show: a year of days, at 06:00 and 18:00
/// UTC, in `WORDS` and the locale's own patterns.
fn date_chars(locale: &str) -> Vec<char> {
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

#[test]
fn cjk_dates_draw_with_real_glyphs() {
    let ctx = fonts();
    let mono = FontId::monospace(13.0);
    for locale in LOCALES {
        let chars = date_chars(locale);
        assert!(chars.len() >= 8, "{locale}: only {chars:?}: are its month and weekday words formatted?");
        let missing: Vec<char> = chars.iter().copied().filter(|c| !drawable(&ctx, *c) || !drawable_in(&ctx, &mono, *c)).collect();
        assert!(missing.is_empty(), "{locale} dates draw these as boxes: {missing:?}");
    }
}

#[test]
fn the_systems_cjk_fonts_draw_the_dates_where_it_has_them() {
    use system_fonts::Script::{Han, Hangul, Kana};
    let found = system_fonts::search();
    let has_script = |s| system_fonts::SCRIPTS.iter().any(|(t, c)| *t == s && found.iter().any(|f| system_fonts::has(f, *c)));
    let fonts: Vec<_> = found.iter().map(|f| &f.path).collect();
    for (locale, needs) in [("ja_JP", &[Han, Kana][..]), ("zh_CN", &[Han][..]), ("ko_KR", &[Hangul][..])] {
        if !needs.iter().all(|s| has_script(*s)) {
            eprintln!("{locale} skipped: this machine has no font for it among the known ones ({fonts:?})");
            continue;
        }
        let missing: Vec<char> = date_chars(locale).into_iter().filter(|c| !found.iter().any(|f| system_fonts::has(f, *c))).collect();
        assert!(missing.is_empty(), "{locale}: {fonts:?} lack {missing:?}");
    }
}
