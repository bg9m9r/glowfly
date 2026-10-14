//! Language overrides from the `linguist-language` gitattribute, and `.jsx` detection.

use super::linguist::{ALIASES, by_language};
use super::syntaxes::SYNTAXES;
use super::tests::{assert_covers, diff, hunk, kinds};
use super::*;
use crate::repo::{self, DiffTarget, LoadDiffOptions, ViewDiff};
use crate::testutil::{git, scratch_repo, write};
use std::path::{Path, PathBuf};

fn resolved(name: &str) -> Option<&'static str> {
    by_language(name).map(|s| s.name.as_str())
}

#[test]
fn linguist_names_resolve_case_insensitively() {
    let cases: &[(&[&str], &str)] = &[
        (&["Rust", "rust", "RUST"], "Rust"),
        (&["TypeScript"], "TypeScript"),
        (&["Shell", "bash", "sh", "zsh"], "Bourne Again Shell (bash)"),
        (&["JSON", "json", "JSON with Comments", "JSON-with-Comments"], "JSON"),
        (&["C++", "cpp"], "C++"),
        (&["C#", "csharp"], "C#"),
        (&["Objective-C"], "Objective-C"),
        (&["Protocol-Buffer", "protocol buffer", "PROTOCOL_BUFFER"], "Protocol Buffer"),
        (&["PHP"], "PHP"),
        (&["Vim Script"], "VimL"),
        (&["Batchfile"], "Batch File"),
        (&["TSX", "JSX"], "TypeScriptReact"),
        (&["pwsh", "PowerShell"], "PowerShell"),
        (&["Containerfile"], "Dockerfile"),
        (&["  Python  "], "Python"),
        (&["yml"], "YAML"),
    ];
    for (names, want) in cases {
        for name in *names {
            assert_eq!(resolved(name), Some(*want), "{name:?}");
        }
    }
    assert!(by_language("PHP").is_some_and(|s| !s.hidden), "the visible PHP, not the hidden PHP Source");
}

#[test]
fn unknown_or_hidden_names_do_not_resolve() {
    for name in ["Klingon", "", "   ", "Shell-Unix-Generic", "PHP Source"] {
        assert_eq!(resolved(name), None, "{name:?}");
    }
}

#[test]
fn alias_targets_exist() {
    for (keys, target) in ALIASES {
        let syntax = SYNTAXES.set.find_syntax_by_name(target);
        assert!(syntax.is_some(), "{target} ({keys:?}) is not bundled");
        assert!(*target == "Plain Text" || syntax.is_some_and(|s| !s.hidden), "{target} is hidden");
    }
}

#[test]
fn override_beats_detection() {
    let hint = |path, language| SyntaxHint { path, language: Some(language) };
    assert_eq!(syntax_name("a.inc", None), Some("x86_64 Assembly"));
    assert_eq!(syntax_name(hint("a.inc", "Rust"), None), Some("Rust"));
    assert_eq!(syntax_name(hint("a.inc", "Klingon"), None), Some("x86_64 Assembly"), "unknown names fall back");
    assert_eq!(syntax_name(hint("run", "Python"), Some("#!/bin/bash")), Some("Python"), "beats the shebang");
    assert_eq!(syntax_name(hint("Dockerfile", "Make"), None), Some("Makefile"), "read as an extension");
    assert_eq!(syntax_name(hint("a.rs", "Text"), None), None, "shown plain");
    assert_eq!(syntax_name(hint("a.rs", "Plain Text"), None), None, "shown plain");
}

#[test]
fn jsx_uses_the_tsx_grammar() {
    assert_eq!(syntax_name("App.jsx", None), Some("TypeScriptReact"));
    assert_eq!(syntax_name("src/a.JSX", None), Some("TypeScriptReact"));
    assert_eq!(syntax_name("a.js", None), Some("JavaScript"));

    let line = r#"const App = () => <div className="x">{1}</div>;"#;
    let hl = highlight_diff("App.jsx", &diff(vec![hunk(&[('+', line)])]), None).expect("highlighted");
    assert_eq!(hl.syntax, "TypeScriptReact");
    assert_covers(hl.line(0, 0), line);
    let k = kinds(hl.line(0, 0));
    for want in [TokenKind::Keyword, TokenKind::Attribute, TokenKind::String, TokenKind::Number] {
        assert!(k.contains(&want), "{want:?} in {k:?}");
    }
}

/// A repository with `.gitattributes` holding `attrs` and `lib.inc` committed as `before`, then
/// changed to `after` in the working tree.
fn repo_with(name: &str, attrs: &str, before: &str, after: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, ".gitattributes", attrs);
    write(&dir, "lib.inc", before);
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    write(&dir, "lib.inc", after);
    dir
}

fn view(dir: &Path, target: DiffTarget) -> ViewDiff {
    repo::load_diff_for_view(dir, &target, LoadDiffOptions::default()).unwrap()
}

fn unstaged(dir: &Path, path: &str) -> ViewDiff {
    view(dir, DiffTarget::Unstaged(path.into()))
}

/// The highlight of `v` as the viewer computes it, with its language override.
fn highlighted(path: &str, v: &ViewDiff) -> DiffHighlight {
    highlight_in_context(SyntaxHint { path, language: v.language.as_deref() }, &v.diff, None, v.old.as_ref()).expect("highlighted")
}

fn all_kinds(hl: &DiffHighlight) -> std::collections::HashSet<TokenKind> {
    hl.lines.iter().flatten().flat_map(|l| kinds(l)).collect()
}

#[test]
fn gitattributes_override_language() {
    let dir = repo_with("ling-override", "*.inc linguist-language=Rust\n", "fn main() {}\n", "fn main() { let x = 1; }\n");
    let v = unstaged(&dir, "lib.inc");
    assert_eq!(v.language.as_deref(), Some("Rust"));
    let hl = highlighted("lib.inc", &v);
    assert_eq!(hl.syntax, "Rust");
    assert!(all_kinds(&hl).contains(&TokenKind::Keyword));

    git(&dir, &["add", "lib.inc"]);
    assert_eq!(view(&dir, DiffTarget::Staged("lib.inc".into())).language.as_deref(), Some("Rust"));
    git(&dir, &["commit", "-q", "-m", "edit"]);
    let head = git2::Repository::open(&dir).unwrap().head().unwrap().peel_to_commit().unwrap().id();
    let v = view(&dir, DiffTarget::Commit(head, "lib.inc".into()));
    assert_eq!(v.language.as_deref(), Some("Rust"));
    assert_eq!(highlighted("lib.inc", &v).syntax, "Rust");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn gitattributes_php_override() {
    let dir = repo_with("ling-php", "*.inc linguist-language=PHP\n", "", "<?php\necho \"x\"; // c\n");
    let hl = highlighted("lib.inc", &unstaged(&dir, "lib.inc"));
    assert_eq!(hl.syntax, "PHP");
    let k = all_kinds(&hl);
    assert!(k.contains(&TokenKind::String) && k.contains(&TokenKind::Comment), "{k:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_language_falls_back() {
    let dir = repo_with("ling-unknown", "*.inc linguist-language=Klingon\n", "mov eax, 1\n", "mov eax, 2\n");
    let v = unstaged(&dir, "lib.inc");
    assert_eq!(v.language.as_deref(), Some("Klingon"), "the raw value");
    assert_eq!(highlighted("lib.inc", &v).syntax, "x86_64 Assembly");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn valueless_or_unset_attribute_is_ignored() {
    let dir = repo_with("ling-valueless", "*.inc linguist-language\n", "a\n", "b\n");
    assert_eq!(unstaged(&dir, "lib.inc").language, None, "set without a value");
    for attrs in ["*.inc -linguist-language\n", "*.inc !linguist-language\n", "*.inc linguist-language=\n"] {
        write(&dir, ".gitattributes", attrs);
        assert_eq!(unstaged(&dir, "lib.inc").language, None, "{attrs:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nested_gitattributes_win() {
    let dir = scratch_repo("ling-nested");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    write(&dir, ".gitattributes", "*.inc linguist-language=Rust\n");
    write(&dir, "sub/.gitattributes", "*.inc linguist-language=Python\n");
    write(&dir, "lib.inc", "a\n");
    write(&dir, "sub/x.inc", "a\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    write(&dir, "lib.inc", "b\n");
    write(&dir, "sub/x.inc", "b\n");
    assert_eq!(unstaged(&dir, "sub/x.inc").language.as_deref(), Some("Python"), "the deepest file wins");
    assert_eq!(unstaged(&dir, "lib.inc").language.as_deref(), Some("Rust"));
    let _ = std::fs::remove_dir_all(&dir);
}
