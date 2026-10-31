//! Finding the characters a source file can show: the non-ASCII characters in Rust string and
//! char literals outside comments and test-only code, and every non-ASCII character in a JSON
//! translation file. Plain text scanning, no fonts.

/// A non-ASCII character and the 1-based line it is on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Found {
    pub line: usize,
    pub ch: char,
}

/// Every non-ASCII character inside a string or char literal of `src`, skipping comments
/// (doc comments too), byte strings and `#[cfg(test)]` items.
pub(super) fn rust_literal_chars(src: &str) -> Vec<Found> {
    Lexer { cs: blank_test_items(src).chars().collect(), i: 0, line: 1, out: Vec::new() }.run()
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

/// Whether a source path (relative to `src/`, `/`-separated) is test code by its name: a
/// component (without `.rs`) that is `tests`, ends with `_tests` or starts with
/// `snapshot_tests`. A name that merely starts with `tests_` is not.
pub(super) fn is_test_path(rel: &str) -> bool {
    rel.split('/')
        .map(|c| c.strip_suffix(".rs").unwrap_or(c))
        .any(|c| c == "tests" || c.ends_with("_tests") || c.starts_with("snapshot_tests"))
}

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
/// `;` or `,`) replaced by empty lines, so line numbers stay put. Braces are counted per line, as
/// `tests/layering.rs` does.
fn blank_test_items(src: &str) -> String {
    let lines: Vec<&str> = src.lines().collect();
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
        if lines.get(j).is_some_and(|l| l.contains('{')) {
            let mut depth = 0i64;
            while end < lines.len() {
                depth += lines[end].matches('{').count() as i64 - lines[end].matches('}').count() as i64;
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

struct Lexer {
    cs: Vec<char>,
    i: usize,
    line: usize,
    out: Vec<Found>,
}

impl Lexer {
    fn at(&self, k: usize) -> Option<char> {
        self.cs.get(self.i + k).copied()
    }

    fn keep(&mut self, c: char, collect: bool) {
        if collect && !c.is_ascii() {
            self.out.push(Found { line: self.line, ch: c });
        }
    }

    fn run(mut self) -> Vec<Found> {
        while let Some(c) = self.at(0) {
            let after_ident = self.i > 0 && is_ident(self.cs[self.i - 1]);
            match (c, self.at(1)) {
                ('\n', _) => {
                    self.line += 1;
                    self.i += 1;
                }
                ('/', Some('/')) => {
                    while self.at(0).is_some_and(|c| c != '\n') {
                        self.i += 1;
                    }
                }
                ('/', Some('*')) => self.block_comment(),
                ('r', Some('"' | '#')) if !after_ident => {
                    self.i += 1;
                    self.raw_string(true);
                }
                ('b', Some('r')) if !after_ident => {
                    self.i += 2;
                    self.raw_string(false);
                }
                ('b', Some('"')) if !after_ident => {
                    self.i += 2;
                    self.string(false);
                }
                ('b', Some('\'')) if !after_ident => {
                    self.i += 1;
                    self.char_literal(false);
                }
                ('"', _) => {
                    self.i += 1;
                    self.string(true);
                }
                ('\'', _) => self.char_literal(true),
                _ => self.i += 1,
            }
        }
        self.out
    }

    fn block_comment(&mut self) {
        let mut depth = 0;
        while let Some(c) = self.at(0) {
            match (c, self.at(1)) {
                ('/', Some('*')) => {
                    depth += 1;
                    self.i += 2;
                }
                ('*', Some('/')) => {
                    depth -= 1;
                    self.i += 2;
                    if depth == 0 {
                        return;
                    }
                }
                ('\n', _) => {
                    self.line += 1;
                    self.i += 1;
                }
                _ => self.i += 1,
            }
        }
    }

    /// At the hashes or quote after `r`: reads a raw string to its end. Reads nothing when this
    /// is not one, e.g. the raw identifier `r#type`.
    fn raw_string(&mut self, collect: bool) {
        let hashes = (0..).take_while(|&k| self.at(k) == Some('#')).count();
        if self.at(hashes) != Some('"') {
            return;
        }
        self.i += hashes + 1;
        while let Some(c) = self.at(0) {
            if c == '"' && (1..=hashes).all(|k| self.at(k) == Some('#')) {
                self.i += hashes + 1;
                return;
            }
            if c == '\n' {
                self.line += 1;
            }
            self.keep(c, collect);
            self.i += 1;
        }
    }

    /// After the opening quote: reads a string to its closing quote, decoding `\u{..}`.
    fn string(&mut self, collect: bool) {
        while let Some(c) = self.at(0) {
            match c {
                '"' => {
                    self.i += 1;
                    return;
                }
                '\\' if self.at(1) == Some('u') && self.at(2) == Some('{') => {
                    self.i += 3;
                    let mut hex = String::new();
                    while let Some(h) = self.at(0).filter(|&h| h != '}' && h != '"') {
                        hex.push(h);
                        self.i += 1;
                    }
                    self.i += 1;
                    if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                        self.keep(c, collect);
                    }
                }
                '\\' => {
                    if self.at(1) == Some('\n') {
                        self.line += 1;
                    }
                    self.i += 2;
                }
                _ => {
                    if c == '\n' {
                        self.line += 1;
                    }
                    self.keep(c, collect);
                    self.i += 1;
                }
            }
        }
    }

    /// At a `'`: reads a char literal (`'c'`, `'\n'`, `'\x41'`, `'\u{..}'`), or steps over the
    /// quote of a lifetime or label.
    fn char_literal(&mut self, collect: bool) {
        match (self.at(1), self.at(2)) {
            (Some('\\'), Some('u')) if self.at(3) == Some('{') => {
                let close = (4..12).find(|&k| self.at(k) == Some('}'));
                if let Some(k) = close.filter(|&k| self.at(k + 1) == Some('\'')) {
                    let hex: String = (4..k).filter_map(|n| self.at(n)).collect();
                    if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                        self.keep(c, collect);
                    }
                    self.i += k + 2;
                    return;
                }
            }
            (Some('\\'), Some('x')) if self.at(5) == Some('\'') => {
                self.i += 6;
                return;
            }
            (Some('\\'), Some(_)) if self.at(3) == Some('\'') => {
                self.i += 4;
                return;
            }
            (Some(c), Some('\'')) if c != '\n' && c != '\\' => {
                self.keep(c, collect);
                self.i += 3;
                return;
            }
            _ => {}
        }
        self.i += 1;
    }
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

#[cfg(test)]
#[path = "scan_tests.rs"]
mod tests;
