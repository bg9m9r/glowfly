//! Keeps slow and outside work out of views, `update`, state and the rest of the model.
//! Everything under glowfly-ui's `src/` and glowfly-model's `src/` (bar its `testing.rs`) must
//! stay pure: views draw `AppState` and push `Msg`s, `update` changes state and returns
//! `Effect`s, and the runner (glowfly-runner) does the I/O. The shell (`src/shell/`), which owns
//! the window, the clock and the startup loads, is outside them.
//!
//! The check denies whole families of calls (files, processes, network, threads, environment,
//! clocks, libgit2 access) and every `glowfly-core` function of a core module that does I/O,
//! worked out from core's sources (`layering/core_io.rs`). Core *types* and the pure functions
//! `PURE_FUNCTIONS` names are fine (e.g. `gitcli::args`, `git2::Oid`). A justified exception goes
//! in `ALLOWED` (`layering/rules.rs`) with its file and reason; an entry that matches nothing
//! fails the check too.
//!
//! Test code is skipped: files named `tests.rs`, `*_tests.rs` or `snapshot_tests*.rs`, modules
//! declared under `#[cfg(test)]` (or the test-support feature's `cfg`), and inline
//! `#[cfg(test)] mod` blocks. Tests build fixture repositories on disk. The sources are read
//! with the shared string-aware lexer, and string text is not code.

#[path = "layering/core_io.rs"]
mod core_io;
#[path = "layering/old_io.rs"]
mod old_io;
#[path = "layering/paths.rs"]
mod paths;
#[path = "layering/rules.rs"]
mod rules;

use crate::lexer::bare_lines;
use crate::views::crate_sources;
use core_io::Core;
use rules::{ALLOWED, CHECKED, DENIED, GIT2_PURE_CALLS, GIT2_TYPES, NOT_DENIED, PURE_FUNCTIONS};
use std::path::PathBuf;
use std::sync::OnceLock;

/// One problem found: the line (1-based), the matched pattern and what it reaches.
#[derive(Debug, PartialEq)]
struct Found {
    line: usize,
    pattern: String,
    what: String,
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Byte offsets where `pat` occurs in `line`; a pattern starting with a word character must
/// start a word.
fn matches_at(line: &str, pat: &str) -> Vec<usize> {
    let word = pat.chars().next().is_some_and(is_word);
    line.match_indices(pat).map(|(i, _)| i).filter(|&i| !word || !line[..i].chars().next_back().is_some_and(is_word)).collect()
}

/// Whether the `git2::` path continuing with `rest` names only plain data types.
fn git2_ok(rest: &str) -> bool {
    if GIT2_PURE_CALLS.iter().any(|c| rest.starts_with(c) && rest[c.len()..].starts_with('(')) {
        return true;
    }
    let names: Vec<&str> = match rest.strip_prefix('{') {
        Some(group) => group.split('}').next().unwrap_or("").split(',').map(str::trim).filter(|s| !s.is_empty()).collect(),
        None => vec![rest.split(|c: char| !is_word(c)).next().unwrap_or("")],
    };
    names.iter().all(|n| GIT2_TYPES.contains(n))
}

/// The outside work `src` reaches by itself: the `DENIED` families and non-data `git2::` paths.
fn direct_hits(src: &str) -> Vec<Found> {
    let mut found = Vec::new();
    for (i, line) in bare_lines(src).iter().enumerate() {
        let mut hit = |pattern: String, what: &str| found.push(Found { line: i + 1, pattern, what: what.to_owned() });
        for (pat, what) in DENIED {
            for at in matches_at(line, pat) {
                if !NOT_DENIED.iter().any(|n| line[at..].starts_with(n)) {
                    hit((*pat).to_owned(), what);
                }
            }
        }
        for at in matches_at(line, "git2::") {
            if !git2_ok(&line[at + "git2::".len()..]) {
                hit("git2::".into(), "the repository (only plain `git2` data types are allowed)");
            }
        }
    }
    found
}

/// Problems in one file's source: its direct hits, and its calls and imports of core functions
/// that do I/O.
fn violations(src: &str, core: &Core) -> Vec<Found> {
    let mut found = direct_hits(src);
    for call in core.calls(src).into_iter().filter(|c| c.io) {
        found.push(Found { line: call.line, pattern: call.path, what: format!("core I/O: {}", call.why) });
    }
    found.sort_by_key(|f| f.line);
    found
}

/// The crates folder, read at run time: a binary built in another worktree (shared target dir)
/// keeps that worktree's compile-time path.
fn crates_dir() -> PathBuf {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").into()));
    manifest.parent().expect("the app crate sits in crates/").to_path_buf()
}

/// Core's functions, read once for all the tests.
fn core() -> &'static Core {
    static CORE: OnceLock<Core> = OnceLock::new();
    CORE.get_or_init(|| Core::read(&crates_dir().join("glowfly-core/src")))
}

#[test]
fn views_update_and_state_do_no_io() {
    let crates = crates_dir();
    let core = core();
    let mut report = Vec::new();
    let mut used = vec![false; ALLOWED.len()];
    for (krate, skipped) in CHECKED {
        let sources = crate_sources(&crates.join(krate).join("src"));
        assert!(sources.len() > 10, "found the files under crates/{krate}/src");
        for (rel, text, test) in &sources {
            if *test || skipped.contains(&rel.as_str()) {
                continue;
            }
            let rel = format!("{krate}/src/{rel}");
            let mut found = violations(text, core);
            found.dedup_by(|a, b| a.line == b.line && a.pattern == b.pattern);
            for v in found {
                match ALLOWED.iter().position(|(f, p, _)| *f == rel && *p == v.pattern) {
                    Some(k) => used[k] = true,
                    None => report.push(format!("crates/{rel}:{}: `{}` reaches {}", v.line, v.pattern, v.what)),
                }
            }
        }
    }
    for (k, (file, pat, _)) in ALLOWED.iter().enumerate() {
        if !used[k] {
            report.push(format!("crates/{file}: the ALLOWED entry for `{pat}` matches nothing; remove it"));
        }
    }
    assert!(
        report.is_empty(),
        "views and the model must not do I/O. Return an Effect and do the work in the runner \
         (glowfly-runner), or add a justified entry to ALLOWED in tests/source_checks/layering/rules.rs:\n{}",
        report.join("\n")
    );
}

#[test]
fn pure_functions_are_core_functions_that_need_the_entry() {
    let stale: Vec<&str> = PURE_FUNCTIONS.iter().copied().filter(|f| core().is_io(f) != Some(true)).collect();
    assert!(stale.is_empty(), "PURE_FUNCTIONS entries that name no core I/O function (pure already, or gone); remove them: {stale:?}");
}

#[test]
fn the_check_catches_io() {
    let core = core();
    let io = [
        "repo::load",
        "repo::load_diff",
        "gitcli::run",
        "forge::pull_requests",
        "askpass::server::start_with",
        "persist::append",
        "terminal::pty::spawn",
        "forge::service",
    ];
    let found: Vec<(&str, Option<bool>)> = io.iter().map(|f| (*f, core.is_io(f))).collect();
    assert!(found.iter().all(|(_, io)| *io == Some(true)), "derived from core's sources: {found:?}");
    assert_eq!(core.is_io("gitcli::args"), Some(false));
    let n = |s: &str| violations(s, core).len();
    assert_eq!(n("use glowfly_core::repo;\nlet d = repo::load_diff(&w, &t);"), 1);
    assert_eq!(n("use glowfly_core::repo::{DiffTarget,\n    load_diff};"), 1);
    assert_eq!(n("let d = glowfly_core::repo::load_diff(&w, &t);"), 1);
    assert_eq!(n("use glowfly_core::repo as r;\nr::load_diff(&w, &t);"), 1);
    assert_eq!(n("std::thread::spawn(move || {});"), 2);
    assert_eq!(n("use glowfly_core::repo;\nlet a = args(&[\"tag\"]); // repo::load(here) is only a comment"), 0);
    assert_eq!(n("let u = \"https://example.com\"; let t = std::fs::read(p);"), 2, "a // in a string is no comment");
    assert_eq!(n("let s = \"std::fs::read(p) in a message\";"), 0, "nor is text in a string code");
    assert_eq!(n("use glowfly_core::repo::{self, DiffTarget, Snapshot};"), 0);
    assert_eq!(n("use glowfly_core::askpass;\nlet s = askpass::server::start_with(t, f);"), 1);
    assert_eq!(n("let s = std::net::TcpStream::connect(a);"), 2);
    assert_eq!(n("use glowfly_core::askpass::{PromptKind, host};"), 0, "a module imported is no call");
    assert_eq!(n("use glowfly_core::persist::{open_append, WriteOptions};"), 1);
    // Whole families, not only known names.
    assert_eq!(n("let ok = Path::new(&p).is_dir();"), 1);
    assert_eq!(n("if path.exists() {"), 1);
    assert_eq!(n("let real = std::fs::canonicalize(a);"), 3);
    assert_eq!(n("let t = fs::read_to_string(p);"), 1);
    assert_eq!(n("let f = File::open(p);"), 1);
    assert_eq!(n("let home = std::env::var(\"HOME\");"), 2);
    assert_eq!(n("let r = git2::Repository::open(w);"), 2);
    assert_eq!(n("let t = Instant::now();"), 1);
    // Word starts, plain data and the model's own modules are fine.
    assert_eq!(n("use crate::state::prefs::PrefsState;"), 0);
    assert_eq!(n("let w = WorkFile::new(p);"), 0);
    assert_eq!(n("use git2::{Oid, RepositoryState};"), 0);
    assert_eq!(n("let o: git2::Oid = git2::Oid::zero();"), 0);
    assert_eq!(n("use git2::{Oid, Repository};"), 1);
    assert_eq!(n("let ok = git2::Reference::is_valid_name(&r);"), 0);
    assert_eq!(n("super::repo::load(st, path, LoadKind::Open, fx);"), 0);
    assert_eq!(n("use crate::lfs;\nlfs::status(st);"), 0);
    // A new function of an I/O module is I/O until PURE_FUNCTIONS says otherwise.
    assert_eq!(n("use glowfly_core::repo;\nrepo::a_function_added_later(w);"), 1);
    // A function named like a module beside it (`repo/load.rs`).
    assert_eq!(n("use glowfly_core::repo;\nlet s = repo::load(&w, kind);"), 1);
    assert_eq!(n("use glowfly_core::repo::load;\nlet s = load(&w, kind);"), 1);
    // A pure function that hands out a trait object whose methods do I/O.
    assert_eq!(n("let s = glowfly_core::forge::service(p).verify(&a, &auth);"), 1);
    // Process-wide state.
    assert_eq!(n("use glowfly_core::git_program;\ngit_program::set(p);"), 1);
    assert_eq!(n("use glowfly_core::gitcli;\ngitcli::observe_runs(o);"), 1);
}

#[test]
fn reports_name_the_line_and_skip_test_code() {
    let core = core();
    let src = "fn a() {}\nfn b() { let _ = p.is_file(); }\n#[cfg(test)]\nmod tests {\n    fn c() { std::fs::write(p, \"\"); }\n}\nfn d() { p.exists(); }\n";
    let lines: Vec<(usize, String)> = violations(src, core).into_iter().map(|f| (f.line, f.pattern)).collect();
    assert_eq!(lines, [(2, ".is_file()".to_owned()), (7, ".exists()".to_owned())]);
    let v = violations("use std::path::Path;\n\nuse glowfly_core::repo::{\n    load_diff,\n};\n", core);
    assert_eq!(v.iter().map(|f| (f.line, f.pattern.as_str())).collect::<Vec<_>>(), [(3, "repo::load_diff")]);
}
