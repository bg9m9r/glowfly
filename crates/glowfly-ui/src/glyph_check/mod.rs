//! Tests only: every non-ASCII character the app can show from its own source (string and char
//! literals in every crate's `src/` under `crates/`, outside comments and test code) and
//! from the translation files (`glowfly-core/locales/<lang>/*.json`) has a glyph in the fonts egui
//! draws with. A character without one draws as a box.
//!
//! It installs the bundled fonts the way the app does (`theme::fonts::install`). In tests the
//! CJK fallback is a bundled subset, so the result doesn't depend on the machine; the app's
//! `platform/system_fonts/tests.rs` asks the machine's own CJK fonts, where it has them.

mod cjk_dates;
#[path = "../../tests/view_checks/support/lexer.rs"]
mod lexer;
mod scan;

use eframe::egui;
use std::path::{Path, PathBuf};

/// Characters that appear in literals but never reach the screen, each with the reason.
/// Only for those: other invisible controls (zero-width and bidi marks, U+FEFF) already pass,
/// since egui gives them an empty glyph.
const NEVER_DRAWN: &[(char, &str)] = &[
    ('\u{061C}', "Arabic letter mark: text.rs matches it to strip bidi controls from server text; never drawn"),
    ('\u{115F}', "text.rs is_invisible matches it to strip invisible characters from server text; never drawn"),
    ('\u{1160}', "text.rs is_invisible matches it to strip invisible characters from server text; never drawn"),
    ('\u{17B4}', "text.rs is_invisible matches it to strip invisible characters from server text; never drawn"),
    ('\u{17B5}', "text.rs is_invisible matches it to strip invisible characters from server text; never drawn"),
    ('\u{180B}', "text.rs is_invisible matches it to strip invisible characters from server text; never drawn"),
    ('\u{180F}', "text.rs is_invisible matches it to strip invisible characters from server text; never drawn"),
    ('\u{2065}', "text.rs is_invisible matches it to strip invisible characters from server text; never drawn"),
    ('\u{3164}', "text.rs is_invisible matches it to strip invisible characters from server text; never drawn"),
    ('\u{FFA0}', "text.rs is_invisible matches it to strip invisible characters from server text; never drawn"),
    ('\u{FFF0}', "text.rs is_invisible matches it to strip invisible characters from server text; never drawn"),
    ('\u{FFF8}', "text.rs is_invisible matches it to strip invisible characters from server text; never drawn"),
    ('\u{1BCA0}', "text.rs is_invisible matches it to strip invisible characters from server text; never drawn"),
    ('\u{1BCA3}', "text.rs is_invisible matches it to strip invisible characters from server text; never drawn"),
    ('\u{1D173}', "text.rs is_invisible matches it to strip invisible characters from server text; never drawn"),
    ('\u{1D17A}', "text.rs is_invisible matches it to strip invisible characters from server text; never drawn"),
    ('\u{E0000}', "text.rs is_invisible matches it to strip invisible characters from server text; never drawn"),
    ('\u{E0FFF}', "text.rs is_invisible matches it to strip invisible characters from server text; never drawn"),
    ('\u{FF0E}', "link_hosts.rs as_read folds it to its ASCII form before judging a link's host; never drawn"),
    ('\u{3002}', "link_hosts.rs as_read folds it to its ASCII form before judging a link's host; never drawn"),
    ('\u{FF61}', "link_hosts.rs as_read folds it to its ASCII form before judging a link's host; never drawn"),
    ('\u{FF0F}', "link_hosts.rs as_read folds it to its ASCII form before judging a link's host; never drawn"),
    ('\u{FF1A}', "link_hosts.rs as_read folds it to its ASCII form before judging a link's host; never drawn"),
    ('\u{A4F8}', "link_hosts.rs DOT_LIKE matches it as a look-alike dot when judging a link's host; never drawn"),
    ('\u{A4F9}', "link_hosts.rs DOT_LIKE matches it as a look-alike dot when judging a link's host; never drawn"),
    ('\u{0701}', "link_hosts.rs DOT_LIKE matches it as a look-alike dot when judging a link's host; never drawn"),
    ('\u{0702}', "link_hosts.rs DOT_LIKE matches it as a look-alike dot when judging a link's host; never drawn"),
    ('\u{30FB}', "link_hosts.rs DOT_LIKE matches it as a look-alike dot when judging a link's host; never drawn"),
];

/// A context with the fonts the app draws with: the bundled ones, installed as the app does.
/// They exist only after the first pass.
fn fonts() -> egui::Context {
    let ctx = egui::Context::default();
    crate::theme::fonts::use_test_cjk();
    crate::theme::fonts::install(&ctx);
    let _ = ctx.run(egui::RawInput::default(), |_| {});
    ctx
}

/// Whether the UI's text font (Proportional; Monospace falls back to the same fonts) has `c`.
fn drawable(ctx: &egui::Context, c: char) -> bool {
    drawable_in(ctx, &egui::FontId::proportional(13.0), c)
}

fn drawable_in(ctx: &egui::Context, font: &egui::FontId, c: char) -> bool {
    ctx.fonts(|f| f.has_glyph(font, c))
}

/// The `crates/` folder, from the manifest folder at run time (target dirs may be shared
/// between checkouts), falling back to the one this was built from.
fn crates_dir() -> PathBuf {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").into()));
    manifest.parent().expect("glowfly-ui sits in crates/").to_path_buf()
}

/// The folders under `crates/` that hold a crate with sources, sorted.
fn crate_names(crates: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(crates)
        .unwrap_or_else(|e| panic!("{}: {e}", crates.display()))
        .flatten()
        .filter(|e| e.path().join("src").is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Every `.rs` file under `dir`, as (path relative to `dir` with `/` separators, contents).
fn rust_files(dir: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap_or_else(|e| panic!("{}: {e}", d.display())) {
            let path = entry.expect("a folder entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let rel = path.strip_prefix(dir).expect("under the folder").to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
                out.push((rel, std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))));
            }
        }
    }
    out.sort();
    out
}

/// The paths in `dir`, sorted (`read_dir`'s order differs by OS).
fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    let mut out: Vec<PathBuf> = entries.map(|e| e.expect("a folder entry").path()).collect();
    out.sort();
    out
}

/// Every character a scanned file can show, as (char, `crates/...` path, line), and how many
/// Rust and locale files were read.
fn shown_characters() -> (Vec<(char, String, usize)>, usize, usize) {
    let crates = crates_dir();
    let mut found = Vec::new();
    let mut rust = 0;
    for krate in crate_names(&crates) {
        let krate = krate.as_str();
        let files = rust_files(&crates.join(krate).join("src"));
        let test_only: Vec<String> = files.iter().flat_map(|(rel, src)| scan::test_only_modules(rel, src)).collect();
        for (rel, src) in &files {
            let module = rel.strip_suffix(".rs").unwrap_or(rel);
            let module = module.strip_suffix("/mod").unwrap_or(module);
            if scan::is_test_path(rel) || test_only.iter().any(|t| module == t || module.starts_with(&format!("{t}/"))) {
                continue;
            }
            rust += 1;
            let shown = format!("crates/{krate}/src/{rel}");
            found.extend(scan::rust_literal_chars(src).into_iter().map(|f| (f.ch, shown.clone(), f.line)));
        }
    }
    let locales = crates.join("glowfly-core").join("locales");
    let mut json = 0;
    for lang in sorted_entries(&locales).into_iter().filter(|p| p.is_dir()) {
        let code = lang.file_name().expect("a folder name").to_string_lossy().into_owned();
        for path in sorted_entries(&lang).into_iter().filter(|p| p.extension().is_some_and(|e| e == "json")) {
            json += 1;
            let name = path.file_name().expect("a file name").to_string_lossy();
            let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let shown = format!("crates/glowfly-core/locales/{code}/{name}");
            found.extend(scan::json_chars(&src).into_iter().map(|f| (f.ch, shown.clone(), f.line)));
        }
    }
    (found, rust, json)
}

#[test]
fn fonts_have_what_the_ui_uses_and_lack_what_it_paints() {
    let ctx = fonts();
    // '\u{2714}' (heavy check mark) comes from egui's emoji fallback; U+200E is invisible.
    // Monospace text (JetBrains Mono, then the same fallbacks) draws them too. U+FFFD stands
    // in for commit text that doesn't decode (commit_text.rs).
    let mono = egui::FontId::monospace(13.0);
    for c in ['a', '\u{e9}', '\u{df}', '\u{2026}', '\u{b7}', '\u{201c}', '\u{2714}', '\u{200e}', '\u{fffd}'] {
        assert!(drawable(&ctx, c), "U+{:04X} should be drawable", c as u32);
        assert!(drawable_in(&ctx, &mono, c), "U+{:04X} should be drawable in monospace", c as u32);
    }
    // Noto Sans has no arrows, midline ellipsis, caret or pencil, so the UI paints them (the
    // ahead/behind arrows too, though JetBrains Mono has arrows). The CJK fallback adds none:
    // in tests it is a subset with only CJK and Hangul characters.
    for c in ['\u{2191}', '\u{2193}', '\u{2192}', '\u{22ef}', '\u{25be}', '\u{270e}'] {
        assert!(!drawable(&ctx, c), "U+{:04X} is expected to be missing", c as u32);
    }
}

#[test]
fn every_shown_character_has_a_glyph() {
    let ctx = fonts();
    let (found, rust, json) = shown_characters();
    assert!(rust > 300, "only {rust} source files scanned: has a folder moved?");
    assert!(json > 100, "only {json} locale files scanned: has the folder moved?");
    assert!(found.iter().any(|(c, ..)| *c == '\u{2026}'), "no ellipsis found in any literal: is the scanner reading them?");
    let mut report: Vec<String> = found
        .iter()
        .filter(|(c, ..)| !drawable(&ctx, *c) && !NEVER_DRAWN.iter().any(|(n, _)| n == c))
        .map(|(c, path, line)| format!("U+{:04X} {c:?} {path}:{line}", *c as u32))
        .collect();
    for (c, why) in NEVER_DRAWN {
        if !found.iter().any(|(f, ..)| f == c) {
            report.push(format!("U+{:04X} {c:?} is in NEVER_DRAWN ({why}) but no longer appears: remove it", *c as u32));
        }
    }
    assert!(
        report.is_empty(),
        "These characters have no glyph in the bundled fonts, so they draw as a box. Paint the symbol as a shape \
         (widgets::paint_icon), use a character the fonts have, or, for a character that never reaches the screen, \
         add it to NEVER_DRAWN with the reason:\n{}",
        report.join("\n")
    );
}
