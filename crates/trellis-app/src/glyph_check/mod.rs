//! Tests only: every non-ASCII character the app can show from its own source (string and char
//! literals in `trellis-app/src` and `trellis-core/src`, outside comments and test code) and
//! from the translation files (`trellis-core/locales/*.json`) has a glyph in the fonts egui
//! draws with. A character without one draws as a box.
//!
//! This lives in the crate rather than `tests/` so that, if the app ever installs fonts of its
//! own, `fonts()` can build its context the same way the app does (only in-crate code can call
//! that).

mod scan;

use eframe::egui;
use std::path::{Path, PathBuf};

/// Characters that appear in literals but never reach the screen, each with the reason.
/// Only for those: other invisible controls (zero-width and bidi marks, U+FEFF) already pass,
/// since egui gives them an empty glyph.
const NEVER_DRAWN: &[(char, &str)] =
    &[('\u{061C}', "Arabic letter mark: text.rs matches it to strip bidi controls from server text; never drawn")];

/// A context with the fonts the app draws with. They exist only after the first pass. If the
/// app ever installs its own fonts, install them here the same way.
fn fonts() -> egui::Context {
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |_| {});
    ctx
}

/// Whether the UI's text font (Proportional; Monospace falls back to the same fonts) has `c`.
fn drawable(ctx: &egui::Context, c: char) -> bool {
    ctx.fonts(|f| f.has_glyph(&egui::FontId::proportional(13.0), c))
}

/// The `crates/` folder, from the manifest folder at run time (target dirs may be shared
/// between checkouts), falling back to the one this was built from.
fn crates_dir() -> PathBuf {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").into()));
    manifest.parent().expect("the app crate sits in crates/").to_path_buf()
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

/// Every character a scanned file can show, as (char, `crates/...` path, line), and how many
/// Rust and locale files were read.
fn shown_characters() -> (Vec<(char, String, usize)>, usize, usize) {
    let crates = crates_dir();
    let mut found = Vec::new();
    let mut rust = 0;
    for krate in ["trellis-app", "trellis-core"] {
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
    let locales = crates.join("trellis-core").join("locales");
    let mut json = 0;
    for entry in std::fs::read_dir(&locales).unwrap_or_else(|e| panic!("{}: {e}", locales.display())) {
        let path = entry.expect("a folder entry").path();
        if path.extension().is_some_and(|e| e == "json") {
            json += 1;
            let name = path.file_name().expect("a file name").to_string_lossy();
            let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            found.extend(scan::json_chars(&src).into_iter().map(|f| (f.ch, format!("crates/trellis-core/locales/{name}"), f.line)));
        }
    }
    (found, rust, json)
}

#[test]
fn fonts_have_what_the_ui_uses_and_lack_what_it_paints() {
    let ctx = fonts();
    // '\u{2714}' (heavy check mark) comes from the emoji fallback; U+200E is invisible.
    for c in ['a', '\u{e9}', '\u{df}', '\u{2026}', '\u{b7}', '\u{201c}', '\u{2714}', '\u{200e}'] {
        assert!(drawable(&ctx, c), "U+{:04X} should be drawable", c as u32);
    }
    // Arrows, the midline ellipsis, the caret, the pencil and CJK are not.
    for c in ['\u{2191}', '\u{2193}', '\u{2192}', '\u{22ef}', '\u{25be}', '\u{270e}', '\u{65e5}'] {
        assert!(!drawable(&ctx, c), "U+{:04X} is expected to be missing", c as u32);
    }
}

#[test]
fn every_shown_character_has_a_glyph() {
    let ctx = fonts();
    let (found, rust, json) = shown_characters();
    assert!(rust > 300, "only {rust} source files scanned: has a folder moved?");
    assert!(json >= 3, "only {json} locale files scanned: has the folder moved?");
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
