//! Tests only: dates in the ja_JP, zh_CN and ko_KR locales draw with real glyphs. Every month
//! and weekday word, long and short, both halves of the day and each locale's own patterns are
//! formatted (glowfly-core's `testkit/cjk_dates.rs`), and each character must be in the fonts
//! egui draws with (here the bundled fonts and the tests' CJK subset). The machine's own CJK
//! fonts are asked in the app's `platform/system_fonts/tests.rs`.

use super::{drawable, drawable_in, fonts};
use eframe::egui::FontId;
use glowfly_core::testkit::cjk_dates::{LOCALES, date_chars};

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
