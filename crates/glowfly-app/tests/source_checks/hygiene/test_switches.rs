//! The hygiene ratchet on `cfg(test)` behaviour switches in the non-test code of every crate but
//! glowfly-core (see hygiene.rs).

use super::{all_sources, code_lines};

/// The files whose non-test code may switch on `cfg(test)`: the shell, which stays in the
/// binary, whose own unit tests (the journeys) are what it switches for. Only shrinks.
const TEST_SWITCH_FILES: &[&str] = &["crates/glowfly-app/src/shell/app.rs"];

/// The lines (1-based) of `src`'s code that switch on `cfg(test)`: a `cfg!` naming `test`, a
/// `cfg` with `not(…)` around `test` (`cfg(not(test))`, `cfg(all(not(test), …))`; lint
/// attributes aside), and a `cfg` naming `test` (`#[cfg(test)]`, `#[cfg(any(test, …))]`) on
/// anything but an item.
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
        let line = &without_strings(line);
        if let Some(at) = line.find("cfg!(")
            && names_test(&inside(line, at + 4))
        {
            found.push(i + 1);
        } else if let Some(at) = line.find("#[cfg(") {
            let pred = inside(line, at + 5);
            if !names_test(&pred) {
                continue;
            }
            if negates_test(&pred) {
                found.push(i + 1);
                continue;
            }
            // The item it is on: the rest of the line after the attribute, else the next line
            // that isn't another attribute.
            let rest = line[at..].split_once("]").map_or("", |(_, r)| r);
            let next = if rest.trim().is_empty() {
                code[i + 1..].iter().map(String::as_str).find(|l| !l.trim().is_empty() && !l.trim_start().starts_with("#["))
            } else {
                Some(rest)
            };
            if !next.is_some_and(item) {
                found.push(i + 1);
            }
        }
    }
    found
}

/// The predicate inside the parentheses that open at byte `open` of `line` (to the matching
/// `)`, or the line's end), with its string literals (`feature = "test-support"`) emptied.
fn inside(line: &str, open: usize) -> String {
    let mut depth = 0;
    let mut end = line.len();
    for (k, c) in line[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = open + k;
                    break;
                }
            }
            _ => {}
        }
    }
    let pred = line.get(open + 1..end).unwrap_or("");
    pred.split('"').step_by(2).collect::<Vec<_>>().join("\"\"")
}

/// `line` with the text of its string literals taken out (a `cfg` in a string is not code).
fn without_strings(line: &str) -> String {
    let (mut out, mut in_string, mut escaped) = (String::new(), false, false);
    for c in line.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
                out.push(c);
            }
        } else {
            in_string = c == '"';
            out.push(c);
        }
    }
    out
}

/// Whether the predicate names `test` (as a word).
fn names_test(pred: &str) -> bool {
    pred.split(|c: char| !(c.is_alphanumeric() || c == '_')).any(|w| w == "test")
}

/// Whether the predicate has a `not(…)` around `test`.
fn negates_test(pred: &str) -> bool {
    pred.match_indices("not(").any(|(k, _)| names_test(&inside(pred, k + 3)))
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
    let wider = "#[cfg(all(not(test), unix))]
const A: u8 = 1;
const B: bool = cfg!(any(test, unix));
struct S {
    #[cfg(any(test, feature = \"test-support\"))]
    c: u8,
}
                 #[cfg(any(test, feature = \"test-support\"))]
pub fn d() {}
#[cfg(feature = \"test-support\")]
let e = 1;
#[cfg(test)] fn f() {}
const G: &str = \"#[cfg(not(test))]\";
";
    assert_eq!(
        test_switches(wider),
        [1, 3, 5],
        "all(not(test)), cfg!(any(test)) and any(test) on a field; not test-support items or strings"
    );
}
