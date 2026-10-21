//! Parsing `git blame --porcelain` output, as bytes.
//!
//! Each line of the file comes as a header (`<sha> <orig line> <final line>[ <lines in group>]`),
//! the commit's details the first time that commit appears (`author`, `author-mail`, …, of
//! which only a few are read), then the line itself after one TAB. The content is taken
//! exactly as git printed it after that TAB, never trimmed. `previous` and `filename` are read
//! on every group: git gives them again when a commit's lines come from more than one path.

use super::blame::{BlameCommit, BlameOrigin, BlameRun};
use crate::error::{Error, Result};
use crate::quoted::unquote_c;
use git2::Oid;
use std::collections::HashMap;

/// What the next line of output must be.
enum Expect {
    Header,
    /// Key lines, then the content of a line blamed to commit `commit`; `fill` while its details
    /// are still to read (its first appearance). `orig_line` is 1-based.
    Details {
        commit: usize,
        fill: bool,
        orig_line: usize,
    },
}

/// What one run of `git blame --porcelain` said.
pub(super) struct Parsed {
    /// In order of first appearance.
    pub commits: Vec<BlameCommit>,
    pub lines: Vec<String>,
    /// Consecutive lines blamed to one commit and origin.
    pub runs: Vec<BlameRun>,
    pub origins: Vec<BlameOrigin>,
    /// 0-based, one per line.
    pub orig_lines: Vec<usize>,
}

fn bad() -> Error {
    Error::Decode("unexpected git blame output".into())
}

/// Parses the whole output.
pub(super) fn parse(out: &[u8]) -> Result<Parsed> {
    let mut p = Parsed { commits: Vec::new(), lines: Vec::new(), runs: Vec::new(), origins: Vec::new(), orig_lines: Vec::new() };
    let mut index: HashMap<Oid, usize> = HashMap::new();
    // The origin each commit's lines come from now, set by its latest `filename`.
    let mut current_origin: Vec<Option<usize>> = Vec::new();
    // A `previous` read in this group, for the `filename` that follows it.
    let mut previous: Option<(Oid, String)> = None;
    let mut expect = Expect::Header;
    for line in out.split(|b| *b == b'\n') {
        match expect {
            Expect::Header => {
                if line.is_empty() {
                    continue; // after the last line's terminator
                }
                let (oid, orig_line, final_line) = header(line).ok_or_else(bad)?;
                if final_line != p.lines.len() + 1 || orig_line == 0 {
                    return Err(bad());
                }
                let (commit, fill) = match index.get(&oid) {
                    Some(&i) => (i, false),
                    None => {
                        index.insert(oid, p.commits.len());
                        let summary = if oid.is_zero() { "Not committed yet".into() } else { String::new() };
                        p.commits.push(BlameCommit {
                            oid,
                            author: String::new(),
                            email: String::new(),
                            time: 0,
                            summary,
                            body: String::new(),
                        });
                        current_origin.push(None);
                        (p.commits.len() - 1, true)
                    }
                };
                previous = None;
                expect = Expect::Details { commit, fill, orig_line };
            }
            Expect::Details { commit, fill, orig_line } => {
                if let Some(content) = line.strip_prefix(b"\t") {
                    let origin = current_origin[commit].ok_or_else(bad)?;
                    let text = String::from_utf8_lossy(content);
                    let text = text.strip_suffix('\r').unwrap_or(&text);
                    p.lines.push(text.to_owned());
                    p.orig_lines.push(orig_line - 1);
                    match p.runs.last_mut() {
                        Some(r) if r.commit == commit && r.origin == origin => r.len += 1,
                        _ => p.runs.push(BlameRun { commit, origin, start: p.lines.len() - 1, len: 1 }),
                    }
                    expect = Expect::Header;
                } else if let Some(value) = line.strip_prefix(b"previous ") {
                    previous = previous_value(value);
                } else if let Some(value) = line.strip_prefix(b"filename ") {
                    let path = unquote_c(&String::from_utf8_lossy(value));
                    let found = p.origins.iter().position(|o| o.commit == commit && o.path == path);
                    let i = match found {
                        Some(i) => i,
                        None => {
                            p.origins.push(BlameOrigin { commit, path, previous: previous.take() });
                            p.origins.len() - 1
                        }
                    };
                    current_origin[commit] = Some(i);
                } else if fill {
                    detail(&mut p.commits[commit], line);
                }
            }
        }
    }
    match expect {
        Expect::Header => Ok(p),
        Expect::Details { .. } => Err(bad()),
    }
}

/// The commit named by 40 or 64 hex digits.
fn sha(s: &str) -> Option<Oid> {
    let ok = (s.len() == 40 || s.len() == 64) && s.bytes().all(|b| b.is_ascii_hexdigit());
    ok.then(|| Oid::from_str(s).ok()).flatten()
}

/// `<sha> <path>` of a `previous` line, the path C-unquoted.
fn previous_value(value: &[u8]) -> Option<(Oid, String)> {
    let text = String::from_utf8_lossy(value);
    let (oid, path) = text.split_once(' ')?;
    Some((sha(oid)?, unquote_c(path)))
}

/// The commit, the 1-based original line number and the 1-based final line number of a header
/// line.
fn header(line: &[u8]) -> Option<(Oid, usize, usize)> {
    let text = std::str::from_utf8(line).ok()?;
    let mut parts = text.split(' ');
    let oid = sha(parts.next()?)?;
    let orig: usize = parts.next()?.parse().ok()?;
    let final_line: usize = parts.next()?.parse().ok()?;
    if let Some(count) = parts.next() {
        count.parse::<usize>().ok()?;
    }
    if parts.next().is_some() {
        return None;
    }
    Some((oid, orig, final_line))
}

/// Reads one key line into `c`; keys other than the author and summary (`boundary` among them)
/// are ignored.
fn detail(c: &mut BlameCommit, line: &[u8]) {
    let (key, value) = match line.iter().position(|b| *b == b' ') {
        Some(i) => (&line[..i], &line[i + 1..]),
        None => (line, &b""[..]),
    };
    let value = || String::from_utf8_lossy(value).into_owned();
    match key {
        b"author" => c.author = value(),
        b"author-mail" => {
            let v = value();
            c.email = v.strip_prefix('<').and_then(|v| v.strip_suffix('>')).map_or_else(|| v.clone(), str::to_owned);
        }
        b"author-time" => c.time = value().trim().parse().unwrap_or(0),
        b"summary" if !c.oid.is_zero() => c.summary = value(),
        _ => {}
    }
}

#[cfg(test)]
#[path = "blame_porcelain_tests.rs"]
mod tests;
