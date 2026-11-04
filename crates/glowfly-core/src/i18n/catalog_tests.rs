//! The per-area catalog files: each parses, holds only its area's keys in sorted order, no key is
//! in two files, and every language folder has exactly the files `catalog::AREAS` names.

use super::Lang;
use super::catalog::{AREAS, entries, files, merge};
use std::path::PathBuf;

/// `crates/glowfly-core/locales`, read at run time: a binary built in another worktree keeps
/// that worktree's compile-time path.
fn locales_dir() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").into());
    PathBuf::from(manifest).join("locales")
}

/// The names in `dir` that pass `keep`, sorted (`read_dir`'s order differs by OS).
fn names(dir: &std::path::Path, keep: impl Fn(&std::fs::DirEntry) -> bool) -> Vec<String> {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    let mut out: Vec<String> =
        entries.map(|e| e.expect("a folder entry")).filter(|e| keep(e)).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    out.sort();
    out
}

#[test]
fn area_files_parse() {
    let mut bad = Vec::new();
    for lang in Lang::ALL {
        for (area, src) in files(lang) {
            let name = format!("{}/{area}.json", lang.code());
            match entries(src) {
                None => bad.push(format!("{name} doesn't parse")),
                Some(pairs) if pairs.is_empty() => bad.push(format!("{name} is empty")),
                Some(pairs) => {
                    let blank: Vec<&str> = pairs.iter().filter(|(_, v)| v.trim().is_empty()).map(|(k, _)| k.as_str()).collect();
                    if !blank.is_empty() {
                        bad.push(format!("{name} has empty texts: {blank:?}"));
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn keys_live_in_their_area_file() {
    let mut stray = Vec::new();
    for lang in Lang::ALL {
        for (area, src) in files(lang) {
            let prefix = format!("{area}.");
            for (key, _) in entries(src).unwrap_or_default() {
                if !key.starts_with(&prefix) {
                    stray.push(format!("{}/{area}.json: {key}", lang.code()));
                }
            }
        }
    }
    assert!(stray.is_empty(), "keys in the wrong area file (move each to <lang>/<first segment>.json):\n{}", stray.join("\n"));
}

#[test]
fn no_key_is_in_two_files() {
    for lang in Lang::ALL {
        let (_, dups) = merge(files(lang));
        assert!(dups.is_empty(), "{}/: keys in more than one place: {dups:?}", lang.code());
    }
}

#[test]
fn merge_reports_duplicates() {
    let (texts, dups) = merge([("a", r#"{"a.x":"1"}"#), ("b", r#"{"a.x":"2"}"#)]);
    assert_eq!(dups, ["a.x"]);
    assert_eq!(texts["a.x"], "1", "the first text wins");

    let (texts, dups) = merge([("a", r#"{"a.x":"1","a.x":"2"}"#)]);
    assert_eq!(dups, ["a.x"]);
    assert_eq!(texts["a.x"], "1");

    let (texts, dups) = merge([("a", "{ not json"), ("b", r#"{"b.y":"ok"}"#), ("c", r#"{"c.z": 3}"#)]);
    assert!(dups.is_empty());
    assert_eq!(texts.len(), 1, "files that don't parse are skipped: {texts:?}");
    assert_eq!(entries(r#"{"a.x":"1"} trailing"#), None);
}

#[test]
fn keys_are_sorted_in_each_file() {
    let mut unsorted = Vec::new();
    for lang in Lang::ALL {
        for (area, src) in files(lang) {
            let pairs = entries(src).unwrap_or_default();
            if let Some(w) = pairs.windows(2).find(|w| w[0].0 >= w[1].0) {
                unsorted.push(format!("{}/{area}.json: {} before {}", lang.code(), w[0].0, w[1].0));
            }
        }
    }
    assert!(unsorted.is_empty(), "keys out of order (sort each file's keys):\n{}", unsorted.join("\n"));
}

#[test]
fn area_list_matches_the_folders() {
    let dir = locales_dir();
    let mut codes: Vec<&str> = Lang::ALL.iter().map(|l| l.code()).collect();
    codes.sort();
    let folders = names(&dir, |e| e.path().is_dir());
    assert_eq!(folders, codes, "{}: one folder per language", dir.display());
    let loose = names(&dir, |e| e.path().is_file() && e.path().extension().is_some_and(|x| x == "json"));
    assert!(loose.is_empty(), "catalog files belong in a language folder, not at the top: {loose:?}");

    let mut sorted = AREAS.to_vec();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted, AREAS, "catalog::AREAS is sorted and has no repeats");
    for code in codes {
        let folder = dir.join(code);
        let stems: Vec<String> = names(&folder, |e| e.path().extension().is_some_and(|x| x == "json"))
            .into_iter()
            .map(|n| n.strip_suffix(".json").unwrap_or(&n).to_owned())
            .collect();
        assert_eq!(stems, AREAS, "{}: its area files must match catalog::AREAS (catalog.rs)", folder.display());
    }
}

#[test]
fn lookups_read_the_merged_files() {
    for lang in Lang::ALL {
        let text = lang.tr("commit.blocked.no_repo");
        assert_ne!(text, "commit.blocked.no_repo", "{}", lang.code());
        let in_files: usize = files(lang).map(|(_, src)| entries(src).unwrap_or_default().len()).sum();
        assert_eq!(lang.keys().count(), in_files, "{}: the table holds every file's keys", lang.code());
    }
    assert_ne!(Lang::En.tr("commit.blocked.no_repo"), Lang::De.tr("commit.blocked.no_repo"));
}
