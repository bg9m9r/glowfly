use super::catalog::{parse, source};
use super::*;
use std::collections::HashMap;

fn file(lang: Lang) -> HashMap<String, String> {
    let src = source(lang);
    serde_json::from_str::<HashMap<String, String>>(src).unwrap_or_else(|e| panic!("{}.json doesn't parse: {e}", lang.code()))
}

const OTHERS: [Lang; 2] = [Lang::De, Lang::Es];

#[test]
fn catalogs_parse() {
    for lang in Lang::ALL {
        let map = file(lang);
        assert!(!map.is_empty(), "{}.json is empty", lang.code());
        assert_eq!(parse(source(lang)).len(), map.len());
        let blank: Vec<&String> = map.iter().filter(|(_, v)| v.trim().is_empty()).map(|(k, _)| k).collect();
        assert!(blank.is_empty(), "{}.json has empty texts: {blank:?}", lang.code());
    }
}

#[test]
fn translations_only_use_english_keys() {
    let en = file(Lang::En);
    for lang in OTHERS {
        let mut stray: Vec<String> = file(lang).into_keys().filter(|k| !en.contains_key(k)).collect();
        stray.sort();
        assert!(stray.is_empty(), "{}.json has keys English doesn't: {stray:?}", lang.code());
    }
}

#[test]
fn translations_cover_english() {
    let en = file(Lang::En);
    for lang in OTHERS {
        let own = file(lang);
        let mut missing: Vec<&String> = en.keys().filter(|k| !own.contains_key(*k)).collect();
        missing.sort();
        assert!(missing.is_empty(), "{}.json lacks: {missing:?}", lang.code());
    }
}

#[test]
fn translations_keep_placeholders() {
    let en = file(Lang::En);
    for lang in OTHERS {
        for (key, text) in file(lang) {
            let Some(base) = en.get(&key) else { continue };
            let (want, got) = (placeholders(base), placeholders(&text));
            assert_eq!(got, want, "{}.json {key}: placeholders {got:?}, English has {want:?}", lang.code());
        }
    }
}

#[test]
fn plural_keys_come_in_pairs() {
    for lang in Lang::ALL {
        let map = file(lang);
        for key in map.keys() {
            if let Some(base) = key.strip_suffix(".one") {
                assert!(map.contains_key(&format!("{base}.other")), "{}.json: {key} has no .other", lang.code());
            }
            if let Some(base) = key.strip_suffix(".other") {
                assert!(map.contains_key(&format!("{base}.one")), "{}.json: {key} has no .one", lang.code());
            }
        }
    }
    for (key, text) in file(Lang::En) {
        if key.ends_with(".other") {
            assert!(placeholders(&text).contains("count"), "en.json {key} doesn't show {{count}}");
        }
    }
}

#[test]
fn unknown_keys_fall_back() {
    let own: HashMap<String, String> = [("a".to_owned(), "own a".to_owned())].into();
    let base: HashMap<String, String> = [("a".to_owned(), "base a".to_owned()), ("b".to_owned(), "base b".to_owned())].into();
    assert_eq!(lookup(&own, &base, "a"), "own a");
    assert_eq!(lookup(&own, &base, "b"), "base b");
    assert_eq!(lookup(&own, &base, "c"), "c");
    assert_eq!(Lang::De.tr("no.such.key"), "no.such.key");
    assert_eq!(Lang::Es.trn("no.such", 2, &[]), "no.such.other");
}

#[test]
fn fill_replaces_named_placeholders_only() {
    let got = fill("{a} {b} {path} { } {} {a", &[("a", &1), ("b", &"x{a}")]);
    assert_eq!(got, "1 x{a} {path} { } {} {a");
    assert_eq!(fill("", &[]), "");
    assert_eq!(fill("{a}{a}", &[("a", &"é")]), "éé");
    assert_eq!(placeholders("{a} {b_2} { } {} {a"), BTreeSet::from(["a", "b_2"]));
}

#[test]
fn trn_picks_one_and_other() {
    assert_eq!((plural_suffix(0), plural_suffix(1), plural_suffix(2)), ("other", "one", "other"));
    let none: [(&str, &dyn Display); 2] = [("push", &""), ("skip", &"")];
    assert_eq!(Lang::En.trn("commit.button.commit", 1, &none), "Commit changes to 1 file");
    assert_eq!(Lang::En.trn("commit.button.commit", 3, &none), "Commit changes to 3 files");
}

#[test]
fn pick_reads_os_locale_tags() {
    assert_eq!(pick(["de-AT"]), Lang::De);
    assert_eq!(pick(["fr-FR", "es-419"]), Lang::Es);
    assert_eq!(pick(["fr-FR"]), Lang::En);
    assert_eq!(pick(["zh-Hans-CN", "de"]), Lang::De);
    assert_eq!(pick(["de_DE.UTF-8@euro"]), Lang::De);
    assert_eq!(pick(["C", "de"]), Lang::En);
    assert_eq!(pick(["POSIX"]), Lang::En);
    assert_eq!(pick(Vec::<String>::new()), Lang::En);
    assert_eq!(pick(["EN-us"]), Lang::En);
    assert_eq!(pick(["ES"]), Lang::Es);
}

#[test]
fn from_setting_resolves_codes() {
    assert_eq!(Lang::from_setting("de"), Lang::De);
    assert_eq!(Lang::from_setting("es"), Lang::Es);
    assert_eq!(Lang::from_setting("en"), Lang::En);
    for other in [SYSTEM, "", "fr", "DE"] {
        assert_eq!(Lang::from_setting(other), system(), "{other}");
    }
    assert_eq!(system(), Lang::En, "tests never read the OS language");
    let codes: Vec<&str> = Lang::ALL.iter().map(|l| l.code()).collect();
    assert_eq!(codes, ["de", "en", "es"]);
    let mut names: Vec<&str> = Lang::ALL.iter().map(|l| l.native_name()).collect();
    let menu = names.clone();
    names.sort();
    assert_eq!(menu, names, "the menu is sorted by native name");
}
