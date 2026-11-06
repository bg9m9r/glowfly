//! A git config entry's value is read only through glowfly-core's `config_entry`. A key written
//! without `=` (a bare key) has no value, and git2's `ConfigEntry::value`/`value_bytes` panic on
//! one (#796); `config_entry` reads it as absent, or as true where git does.
//!
//! An inline read is `.value_bytes(` or `.has_value(` anywhere (only git2's config entries have
//! these names here), or `.value()` in a file that reads config entries (it names one of
//! `ENTRY_APIS`): the file-level gate keeps other `value()` methods, such as egui's or
//! accesskit's, out. Comments and literal text don't count.

use crate::hygiene::all_sources;
use crate::lexer::bare_lines;

/// The one file that reads entry values.
const HELPER: &str = "crates/glowfly-core/src/config_entry.rs";

/// Files that still read entries inline, each with why. The list only shrinks.
const ALLOWED: &[(&str, &str)] =
    &[("crates/glowfly-core/src/repo/push_config.rs", "#599 rewrites it; read through config_entry once that lands")];

/// What a file that reads config entries names.
const ENTRY_APIS: &[&str] = &["multivar(", "get_entry(", ".entries(None", ".entries(Some(", "ConfigEntr"];

/// The 1-based lines of `src` that read a config entry's value inline.
fn inline_reads(src: &str) -> Vec<usize> {
    let lines = bare_lines(src);
    let reads_entries = lines.iter().any(|l| ENTRY_APIS.iter().any(|api| l.contains(api)));
    lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.contains(".value_bytes(") || l.contains(".has_value(") || (reads_entries && l.contains(".value()")))
        .map(|(i, _)| i + 1)
        .collect()
}

#[test]
fn config_values_are_read_through_config_entry() {
    let mut problems = Vec::new();
    let mut allowed_hit = Vec::new();
    for source in all_sources() {
        if source.path == HELPER {
            continue;
        }
        let hits = inline_reads(&source.text);
        if hits.is_empty() {
            continue;
        }
        if ALLOWED.iter().any(|(p, _)| *p == source.path) {
            allowed_hit.push(source.path);
            continue;
        }
        problems.extend(hits.into_iter().map(|line| format!("{}:{line}", source.path)));
    }
    assert!(
        problems.is_empty(),
        "Read a git config entry's value through glowfly_core::config_entry (raw/utf8/lossy, or bool_or_word for a key git reads bare as true): git2's ConfigEntry::value/value_bytes panic on a key written without '=' (#796):\n{}",
        problems.join("\n")
    );
    let stale: Vec<String> = ALLOWED
        .iter()
        .filter(|(p, _)| !allowed_hit.iter().any(|h| h == p))
        .map(|(p, _)| format!("{p}: no inline reads left; remove it from ALLOWED in config_values.rs"))
        .collect();
    assert!(stale.is_empty(), "{}", stale.join("\n"));
}

#[test]
fn the_config_value_check_catches_what_it_should() {
    let reads = "let mut it = cfg.multivar(\"k\", None)?;\nlet a = e.value();\nlet b = e.value_bytes();\n// e.value() in a comment\n";
    assert_eq!(inline_reads(reads), [2, 3]);
    assert!(inline_reads("let v = id.value();\n").is_empty(), "no config entry read in the file");
    assert!(inline_reads("let s = cfg.get_entry(k);\nlet t = \".value()\";\n").is_empty(), "literal text");
}
