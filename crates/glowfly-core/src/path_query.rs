//! Path terms of the commit search (`path:` and `file:`): a folder, a file name or a glob,
//! tested against the paths a commit changed.
//!
//! This is matching only: a term's value is never used to read or change a file, so both the
//! value and every repo path are folded (`names::fold`: NFC, then lowercase) and compared under
//! that key on every OS (PLATFORMS.md, path-identity rule 9). A typed `\` counts as a
//! separator: the value is typed text, not one of git's paths, and on Windows people write
//! folders with it. Repo paths themselves are git's bytes and are only read here, through
//! [`fold_path`].
//!
//! Kinds of value, after normalising (trim, `\` to `/`, repeated `/` collapsed, a leading `./`
//! or `/` dropped, a trailing `/` meaning "folders only"):
//! - A literal with a `/` (or written with a leading `./` or `/`) is anchored at the root: the
//!   path itself or anything under that folder.
//! - A literal without one is a name: some component of the path equals it.
//! - A value with `*`, `?` or `[` is a glob. `*` and `?` never cross a `/`, `**` does, and `**/`
//!   may stand for no folder at all. `[abc]`, `[a-z]` and `[!x]` (or `[^x]`) are classes; a `[`
//!   without its `]` is literal; there is no escape character. A glob without `/` is tested
//!   against each component; an anchored one against the whole path and each of its leading
//!   folders.

use crate::names;

#[cfg(test)]
mod tests;

/// One piece of a compiled glob.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Tok {
    Char(char),
    /// `?`: one character other than `/`.
    One,
    /// `*`: any run of characters without `/`.
    Star,
    /// `**`: any run of characters.
    Stars,
    /// `**/`: nothing, or any run ending in `/`.
    StarsSlash,
    /// `[...]`: one character (other than `/`) in, or with `negated` not in, the ranges.
    Class {
        negated: bool,
        ranges: Vec<(char, char)>,
    },
}

/// A parsed `path:` value.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PathTerm {
    /// The value, normalised and folded, without a trailing `/`.
    pattern: String,
    /// The value has `*`, `?` or `[`.
    glob: bool,
    /// The value names a place from the root (it had a `/`).
    anchored: bool,
    /// The value ended in `/`: only folders match.
    dir_only: bool,
    /// `pattern` compiled, for a glob.
    toks: Vec<Tok>,
}

impl PathTerm {
    /// Parses a typed value; `None` when nothing is left after normalising (`""`, `/`, `./`).
    pub fn parse(value: &str) -> Option<PathTerm> {
        let mut v = String::with_capacity(value.len());
        for c in value.trim().chars().map(|c| if c == '\\' { '/' } else { c }) {
            if !(c == '/' && v.ends_with('/')) {
                v.push(c);
            }
        }
        let mut rest = v.as_str();
        let mut anchored = false;
        while let Some(r) = rest.strip_prefix("./").or_else(|| rest.strip_prefix('/')) {
            rest = r;
            anchored = true;
        }
        let dir_only = rest.ends_with('/');
        let rest = rest.strip_suffix('/').unwrap_or(rest);
        if rest.is_empty() {
            return None;
        }
        let pattern = names::fold(rest);
        let glob = pattern.contains(['*', '?', '[']);
        let toks = if glob { compile(&pattern) } else { Vec::new() };
        let anchored = anchored || pattern.contains('/');
        Some(PathTerm { pattern, glob, anchored, dir_only, toks })
    }

    /// Whether repo path `path` (git's bytes) is, or lies under, what this term names.
    pub fn matches(&self, path: &[u8]) -> bool {
        self.matches_folded(&fold_path(path))
    }

    /// [`matches`](Self::matches) for a path already folded with [`fold_path`].
    pub fn matches_folded(&self, folded: &str) -> bool {
        let p = folded;
        if !self.anchored {
            let mut parts: Vec<&str> = p.split('/').collect();
            if self.dir_only {
                parts.pop();
            }
            return parts.into_iter().any(|part| self.test(part));
        }
        if !self.glob {
            let under = p.len() > self.pattern.len() && p.starts_with(&self.pattern) && p.as_bytes()[self.pattern.len()] == b'/';
            return under || (!self.dir_only && p == self.pattern);
        }
        let folders = p.match_indices('/').map(|(i, _)| &p[..i]);
        folders.into_iter().any(|f| self.test(f)) || (!self.dir_only && self.test(p))
    }

    /// For an anchored literal of ASCII characters: the folder or file it names from the root,
    /// which may be given to libgit2 (as a literal pathspec) to skip unrelated trees.
    pub fn literal_prefix(&self) -> Option<&str> {
        (self.anchored && !self.glob && self.pattern.is_ascii()).then_some(self.pattern.as_str())
    }

    /// Whether `text` (one component, a folder prefix or the whole path) is the pattern.
    fn test(&self, text: &str) -> bool {
        if self.glob {
            let text: Vec<char> = text.chars().collect();
            glob_match(&self.toks, &text)
        } else {
            text == self.pattern
        }
    }
}

/// The key a repo path is matched under: lossily decoded (a name that isn't UTF-8 still matches
/// what it shows as), composed and lowercased.
pub fn fold_path(path: &[u8]) -> String {
    if path.is_ascii() {
        return String::from_utf8_lossy(path).to_ascii_lowercase();
    }
    names::fold(&String::from_utf8_lossy(path))
}

/// Whether `path` matches any of `terms`, folding it once.
pub fn any_matches(terms: &[PathTerm], path: &[u8]) -> bool {
    if terms.is_empty() {
        return false;
    }
    let folded = fold_path(path);
    terms.iter().any(|t| t.matches_folded(&folded))
}

/// Compiles a glob pattern.
fn compile(pattern: &str) -> Vec<Tok> {
    let p: Vec<char> = pattern.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < p.len() {
        match p[i] {
            '*' => {
                let run = p[i..].iter().take_while(|&&c| c == '*').count();
                i += run;
                out.push(match run {
                    1 => Tok::Star,
                    // More than two stars add nothing.
                    _ if p.get(i) == Some(&'/') => {
                        i += 1;
                        Tok::StarsSlash
                    }
                    _ => Tok::Stars,
                });
            }
            '?' => {
                out.push(Tok::One);
                i += 1;
            }
            '[' => match class(&p[i + 1..]) {
                Some((tok, used)) => {
                    out.push(tok);
                    i += 1 + used;
                }
                None => {
                    out.push(Tok::Char('['));
                    i += 1;
                }
            },
            c => {
                out.push(Tok::Char(c));
                i += 1;
            }
        }
    }
    out
}

/// The class starting just after a `[`, and how many characters it used (through its `]`);
/// `None` when it has no `]`. A `]` straight after the `[` (or its `!`/`^`) is a member.
fn class(p: &[char]) -> Option<(Tok, usize)> {
    let negated = matches!(p.first(), Some('!' | '^'));
    let mut i = negated as usize;
    let mut ranges = Vec::new();
    let first = i;
    loop {
        let c = *p.get(i)?;
        if c == ']' && i > first {
            return Some((Tok::Class { negated, ranges }, i + 1));
        }
        match (p.get(i + 1), p.get(i + 2)) {
            (Some('-'), Some(&end)) if end != ']' => {
                ranges.push((c, end));
                i += 3;
            }
            _ => {
                ranges.push((c, c));
                i += 1;
            }
        }
    }
}

/// Whether `toks` match all of `text`. Works forward over the set of text positions the
/// pattern so far can end at, so no input backtracks exponentially.
fn glob_match(toks: &[Tok], text: &[char]) -> bool {
    let n = text.len();
    let mut cur = vec![false; n + 1];
    cur[0] = true;
    for tok in toks {
        let mut next = vec![false; n + 1];
        for i in (0..=n).filter(|&i| cur[i]) {
            match tok {
                Tok::Char(c) => {
                    if text.get(i) == Some(c) {
                        next[i + 1] = true;
                    }
                }
                Tok::One => {
                    if text.get(i).is_some_and(|&c| c != '/') {
                        next[i + 1] = true;
                    }
                }
                Tok::Class { negated, ranges } => {
                    if text.get(i).is_some_and(|&c| c != '/' && ranges.iter().any(|&(a, b)| a <= c && c <= b) != *negated) {
                        next[i + 1] = true;
                    }
                }
                Tok::Star => {
                    next[i] = true;
                    for j in i..n {
                        if text[j] == '/' {
                            break;
                        }
                        next[j + 1] = true;
                    }
                }
                Tok::Stars => next[i..].iter_mut().for_each(|b| *b = true),
                Tok::StarsSlash => {
                    next[i] = true;
                    for j in (i..n).filter(|&j| text[j] == '/') {
                        next[j + 1] = true;
                    }
                }
            }
        }
        if !next.contains(&true) {
            return false;
        }
        cur = next;
    }
    cur[n]
}
