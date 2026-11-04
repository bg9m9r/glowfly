//! Every non-test source file under `crates/*/src/` is named in `docs/architecture/`, so the
//! per-area files stay a full map of the code (CLAUDE.md: "Adding, moving or removing a source
//! file: update its line in the matching `docs/architecture/<area>.md`").
//!
//! A file counts as named when some backticked path in those files ends with its path, a whole
//! component at a time: `repo/collide.rs`, `collide.rs` or
//! `crates/glowfly-core/src/repo/collide.rs` all name glowfly-core's `src/repo/collide.rs`. A
//! `::item` after the path is ignored (`update/jobs.rs::start_job`). The docs' shorthands count
//! too: `runner/x.rs` is glowfly-runner's `src/x.rs` and `ui/x.rs` glowfly-ui's `src/x.rs`. A
//! folder's `mod.rs` is also named by its folder's topic line (`blame/`).
//!
//! The files not named today are listed in `UNDOCUMENTED`; the list only shrinks. A file in it
//! that gains a line, or is gone, fails until its entry is removed. Test code (as `hygiene.rs`
//! defines it) is not checked: its lines are optional.

use crate::hygiene::all_sources;
use crate::views::app_src;
use std::collections::BTreeSet;
use std::path::PathBuf;

/// Non-test source files with no line in `docs/architecture/` yet. Remove an entry when its file
/// gets a line; never add one (write the file's line instead).
const UNDOCUMENTED: &[&str] = &[];

/// The docs' shorthands for a crate's `src/`: `runner/x.rs` is glowfly-runner's `src/x.rs`.
const ALIASES: &[(&str, &str)] = &[("glowfly-runner", "runner"), ("glowfly-ui", "ui")];

/// The repository's `docs/architecture/` folder.
fn architecture_dir() -> PathBuf {
    let crates = app_src().parent().and_then(|app| app.parent()).map(PathBuf::from).expect("the crates folder");
    crates.parent().expect("the repository root").join("docs").join("architecture")
}

/// The backticked paths in `text`, each without a `::item` after it. Spans with spaces are
/// prose or code, not paths.
fn doc_paths(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in text.lines() {
        for span in line.split('`').skip(1).step_by(2) {
            let path = span.split("::").next().unwrap_or("");
            if !path.is_empty() && !path.contains(char::is_whitespace) {
                out.insert(path.to_owned());
            }
        }
    }
    out
}

/// Every name that may stand for the source file at `path` (from the repository root): each
/// tail of its path and of its shorthand path, and for a `mod.rs` its folder's (with a `/`).
fn names_of(path: &str) -> Vec<String> {
    let mut full = vec![path.to_owned()];
    if let Some((krate, rel)) = path.strip_prefix("crates/").and_then(|p| p.split_once("/src/")) {
        full.extend(ALIASES.iter().filter(|(name, _)| *name == krate).map(|(_, alias)| format!("{alias}/{rel}")));
    }
    let mut out = Vec::new();
    for p in full {
        let parts: Vec<&str> = p.split('/').collect();
        for i in 0..parts.len() {
            out.push(parts[i..].join("/"));
            if parts.last() == Some(&"mod.rs") && i + 1 < parts.len() {
                out.push(format!("{}/", parts[i..parts.len() - 1].join("/")));
            }
        }
    }
    out
}

/// Whether `paths` (from the docs) name the source file at `path`.
fn is_named(path: &str, paths: &BTreeSet<String>) -> bool {
    names_of(path).iter().any(|n| paths.contains(n))
}

/// The backticked paths of every file in `docs/architecture/`.
fn architecture_paths() -> BTreeSet<String> {
    let dir = architecture_dir();
    let mut paths = BTreeSet::new();
    for entry in std::fs::read_dir(&dir).expect("docs/architecture").flatten() {
        if entry.path().extension().is_some_and(|e| e == "md") {
            paths.extend(doc_paths(&std::fs::read_to_string(entry.path()).expect("an architecture file")));
        }
    }
    assert!(paths.len() > 500, "found the architecture files' paths in {}", dir.display());
    paths
}

#[test]
fn every_source_file_is_named_in_the_architecture_docs() {
    let paths = architecture_paths();
    let sources: Vec<String> = all_sources().into_iter().filter(|s| !s.test).map(|s| s.path).collect();
    let mut problems = Vec::new();
    for path in &sources {
        let listed = UNDOCUMENTED.contains(&path.as_str());
        match (is_named(path, &paths), listed) {
            (false, false) => problems.push(format!(
                "{path}: not named in docs/architecture/; add a line for it to the matching <area>.md \
                 (\"- `<path>`: what it holds.\", the path as that file writes its others)"
            )),
            (true, true) => problems.push(format!(
                "{path}: named in docs/architecture/ now; remove it from UNDOCUMENTED in tests/source_checks/architecture_docs.rs"
            )),
            _ => {}
        }
    }
    for path in UNDOCUMENTED.iter().filter(|p| !sources.iter().any(|s| s == *p)) {
        problems.push(format!("{path}: in UNDOCUMENTED but no longer a non-test source file; remove the entry"));
    }
    assert!(problems.is_empty(), "source files and docs/architecture/ disagree:\n{}", problems.join("\n"));
}

#[test]
fn the_names_match_whole_components() {
    let paths = doc_paths(
        "- `repo/collide.rs`: x; `update/jobs.rs::start_job`, `runner/git_ops.rs` and `blame/`.\n\
         - `a b.rs` is prose.",
    );
    assert!(is_named("crates/glowfly-core/src/repo/collide.rs", &paths));
    assert!(!is_named("crates/glowfly-core/src/repo/repo_collide.rs", &paths), "only whole components");
    assert!(!is_named("crates/glowfly-core/src/collide.rs", &paths), "the folder must match too");
    assert!(is_named("crates/glowfly-model/src/update/jobs.rs", &paths), "a `::item` is ignored");
    assert!(is_named("crates/glowfly-runner/src/git_ops.rs", &paths), "runner/ is glowfly-runner's src/");
    assert!(!is_named("crates/glowfly-model/src/git_ops.rs", &paths), "runner/ is only glowfly-runner's");
    assert!(is_named("crates/glowfly-ui/src/blame/mod.rs", &paths), "a folder line names its mod.rs");
    assert!(!is_named("crates/glowfly-ui/src/blame/body.rs", &paths), "but not the files beside it");
    assert!(!paths.contains("a b.rs"), "a span with a space is not a path");
    assert!(names_of("crates/glowfly-ui/src/blame/mod.rs").contains(&"ui/blame/".to_owned()));
}
