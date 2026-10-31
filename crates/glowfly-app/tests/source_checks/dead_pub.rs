//! A ratchet for public items nobody uses. A library crate's `pub` items never get a dead-code
//! warning, so in glowfly-model, glowfly-runner and glowfly-ui a `pub fn`, `const`, `static`,
//! `type`, `struct` or `enum` must be named in some other file of the workspace (any crate's
//! `src/` or `tests/`). The ones that aren't today are in `DEAD_PUB`, which only shrinks: delete
//! the item or make it private, then drop its entry. A type also counts as used when another
//! public item of its file names it (a `pub fn` returning it, a `pub` field holding it), since it
//! has to stay public for that.

use super::views::{code_lines, is_test_file};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The library crates whose public items are checked.
const CHECKED: &[&str] = &["glowfly-model", "glowfly-runner", "glowfly-ui"];

/// Public items named in no other file today, as `<path>: <name>`. Only shrinks.
const DEAD_PUB: &[&str] = &[];

/// Every `.rs` file under `dir`, skipping the snapshot folders.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for path in entries.flatten().map(|e| e.path()) {
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n != "snapshots" && n != "target") {
                rust_files(&path, out);
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The identifiers in `text`.
fn words(text: &str) -> BTreeSet<&str> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).collect()
}

/// The `pub` items `src` declares outside comments and inline test modules, as (name, whether
/// it is a type that another `pub` line of the file names).
fn public_items(src: &str) -> Vec<(String, bool)> {
    let code = code_lines(src);
    let mut out = Vec::new();
    for (at, line) in code.iter().enumerate() {
        let Some(rest) = line.trim_start().strip_prefix("pub ") else { continue };
        let rest = ["const fn ", "unsafe fn ", "async fn "].iter().find_map(|p| rest.strip_prefix(p).map(|r| ("fn", r)));
        let (kind, after) = match rest {
            Some(found) => found,
            None => {
                let rest = line.trim_start().strip_prefix("pub ").unwrap_or("");
                let Some((kind, after)) = rest.split_once(' ') else { continue };
                (kind, after)
            }
        };
        if !["fn", "const", "static", "type", "struct", "enum"].contains(&kind) {
            continue;
        }
        let after = after.strip_prefix("mut ").unwrap_or(after);
        let name: String = after.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        if !name.is_empty() && name != "_" {
            let is_type = ["struct", "enum", "type"].contains(&kind);
            let exposed = is_type
                && code.iter().enumerate().any(|(i, l)| i != at && l.trim_start().starts_with("pub") && words(l).contains(name.as_str()));
            out.push((name, exposed));
        }
    }
    out
}

#[test]
fn public_items_are_used_elsewhere() {
    let crates = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    rust_files(&crates, &mut files);
    let texts: Vec<(String, String)> = files
        .iter()
        .map(|p| {
            let rel = p.strip_prefix(&crates).unwrap_or(p).to_string_lossy().replace('\\', "/");
            (format!("crates/{rel}"), std::fs::read_to_string(p).unwrap_or_default())
        })
        .collect();
    assert!(texts.len() > 500, "found the workspace's sources");
    let mut seen_in: BTreeMap<&str, BTreeSet<usize>> = BTreeMap::new();
    for (i, (_, text)) in texts.iter().enumerate() {
        for w in words(text) {
            seen_in.entry(w).or_default().insert(i);
        }
    }
    let mut dead = BTreeSet::new();
    for (i, (path, text)) in texts.iter().enumerate() {
        let library = CHECKED.iter().any(|c| path.starts_with(&format!("crates/{c}/src/")));
        if !library || is_test_file(path) {
            continue;
        }
        for (name, exposed) in public_items(text) {
            if !exposed && seen_in.get(name.as_str()).is_none_or(|files| files.iter().all(|f| *f == i)) {
                dead.insert(format!("{path}: {name}"));
            }
        }
    }
    let allowed: BTreeSet<String> = DEAD_PUB.iter().map(|s| s.to_string()).collect();
    let new: Vec<&String> = dead.difference(&allowed).collect();
    let gone: Vec<&String> = allowed.difference(&dead).collect();
    assert!(
        new.is_empty(),
        "public items no other file names; make them private (pub(crate) or none) or delete them:\n{}",
        new.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n")
    );
    assert!(gone.is_empty(), "no longer unused; drop from DEAD_PUB:\n{}", gone.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
}

#[test]
fn public_items_finds_the_declarations() {
    let src = "pub fn a() {}\npub const fn b() {}\npub(crate) fn c() {}\n  pub struct D;\npub enum E {}\npub type F = u8;\n\
               pub static mut G: u8 = 0;\npub const H: u8 = 1;\npub mod m;\npub use x::y;\n// pub fn hidden() {}\n";
    let names: Vec<String> = public_items(src).into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, ["a", "b", "D", "E", "F", "G", "H"]);
    let exposed = public_items(
        "pub struct Shown;
pub struct Hidden;
pub fn f() -> Shown { Shown }
fn g(_: Hidden) {}
",
    );
    assert_eq!(exposed, [("Shown".into(), true), ("Hidden".into(), false), ("f".into(), false)]);
}
