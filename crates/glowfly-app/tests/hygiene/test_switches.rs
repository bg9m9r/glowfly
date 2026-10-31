//! The hygiene ratchet on `cfg(test)` behaviour switches in the non-test code of every crate but
//! glowfly-core (see hygiene.rs).

use super::{all_sources, code_lines};

/// The files whose non-test code may switch on `cfg(test)`: the shell, which stays in the
/// binary, whose own unit tests (the journeys) are what it switches for. Only shrinks.
const TEST_SWITCH_FILES: &[&str] = &["crates/glowfly-app/src/shell/app.rs"];

/// The lines (1-based) of `src`'s code that switch on `cfg(test)`: `cfg!(test)`, a
/// `cfg(not(test))` (lint attributes aside), and `#[cfg(test)]` on anything but an item.
fn test_switches(src: &str) -> Vec<usize> {
    const ITEMS: &[&str] = &["fn ", "mod ", "use ", "impl", "struct ", "enum ", "const ", "static ", "type ", "trait ", "macro_rules!"];
    let code = code_lines(src);
    let item = |line: &str| {
        let l = line.trim_start();
        // `pub`, `pub(crate)`, `pub(in crate::ui)` and the like.
        let l = l.strip_prefix("pub").map(|r| r.strip_prefix('(').and_then(|r| r.split_once(')')).map_or(r, |(_, rest)| rest)).unwrap_or(l);
        let l = l.trim_start();
        ITEMS.iter().any(|i| l.starts_with(i))
    };
    let mut found = Vec::new();
    for (i, line) in code.iter().enumerate() {
        let negated = line.contains("cfg(not(test))") || line.contains("cfg(not(any(test");
        if line.contains("cfg!(test)") || line.contains("cfg!(not(test))") || (negated && !line.contains("cfg_attr(")) {
            found.push(i + 1);
        } else if line.trim() == "#[cfg(test)]" {
            let next = code[i + 1..].iter().find(|l| !l.trim().is_empty() && !l.trim_start().starts_with("#["));
            if !next.is_some_and(|l| item(l)) {
                found.push(i + 1);
            }
        }
    }
    found
}

#[test]
fn non_test_code_does_not_switch_on_cfg_test() {
    let sources = all_sources();
    let mut problems = Vec::new();
    for s in sources.iter().filter(|s| !s.test && !s.path.starts_with("crates/glowfly-core/src/")) {
        let lines = test_switches(&s.text);
        if TEST_SWITCH_FILES.contains(&s.path.as_str()) {
            if lines.is_empty() {
                problems.push(format!("{}: no cfg(test) switch left; drop it from TEST_SWITCH_FILES", s.path));
            }
            continue;
        }
        problems.extend(lines.iter().map(|l| {
            format!("{}:{l}: behaves differently under cfg(test); pass the test's stand-in in instead (runner::testing)", s.path)
        }));
    }
    for path in TEST_SWITCH_FILES {
        if !sources.iter().any(|s| s.path == *path) {
            problems.push(format!("{path}: in TEST_SWITCH_FILES but no longer a source file; remove the entry"));
        }
    }
    assert!(problems.is_empty(), "cfg(test) behaviour switches:\n{}", problems.join("\n"));
}

#[test]
fn the_switch_check_catches_what_it_should() {
    let src = "const A: u8 = if cfg!(test) { 1 } else { 2 };\n#[cfg(not(test))]\nconst B: u8 = 2;\n#[cfg(test)]\npub(crate) fn c() {}\n\
               #[cfg(test)]\npub(in crate::ui) use c as i;\nstruct S {\n    #[cfg(test)]\n    d: u8,\n}\nfn e() {\n    #[cfg(test)]\n    #[allow(unused)]\n    let f = 1;\n}\n\
               #[cfg_attr(not(test), allow(dead_code))]\nstruct G;\n#[cfg(test)]\nmod tests {\n    const H: bool = cfg!(test);\n}\n";
    assert_eq!(test_switches(src), [1, 2, 9, 13], "switches on values, fields and statements; not test items, lint attributes or tests");
}
