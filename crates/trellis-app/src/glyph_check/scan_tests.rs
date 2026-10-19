use super::*;

fn found(src: &str) -> Vec<(usize, char)> {
    rust_literal_chars(src).into_iter().map(|f| (f.line, f.ch)).collect()
}

#[test]
fn finds_characters_in_strings() {
    // The source text holds the arrow itself here.
    assert_eq!(found("let s = \"a\u{2191}b\";"), [(1, '\u{2191}')]);
}

#[test]
fn decodes_unicode_escapes_but_not_an_escaped_backslash() {
    assert_eq!(found(r#"let s = "x\u{2193}";"#), [(1, '\u{2193}')]);
    assert_eq!(found(r#"let s = "\\u{2193}";"#), []);
    assert_eq!(found(r#"let s = "\"\n\x41\\"; let t = "\u{e9}";"#), [(1, '\u{e9}')]);
}

#[test]
fn reads_raw_strings_and_keeps_later_line_numbers() {
    assert_eq!(found("let s = r#\"x \"\u{2192}\" y\"#;"), [(1, '\u{2192}')]);
    let src = "let a = \"one\ntwo\";\nlet b = r\"three\nfour\";\nlet c = \"\u{2026}\";\n";
    assert_eq!(found(src), [(5, '\u{2026}')]);
    // A string continued with a backslash at the end of a line.
    assert_eq!(found("let a = \"x\\\n  y\";\nlet b = \"\u{b7}\";"), [(3, '\u{b7}')]);
    // A raw identifier is not a raw string.
    assert_eq!(found("let r#type = 1; let s = \"\u{e9}\";"), [(1, '\u{e9}')]);
}

#[test]
fn skips_byte_strings() {
    assert_eq!(found("let a = b\"\\xff\"; let b = br\"x\"; let c = b'\\''; let d = \"\u{e9}\";"), [(1, '\u{e9}')]);
}

#[test]
fn finds_char_literals_but_not_lifetimes() {
    assert_eq!(found("let c = '\u{2026}';"), [(1, '\u{2026}')]);
    assert_eq!(found(r"let c = '\u{22EF}';"), [(1, '\u{22EF}')]);
    assert_eq!(found("fn f<'a>(x: &'a str) -> &'a str { \"\u{e9}\" }"), [(1, '\u{e9}')]);
    // A quote in a char literal does not start a string.
    assert_eq!(found("let q = '\"'; let e = '\\''; let s = \"\u{b7}\";"), [(1, '\u{b7}')]);
    assert_eq!(found("'outer: loop { break 'outer; } let s = \"\u{b7}\";"), [(1, '\u{b7}')]);
}

#[test]
fn skips_comments() {
    assert_eq!(
        found("// \"\u{2191}\"\n/// \u{2191}\n//! '\u{2191}'\n/* \u{2191} /* \u{2193} */ \"\u{2191}\" */ let s = \"\u{b7}\";"),
        [(4, '\u{b7}')]
    );
}

#[test]
fn skips_test_only_items() {
    let src = [
        "#[cfg(test)]",
        "mod tests {",
        "    fn f() { \"\u{2191}\"; }",
        "}",
        "#[cfg(test)]",
        "#[allow(dead_code)]",
        "fn helper() -> &'static str {",
        "    \"\u{2193}\"",
        "}",
        "#[cfg(test)] pub(crate) const T: &str = \"\u{2192}\";",
        "let s = S {",
        "    #[cfg(test)]",
        "    probe: \"\u{2192}\",",
        "    name: 'x',",
        "};",
        "const A: &str = \"\u{b7}\";",
    ]
    .join("\n");
    assert_eq!(found(&src), [(16, '\u{b7}')]);
}

#[test]
fn test_paths_by_name() {
    for p in ["ui/snapshot_tests_branch.rs", "update/tests.rs", "runner/logs_tests.rs", "forge/api_tests/mod.rs", "tests/x.rs"] {
        assert!(is_test_path(p), "{p}");
    }
    for p in ["ui/sidebar.rs", "state/tests_helper_free.rs", "main.rs", "ui/contests.rs"] {
        assert!(!is_test_path(p), "{p}");
    }
}

#[test]
fn test_only_module_files() {
    assert_eq!(test_only_modules("main.rs", "mod ui;\n#[cfg(test)]\nmod journey;\n"), ["journey"]);
    assert_eq!(test_only_modules("runner/mod.rs", "#[cfg(test)]\npub(crate) mod sandbox;\nmod saves;\n"), ["runner/sandbox"]);
    assert_eq!(test_only_modules("ui/app.rs", "#[cfg(test)]\n#[path = \"app_testing.rs\"]\nmod testing;\n"), ["ui/app_testing"]);
    assert_eq!(test_only_modules("state/conflict.rs", "#[cfg(test)]\nmod probe;\n"), ["state/conflict/probe"]);
    // An inline test module has no file of its own.
    assert_eq!(test_only_modules("lib.rs", "#[cfg(test)]\nmod tests {\n}\n"), Vec::<String>::new());
}

#[test]
fn json_characters_by_line() {
    let src = "{\n  \"a\": \"caf\u{e9}\",\n  \"b\": \"up \\u2191 \\\\u2193\"\n}";
    let got: Vec<(usize, char)> = json_chars(src).into_iter().map(|f| (f.line, f.ch)).collect();
    assert_eq!(got, [(2, '\u{e9}'), (3, '\u{2191}')]);
}
