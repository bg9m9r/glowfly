//! Keeps form controls readable in every theme. egui's own text fields, checkboxes, radio
//! buttons and modals take their colours from egui's widget style, which the palette can't make
//! readable everywhere (a field's edge, an unchecked box and a modal's edge would vanish on some
//! themes). Views under `src/ui/` use the themed helpers instead: `theme::controls::field` /
//! `Field`, `checkbox`, `radio`, `radio_value`, `Check` and `Radio`, and `theme::modal`.
//!
//! A text field built in a `let` and added later is caught by a count: each file needs at least
//! as many `field(` / `Field(` wrappers as lines that make a `TextEdit`.
//!
//! Test code is skipped: files named `tests.rs`, `*_tests.rs` or `snapshot_tests*.rs`, modules
//! declared under `#[cfg(test)]`, and inline `#[cfg(test)] mod` blocks.

use std::path::{Path, PathBuf};

/// The files that define the helpers, and may use egui's widgets.
const ALLOWED_FILES: &[&str] = &["ui/theme/controls.rs", "ui/theme/mod.rs"];

/// egui's own controls, as (pattern, the helper to use instead).
const DENIED: &[(&str, &str)] = &[
    (".checkbox(", "controls::checkbox"),
    (".radio(", "controls::radio"),
    (".radio_value(", "controls::radio_value"),
    ("Checkbox::", "controls::Check"),
    ("RadioButton::", "controls::Radio"),
    ("text_edit_singleline", "controls::field"),
    ("text_edit_multiline", "controls::field"),
    ("add(egui::TextEdit", "controls::field"),
    ("add(TextEdit", "controls::field"),
    ("Modal::new(", "theme::modal"),
];

/// The source with comments and inline `#[cfg(test)] mod … { … }` blocks blanked out, line for
/// line, so line numbers still match the file.
fn code_lines(src: &str) -> Vec<String> {
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

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// How often `pat` occurs in `line`; a pattern starting with a word character must start a word.
fn count(line: &str, pat: &str) -> usize {
    let word = pat.chars().next().is_some_and(is_word);
    line.match_indices(pat).filter(|&(i, _)| !word || !line[..i].chars().next_back().is_some_and(is_word)).count()
}

/// Problems in one file's source, as "line: message".
fn violations(src: &str) -> Vec<String> {
    let code = code_lines(src);
    let mut found = Vec::new();
    let (mut edits, mut wrappers, mut first_edit) = (0, 0, None);
    for (i, line) in code.iter().enumerate() {
        for (pat, helper) in DENIED {
            if count(line, pat) > 0 {
                found.push(format!("{}: `{pat}`; use {helper}", i + 1));
            }
        }
        if line.contains("TextEdit::singleline") || line.contains("TextEdit::multiline") {
            edits += 1;
            first_edit.get_or_insert(i + 1);
        }
        wrappers += count(line, "field(") + count(line, "Field(");
    }
    if edits > wrappers {
        let at = first_edit.unwrap_or(1);
        found.push(format!("{at}: {edits} lines make a TextEdit but only {wrappers} add one through controls::field / Field"));
    }
    found
}

fn is_test_file(rel: &str) -> bool {
    let stem = rel.rsplit('/').next().unwrap_or(rel).trim_end_matches(".rs");
    stem == "tests" || stem.ends_with("_tests") || stem.starts_with("snapshot_tests")
}

/// The modules `src` declares only for tests (`#[cfg(test)]` followed by `mod name;`).
fn test_only_modules(src: &str) -> Vec<String> {
    let lines: Vec<&str> = src.lines().map(str::trim).collect();
    lines
        .windows(2)
        .filter(|w| w[0] == "#[cfg(test)]")
        .filter_map(|w| w[1].strip_prefix("pub(crate) ").or(Some(w[1]))?.strip_prefix("mod ")?.strip_suffix(';'))
        .map(str::to_owned)
        .collect()
}

/// The path (under `src/`, without `.rs`) of module `name` declared in the file at `rel`.
fn child_module(rel: &str, name: &str) -> String {
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

#[test]
fn views_use_the_themed_controls() {
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
    let mut report = Vec::new();
    for (rel, text) in &sources {
        let module = rel.trim_end_matches(".rs").trim_end_matches("/mod");
        let skipped = is_test_file(rel) || test_only.iter().any(|t| module == t || module.starts_with(&format!("{t}/")));
        if skipped || ALLOWED_FILES.contains(&rel.as_str()) {
            continue;
        }
        report.extend(violations(text).into_iter().map(|v| format!("src/{rel}:{v}")));
    }
    assert!(
        report.is_empty(),
        "views draw text fields, checkboxes, radio buttons and modals with the themed helpers in \
         src/ui/theme/controls.rs and theme::modal, so they stay readable in every theme:\n{}",
        report.join("\n")
    );
}

#[test]
fn the_check_catches_egui_controls() {
    let n = |s: &str| violations(s).len();
    assert_eq!(n("if ui.checkbox(&mut on, \"Sign\").changed() {"), 1);
    assert_eq!(n("ui.radio_value(&mut v, 1, \"One\");"), 1);
    assert_eq!(n("ui.radio(on, \"One\");"), 1);
    assert_eq!(n("ui.add(egui::Checkbox::without_text(&mut on));"), 1);
    assert_eq!(n("ui.add(egui::RadioButton::new(on, \"x\"));"), 1);
    assert_eq!(n("ui.add(egui::TextEdit::singleline(&mut s));"), 2, "and no wrapper");
    assert_eq!(n("ui.text_edit_singleline(&mut s);"), 1);
    assert_eq!(n("egui::Modal::new(id).show(ctx, |ui| {});"), 1);
    // A let-bound edit added without the wrapper.
    assert_eq!(n("let e = egui::TextEdit::multiline(&mut s);\nui.add(e);"), 1);
    // The themed helpers are fine.
    assert_eq!(n("controls::checkbox(ui, &mut on, \"Sign\");"), 0);
    assert_eq!(n("controls::radio_value(ui, &mut v, 1, \"One\");"), 0);
    assert_eq!(n("ui.add(controls::Check::without_text(&mut on));"), 0);
    assert_eq!(n("let e = egui::TextEdit::multiline(&mut s);\ncontrols::field(ui, e);"), 0);
    assert_eq!(n("ui.add_enabled(on, controls::Field(e));\nlet e = egui::TextEdit::multiline(&mut s);"), 0);
    assert_eq!(n("super::theme::modal(ctx, id).show(ctx, |ui| {});"), 0);
    // A local helper named like the wrapper doesn't count as one.
    assert_eq!(n("let e = egui::TextEdit::singleline(&mut s);\nname_field(&ui.add(e), \"Name\");"), 1);
}

#[test]
fn test_code_is_skipped() {
    assert!(violations("fn a() {}\n#[cfg(test)]\nmod tests {\n    fn b(ui: &mut Ui) { ui.checkbox(&mut on, \"x\"); }\n}\n").is_empty());
    assert_eq!(violations("fn a() {}\nfn b() { ui.checkbox(&mut on, \"x\"); }\n")[0], "2: `.checkbox(`; use controls::checkbox");
    assert!(is_test_file("ui/theme/controls_tests.rs") && is_test_file("ui/snapshot_tests_clone.rs") && is_test_file("ui/tests.rs"));
    assert_eq!(test_only_modules("#[cfg(test)]\nmod snapshot_check;\n"), ["snapshot_check"]);
    assert_eq!(child_module("ui/theme/mod.rs", "x"), "ui/theme/x");
}
