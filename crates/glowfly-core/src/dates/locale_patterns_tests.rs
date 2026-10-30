//! Each locale's usual date patterns, the ISO 8601 fallback and locale-tag normalisation.
//! Every case names its locale or injects the environment, so nothing here reads the OS.

use super::locale_patterns::{DateKind, default_pattern, locale_tag, time_locale};

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

/// `time_locale` over a fixed set of variables instead of the process environment.
fn time_locale_of(vars: &[(&str, &str)]) -> Option<String> {
    time_locale(|name| vars.iter().find(|(k, _)| *k == name).map(|(_, v)| (*v).to_owned()))
}

#[test]
fn the_date_locale_follows_lc_all_lc_time_then_lang() {
    let all = [("LC_ALL", "de_DE.UTF-8"), ("LC_TIME", "fr_FR.UTF-8"), ("LANG", "en_US.UTF-8")];
    assert_eq!(time_locale_of(&all).as_deref(), Some("de_DE.UTF-8"));
    assert_eq!(time_locale_of(&all[1..]).as_deref(), Some("fr_FR.UTF-8"));
    assert_eq!(time_locale_of(&all[2..]).as_deref(), Some("en_US.UTF-8"));
    // LANGUAGE orders translations only; it never picks the date format.
    assert_eq!(time_locale_of(&[("LANGUAGE", "de:fr"), ("LANG", "en_GB.UTF-8")]).as_deref(), Some("en_GB.UTF-8"));
    assert_eq!(time_locale_of(&[("LANGUAGE", "de")]), None);
    // Empty values count as unset.
    assert_eq!(time_locale_of(&[("LC_ALL", ""), ("LC_TIME", ""), ("LANG", "sv_SE")]).as_deref(), Some("sv_SE"));
    assert_eq!(time_locale_of(&[("LC_ALL", ""), ("LANG", "")]), None);
    assert_eq!(time_locale_of(&[]), None);
}

#[test]
fn a_c_date_locale_overrides_lang_and_means_iso() {
    for c in ["C", "POSIX", "C.UTF-8"] {
        let found = time_locale_of(&[("LC_ALL", c), ("LANG", "de_DE.UTF-8")]);
        assert_eq!(found.as_deref(), Some(c));
        assert_eq!(found.as_deref().and_then(locale_tag), None, "{c}");
        assert_eq!(row(c), ISO, "{c}");
    }
    let found = time_locale_of(&[("LC_TIME", "C"), ("LANG", "de_DE.UTF-8")]);
    assert_eq!(found.as_deref(), Some("C"));
    let found = time_locale_of(&[("LC_TIME", "en_US.UTF-8"), ("LANG", "C")]);
    assert_eq!(found.as_deref().and_then(locale_tag).as_deref(), Some("en_US"));
}
