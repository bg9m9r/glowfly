//! Ratchets on source hygiene across both crates (`glowfly-app` and `glowfly-core`), so the
//! rules in CLAUDE.md hold without anyone checking by hand, and today's exceptions can only shrink:
//!
//! - A non-test `.rs` file stays under `LINE_LIMIT` lines. The files over it today are listed in
//!   `LONG_FILES` with their current length as a ceiling: they may shrink but not grow, and an
//!   entry is removed once its file is back under the limit.
//! - Every non-test file opens with a `//!` module doc saying what it holds.
//! - The `update/` and `state/` log and toast calls with an English string literal (rather than a
//!   `t.tr`/`trf`/`trn` lookup) number at most `ENGLISH_LOG_CALLS`; the number only goes down.
//! - The shared fixture builders are not written again: each helper in `FIXTURE_HELPERS` is
//!   defined only in the files listed for it, and no two tests pass `scratch_repo` or `long_repo`
//!   the same folder name (tests run in parallel and would share the folder).
//! - View tests build their harnesses with `ui::snapshot_check::builder()`, which lends them
//!   pooled wgpu devices, not with `Harness::builder()` or `Harness::new*` (`HARNESS_MAKERS`).
//!
//! Test code is what `support/views.rs` calls test code: files named `tests.rs`, `*_tests.rs` or
//! `snapshot_tests*.rs`, modules declared under `#[cfg(test)]`, and inline `#[cfg(test)] mod`
//! blocks (blanked out for the log check).

#[path = "support/views.rs"]
mod views;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use views::{app_src, code_lines, crate_sources};

/// The line limit for a non-test file (CLAUDE.md: "Keep files under ~400 lines").
const LINE_LIMIT: usize = 400;

/// The non-test files over `LINE_LIMIT` today, with their length as a ceiling.
const LONG_FILES: &[(&str, usize)] = &[
    ("crates/glowfly-app/src/effect/mod.rs", 422),
    ("crates/glowfly-app/src/runner/guard.rs", 498),
    ("crates/glowfly-app/src/state/mod.rs", 695),
    ("crates/glowfly-app/src/ui/mod.rs", 591),
    ("crates/glowfly-app/src/update/repo.rs", 411),
    ("crates/glowfly-core/src/forge/github.rs", 406),
    ("crates/glowfly-core/src/repo/mod.rs", 514),
    ("crates/glowfly-core/src/settings.rs", 463),
];

/// How many log and toast calls in `update/` and `state/` still pass English text. Lower it when
/// you translate some; never raise it.
const ENGLISH_LOG_CALLS: usize = 180;

/// Fixture builders and the only files that may define them (as `fn <name>(`, or the full
/// signature for `harness`). A new test reuses one of these instead of writing its own copy.
const FIXTURE_HELPERS: &[(&str, &[&str])] = &[
    ("fn git(dir: &Path, a: &[&str])", &["crates/glowfly-core/src/testkit/mod.rs"]),
    ("fn scratch_repo(", &["crates/glowfly-app/src/update/tests.rs", "crates/glowfly-core/src/testutil.rs"]),
    ("fn long_repo(", &["crates/glowfly-app/src/update/tests.rs"]),
    ("fn fixture_repo(", &["crates/glowfly-app/src/ui/snapshot_tests.rs"]),
    (
        "fn with_repo(",
        &[
            "crates/glowfly-app/src/state/menu_model_tests.rs",
            "crates/glowfly-app/src/ui/snapshot_tests.rs",
            "crates/glowfly-app/src/update/lines_tests.rs",
            "crates/glowfly-app/src/update/profile_rules_tests.rs",
            "crates/glowfly-app/src/update/shortcuts_tests.rs",
        ],
    ),
    (
        "fn harness(st: AppState, field: bool)",
        &[
            "crates/glowfly-app/src/ui/shortcuts/command_keys_tests.rs",
            "crates/glowfly-app/src/ui/shortcuts/fold_key_tests.rs",
            "crates/glowfly-app/src/ui/shortcuts/history_tests.rs",
            "crates/glowfly-app/src/ui/shortcuts/layout_tests.rs",
            "crates/glowfly-app/src/ui/shortcuts/rebound_tests.rs",
            "crates/glowfly-app/src/ui/shortcuts/select_tests.rs",
            "crates/glowfly-app/src/ui/shortcuts/tests.rs",
        ],
    ),
];

/// The files that may build an egui_kittest harness directly: the shared builder itself, and
/// the journeys' harness, which never renders.
const HARNESS_MAKERS: &[&str] = &["crates/glowfly-app/src/ui/snapshot_check.rs", "crates/glowfly-app/src/journey/harness.rs"];

/// One source file: its path from the repository root (with `/`), its text, and whether it is
/// test code.
struct Source {
    path: String,
    text: String,
    test: bool,
}

/// Every `.rs` file under both crates' `src/`.
fn all_sources() -> Vec<Source> {
    let crates = app_src().parent().and_then(|app| app.parent()).map(PathBuf::from).expect("the crates folder");
    let mut out = Vec::new();
    for name in ["glowfly-app", "glowfly-core"] {
        for (rel, text, test) in crate_sources(&crates.join(name).join("src")) {
            out.push(Source { path: format!("crates/{name}/src/{rel}"), text, test });
        }
    }
    assert!(out.len() > 500, "found the sources of both crates");
    out
}

/// The problem with a file of `lines` lines at `path`, if any, against `LONG_FILES`.
fn length_problem(path: &str, lines: usize) -> Option<String> {
    match LONG_FILES.iter().find(|(p, _)| *p == path) {
        Some((_, ceiling)) if lines > *ceiling => Some(format!(
            "{path}:{lines}: {lines} lines, over its ceiling of {ceiling} in LONG_FILES; split it by responsibility rather than growing it"
        )),
        Some(_) if lines <= LINE_LIMIT => {
            Some(format!("{path}: {lines} lines, back under {LINE_LIMIT}; remove it from LONG_FILES in tests/hygiene.rs"))
        }
        Some(_) => None,
        None if lines > LINE_LIMIT => Some(format!(
            "{path}:{lines}: {lines} lines, over the {LINE_LIMIT}-line limit; split it by responsibility (move a part into a sibling module)"
        )),
        None => None,
    }
}

#[test]
fn source_files_stay_under_the_line_limit() {
    let sources = all_sources();
    let mut problems = Vec::new();
    for (path, _) in LONG_FILES {
        if !sources.iter().any(|s| s.path == *path && !s.test) {
            problems.push(format!("{path}: in LONG_FILES but no longer a source file; remove the entry"));
        }
    }
    for s in sources.iter().filter(|s| !s.test) {
        problems.extend(length_problem(&s.path, s.text.lines().count()));
    }
    assert!(problems.is_empty(), "files over the line limit:\n{}", problems.join("\n"));
}

#[test]
fn source_files_open_with_a_module_doc() {
    let missing: Vec<String> = all_sources()
        .iter()
        .filter(|s| !s.test)
        .filter(|s| !s.text.lines().find(|l| !l.trim().is_empty()).is_some_and(|l| l.starts_with("//!")))
        .map(|s| format!("{}:1: no `//!` module doc; start the file with one saying what it holds", s.path))
        .collect();
    assert!(missing.is_empty(), "source files without a module doc:\n{}", missing.join("\n"));
}

/// The string literals in `text` from byte `start` up to the `)` that closes the call there.
fn call_literals(text: &str, start: usize) -> Vec<String> {
    let bytes = text.as_bytes();
    let (mut i, mut depth, mut out) = (start, 1usize, Vec::new());
    while i < bytes.len() && depth > 0 {
        match bytes[i] {
            b'"' => {
                let mut lit = String::new();
                let mut j = i + 1;
                while j < bytes.len() && bytes[j] != b'"' {
                    let step = if bytes[j] == b'\\' { 2 } else { 1 };
                    lit.push_str(text.get(j..j + step).unwrap_or(""));
                    j += step;
                }
                out.push(lit);
                i = j + 1;
                continue;
            }
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    out
}

/// Whether a literal is English text for the user, not a translation key or a bare format
/// string: it has a word of two letters outside its `{…}` placeholders.
fn is_english(lit: &str) -> bool {
    if lit.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_.-".contains(c)) {
        return false;
    }
    let mut outside = String::new();
    let mut depth = 0;
    for c in lit.chars() {
        match c {
            '{' => depth += 1,
            '}' if depth > 0 => depth -= 1,
            _ if depth == 0 => outside.push(c),
            _ => {}
        }
    }
    outside.as_bytes().windows(2).any(|w| w[0].is_ascii_alphabetic() && w[1].is_ascii_alphabetic())
}

/// The lines (1-based) in `src` where a log or toast call passes English text.
fn english_log_calls(src: &str) -> Vec<usize> {
    let code = code_lines(src).join("\n");
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(off) = code[at..].find('.') {
        let dot = at + off;
        at = dot + 1;
        let rest = &code[at..];
        let Some(name) = ["log", "toast"].into_iter().find(|n| rest.starts_with(n)) else { continue };
        let Some(paren) = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) else { continue };
        if rest.as_bytes()[paren] != b'(' || !rest[name.len()..paren].chars().all(|c| c.is_ascii_lowercase() || c == '_') {
            continue;
        }
        if call_literals(&code, at + paren + 1).iter().any(|l| is_english(l)) {
            found.push(code[..dot].matches('\n').count() + 1);
        }
    }
    found
}

#[test]
fn log_and_toast_text_is_translated() {
    let mut sites = Vec::new();
    for s in all_sources().iter().filter(|s| !s.test) {
        let area = s.path.strip_prefix("crates/glowfly-app/src/").unwrap_or("");
        if area.starts_with("update/") || area.starts_with("state/") {
            sites.extend(english_log_calls(&s.text).into_iter().map(|line| format!("{}:{line}", s.path)));
        }
    }
    assert!(
        sites.len() <= ENGLISH_LOG_CALLS,
        "{} log/toast calls in update/ and state/ pass English text, more than ENGLISH_LOG_CALLS ({ENGLISH_LOG_CALLS}). \
         Translate the new message instead: add a key to crates/glowfly-core/locales/*.json and pass t.tr(\"key\"), \
         t.trf(\"key\", &[(\"name\", value)]) or t.trn(\"key\", n, &[]). The calls:\n{}",
        sites.len(),
        sites.join("\n")
    );
    assert!(
        sites.len() == ENGLISH_LOG_CALLS,
        "only {} log/toast calls in update/ and state/ pass English text now: lower ENGLISH_LOG_CALLS in tests/hygiene.rs to {}",
        sites.len(),
        sites.len()
    );
}

/// The fixture folder names passed as a literal to `scratch_repo` or `long_repo` in `src`, with
/// the line (1-based) and the function they are in.
fn fixture_names(src: &str) -> Vec<(String, usize, String)> {
    let mut out = Vec::new();
    let mut func = String::new();
    for (i, line) in code_lines(src).iter().enumerate() {
        if let Some(rest) = line.split("fn ").nth(1) {
            func = rest.split(|c: char| !(c.is_alphanumeric() || c == '_')).next().unwrap_or("").to_owned();
        }
        for helper in ["scratch_repo(\"", "long_repo(\""] {
            for (pos, _) in line.match_indices(helper) {
                let word_start = pos == 0 || !line[..pos].ends_with(|c: char| c.is_alphanumeric() || c == '_');
                let name = line[pos + helper.len()..].split('"').next().unwrap_or("");
                if word_start {
                    out.push((name.to_owned(), i + 1, func.clone()));
                }
            }
        }
    }
    out
}

#[test]
fn fixture_helpers_are_shared_not_copied() {
    let sources = all_sources();
    let mut problems = Vec::new();
    for (signature, allowed) in FIXTURE_HELPERS {
        for s in &sources {
            for (i, line) in s.text.lines().enumerate() {
                let defines = line.find(signature).is_some_and(|p| p == 0 || line[..p].ends_with(' '));
                if defines && !allowed.contains(&s.path.as_str()) {
                    problems.push(format!(
                        "{}:{}: defines `{signature}` again; reuse the one in {} (make it pub(crate) if needed) \
                         or give a helper that differs its own name",
                        s.path,
                        i + 1,
                        allowed.join(" or ")
                    ));
                }
            }
        }
        for path in *allowed {
            if !sources.iter().any(|s| s.path == *path && s.text.contains(signature)) {
                problems.push(format!("{path}: no longer defines `{signature}`; drop it from FIXTURE_HELPERS"));
            }
        }
    }
    for krate in ["crates/glowfly-app/", "crates/glowfly-core/"] {
        let mut uses: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for s in sources.iter().filter(|s| s.path.starts_with(krate)) {
            for (name, line, func) in fixture_names(&s.text) {
                uses.entry(name).or_default().insert(format!("{}:{line} (in {func})", s.path));
            }
        }
        for (name, at) in &uses {
            let funcs: BTreeSet<&str> = at.iter().filter_map(|a| a.split(" (in ").nth(1)).collect();
            let files: BTreeSet<&str> = at.iter().filter_map(|a| a.split(':').next()).collect();
            if funcs.len() > 1 || files.len() > 1 {
                problems.push(format!(
                    "fixture folder \"{name}\" is used by several tests, which run in parallel and would share it; give each its own name:\n  {}",
                    at.iter().cloned().collect::<Vec<_>>().join("\n  ")
                ));
            }
        }
    }
    assert!(problems.is_empty(), "duplicated fixture helpers:\n{}", problems.join("\n"));
}

#[test]
fn harnesses_come_from_the_shared_builder() {
    let mut problems = Vec::new();
    for s in all_sources().iter().filter(|s| !HARNESS_MAKERS.contains(&s.path.as_str())) {
        for (i, line) in code_lines(&s.text).iter().enumerate() {
            if line.contains("Harness::builder(") || line.contains("Harness::new") {
                problems.push(format!("{}:{}: build the harness with crate::ui::snapshot_check::builder()", s.path, i + 1));
            }
        }
    }
    for path in HARNESS_MAKERS {
        if !all_sources().iter().any(|s| s.path == *path && s.text.contains("Harness::builder(")) {
            problems.push(format!("{path}: no longer builds a harness; drop it from HARNESS_MAKERS"));
        }
    }
    assert!(problems.is_empty(), "harnesses built without the shared builder:\n{}", problems.join("\n"));
}

#[test]
fn the_checks_catch_what_they_should() {
    assert_eq!(length_problem("crates/x.rs", LINE_LIMIT), None);
    assert!(length_problem("crates/x.rs", LINE_LIMIT + 1).unwrap().starts_with("crates/x.rs:401:"));
    let (long, ceiling) = LONG_FILES[0];
    assert_eq!(length_problem(long, ceiling), None, "at its ceiling");
    assert!(length_problem(long, ceiling + 1).unwrap().contains("over its ceiling"));
    assert!(length_problem(long, 10).unwrap().contains("remove it from LONG_FILES"));

    let src = "fn a(st: &mut S) {\n    st.log(false, t.tr(\"repo.gone\"));\n    st.toast(true, format!(\"Copied {what}\"), now);\n    \
               st.log_quiet(\n        true,\n        \"Saved.\",\n    );\n    st.log(false, format!(\"{e}\"));\n    \
               st.logged(\"x y\");\n    // st.log(false, \"Hidden in a comment\");\n}\n#[cfg(test)]\nmod tests {\n    \
               fn b() { st.log(false, \"Test text\"); }\n}\n";
    assert_eq!(english_log_calls(src), [3, 4], "English text, also across lines; not keys, placeholders, comments or tests");
    assert!(!is_english("commands.run.gone") && !is_english("{name}: {e}") && !is_english("s"));
    assert!(is_english("Branch {name} no longer exists.") && is_english("Saved"));

    let names = fixture_names(
        "fn one() {\n    let d = scratch_repo(\"one\");\n}\nfn two() {\n    long_repo(\"two\", 3);\n    my_scratch_repo(\"no\");\n}\n",
    );
    assert_eq!(names, [("one".into(), 2, "one".into()), ("two".into(), 5, "two".into())]);
}
