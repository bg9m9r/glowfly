//! The hygiene ratchet on English text in glowfly-ui's and glowfly-runner's non-test code (see
//! hygiene.rs): a string literal that `is_english` calls text for the user, outside the calls and
//! attributes that only developers read (`DEV_ONLY`) and other than a token (`token_like`). The files with
//! such text today are listed in `english_allow.rs` with their count, which only goes down; any
//! other file has none.

use super::{all_sources, is_english};
use crate::lexer::{Kind, code_lines, pieces};

#[path = "english_allow.rs"]
mod english_allow;

use english_allow::ENGLISH_TEXT;

/// The crates whose non-test code the check reads.
const CRATES: &[&str] = &["crates/glowfly-ui/src/", "crates/glowfly-runner/src/"];

/// Calls, macros and attributes whose text only developers read: logs, panics, asserts, egui
/// ids, environment variables, process arguments and the like. A literal anywhere inside one is
/// not user text.
const DEV_ONLY: &[&str] = &[
    "#[",
    "debug!",
    "trace!",
    "info!",
    "warn!",
    "error!",
    "log!",
    "panic!",
    "unreachable!",
    "todo!",
    "unimplemented!",
    "assert!",
    "assert_eq!",
    "assert_ne!",
    "debug_assert!",
    "debug_assert_eq!",
    "expect",
    "expect_err",
    "env!",
    "include_str!",
    "include_bytes!",
    "Id::new",
    "id_salt",
    "push_id",
    "make_persistent_id",
    "with",
    "arg",
    "args",
    "env",
    "env_remove",
    "var",
    "var_os",
    "Command::new",
];

/// The lines (1-based) of `src`'s code where a string literal holds English text outside the
/// `DEV_ONLY` calls. Comments and inline test modules are skipped.
fn english_text(src: &str) -> Vec<usize> {
    english_literals(src).into_iter().map(|(line, _)| line).collect()
}

/// Each English literal of `src` (as `english_text`), with its line.
///
/// The open brackets are a stack of frames: each `(` the name it calls, each `[` itself, each
/// `{` a barrier. A `DEV_ONLY` frame covers the text inside it up to the next barrier, and only
/// until a closure starts in it (its `|`): the body of `ui.push_id(i, |ui| ui.label("…"))` or
/// `x.with(|v| { … })` is the user's text again.
fn english_literals(src: &str) -> Vec<(usize, String)> {
    let code = code_lines(src).join("\n");
    // (name, a closure started in it)
    let mut stack: Vec<(String, bool)> = Vec::new();
    let mut found = Vec::new();
    for p in pieces(&code) {
        match p.kind {
            Kind::Code => {
                for (k, c) in code[p.span.clone()].char_indices() {
                    let before = &code[..p.span.start + k];
                    match c {
                        '[' if before.trim_end().trim_end_matches('!').ends_with('#') => stack.push(("#[".into(), false)),
                        '[' => stack.push(("[".into(), false)),
                        '(' => stack.push((callee(before), false)),
                        '{' => stack.push(("{".into(), false)),
                        // A closure's `|` follows `(`, `,`, `=` or `move`; `a || b` is no closure.
                        '|' if closure_starts(before) => {
                            if let Some(top) = stack.last_mut() {
                                top.1 = true;
                            }
                        }
                        ')' | ']' | '}' => {
                            stack.pop();
                        }
                        _ => {}
                    }
                }
            }
            Kind::Str if !p.byte => {
                let lit = &code[p.inner.clone()];
                let dev = stack
                    .iter()
                    .rev()
                    .take_while(|(name, _)| name != "{")
                    .any(|(name, closure)| !closure && DEV_ONLY.contains(&name.as_str()));
                if is_english(lit) && !token_like(lit) && !dev {
                    found.push((code[..p.span.start].matches('\n').count() + 1, lit.to_owned()));
                }
            }
            _ => {}
        }
    }
    found
}

/// Whether a `|` after `before` opens a closure's parameters: it follows `(`, `,`, `=` or `move`.
fn closure_starts(before: &str) -> bool {
    let b = before.trim_end();
    b.ends_with(['(', ',', '=']) || b.ends_with("move") && !b[..b.len() - 4].ends_with(|c: char| c.is_alphanumeric() || c == '_')
}

/// Whether a literal is a token rather than text: a command-line flag (`-NoLogo …`), or one word
/// that is a path, file name, URL or name (`.gitattributes`, `https://git-lfs.com`, `--type={k}`,
/// `glowfly-{name}`): with a `/`, `\`, `@` or `=` in it, a `.` or `:` before its end, or no
/// capital letter. A word that only ends in `.` or `:` (`Steps:`, `Done.`) is text.
fn token_like(lit: &str) -> bool {
    let one_word = !lit.contains(char::is_whitespace);
    let inside = lit.trim_end_matches(['.', ':']);
    lit.starts_with('-')
        || one_word && (lit.contains(['/', '\\', '@', '=']) || inside.contains(['.', ':']) || !lit.contains(char::is_uppercase))
}

/// The name called by a `(` that follows `before`: the name and `!` just before it, with its
/// type for a type's function (`ui.label` gives `label`, `egui::Id::new` gives `Id::new`,
/// `log::debug!` gives `debug!`), or empty for a bare group.
fn callee(before: &str) -> String {
    let before = before.trim_end();
    let start = before.rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':' || c == '!')).map_or(0, |i| i + 1);
    let parts: Vec<&str> = before[start..].split("::").filter(|s| !s.is_empty()).collect();
    match parts.as_slice() {
        [.., ty, name] if ty.starts_with(char::is_uppercase) => format!("{ty}::{name}"),
        [.., name] => (*name).to_owned(),
        [] => String::new(),
    }
}

#[test]
fn ui_and_runner_text_is_translated() {
    let mut problems = Vec::new();
    let sources = all_sources();
    for (path, _) in ENGLISH_TEXT {
        if !sources.iter().any(|s| s.path == *path && !s.test) {
            problems.push(format!("{path}: in ENGLISH_TEXT but no longer a source file; remove the entry"));
        }
    }
    for s in sources.iter().filter(|s| !s.test && CRATES.iter().any(|c| s.path.starts_with(c))) {
        let lines = english_literals(&s.text);
        let ceiling = ENGLISH_TEXT.iter().find(|(p, _)| *p == s.path).map_or(0, |(_, n)| *n);
        if lines.len() > ceiling {
            let at: Vec<String> = lines.iter().map(|(l, text)| format!("{}:{l}  \"{text}\"", s.path)).collect();
            problems.push(format!(
                "{}: {} English literals, over its count of {ceiling} in ENGLISH_TEXT. Translate the new text instead: add a key to \
                 crates/glowfly-core/locales/*.json and pass t.tr(\"key\") (or trf/trn). The literals:\n  {}",
                s.path,
                lines.len(),
                at.join("\n  ")
            ));
        } else if lines.len() < ceiling {
            problems.push(format!(
                "{}: only {} English literals now; lower its count in tests/source_checks/hygiene/english_allow.rs to {} \
                 (drop the entry at 0)",
                s.path,
                lines.len(),
                lines.len()
            ));
        }
    }
    assert!(problems.is_empty(), "English text in glowfly-ui and glowfly-runner:\n{}", problems.join("\n"));
}

#[test]
fn the_english_text_check_catches_what_it_should() {
    let src = "fn a(ui: &mut Ui, t: &T) {\n    ui.label(\"Open a repository\");\n    ui.label(t.tr(\"repo.open\"));\n    \
               log::debug!(\"Loaded {n} rows\");\n    let id = egui::Id::new(\"commit list\");\n    x.expect(\"a lock held\");\n    \
               // ui.label(\"Hidden in a comment\");\n    ui.button(format!(\"Push {n} commits\"));\n    let k = \"{a}: {b}\";\n    \
               open(\".gitattributes\", \"https://git-lfs.com\");\n}\n#[cfg(test)]\nmod tests {\n    fn b() { ui.label(\"Test text\"); }\n}\n";
    assert_eq!(english_text(src), [2, 8], "user text, also inside format!; not keys, logs, ids, expects, paths, comments or tests");
    assert_eq!(english_text("#[doc = \"Some words\"]\nconst A: &str = \"Some words\";\n"), [2], "not in attributes");
    assert!(["--type={k}", "glowfly-{name}", "stash@{{{n}}}", ".gitattributes", "-NoLogo -NoExit"].into_iter().all(token_like));
    assert!(!token_like("Status") && !token_like("Open a file.") && !token_like("{n} files"));
    assert!(["Steps:", "Done.", "Error:"].into_iter().all(|w| !token_like(w)), "a word ending in . or : is text");
    let closures = "fn c(ui: &mut Ui) {\n    ui.push_id(i, |ui| {\n        ui.label(\"Delete everything\");\n    });\n    \
                    x.with(|v| ui.label(\"Keep it\"));\n    ui.push_id(\"row id\", |ui| ui.label(t.tr(\"k\")));\n    \
                    v.expect(\"Not a closure\");\n    debug_assert!(a || b, \"Nor is a || b\");\n}\n";
    assert_eq!(english_text(closures), [3, 5], "a closure's body passed to a DEV_ONLY call is text; the call's own argument isn't");
    assert_eq!(callee("    egui::Id::new"), "Id::new");
    assert_eq!(callee("x.label "), "label");
    assert_eq!(callee("std::fs::read"), "read");
    assert_eq!(callee("    log::debug!"), "debug!");
    assert_eq!(callee("    "), "");
}
