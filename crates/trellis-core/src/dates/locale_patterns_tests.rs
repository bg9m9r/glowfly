//! Each locale's usual date patterns, the ISO 8601 fallback and locale-tag normalisation.
//! Every case names its locale, so nothing here reads the OS.

use super::locale_patterns::{DateKind, default_pattern, locale_tag};

const KINDS: [DateKind; 4] = [DateKind::Date, DateKind::LongDate, DateKind::DateTime, DateKind::Full];

fn row(locale: &str) -> [&'static str; 4] {
    KINDS.map(|k| default_pattern(locale, k))
}

const ISO: [&str; 4] = ["YYYY-MM-DD", "YYYY-MM-DD", "YYYY-MM-DD HH:mm", "YYYY-MM-DD HH:mm:ss"];

/// One locale of each table row.
const ROWS: [&str; 13] =
    ["en_US", "en_GB", "de_DE", "fr_FR", "es_ES", "it_IT", "pt_BR", "nl_NL", "sv_SE", "pl_PL", "ja_JP", "zh_CN", "ko_KR"];

#[test]
fn us_english_keeps_month_first() {
    assert_eq!(row("en_US"), ["MM/DD/YYYY", "MMMM D, YYYY", "MM/DD/YYYY h:mm A", "MMMM D, YYYY h:mm A"]);
    assert_eq!(row("en-US"), row("en_US"));
}

#[test]
fn other_english_regions_put_the_day_first() {
    let day_first = ["DD/MM/YYYY", "D MMMM YYYY", "DD/MM/YYYY HH:mm", "D MMMM YYYY HH:mm"];
    for tag in ["en_GB", "en-AU", "en"] {
        assert_eq!(row(tag), day_first, "{tag}");
    }
}

#[test]
fn regions_and_charsets_fall_back_to_the_language() {
    for tag in ["de_DE", "de-AT", "de_DE.UTF-8@euro"] {
        assert_eq!(default_pattern(tag, DateKind::Date), "DD.MM.YYYY", "{tag}");
    }
    assert_eq!(row("es-419"), row("es_ES"));
    assert_eq!(row("es_ES")[1], "D [de] MMMM [de] YYYY");
    assert_eq!(row("zh-Hans-CN"), ["YYYY/MM/DD", "YYYY/MM/DD", "YYYY/MM/DD HH:mm", "YYYY/MM/DD HH:mm:ss"]);
}

#[test]
fn unknown_locales_use_iso_8601() {
    for tag in ["C", "POSIX", "C.UTF-8", "xx_YY", "tlh", "12"] {
        assert_eq!(row(tag), ISO, "{tag}");
    }
}

#[test]
fn cjk_defaults_use_digits_only() {
    for tag in ["ja_JP", "zh_CN", "zh_TW", "ko_KR"] {
        for p in row(tag) {
            assert!(!p.contains("MMM") && !p.contains("ddd"), "{tag}: {p}");
        }
    }
}

#[test]
fn every_row_formats_cleanly() {
    // 2026-10-13 12:00 UTC.
    for tag in ROWS {
        for p in row(tag) {
            let out = crate::dates::format(1_791_892_800, p, "en_US", 0);
            assert!(out.contains("2026") && !out.contains('%'), "{tag}: {p} gave {out:?}");
        }
    }
    assert_eq!(crate::dates::format(1_791_892_800, row("de_DE")[3], "de_DE", 0), "13. Oktober 2026 12:00");
    assert_eq!(crate::dates::format(1_791_892_800, row("es_ES")[1], "es_ES", 0), "13 de octubre de 2026");
}

#[test]
fn locale_tags_are_normalised() {
    assert_eq!(locale_tag("de-DE").as_deref(), Some("de_DE"));
    assert_eq!(locale_tag("pt_br").as_deref(), Some("pt_BR"));
    assert_eq!(locale_tag("zh-Hans-CN").as_deref(), Some("zh_CN"));
    assert_eq!(locale_tag("es-419").as_deref(), Some("es_419"));
    assert_eq!(locale_tag("de_DE.UTF-8@euro").as_deref(), Some("de_DE"));
    assert_eq!(locale_tag("EN").as_deref(), Some("en"));
    assert_eq!(locale_tag("C"), None);
    assert_eq!(locale_tag("POSIX"), None);
}
