//! Keeps slow and outside work out of views, `update` and state. Everything under `src/ui/`,
//! `src/update/` and `src/state/` must stay pure: views draw `AppState` and push `Msg`s, `update`
//! changes state and returns `Effect`s, and the runner (`src/runner/`) does the I/O.
//!
//! The check denies whole families of calls (files, processes, network, threads, environment,
//! clocks, libgit2 access) and the `trellis-core` functions that do I/O. Core *types* and pure
//! helpers are fine (e.g. `gitcli::args`, `forge::parse_host`, `git2::Oid`). A justified exception
//! goes in `ALLOWED` with its file and reason; an entry that matches nothing fails the check too.
//!
//! Test code is skipped: files named `tests.rs`, `*_tests.rs` or `snapshot_tests*.rs`, modules
//! declared under `#[cfg(test)]`, and inline `#[cfg(test)] mod` blocks. Tests build fixture
//! repositories on disk.

use std::path::{Path, PathBuf};

/// The source folders that must do no I/O.
const DIRS: &[&str] = &["ui", "update", "state"];

const FILES: &str = "the file system";
const CLOCK: &str = "the clock (update gets `now`; views read `st.view.now`)";

/// Families of outside work, as (pattern, what it reaches). A pattern that starts with a letter
/// matches only at the start of a word, so `fs::` doesn't match `prefs::`.
const DENIED: &[(&str, &str)] = &[
    ("std::fs", FILES),
    ("fs::", FILES),
    ("File::", FILES),
    ("OpenOptions", FILES),
    (".is_dir()", FILES),
    (".is_file()", FILES),
    (".is_symlink()", FILES),
    (".exists()", FILES),
    (".try_exists(", FILES),
    (".metadata(", FILES),
    ("symlink_metadata", FILES),
    ("canonicalize", FILES),
    ("read_dir", FILES),
    ("read_link", FILES),
    ("remove_file", FILES),
    ("remove_dir", FILES),
    ("create_dir", FILES),
    ("trash::", FILES),
    ("std::process", "other processes"),
    ("Command::new", "other processes"),
    ("portable_pty", "other processes"),
    ("std::net", "the network"),
    ("TcpStream", "the network"),
    ("TcpListener", "the network"),
    ("UdpSocket", "the network"),
    ("ureq", "the network"),
    ("keyring::", "the credential store"),
    ("Repository::", "the repository"),
    ("std::thread", "threads"),
    ("thread::spawn", "threads"),
    ("thread::Builder", "threads"),
    ("thread::sleep", "threads"),
    ("std::env", "the environment"),
    ("env::var", "the environment"),
    ("env::current_dir", "the environment"),
    ("Instant::now", CLOCK),
    ("SystemTime::now", CLOCK),
    ("Local::now", CLOCK),
    ("Utc::now", CLOCK),
    ("Settings::load", "the settings file"),
    (".save()", "the settings file"),
];

/// `git2::` items that are plain data and may be used anywhere.
const GIT2_TYPES: &[&str] = &["Oid", "RepositoryState"];

/// Core functions that touch the repository, network, credential store or other processes.
const IO_FUNCTIONS: &[(&str, &[&str])] = &[
    (
        "repo",
        &[
            "load",
            "status_at",
            "messages_between",
            "commit_stats",
            "commit_detail",
            "compare",
            "load_diff",
            "load_diff_with",
            "load_diff_for_view",
            "apply_hunk",
            "apply_lines",
            "stage_file",
            "stage_all",
            "unstage_paths",
            "discard_file",
            "discard_untracked",
            "load_conflict",
            "file_history",
            "write_resolution",
            "resolve_with_side",
            "restore_file",
            "create_file",
            "rename_file",
            "delete_file",
            "delete_staged_new_file",
            "unstage_gone_new_file",
            "os_trash",
            "submodules_at",
            "worktrees_at",
        ],
    ),
    ("gitcli", &["run", "run_with_input", "version"]),
    ("conflict_check", &["predict"]),
    (
        "forge",
        &[
            "verify",
            "pull_requests",
            "default_branch",
            "create_pull_request",
            "my_issues",
            "issues",
            "resolve_auth",
            "save_token",
            "delete_token",
            "from_credential_manager",
            "forget_credential_manager",
            "service",
            "repo_owners",
            "create_repo",
        ],
    ),
    ("avatars", &["fetch"]),
    ("signature_check", &["verify_commit", "signature_format"]),
    ("signing", &["list_gpg_keys", "list_ssh_keys", "generate_gpg_key", "export_public_key"]),
    ("ssh_keys", &["find_keygen", "fingerprint", "list_key_pairs", "list", "generate"]),
    ("undo", &["before", "after", "plan", "redone", "discard_saving", "restore_saving", "write_saved"]),
    ("rebase", &["plan", "start", "stopped_at", "install_todo"]),
    ("askpass", &["start", "start_with", "ask", "ask_from_env"]),
    ("server", &["start", "start_with"]),
    ("pty", &["spawn", "default_shell"]),
    ("lfs", &["status", "installed", "hook_state"]),
    ("gitflow", &["read_config"]),
    ("worktree_ops", &["check_folder", "remove", "dirty_count"]),
    ("paths", &["resolve", "temp_dir", "same_folder"]),
    ("FolderId", &["of"]),
    (
        "platform",
        &[
            "load_recent",
            "save_recent",
            "check_folders",
            "load_tabs",
            "save_tabs",
            "open_terminal",
            "open_in_file_manager",
            "reveal_in_file_manager",
        ],
    ),
];

/// Justified exceptions, as (file under `src/`, pattern as reported, reason).
const ALLOWED: &[(&str, &str, &str)] = &[
    ("ui/mod.rs", "Settings::load", "the shell loads the settings once, at startup"),
    ("ui/mod.rs", "platform::load_recent", "the shell loads the recent list once, at startup"),
    ("ui/mod.rs", "platform::load_tabs", "the shell loads the last session's tabs once, at startup"),
    ("ui/mod.rs", "Instant::now", "the shell stamps each frame and message with the time"),
    ("ui/mod.rs", "Local::now", "the shell sets `st.view.now` once per frame"),
    ("state/mod.rs", "std::env", "`any_dir` falls back to the temp folder's path; nothing is read"),
];

/// One problem found: the line (1-based), the matched pattern and what it reaches.
#[derive(Debug, PartialEq)]
struct Found {
    line: usize,
    pattern: String,
    what: &'static str,
}

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

/// Byte offsets where `pat` occurs in `line`; a pattern starting with a word character must
/// start a word.
fn matches_at(line: &str, pat: &str) -> Vec<usize> {
    let word = pat.chars().next().is_some_and(is_word);
    line.match_indices(pat).map(|(i, _)| i).filter(|&i| !word || !line[..i].chars().next_back().is_some_and(is_word)).collect()
}

/// Whether the `git2::` path continuing with `rest` names only plain data types.
fn git2_ok(rest: &str) -> bool {
    let names: Vec<&str> = match rest.strip_prefix('{') {
        Some(group) => group.split('}').next().unwrap_or("").split(',').map(str::trim).filter(|s| !s.is_empty()).collect(),
        None => vec![rest.split(|c: char| !is_word(c)).next().unwrap_or("")],
    };
    names.iter().all(|n| GIT2_TYPES.contains(n))
}

/// Problems in one file's source.
fn violations(src: &str) -> Vec<Found> {
    let code = code_lines(src);
    let mut found = Vec::new();
    for (i, line) in code.iter().enumerate() {
        let mut hit = |pattern: String, what| found.push(Found { line: i + 1, pattern, what });
        for (pat, what) in DENIED {
            for _ in matches_at(line, pat) {
                hit((*pat).to_owned(), what);
            }
        }
        for at in matches_at(line, "git2::") {
            if !git2_ok(&line[at + "git2::".len()..]) {
                hit("git2::".into(), "the repository (only plain `git2` data types are allowed)");
            }
        }
        for (module, names) in IO_FUNCTIONS {
            for name in *names {
                for at in matches_at(line, &format!("{module}::{name}(")) {
                    // `super::repo::load` is the app's own `update::repo`, not core's.
                    if !line[..at].ends_with("super::") {
                        hit(format!("{module}::{name}"), "core I/O");
                    }
                }
            }
        }
    }
    // Imported by name, e.g. `use trellis_core::repo::{load_diff, DiffTarget};`.
    let joined = code.join("\n");
    let mut line = 1;
    for chunk in joined.split_inclusive(';') {
        let lead = chunk.len() - chunk.trim_start().len();
        let at = line + chunk[..lead].matches('\n').count();
        line += chunk.matches('\n').count();
        let stmt = chunk.trim();
        let Some(stmt) = stmt.strip_prefix("use ").or_else(|| stmt.strip_prefix("pub use ")) else { continue };
        for (module, names) in IO_FUNCTIONS {
            let Some((_, rest)) = stmt.split_once(&format!("{module}::")) else { continue };
            let words: Vec<&str> = rest.split(|c: char| !is_word(c)).collect();
            for name in *names {
                if words.contains(name) {
                    found.push(Found { line: at, pattern: format!("use {module}::{name}"), what: "core I/O" });
                }
            }
        }
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
fn views_update_and_state_do_no_io() {
    // Read at run time: a binary built in another worktree (shared target dir) keeps that
    // worktree's compile-time path.
    let src =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").into())).join("src");
    let mut report = Vec::new();
    let mut used = vec![false; ALLOWED.len()];
    for dir in DIRS {
        let mut files = Vec::new();
        rust_files(&src.join(dir), &mut files);
        assert!(files.len() > 10, "found the files under src/{dir}");
        let rel = |f: &Path| f.strip_prefix(&src).unwrap().to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
        let sources: Vec<(String, String)> = files.iter().map(|f| (rel(f), std::fs::read_to_string(f).unwrap())).collect();
        let test_only: Vec<String> =
            sources.iter().flat_map(|(rel, text)| test_only_modules(text).into_iter().map(|m| child_module(rel, &m))).collect();
        for (rel, text) in &sources {
            let module = rel.trim_end_matches(".rs").trim_end_matches("/mod");
            if is_test_file(rel) || test_only.iter().any(|t| module == t || module.starts_with(&format!("{t}/"))) {
                continue;
            }
            let mut found = violations(text);
            found.dedup_by(|a, b| a.line == b.line && a.pattern == b.pattern);
            for v in found {
                match ALLOWED.iter().position(|(f, p, _)| f == rel && *p == v.pattern) {
                    Some(k) => used[k] = true,
                    None => report.push(format!("src/{rel}:{}: `{}` reaches {}", v.line, v.pattern, v.what)),
                }
            }
        }
    }
    for (k, (file, pat, _)) in ALLOWED.iter().enumerate() {
        if !used[k] {
            report.push(format!("src/{file}: the ALLOWED entry for `{pat}` matches nothing; remove it"));
        }
    }
    assert!(
        report.is_empty(),
        "views, update and state must not do I/O. Return an Effect and do the work in the runner \
         (src/runner/), or add a justified entry to ALLOWED in tests/layering.rs:\n{}",
        report.join("\n")
    );
}

#[test]
fn the_check_catches_io() {
    let n = |s: &str| violations(s).len();
    assert_eq!(n("let d = repo::load_diff(&w, &t);"), 1);
    assert_eq!(n("use trellis_core::repo::{DiffTarget,\n    load_diff};"), 1);
    assert_eq!(n("std::thread::spawn(move || {});"), 2);
    assert_eq!(n("let a = args(&[\"tag\"]); // repo::load(here) is only a comment"), 0);
    assert_eq!(n("use trellis_core::repo::{self, DiffTarget, Snapshot};"), 0);
    assert_eq!(n("let s = askpass::server::start_with(t, f);"), 1);
    assert_eq!(n("askpass::ask_from_env(p)"), 1);
    assert_eq!(n("let s = std::net::TcpStream::connect(a);"), 2);
    assert_eq!(n("use trellis_core::askpass::{PromptKind, host};"), 0);
    assert_eq!(n("let s = pty::spawn(&d, 24, 80, &sh, o, e);"), 1);
    assert_eq!(n("let sh = pty::default_shell();"), 1);
    assert_eq!(n("use trellis_core::terminal::{TermScreen, encode};"), 0);
    // Whole families, not only known names.
    assert_eq!(n("let ok = Path::new(&p).is_dir();"), 1);
    assert_eq!(n("if path.exists() {"), 1);
    assert_eq!(n("let real = std::fs::canonicalize(a);"), 3);
    assert_eq!(n("let t = fs::read_to_string(p);"), 1);
    assert_eq!(n("let f = File::open(p);"), 1);
    assert_eq!(n("let home = std::env::var(\"HOME\");"), 2);
    assert_eq!(n("let r = git2::Repository::open(w);"), 2);
    assert_eq!(n("let t = Instant::now();"), 1);
    // Word starts and plain data are fine.
    assert_eq!(n("use crate::state::prefs::PrefsState;"), 0);
    assert_eq!(n("let w = WorkFile::new(p);"), 0);
    assert_eq!(n("use git2::{Oid, RepositoryState};"), 0);
    assert_eq!(n("let o: git2::Oid = git2::Oid::zero();"), 0);
    assert_eq!(n("use git2::{Oid, Repository};"), 1);
    assert_eq!(n("super::repo::load(st, path, LoadKind::Open, fx);"), 0);
}

#[test]
fn reports_name_the_line_and_skip_test_code() {
    let src = "fn a() {}\nfn b() { let _ = p.is_file(); }\n#[cfg(test)]\nmod tests {\n    fn c() { std::fs::write(p, \"\"); }\n}\nfn d() { p.exists(); }\n";
    let lines: Vec<(usize, String)> = violations(src).into_iter().map(|f| (f.line, f.pattern)).collect();
    assert_eq!(lines, [(2, ".is_file()".to_owned()), (7, ".exists()".to_owned())]);
    let v = violations("use std::path::Path;\n\nuse trellis_core::repo::{\n    load_diff,\n};\n");
    assert_eq!(v.iter().map(|f| f.line).collect::<Vec<_>>(), [3]);
    assert!(is_test_file("update/tabs_tests.rs") && is_test_file("ui/snapshot_tests_clone.rs") && is_test_file("update/tests.rs"));
    assert!(!is_test_file("update/tabs.rs"));
    let declared = test_only_modules("mod a;\n#[cfg(test)]\nmod snapshot_check;\n#[cfg(test)]\npub(crate) mod helpers;\n");
    assert_eq!(declared, ["snapshot_check", "helpers"]);
    assert_eq!(child_module("ui/mod.rs", "snapshot_check"), "ui/snapshot_check");
    assert_eq!(child_module("ui/graph.rs", "x"), "ui/graph/x");
}
