//! Reading the view sources for the source checks (`controls.rs`, `message_styles.rs`): every
//! non-test file under `src/ui/`, and each file's code with comments and inline test modules
//! blanked out.

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

/// The modules `src` declares only for tests (`#[cfg(test)]` followed by `mod name;`).
pub fn test_only_modules(src: &str) -> Vec<String> {
    let lines: Vec<&str> = src.lines().map(str::trim).collect();
    lines
        .windows(2)
        .filter(|w| w[0] == "#[cfg(test)]")
        .filter_map(|w| w[1].strip_prefix("pub(crate) ").or(Some(w[1]))?.strip_prefix("mod ")?.strip_suffix(';'))
        .map(str::to_owned)
        .collect()
}

/// The path (under `src/`, without `.rs`) of module `name` declared in the file at `rel`.
pub fn child_module(rel: &str, name: &str) -> String {
    let stem = rel.trim_end_matches(".rs");
    match stem.strip_suffix("/mod") {
        Some(dir) => format!("{dir}/{name}"),
        None => format!("{stem}/{name}"),
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

/// Every non-test file under `src/ui/`, as (path under `src/` with `/`, text).
pub fn view_sources() -> Vec<(String, String)> {
    // Read at run time: a binary built in another worktree (shared target dir) keeps that
    // worktree's compile-time path.
    let src =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").into())).join("src");
    let mut files = Vec::new();
    rust_files(&src.join("ui"), &mut files);
    assert!(files.len() > 10, "found the files under src/ui");
    let rel = |f: &Path| f.strip_prefix(&src).unwrap().to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
    let sources: Vec<(String, String)> = files.iter().map(|f| (rel(f), std::fs::read_to_string(f).unwrap())).collect();
    let test_only: Vec<String> =
        sources.iter().flat_map(|(rel, text)| test_only_modules(text).into_iter().map(|m| child_module(rel, &m))).collect();
    sources
        .into_iter()
        .filter(|(rel, _)| {
            let module = rel.trim_end_matches(".rs").trim_end_matches("/mod");
            !is_test_file(rel) && !test_only.iter().any(|t| module == t || module.starts_with(&format!("{t}/")))
        })
        .collect()
}
