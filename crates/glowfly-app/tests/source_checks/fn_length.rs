//! A ratchet on function length. Clippy's `too_many_lines` (turned on for every crate in the
//! workspace `Cargo.toml`, its threshold in `clippy.toml`) fails a function whose body has more
//! than `THRESHOLD` code lines, test functions too. The longer functions of today carry
//! `#[expect(clippy::too_many_lines, reason = "...")]`, never `allow`: once a marked function
//! is split below the threshold its expectation goes unfulfilled, which fails clippy too, so the
//! mark has to go. Each mark sits on a function and is listed in `LONG_FUNCTIONS`, which only
//! shrinks: a new long function is split by responsibility rather than marked. Nothing may turn
//! the lint off for a module or a crate (an inner `#![..]` mark, a `clippy::pedantic` allow).

use crate::lexer::blanked;
use crate::views;
use std::path::{Path, PathBuf};

/// The most code lines a function may have (`too-many-lines-threshold` in `clippy.toml`). Lower
/// it when the functions allow; never raise it.
const THRESHOLD: usize = 120;

/// The functions marked with an `expect` of the lint today, as `<path from the repo root>: <fn
/// name>`, sorted. Only shrinks: split one, drop its mark and its entry. Renaming or moving a
/// marked function means editing its entry, never adding one.
const LONG_FUNCTIONS: &[&str] = &[
    "crates/glowfly-core/src/trust/pull_run.rs: run",
    "crates/glowfly-core/src/trust/targets.rs: targets",
    "crates/glowfly-core/src/undo/planning.rs: plan_with",
    "crates/glowfly-model/src/state/palette/recent.rs: recall_key",
    "crates/glowfly-model/src/update/forge.rs: replied",
    "crates/glowfly-model/src/update/palette.rs: run",
    "crates/glowfly-ui/src/icons.rs: paint",
    "crates/glowfly-ui/src/prefs/mod.rs: show",
    "crates/glowfly-ui/src/prefs/signing.rs: signing_page",
    "crates/glowfly-ui/src/shortcuts/global.rs: fire",
    "crates/glowfly-ui/src/sidebar.rs: sections",
    "crates/glowfly-ui/src/tabs.rs: strip",
    "crates/glowfly-ui/src/widgets/icon.rs: paint_icon_stroke",
];

/// The lint's name, as code spells it.
const LINT: &str = "too_many_lines";

/// The workspace root, from this crate's `src` folder read at run time (`views::app_src`).
fn root() -> PathBuf {
    let src = views::app_src();
    src.ancestors().nth(3).expect("the workspace root above crates/glowfly-app/src").to_path_buf()
}

/// Every `.rs` file under `dir`, skipping snapshot and build folders.
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

/// Every crate's `.rs` files (`src/`, `tests/`, `benches/`), as (path from the root with `/`,
/// text).
fn sources() -> Vec<(String, String)> {
    let root = root();
    let mut files = Vec::new();
    rust_files(&root.join("crates"), &mut files);
    let rel = |f: &Path| f.strip_prefix(&root).unwrap().to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
    let out: Vec<_> = files.iter().map(|f| (rel(f), std::fs::read_to_string(f).unwrap())).collect();
    assert!(out.len() > 500, "found the crates' sources under {}", root.display());
    out
}

/// What a file's text says about the lint: the functions it marks, and the problems with its
/// marks (`line N: ...`).
#[derive(Debug, Default, PartialEq)]
struct Scan {
    marked: Vec<String>,
    problems: Vec<String>,
}

/// Where the bracket opened at `open` closes in `b`, or the end.
fn closing(b: &str, open: usize) -> usize {
    let mut depth = 0usize;
    for (i, c) in b[open..].char_indices() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => {
                depth -= 1;
                if depth == 0 {
                    return open + i;
                }
            }
            _ => {}
        }
    }
    b.len()
}

/// The name of the function `rest` starts with (after any `pub(..)`, `const`, `async`, `unsafe`
/// or `extern ".."`), with its own attributes, comments and blank lines before it skipped.
fn fn_name(mut rest: &str) -> Option<&str> {
    loop {
        rest = rest.trim_start();
        if rest.starts_with("#[") {
            rest = &rest[closing(rest, 1) + 1..];
        } else if rest.starts_with("pub(") {
            rest = &rest[closing(rest, 3) + 1..];
        } else if rest.starts_with('"') {
            rest = &rest[1 + rest[1..].find('"')? + 1..];
        } else if let Some(r) = ["pub", "const", "async", "unsafe", "extern"]
            .iter()
            .find_map(|kw| rest.strip_prefix(kw).filter(|r| r.starts_with(char::is_whitespace)))
        {
            rest = r;
        } else {
            break;
        }
    }
    let name = rest.strip_prefix("fn").filter(|r| r.starts_with(char::is_whitespace))?.trim_start();
    let end = name.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(name.len());
    (end > 0).then(|| &name[..end])
}

/// Reads the marks of the lint in `src`, outside comments and literals.
fn scan(src: &str) -> Scan {
    let b = blanked(src, false);
    let line = |at: usize| b[..at].matches('\n').count() + 1;
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = Scan::default();
    for (at, _) in b.match_indices(LINT) {
        let whole_word = !b[..at].ends_with(ident) && !b[at + LINT.len()..].starts_with(ident);
        let Some(hash) = b[..at].rfind('#').filter(|_| whole_word) else { continue };
        let open = hash + b[hash..].find('[').unwrap_or(0);
        let end = closing(&b, open);
        let attr: String = b[hash..=end.min(b.len() - 1)].chars().filter(|c| !c.is_whitespace()).collect();
        let n = line(at);
        let problem = if attr.starts_with("#!") {
            "an inner attribute names the lint; marks go on functions, not on modules or crates"
        } else if attr.starts_with("#[allow(") {
            "allows the lint; use #[expect(clippy::too_many_lines, reason = \"...\")] so the mark fails once the function is short"
        } else if !attr.starts_with(&format!("#[expect(clippy::{LINT}")) {
            "names the lint in an attribute that isn't #[expect(clippy::too_many_lines, reason = \"...\")]"
        } else if !attr.contains("reason=") {
            "the expect has no reason; say why the function is long and how to split it"
        } else if let Some(name) = fn_name(&b[end + 1..]) {
            out.marked.push(name.to_owned());
            continue;
        } else {
            "the mark isn't on a function"
        };
        out.problems.push(format!("line {n}: {problem}"));
    }
    let compact: String = b.chars().filter(|c| !c.is_whitespace()).collect();
    if ["allow(clippy::pedantic", "expect(clippy::pedantic"].iter().any(|p| compact.contains(p)) {
        out.problems.push("turns clippy::pedantic off, which silences the length cap".to_owned());
    }
    out
}

/// The `key = value` lines (comments dropped, spaces removed) of the TOML table `[name]`.
fn table(toml: &str, name: &str) -> Option<Vec<String>> {
    let mut lines = toml.lines().map(|l| l.split('#').next().unwrap_or("").replace(' ', ""));
    lines.find(|l| *l == format!("[{name}]"))?;
    Some(lines.take_while(|l| !l.starts_with('[')).filter(|l| !l.is_empty()).collect())
}

#[test]
fn clippy_caps_function_length() {
    let root = root();
    let cargo = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    let lints = table(&cargo, "workspace.lints.clippy").expect("Cargo.toml has [workspace.lints.clippy]");
    assert!(
        lints.iter().any(|l| *l == format!("{LINT}=\"warn\"")),
        "[workspace.lints.clippy] keeps {LINT} = \"warn\" (clippy runs with -D warnings)"
    );
    let clippy = std::fs::read_to_string(root.join("clippy.toml")).unwrap();
    let threshold: usize = clippy
        .lines()
        .find_map(|l| l.split('#').next().unwrap().split_once('=').filter(|(k, _)| k.trim() == "too-many-lines-threshold"))
        .and_then(|(_, v)| v.trim().parse().ok())
        .expect("clippy.toml sets too-many-lines-threshold to a number");
    assert!(threshold <= THRESHOLD, "too-many-lines-threshold is {threshold}, over {THRESHOLD}: split the function instead");
    let mut crates = 0;
    for dir in std::fs::read_dir(root.join("crates")).unwrap().flatten().map(|e| e.path()) {
        let Ok(manifest) = std::fs::read_to_string(dir.join("Cargo.toml")) else { continue };
        crates += 1;
        let inherits = table(&manifest, "lints").is_some_and(|l| l.iter().any(|l| l == "workspace=true"));
        assert!(inherits, "{}/Cargo.toml inherits the workspace lints ([lints] workspace = true)", dir.display());
    }
    assert!(crates >= 6, "found the crates' manifests");
}

#[test]
fn long_functions_only_shrink() {
    assert!(LONG_FUNCTIONS.is_sorted(), "LONG_FUNCTIONS stays sorted");
    let mut problems = Vec::new();
    let mut found = Vec::new();
    for (path, text) in sources() {
        let s = scan(&text);
        problems.extend(s.problems.iter().map(|p| format!("{path}: {p}")));
        for name in s.marked {
            let entry = format!("{path}: {name}");
            if !LONG_FUNCTIONS.contains(&entry.as_str()) {
                problems.push(format!(
                    "{path}: {name} is marked with expect(clippy::{LINT}) but not in LONG_FUNCTIONS; split it by responsibility instead"
                ));
            }
            found.push(entry);
        }
    }
    for entry in LONG_FUNCTIONS.iter().filter(|e| !found.iter().any(|f| f == *e)) {
        problems.push(format!("{entry}: no longer marked; remove it from LONG_FUNCTIONS (tests/source_checks/fn_length.rs)"));
    }
    assert!(problems.is_empty(), "function length:\n{}", problems.join("\n"));
}

#[test]
fn a_mark_on_a_function_names_it() {
    let one_line = "#[expect(clippy::too_many_lines, reason = \"x\")]\npub(super) fn f(a: u8) {}\n";
    assert_eq!(scan(one_line), Scan { marked: vec!["f".into()], problems: vec![] });
    let wrapped = "    #[expect(\n        clippy::too_many_lines,\n        reason = \"a long one\"\n    )]\n    pub fn g() {}\n";
    assert_eq!(scan(wrapped).marked, ["g"]);
    let between =
        "#[expect(clippy::too_many_lines, reason = \"x\")]\n/// Docs.\n#[must_use]\n\npub(in crate::a) const fn h() -> u8 { 1 }\n";
    assert_eq!(scan(between).marked, ["h"]);
}

#[test]
fn the_lint_named_in_text_is_no_mark() {
    let src = "// clippy::too_many_lines\nlet s = \"#[expect(clippy::too_many_lines, reason = \\\"x\\\")]\";\n/* too_many_lines */\n";
    assert_eq!(scan(src), Scan::default());
}

#[test]
fn marks_that_are_not_expects_on_functions_are_reported() {
    for src in [
        "#[allow(clippy::too_many_lines)]\nfn a() {}\n",
        "#![expect(clippy::too_many_lines, reason = \"x\")]\n",
        "#[expect(clippy::too_many_lines)]\nfn b() {}\n",
        "#[expect(clippy::too_many_lines, reason = \"x\")]\nmod x;\n",
        "#[cfg_attr(test, expect(clippy::too_many_lines, reason = \"x\"))]\nfn c() {}\n",
        "#![allow(clippy::pedantic)]\n",
        "#[allow(clippy::pedantic)]\nfn d() {}\n",
    ] {
        let s = scan(src);
        assert!(s.marked.is_empty() && s.problems.len() == 1, "{src:?} gives one problem: {s:?}");
    }
}
