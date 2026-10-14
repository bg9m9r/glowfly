//! Keeps views from doing slow or outside work themselves. Everything under `src/ui/` draws
//! `AppState` and pushes `Msg`s; git, network, files, processes and threads belong in
//! `trellis-core` and run through `Effect`s in `runner.rs`. Views may use core *types* and pure
//! helpers (e.g. `gitcli::args`, `forge::parse_host`), so this checks for the functions that do
//! I/O rather than for any use of a module.
//!
//! The shell's startup in `ui/mod.rs` (loading settings, recent repos and tabs) and the snapshot
//! tests are exempt.

use std::path::{Path, PathBuf};

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
            "load_diff",
            "load_diff_with",
            "apply_hunk",
            "apply_lines",
            "stage_file",
            "stage_all",
            "unstage_paths",
            "discard_file",
            "load_conflict",
            "file_history",
            "write_resolution",
            "resolve_with_side",
        ],
    ),
    ("gitcli", &["run", "run_with_input"]),
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
    ("signing", &["list_gpg_keys", "list_ssh_keys", "generate_gpg_key", "export_public_key"]),
    ("undo", &["before", "after", "plan", "redone"]),
    ("platform", &["load_recent", "save_recent", "load_tabs", "save_tabs", "open_terminal", "open_in_file_manager"]),
];

/// Ways to reach the outside world without going through core at all.
const RAW_IO: &[&str] = &[
    "std::thread",
    "thread::spawn",
    "std::process",
    "Command::new",
    "std::fs",
    "ureq::",
    "keyring::",
    "Repository::",
    "Settings::load",
    ".save()",
];

const EXEMPT: &[&str] = &[
    "mod.rs",
    "snapshot_tests.rs",
    "snapshot_tests_diff.rs",
    "snapshot_tests_revert.rs",
    "snapshot_tests_select.rs",
    "snapshot_tests_whole.rs",
];

/// Problems in one file's source, as "what: line".
fn violations(src: &str) -> Vec<String> {
    let code: Vec<&str> = src.lines().map(|l| l.split("//").next().unwrap_or("")).collect();
    let mut found = Vec::new();
    for (i, line) in code.iter().enumerate() {
        for pat in RAW_IO {
            if line.contains(pat) {
                found.push(format!("`{pat}` on line {}", i + 1));
            }
        }
        for (module, names) in IO_FUNCTIONS {
            for name in *names {
                if line.contains(&format!("{module}::{name}(")) {
                    found.push(format!("`{module}::{name}` on line {}", i + 1));
                }
            }
        }
    }
    // Imported by name, e.g. `use trellis_core::repo::{load_diff, DiffTarget};`.
    let joined = code.join("\n");
    for stmt in joined.split(';').filter_map(|s| s.trim().strip_prefix("use ")) {
        for (module, names) in IO_FUNCTIONS {
            let Some(rest) = stmt.split_once(&format!("{module}::")).map(|(_, r)| r) else { continue };
            let words: Vec<&str> = rest.split(|c: char| !c.is_alphanumeric() && c != '_').collect();
            for name in *names {
                if words.contains(name) {
                    found.push(format!("`use {module}::{name}`"));
                }
            }
        }
    }
    found
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
fn views_do_no_io() {
    let ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
    let mut files = Vec::new();
    rust_files(&ui, &mut files);
    assert!(files.len() > 10, "found the views");
    let mut report = Vec::new();
    for file in files {
        let rel = file.strip_prefix(&ui).unwrap().to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
        if EXEMPT.contains(&rel.as_str()) {
            continue;
        }
        for v in violations(&std::fs::read_to_string(&file).unwrap()) {
            report.push(format!("ui/{rel}: {v}"));
        }
    }
    assert!(report.is_empty(), "views must push a Msg and let update/runner do this work:\n{}", report.join("\n"));
}

#[test]
fn the_check_catches_io() {
    assert!(violations("let d = repo::load_diff(&w, &t);").len() == 1);
    assert!(violations("use trellis_core::repo::{DiffTarget,\n    load_diff};").len() == 1);
    assert!(violations("std::thread::spawn(move || {});").len() == 2);
    assert!(violations("let a = args(&[\"tag\"]); // repo::load(here) is only a comment").is_empty());
    assert!(violations("use trellis_core::repo::{self, DiffTarget, Snapshot};").is_empty());
}
