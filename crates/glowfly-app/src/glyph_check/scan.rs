//! Finding the characters a source file can show: the non-ASCII characters in Rust string and
//! char literals outside comments and test-only code, and every non-ASCII character in a JSON
//! translation file. Plain text scanning, no fonts; the Rust sources are cut up by the source
//! checks' shared lexer (glowfly-ui's `tests/view_checks/support/lexer.rs`).

use super::lexer;

/// A non-ASCII character and the 1-based line it is on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Found {
    pub line: usize,
    pub ch: char,
}

/// Every non-ASCII character inside a string or char literal of `src`, skipping comments
/// (doc comments too), byte strings and `#[cfg(test)]` items.
pub(super) fn rust_literal_chars(src: &str) -> Vec<Found> {
    let src = blank_test_items(src);
    let mut out = Vec::new();
    for lit in lexer::literals(&src).into_iter().filter(|l| !l.piece.byte) {
        let mut line = lit.line;
        let mut keep = |c: char, line: usize| {
            if !c.is_ascii() {
                out.push(Found { line, ch: c });
            }
        };
        if lit.piece.raw {
            for c in lit.text.chars() {
                line += usize::from(c == '\n');
                keep(c, line);
            }
            continue;
        }
        let mut cs = lit.text.chars().peekable();
        while let Some(c) = cs.next() {
            match c {
                '\\' if cs.peek() == Some(&'u') => {
                    cs.next();
                    let hex: String = cs.by_ref().skip_while(|&h| h == '{').take_while(|&h| h != '}').collect();
                    if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                        keep(c, line);
                    }
                }
                '\\' => {
                    if cs.next() == Some('\n') {
                        line += 1;
                    }
                }
                _ => {
                    line += usize::from(c == '\n');
                    keep(c, line);
                }
            }
        }
    }
    out
}

/// Every non-ASCII character in a JSON file, with `\uXXXX` escapes decoded.
pub(super) fn json_chars(src: &str) -> Vec<Found> {
    let mut out = Vec::new();
    for (n, text) in src.lines().enumerate() {
        let cs: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < cs.len() {
            if cs[i] == '\\' && cs.get(i + 1) == Some(&'u') {
                let hex: String = cs.iter().skip(i + 2).take(4).collect();
                if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32).filter(|c| !c.is_ascii()) {
                    out.push(Found { line: n + 1, ch: c });
                }
                i += 6;
            } else if cs[i] == '\\' {
                i += 2;
            } else {
                if !cs[i].is_ascii() {
                    out.push(Found { line: n + 1, ch: cs[i] });
                }
                i += 1;
            }
        }
    }
    out
}

/// Whether a source path (relative to `src/`, `/`-separated) is test code by its name.
pub(super) use lexer::is_test_file as is_test_path;

/// The module files `rel` declares under `#[cfg(test)]` (`mod name;`), as paths relative to
/// `src/` without `.rs` (and without a trailing `/mod`), honouring `#[path = "..."]`.
pub(super) fn test_only_modules(rel: &str, src: &str) -> Vec<String> {
    let rel = rel.strip_suffix(".rs").unwrap_or(rel);
    let (dir, stem) = rel.rsplit_once('/').unwrap_or(("", rel));
    // Where child modules' files live: beside a crate root or mod.rs, else in a folder named
    // after the file. A #[path] is relative to the declaring file's folder either way.
    let child_dir = if matches!(stem, "main" | "lib" | "mod") { dir.to_owned() } else { join(dir, stem) };
    let lines: Vec<&str> = src.lines().map(str::trim).collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let Some(rest) = strip_test_cfg(l) else { continue };
        let mut path = None;
        let mut item = rest.trim();
        let mut j = i;
        while item.is_empty() || item.starts_with("#[") {
            if let Some(p) = item.strip_prefix("#[path = \"").and_then(|p| p.strip_suffix("\"]")) {
                path = Some(p);
            }
            j += 1;
            let Some(next) = lines.get(j) else { break };
            item = next;
        }
        let item = ["pub(crate) ", "pub(super) ", "pub "].iter().fold(item, |s, v| s.strip_prefix(v).unwrap_or(s));
        let Some(name) = item.strip_prefix("mod ").and_then(|m| m.strip_suffix(';')) else { continue };
        let file = match path {
            Some(p) => {
                let p = p.strip_suffix(".rs").unwrap_or(p);
                join(dir, p.strip_suffix("/mod").unwrap_or(p))
            }
            None => join(&child_dir, name.trim()),
        };
        out.push(file);
    }
    out
}

/// What follows a test-only `cfg` at the start of `line`: `#[cfg(test)]`, or the `test-support`
/// feature's `#[cfg(any(test, feature = "test-support"))]` (test helpers other crates' tests use).
fn strip_test_cfg(line: &str) -> Option<&str> {
    line.strip_prefix("#[cfg(test)]").or_else(|| line.strip_prefix("#[cfg(any(test, feature = \"test-support\"))]"))
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() { name.to_owned() } else { format!("{dir}/{name}") }
}

/// `src` with each `#[cfg(test)]` item (its attributes, and a braced body or the lines up to a
/// `;` or `,`) replaced by empty lines, so line numbers stay put. Braces are counted per line
/// outside comments and literals.
fn blank_test_items(src: &str) -> String {
    let bare_text = lexer::blanked(src, false);
    let lines: Vec<&str> = src.lines().collect();
    let bare: Vec<&str> = bare_text.lines().collect();
    let mut out: Vec<&str> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        let Some(rest) = strip_test_cfg(lines[i].trim()) else {
            out.push(lines[i]);
            i += 1;
            continue;
        };
        let mut j = i;
        if rest.trim().is_empty() || rest.trim().starts_with("#[") {
            j += 1;
            while lines.get(j).is_some_and(|l| l.trim().starts_with("#[")) {
                j += 1;
            }
        }
        let mut end = j;
        if bare.get(j).is_some_and(|l| l.contains('{')) {
            let mut depth = 0i64;
            while end < lines.len() {
                depth += bare[end].matches('{').count() as i64 - bare[end].matches('}').count() as i64;
                if depth <= 0 {
                    break;
                }
                end += 1;
            }
        } else {
            while lines.get(end).is_some_and(|l| !(l.trim_end().ends_with(';') || l.trim_end().ends_with(','))) {
                end += 1;
            }
        }
        let end = end.min(lines.len() - 1);
        out.extend(std::iter::repeat_n("", end + 1 - i));
        i = end + 1;
    }
    out.join("\n")
}

#[cfg(test)]
#[path = "scan_tests.rs"]
mod tests;
