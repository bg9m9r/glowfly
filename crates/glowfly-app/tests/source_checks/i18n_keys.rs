//! Every translation key the code looks up exists in the English catalog, and every English key
//! is looked up somewhere, so a typo shows up here rather than as a raw key on screen and the
//! catalogs don't collect dead text. Keys are found as string literals right after `.tr(`,
//! `.trf(` or `.trn(` (for `trn`, the `.one` and `.other` forms are what exists). Also: no view
//! brings back an English "Cancel" or "Close" button (or the old "Close  (Esc)"); they come from
//! `common.cancel.button` and `common.close.button`, and a view's close × is
//! `widgets::ghost::close_view`.

use crate::views;

use glowfly_core::i18n::Lang;
use std::collections::BTreeSet;
use std::path::PathBuf;

/// The source without comments and without inline `#[cfg(test)] mod … { … }` blocks.
fn code(src: &str) -> String {
    views::code_lines(src).join("\n")
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

/// Every crate's non-test sources, as (path, code).
fn sources() -> Vec<(String, String)> {
    let crates = views::app_src().parent().and_then(|app| app.parent()).map(PathBuf::from).expect("the crates folder");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&crates).expect("the crates folder").flatten() {
        let src = entry.path().join("src");
        if src.is_dir() {
            let name = entry.file_name().to_string_lossy().into_owned();
            out.extend(
                views::crate_sources(&src)
                    .into_iter()
                    .filter(|s| !s.2)
                    .map(|(rel, text, _)| (format!("crates/{name}/src/{rel}"), code(&text))),
            );
        }
    }
    assert!(out.len() > 100, "found the sources");
    out
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

/// The button calls a view's Cancel or Close could be made with.
const BUTTON_CALLS: [&str; 4] = ["secondary(", "Button::new(", "ui.button(", "close_label("];
const ENGLISH_DISMISS: [&str; 3] = ["\"Cancel\"", "\"Close\"", "\"Close  (Esc)\""];

/// The places in `line` that pass an English Cancel or Close straight to a button.
fn literal_dismiss(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    for call in BUTTON_CALLS {
        for (at, _) in line.match_indices(call) {
            let arg = line[at + call.len()..].trim_start();
            if let Some(lit) = ENGLISH_DISMISS.iter().find(|l| arg.starts_with(**l)) {
                out.push(format!("{call}{lit}"));
            }
        }
    }
    out
}

#[test]
fn views_have_no_literal_cancel_or_close() {
    let mut found = Vec::new();
    for (rel, text) in views::view_sources() {
        for (i, line) in views::code_lines(&text).iter().enumerate() {
            for hit in literal_dismiss(line) {
                found.push(format!("{rel}:{}: {hit}", i + 1));
            }
        }
    }
    assert!(
        found.is_empty(),
        "use t.tr(\"common.cancel.button\") / t.tr(\"common.close.button\") instead:
{}",
        found.join(
            "
"
        )
    );
}

#[test]
fn the_dismiss_scan_finds_literal_buttons() {
    assert_eq!(literal_dismiss("if ui.add(buttons::secondary(\"Cancel\")).clicked() {"), ["secondary(\"Cancel\""]);
    assert_eq!(literal_dismiss("ui.button( \"Close\")"), ["ui.button(\"Close\""]);
    assert_eq!(literal_dismiss("Footer::new().close_label(\"Close  (Esc)\")"), ["close_label(\"Close  (Esc)\""]);
    assert_eq!(literal_dismiss("egui::Button::new(\"Close\")"), ["Button::new(\"Close\""]);
    assert!(literal_dismiss("ui.button(t.tr(\"common.cancel.button\"))").is_empty());
    assert!(literal_dismiss("ui.button(\"Cancel rebase\")").is_empty());
}

/// Preference row labels stay short enough for the label column: what a row does at length
/// goes in its checkbox text or help.
#[test]
fn preference_row_labels_are_short() {
    let long: Vec<String> = Lang::En
        .keys()
        .filter(|(k, v)| k.starts_with("prefs.") && k.ends_with(".label") && v.chars().count() > 32)
        .map(|(k, v)| format!("{k}: {v:?}"))
        .collect();
    assert!(long.is_empty(), "row labels of more than 32 characters:\n{}", long.join("\n"));
}
