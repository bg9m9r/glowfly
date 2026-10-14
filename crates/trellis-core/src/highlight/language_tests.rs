//! The bundled syntax set: language detection, samples of the added languages, the scope rules
//! and the grammars' regexes.

use super::parse::Classifier;
use super::tests::{assert_covers, diff, hunk, kinds};
use super::*;
use std::collections::HashSet;
use syntect::parsing::syntax_definition::Pattern;
use syntect::parsing::{Regex, Scope};

/// The languages this set adds over syntect's defaults.
const NEW_LANGUAGES: [&str; 7] = ["TOML", "TypeScript", "TypeScriptReact", "Dockerfile", "Kotlin", "Swift", "PowerShell"];

#[test]
fn new_languages_are_detected() {
    let cases: &[(&[&str], &str)] = &[
        (&["Cargo.toml", "cfg/app.toml", "Cargo.lock"], "TOML"),
        (&["src/a.ts", "a.mts", "a.cts"], "TypeScript"),
        (&["ui/App.tsx"], "TypeScriptReact"),
        (&["build.ps1", "Mod.psm1", "Mod.psd1"], "PowerShell"),
        (
            &["Dockerfile", "docker/Dockerfile", "DOCKERFILE", "app.dockerfile", "Dockerfile.dev", "Containerfile", "Containerfile.prod"],
            "Dockerfile",
        ),
        (&["Main.kt", "build.gradle.kts"], "Kotlin"),
        (&["App.swift"], "Swift"),
    ];
    for (paths, want) in cases {
        for path in *paths {
            assert_eq!(syntax_name(path, None), Some(*want), "{path}");
        }
    }
    assert_eq!(syntax_name("run", Some("#!/usr/bin/env pwsh")), Some("PowerShell"));
    assert_eq!(syntax_name("x", Some("FROM alpine:3")), Some("Dockerfile"));
    assert_eq!(syntax_name("CMakeLists.txt", None), Some("CMake"), "the whole name comes before the extension");
    assert_eq!(syntax_name("notes.txt", None), None);
    assert_eq!(syntax_name("notes.xyz", None), None);
}

#[test]
fn existing_language_names_are_unchanged() {
    // Highlights carried over a reload are matched by these names (`same_basis`).
    assert_eq!(syntax_name("a.rs", None), Some("Rust"));
    assert_eq!(syntax_name("x.py", None), Some("Python"));
    assert_eq!(syntax_name("Makefile", None), Some("Makefile"));
    assert_eq!(syntax_name("run", Some("#!/bin/bash")), Some("Bourne Again Shell (bash)"));
}

#[test]
fn each_new_language_highlights_a_sample() {
    use TokenKind::*;
    let samples: &[(&str, &str, &[&str], &[TokenKind])] = &[
        (
            "Cargo.toml",
            "TOML",
            &["[package]", "name = \"x\" # c", "version = 1", "enabled = true"],
            &[String, Comment, Number, Constant, Keyword, Type],
        ),
        (
            "src/a.ts",
            "TypeScript",
            &["interface Foo { x: number }", "export function f(a: string): void { return; } // c"],
            &[Keyword, Type, Function, Comment],
        ),
        ("ui/App.tsx", "TypeScriptReact", &["const App = () => <div className=\"x\">{1}</div>;"], &[Keyword, Attribute, String, Number]),
        (
            "build.ps1",
            "PowerShell",
            &[
                "# c",
                "[CmdletBinding()]",
                "param([string]$Name = 'x')",
                "function Get-Thing { if ($true) { Write-Host \"hi\" -NoNewline }; return 10MB }",
            ],
            &[Comment, Attribute, Type, String, Keyword, Function, Constant, Number],
        ),
        ("Dockerfile", "Dockerfile", &["# base", "FROM rust:1.80 AS build", "RUN echo \"hi\""], &[Comment, Keyword, String]),
        (
            "Main.kt",
            "Kotlin",
            &["// c", "fun main(args: Array<String>) { val n = 42; println(\"hi\") }"],
            &[Comment, Keyword, Function, Type, Number, String],
        ),
        (
            "App.swift",
            "Swift",
            &["import Foundation", "@objc final class A {}", "func f(_ s: String) -> Bool { return true } // c", "let x = 42"],
            &[Keyword, Type, Attribute, Constant, Function, Comment, Number],
        ),
    ];
    for (path, syntax, lines, want) in samples {
        let added: Vec<_> = lines.iter().map(|l| ('+', *l)).collect();
        let d = diff(vec![hunk(&added)]);
        let hl = highlight_diff(path, &d, None).unwrap_or_else(|| panic!("{path} is highlighted"));
        assert_eq!(hl.syntax, *syntax, "{path}");
        let mut all = HashSet::new();
        for (l, text) in lines.iter().enumerate() {
            assert_covers(hl.line(0, l), text);
            all.extend(kinds(hl.line(0, l)));
        }
        assert!(all.iter().any(|k| *k != Plain), "{path}: nothing colored");
        for k in *want {
            assert!(all.contains(k), "{path} lacks {k:?}: {:?}", (0..lines.len()).map(|l| hl.line(0, l)).collect::<Vec<_>>());
        }
    }
}

/// The kind of the span that covers byte `at` of line `l` in hunk 0.
fn span_at(hl: &DiffHighlight, l: usize, at: u32) -> Span {
    *hl.line(0, l).iter().find(|s| s.start <= at && at < s.end).unwrap_or_else(|| panic!("no span at {at}: {:?}", hl.line(0, l)))
}

#[test]
fn powershell_spans() {
    let lines = ["ForEach-Object { $_ }", "if ($a -eq 1) {}", "<# one", "two", "#>", "$s = @\"", "here $x", "\"@"];
    let d = diff(vec![hunk(&lines.map(|l| ('+', l)))]);
    let hl = highlight_diff("a.ps1", &d, None).expect("PowerShell is highlighted");
    let cmdlet = span_at(&hl, 0, 0);
    assert_eq!((cmdlet.start, cmdlet.end, cmdlet.kind), (0, 14, TokenKind::Function), "one command name, not `ForEach` + rest");
    assert_eq!(span_at(&hl, 0, 17).kind, TokenKind::Constant, "$_");
    let eq = span_at(&hl, 1, 7);
    assert_eq!((eq.start, eq.end, eq.kind), (7, 10, TokenKind::Keyword), "-eq");
    for (l, text) in lines.iter().enumerate().take(5).skip(2) {
        assert_eq!(hl.line(0, l).iter().map(|s| s.kind).collect::<Vec<_>>(), [TokenKind::Comment], "{text}");
    }
    assert_eq!(span_at(&hl, 5, 5).kind, TokenKind::String, "@\" opens a here-string");
    for (l, text) in lines.iter().enumerate().skip(6) {
        assert_eq!(hl.line(0, l).iter().map(|s| s.kind).collect::<Vec<_>>(), [TokenKind::String], "{text}");
    }
}

#[test]
fn scope_rules_prefer_the_most_specific() {
    let mut cx = Classifier::new();
    let mut kind = |s: &str| cx.classify(&[Scope::new(s).unwrap()]);
    assert_eq!(kind("storage.type.buildin.kotlin"), TokenKind::Type);
    assert_eq!(kind("storage.type.rust"), TokenKind::Keyword);
    assert_eq!(kind("keyword.constant.boolean.swift"), TokenKind::Constant);
    assert_eq!(kind("keyword.control.rust"), TokenKind::Keyword);
    assert_eq!(kind("storage.modifier.attribute.swift"), TokenKind::Attribute);
    assert_eq!(kind("entity.name.table.toml"), TokenKind::Type);
    assert_eq!(kind("constant.numeric.integer.rust"), TokenKind::Number);
    assert_eq!(kind("constant.language.rust"), TokenKind::Constant);
}

#[test]
fn new_grammars_compile_under_fancy_regex() {
    let builder = SYNTAXES.set.clone().into_builder();
    let defs = builder.syntaxes();
    let names: Vec<&str> = defs.iter().map(|s| s.name.as_str()).collect();
    let unique: HashSet<&str> = names.iter().copied().collect();
    assert_eq!(unique.len(), names.len(), "syntax names are unique");
    for want in NEW_LANGUAGES {
        assert!(unique.contains(want), "{want} is bundled");
    }
    let mut checked = 0;
    for def in defs.iter().filter(|s| NEW_LANGUAGES.contains(&s.name.as_str())) {
        for (context, ctx) in &def.contexts {
            for pattern in &ctx.patterns {
                let Pattern::Match(m) = pattern else { continue };
                if m.has_captures {
                    continue; // refers to the captures of the match that pushed it
                }
                let regex = m.regex.regex_str();
                let err = Regex::try_compile(regex);
                assert!(err.is_none(), "{} / {context}: {regex:?}: {err:?}", def.name);
                checked += 1;
            }
        }
    }
    assert!(checked > 1_000, "{checked}");
}

#[test]
fn bundled_set_has_more_than_the_defaults() {
    let syntaxes = SYNTAXES.set.syntaxes();
    assert!(syntaxes.len() > 150, "{}", syntaxes.len());
    assert_eq!(syntaxes.iter().filter(|s| s.name == "PowerShell").count(), 1);
}
