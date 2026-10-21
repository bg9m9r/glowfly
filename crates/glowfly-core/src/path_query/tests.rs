use super::*;

fn term(v: &str) -> PathTerm {
    PathTerm::parse(v).unwrap_or_else(|| panic!("{v:?} parses"))
}

/// Asserts which of `hits` and `misses` term `v` matches.
fn check(v: &str, hits: &[&str], misses: &[&str]) {
    let t = term(v);
    for p in hits {
        assert!(t.matches(p.as_bytes()), "{v:?} should match {p:?}");
    }
    for p in misses {
        assert!(!t.matches(p.as_bytes()), "{v:?} should not match {p:?}");
    }
}

#[test]
fn parse_normalises_the_value() {
    let t = term("./src/");
    assert_eq!((t.pattern.as_str(), t.anchored, t.dir_only, t.glob), ("src", true, true, false));
    let t = term("/src");
    assert_eq!((t.pattern.as_str(), t.anchored, t.dir_only), ("src", true, false));
    assert_eq!(term("src\\ui").pattern, "src/ui");
    assert_eq!(term("a//b").pattern, "a/b");
    assert_eq!(term("  .//a\\\\b/  ").pattern, "a/b");
    assert!(term("a//b").anchored);
    for empty in ["", "  ", "/", "./", ".//", "\\", "//"] {
        assert_eq!(PathTerm::parse(empty), None, "{empty:?}");
    }
    let t = term("tests/");
    assert!(t.dir_only && !t.anchored);
    assert!(term("*.rs").glob && term("a?").glob && term("[ab]").glob);
}

#[test]
fn an_anchored_literal_is_the_path_or_under_it() {
    check("src/ui", &["src/ui", "src/ui/view.rs", "src/ui/a/b.rs"], &["src/uix.rs", "lib/src/ui/a.rs", "src", "src/u"]);
    check("./README.md", &["README.md"], &["docs/README.md"]);
}

#[test]
fn a_bare_literal_is_any_component() {
    check("Cargo.toml", &["Cargo.toml", "crates/core/Cargo.toml"], &["Cargo.toml.bak", "Carg", "x/Cargo.tomlx"]);
    check("tests", &["crates/app/tests/x.rs", "tests"], &["tests.rs", "crates/attests/x.rs"]);
    check("Carg", &[], &["Cargo.toml"]);
    check("Carg*", &["Cargo.toml", "a/Cargo.lock"], &["xCargo"]);
}

#[test]
fn folders_only() {
    check("tests/", &["tests/a.rs", "x/tests/b.rs"], &["tests", "x/tests"]);
    check("src/ui/", &["src/ui/view.rs"], &["src/ui"]);
    check("src/*/", &["src/x/b.rs"], &["src/a.rs"]);
}

#[test]
fn globs() {
    check("*.rs", &["a.rs", "src/x/a.rs"], &["a.rsx", "src/a.rsx"]);
    // A folder named like the glob also counts: its files are under it.
    check("*.d", &["x.d/a.txt"], &[]);
    check("src/*.rs", &["src/a.rs"], &["src/x/a.rs", "lib/src/a.rs"]);
    check("src/**/*.rs", &["src/a.rs", "src/x/y/a.rs"], &["src/a.txt", "lib/a.rs"]);
    check("src/*", &["src/a.rs", "src/x/b.rs"], &["src", "lib/x"]);
    check("**/view.rs", &["view.rs", "a/b/view.rs"], &["a/b/view.rsx"]);
    check("src/**", &["src/a", "src/a/b/c"], &["lib/src/a"]);
    check("a?.rs", &["ab.rs", "x/a1.rs"], &["a.rs", "abc.rs"]);
    check("a?b/x", &[], &["a/b/x"]);
    check("[ab].txt", &["a.txt", "b.txt"], &["c.txt", "ab.txt"]);
    check("[a-c].txt", &["b.txt"], &["d.txt"]);
    check("[!a].txt", &["b.txt"], &["a.txt"]);
    check("[^a].txt", &["b.txt"], &["a.txt"]);
    check("[]].txt", &["].txt"], &["a.txt"]);
    // An unterminated class is literal.
    check("a[b", &["a[b", "x/a[b"], &["ab"]);
    check("[ab", &["[ab"], &["a"]);
    // Character classes fold too.
    check("[A-C].TXT", &["b.txt", "B.txt"], &["d.txt"]);
}

#[test]
fn case_and_unicode_form_are_ignored() {
    check("README.MD", &["readme.md", "Readme.md"], &[]);
    check("café.txt", &["cafe\u{301}.txt", "docs/CAFÉ.txt"], &["cafe.txt"]);
    check("cafe\u{301}.txt", &["café.txt"], &[]);
    check("Ü*", &["ü.txt", "u\u{308}ber"], &[]);
    // A name that isn't UTF-8 matches lossily, without panicking.
    let t = term("bad\u{fffd}.txt");
    assert!(t.matches(b"bad\xff.txt"));
    assert!(term("*.txt").matches(b"dir/\xfe\xff.txt"));
    assert!(!term("x").matches(b"\xff\xfe"));
}

#[test]
fn any_matches_tests_each_term() {
    let terms = [term("docs/"), term("*.rs")];
    assert!(any_matches(&terms, b"docs/a.md"));
    assert!(any_matches(&terms, b"src/A.RS"));
    assert!(!any_matches(&terms, b"src/a.md"));
    assert!(!any_matches(&[], b"a"));
}

#[test]
fn literal_prefix_is_for_anchored_ascii_literals() {
    assert_eq!(term("src/ui").literal_prefix(), Some("src/ui"));
    assert_eq!(term("SRC/UI/").literal_prefix(), Some("src/ui"));
    assert_eq!(term("/README.md").literal_prefix(), Some("readme.md"));
    assert_eq!(term("src/*.rs").literal_prefix(), None);
    assert_eq!(term("Cargo.toml").literal_prefix(), None);
    assert_eq!(term("docs/café").literal_prefix(), None);
}

#[test]
fn long_globs_stay_fast() {
    let t = term("**/a*a*a*a*a*a*a*a*b");
    let path = format!("{}/{}", "a".repeat(200), "a".repeat(200));
    assert!(!t.matches(path.as_bytes()));
}
