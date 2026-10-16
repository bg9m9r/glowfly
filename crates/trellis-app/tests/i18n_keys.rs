//! Every translation key the code looks up exists in the English catalog, and every English key
//! is looked up somewhere, so a typo shows up here rather than as a raw key on screen and the
//! catalogs don't collect dead text. Keys are found as string literals right after `.tr(`,
//! `.trf(` or `.trn(` (for `trn`, the `.one` and `.other` forms are what exists).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use trellis_core::i18n::Lang;

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn is_test_file(path: &Path) -> bool {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    stem == "tests" || stem.ends_with("_tests") || stem.starts_with("snapshot_tests")
}

/// The source without comment lines and without inline `#[cfg(test)] mod … { … }` blocks.
fn code(src: &str) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < lines.len() {
        let next = lines.get(i + 1).map(|l| l.trim()).unwrap_or("");
        if lines[i].trim() == "#[cfg(test)]" && next.starts_with("mod ") && next.ends_with('{') {
            i += 1;
            let mut depth = 0i64;
            while i < lines.len() {
                depth += lines[i].matches('{').count() as i64 - lines[i].matches('}').count() as i64;
                i += 1;
                if depth <= 0 {
                    break;
                }
            }
            continue;
        }
        if !lines[i].trim_start().starts_with("//") {
            out.push_str(lines[i]);
            out.push('\n');
        }
        i += 1;
    }
    out
}

/// The literal keys `code` passes to `.<call>(`: (key, plural).
fn keys_in(code: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    for (call, plural) in [(".tr(", false), (".trf(", false), (".trn(", true)] {
        for (at, _) in code.match_indices(call) {
            let rest = code[at + call.len()..].trim_start();
            let Some(rest) = rest.strip_prefix('"') else { continue };
            let Some(end) = rest.find('"') else { continue };
            out.push((rest[..end].to_owned(), plural));
        }
    }
    out
}

fn sources() -> Vec<(String, String)> {
    // Read at run time: a binary built in another worktree (shared target dir) keeps that
    // worktree's compile-time path.
    let app = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").into()));
    let mut files = Vec::new();
    rust_files(&app.join("src"), &mut files);
    rust_files(&app.join("../trellis-core/src"), &mut files);
    assert!(files.len() > 100, "found the sources");
    files.into_iter().filter(|f| !is_test_file(f)).map(|f| (f.display().to_string(), code(&std::fs::read_to_string(&f).unwrap()))).collect()
}

#[test]
fn every_key_in_code_exists_and_every_english_key_is_used() {
    let mut missing = Vec::new();
    let mut used = BTreeSet::new();
    for (file, code) in sources() {
        for (key, plural) in keys_in(&code) {
            let forms = if plural { vec![format!("{key}.one"), format!("{key}.other")] } else { vec![key] };
            for form in forms {
                if !Lang::En.has(&form) {
                    missing.push(format!("{file}: {form}"));
                }
                used.insert(form);
            }
        }
    }
    assert!(missing.is_empty(), "keys missing from en.json:\n{}", missing.join("\n"));
    let unused: Vec<&str> = Lang::En.keys().map(|(k, _)| k).filter(|k| !used.contains(*k)).collect::<BTreeSet<_>>().into_iter().collect();
    assert!(unused.is_empty(), "en.json keys no code looks up: {unused:?}");
}

#[test]
fn the_scan_finds_keys_across_lines_and_skips_tests_and_comments() {
    let src = "fn a() { t.tr(\"a.b\"); t.trf(\n    \"c.d\", &[]); t.trn(\"e\", 2, &[]); x.str(\"no\"); t.tr(key); }\n\
               // t.tr(\"in.comment\")\n#[cfg(test)]\nmod tests {\n    fn b() { t.tr(\"in.test\"); }\n}\n";
    let keys = keys_in(&code(src));
    assert_eq!(keys, [("a.b".to_owned(), false), ("c.d".to_owned(), false), ("e".to_owned(), true)]);
}
