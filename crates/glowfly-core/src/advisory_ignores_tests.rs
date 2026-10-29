//! The advisory exceptions are kept twice: `deny.toml`'s `[advisories] ignore` for cargo-deny
//! (CI) and `osv-scanner.toml`'s `[[IgnoredVulns]]` for osv-scanner (OpenSSF Scorecard). Both
//! must list the same advisories with the same reasons, and each osv-scanner entry lapses on an
//! `ignoreUntil` date so it gets reviewed again. SECURITY.md's Known advisories table lists the
//! same advisories, for readers.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The workspace root, from the manifest folder at run time (a binary built in another worktree
/// keeps that worktree's compile-time path).
fn root() -> PathBuf {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").into());
    Path::new(&dir).join("../..")
}

fn read(file: &str) -> String {
    let path = root().join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())).replace("\r\n", "\n")
}

fn table(file: &str) -> toml::Table {
    read(file).parse().unwrap_or_else(|e| panic!("{file}: {e}"))
}

fn text<'a>(entry: &'a toml::Value, key: &str, file: &str) -> &'a str {
    entry.get(key).and_then(toml::Value::as_str).unwrap_or_else(|| panic!("{file}: an entry without a {key}: {entry}"))
}

/// Advisory id to reason, failing on a repeated id.
fn ignores<'a>(entries: &'a [toml::Value], file: &str) -> BTreeMap<&'a str, &'a str> {
    let mut out = BTreeMap::new();
    for e in entries {
        let id = text(e, "id", file);
        assert!(out.insert(id, text(e, "reason", file)).is_none(), "{file} lists {id} twice");
    }
    out
}

/// The advisory ids in the first column of SECURITY.md's Known advisories table (a cell may
/// hold several, comma-separated).
fn security_md_ids() -> BTreeSet<String> {
    let md = read("SECURITY.md");
    let (_, section) = md.split_once("\n## Known advisories\n").expect("SECURITY.md has a Known advisories section");
    let section = section.split_once("\n## ").map_or(section, |(s, _)| s);
    // The table's rows, after its header and separator lines.
    let rows = section.lines().filter(|l| l.starts_with("| ")).skip(2);
    rows.flat_map(|row| row.split('|').nth(1).unwrap_or("").split(',').map(|id| id.trim().to_owned()).collect::<Vec<_>>()).collect()
}

#[test]
fn osv_scanner_ignores_what_cargo_deny_ignores() {
    let deny = table("deny.toml");
    let deny = deny["advisories"]["ignore"].as_array().expect("deny.toml's [advisories] ignore list");
    let osv = table("osv-scanner.toml");
    let osv = osv["IgnoredVulns"].as_array().expect("osv-scanner.toml's [[IgnoredVulns]]");
    assert_eq!(
        ignores(osv, "osv-scanner.toml"),
        ignores(deny, "deny.toml"),
        "osv-scanner.toml and deny.toml must ignore the same advisories, with the same reasons"
    );
    for e in osv {
        // osv-scanner 2.5, which Scorecard runs, reads `ignoreUntil`, a TOML date.
        assert!(
            matches!(e.get("ignoreUntil"), Some(toml::Value::Datetime(_))),
            "osv-scanner.toml: {} needs an ignoreUntil date",
            text(e, "id", "osv-scanner.toml")
        );
    }
}

#[test]
fn security_md_lists_the_ignored_advisories() {
    let deny = table("deny.toml");
    let deny = deny["advisories"]["ignore"].as_array().expect("deny.toml's [advisories] ignore list");
    let ids: BTreeSet<String> = ignores(deny, "deny.toml").into_keys().map(str::to_owned).collect();
    assert_eq!(security_md_ids(), ids, "SECURITY.md's Known advisories table must list what deny.toml ignores");
}
