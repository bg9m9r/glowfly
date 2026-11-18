//! A git command whose output is parsed for commit text is asked for UTF-8. git's log family
//! (log, show, blame, format-patch, range-diff, ...) re-encodes names and messages into
//! `i18n.logOutputEncoding`, or `i18n.commitEncoding` when that is unset, while Glowfly reads
//! output as UTF-8: such a command line is built with `gitcli::utf8_args(&[..])`, which puts a
//! `-c` override first.
//!
//! A command line is an argument list, a `[..]` group of literals, that names one of `PARSED`
//! (or one of `WITH_FORMAT` next to a `--format`/`--pretty` literal) and is built by `args(&[..])`
//! or holds an option (a literal starting with `-`). The literals count when they are directly
//! inside the brackets: `"blame".to_owned()` does, a `format!("..")` argument doesn't. Comments
//! and inline test modules don't count.

use crate::hygiene::all_sources;
use crate::lexer::{Kind, code_lines, pieces};

/// Subcommands that print commit text.
const PARSED: &[&str] =
    &["annotate", "blame", "format-patch", "log", "range-diff", "reflog", "shortlog", "show", "show-branch", "whatchanged"];

/// Subcommands that print commit text only with a format.
const WITH_FORMAT: &[&str] = &["diff-tree", "rev-list", "stash"];

/// Calls left alone on purpose: (path, subcommand, why its output is shown as is). The list only
/// shrinks; an entry that no longer matches a call fails.
const ALLOWED: &[(&str, &str, &str)] = &[];

/// One open delimiter: its character, where it opens, and the literals directly inside it.
type Open = (char, usize, Vec<(usize, String)>);

/// The (line, subcommand) of each command line in `src` that names a commit-text command without
/// `utf8_args`.
fn unwrapped(src: &str) -> Vec<(usize, String)> {
    let code = code_lines(src).join("\n");
    let mut found = Vec::new();
    let mut stack: Vec<Open> = Vec::new();
    let (mut line, mut at) = (1, 0);
    for p in pieces(&code) {
        line += code[at..p.span.start].matches('\n').count();
        at = p.span.start;
        match p.kind {
            Kind::Str => {
                if let Some(top) = stack.last_mut() {
                    top.2.push((line, code[p.inner.clone()].to_owned()));
                }
            }
            Kind::Code => {
                for (i, c) in code[p.span.clone()].char_indices() {
                    match c {
                        '(' | '[' | '{' => stack.push((c, p.span.start + i, Vec::new())),
                        ')' | ']' | '}' => {
                            if let Some(('[', opener, literals)) = stack.pop() {
                                found.extend(command_line(&code[..opener], &literals));
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    found
}

/// The commit-text commands of a `[..]` group (opened after `before`) that is a command line
/// without `utf8_args`.
fn command_line(before: &str, literals: &[(usize, String)]) -> Vec<(usize, String)> {
    let before = before.trim_end();
    if before.ends_with("utf8_args(&") {
        return Vec::new();
    }
    let formatted = literals.iter().any(|(_, l)| l.starts_with("--format") || l.starts_with("--pretty"));
    let is_command = before.ends_with("args(&") || literals.iter().any(|(_, l)| l.starts_with('-'));
    if !is_command {
        return Vec::new();
    }
    literals
        .iter()
        .filter(|(_, l)| PARSED.contains(&l.as_str()) || (formatted && WITH_FORMAT.contains(&l.as_str())))
        .map(|(line, l)| (*line, l.clone()))
        .collect()
}

#[test]
fn parsed_commit_text_commands_use_utf8_args() {
    let mut problems = Vec::new();
    let mut used = Vec::new();
    for source in all_sources().iter().filter(|s| !s.test) {
        for (line, sub) in unwrapped(&source.text) {
            if ALLOWED.iter().any(|(p, s, _)| *p == source.path && *s == sub) {
                used.push((source.path.clone(), sub));
            } else {
                problems.push(format!("{}:{line}: git {sub}", source.path));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "git prints commit text in i18n.logOutputEncoding (or i18n.commitEncoding): build this command line with glowfly_core::gitcli::utf8_args(&[..]) so its output is UTF-8, or add it to ALLOWED in log_encoding.rs with the reason its output is shown as is:\n{}",
        problems.join("\n")
    );
    let stale: Vec<String> = ALLOWED
        .iter()
        .filter(|(p, s, _)| !used.iter().any(|(up, us)| up == p && us == s))
        .map(|(p, s, _)| format!("{p}: git {s} is no longer called without utf8_args; remove it from ALLOWED in log_encoding.rs"))
        .collect();
    assert!(stale.is_empty(), "{}", stale.join("\n"));
}

#[test]
fn the_log_encoding_check_catches_what_it_should() {
    let flagged = [
        ("let a = args(&[\"log\", \"-1\"]);\n", "log"),
        ("let v = vec![\"range-diff\".to_owned(), \"--no-color\".into()];\n", "range-diff"),
        ("let a = args(&[\"show\", spec]);\n", "show"),
        ("let a = args(&[\"rev-list\", \"--format=%s\", x]);\n", "rev-list"),
    ];
    for (src, sub) in flagged {
        assert_eq!(unwrapped(src), [(1, sub.to_owned())], "{src}");
    }
    let fine = [
        "let a = utf8_args(&[\"log\", \"-1\"]);\n",
        "let a = gitcli::utf8_args(&[\n    \"--literal-pathspecs\",\n    \"blame\",\n]);\n",
        "const INERT: &[&str] = &[\"blame\", \"log\", \"show\"];\n",
        "let n = match x { \"log\" => 1, _ => 2 };\n",
        "let a = cmd(&[\"rev-list\"]);\n",
        "let a = args(&[\"rev-list\", \"--count\", x]);\n",
        "// let a = args(&[\"log\", \"-1\"]);\n",
        "#[cfg(test)]\nmod t {\n    fn f() {\n        let a = args(&[\"log\", \"-1\"]);\n    }\n}\n",
    ];
    for src in fine {
        assert!(unwrapped(src).is_empty(), "{src}");
    }
}
