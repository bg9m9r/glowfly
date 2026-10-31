//! Reading the sources for the source checks (glowfly-ui's view checks, and the app crate's
//! `hygiene.rs` and `i18n_keys.rs`, which include this file too): every file of a crate marked
//! test or not, the non-test files of glowfly-ui (the views), and each file's code with comments
//! and inline test modules blanked out.

// Each check uses only some of these.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// The source with comments and inline `#[cfg(test)] mod … { … }` blocks blanked out, line for
/// line, so line numbers still match the file.
pub fn code_lines(src: &str) -> Vec<String> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        let next = lines.get(i + 1).map(|l| l.trim()).unwrap_or("");
        if lines[i].trim() == "#[cfg(test)]" && next.starts_with("mod ") && next.ends_with('{') {
            out.push(String::new());
            i += 1;
            let mut depth = 0i64;
            while i < lines.len() {
                depth += lines[i].matches('{').count() as i64 - lines[i].matches('}').count() as i64;
                out.push(String::new());
                i += 1;
                if depth <= 0 {
                    break;
                }
            }
            continue;
        }
        out.push(lines[i].split("//").next().unwrap_or("").to_owned());
        i += 1;
    }
    out
}

pub fn is_test_file(rel: &str) -> bool {
    let stem = rel.rsplit('/').next().unwrap_or(rel).trim_end_matches(".rs");
    stem == "tests" || stem.ends_with("_tests") || stem.starts_with("snapshot_tests")
}

/// The modules `src` declares only for tests: `#[cfg(test)]`, or the `test-support` feature's
/// `#[cfg(any(test, feature = "test-support"))]`, then any other attributes, then `mod name;`.
pub fn test_only_modules(src: &str) -> Vec<String> {
    let lines: Vec<&str> = src.lines().map(str::trim).collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if *l != "#[cfg(test)]" && *l != "#[cfg(any(test, feature = \"test-support\"))]" {
            continue;
        }
        let Some(item) = lines[i + 1..].iter().find(|l| !l.starts_with("#[")) else { continue };
        let item = ["pub(crate) ", "pub "].iter().fold(*item, |s, v| s.strip_prefix(v).unwrap_or(s));
        if let Some(name) = item.strip_prefix("mod ").and_then(|m| m.strip_suffix(';')) {
            out.push(name.to_owned());
        }
    }
    out
}

/// The path (under `src/`, without `.rs`) of module `name` declared in the file at `rel`.
pub fn child_module(rel: &str, name: &str) -> String {
    let stem = rel.trim_end_matches(".rs");
    let (dir, file) = stem.rsplit_once('/').unwrap_or(("", stem));
    // A `mod.rs`, or a crate root (`lib.rs` or `main.rs` straight under `src`), declares its
    // modules beside it; any other file, in the folder named after it.
    let crate_root = (file == "lib" || file == "main") && (dir.is_empty() || dir == "src" || dir.ends_with("/src"));
    match (file == "mod" || crate_root, dir.is_empty()) {
        (true, true) => name.to_owned(),
        (true, false) => format!("{dir}/{name}"),
        (false, _) => format!("{stem}/{name}"),
    }
}

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

/// Every `.rs` file under `src`, as (path under `src` with `/`, text, whether it is test code:
/// a test file by name, or a module declared only for tests and everything under it).
pub fn crate_sources(src: &Path) -> Vec<(String, String, bool)> {
    let mut files = Vec::new();
    rust_files(src, &mut files);
    let rel = |f: &Path| f.strip_prefix(src).unwrap().to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
    let sources: Vec<(String, String)> = files.iter().map(|f| (rel(f), std::fs::read_to_string(f).unwrap())).collect();
    let test_only: Vec<String> =
        sources.iter().flat_map(|(rel, text)| test_only_modules(text).into_iter().map(|m| child_module(rel, &m))).collect();
    sources
        .into_iter()
        .map(|(rel, text)| {
            let module = rel.trim_end_matches(".rs").trim_end_matches("/mod");
            let test = is_test_file(&rel) || test_only.iter().any(|t| module == t || module.starts_with(&format!("{t}/")));
            (rel, text, test)
        })
        .collect()
}

/// This crate's `src` folder, read at run time: a binary built in another worktree (shared
/// target dir) keeps that worktree's compile-time path.
pub fn app_src() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").into())).join("src")
}

/// glowfly-ui's `src` folder (the views), from whichever crate's tests ask.
pub fn ui_src() -> PathBuf {
    app_src().parent().and_then(|krate| krate.parent()).expect("the crates folder").join("glowfly-ui").join("src")
}

/// Every non-test file of glowfly-ui, as (path under its `src/` with `/`, text).
pub fn view_sources() -> Vec<(String, String)> {
    let sources: Vec<(String, String)> =
        crate_sources(&ui_src()).into_iter().filter(|(_, _, test)| !test).map(|(rel, text, _)| (rel, text)).collect();
    assert!(sources.len() > 10, "found glowfly-ui's files");
    sources
}
