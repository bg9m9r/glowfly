//! Reading what `git range-diff --no-color` printed (pure). Each pair starts with a header line
//! `<a>:  <hex> <status> <b>:  <hex> <subject>`, where a missing side is `-` and a run of
//! dashes; a changed pair (`!`) is followed by its interdiff, every line indented 4 spaces and
//! then an outer marker (' ', '-' or '+') before the inner line. Outer hunk headers are the
//! lines whose text right after the 4 spaces starts with `@@`.

use super::Status;
use crate::error::{Error, Result};
use crate::repo::{DiffLine, FileDiff, Hunk};
use crate::text::printable;

/// One pair as git printed it: indexes and hex as written, the header's subject (the old
/// commit's when it has one) and the interdiff of a changed pair.
#[derive(Clone, Debug, PartialEq)]
pub struct RawPair {
    pub status: Status,
    /// git's 1-based index and the hex it printed.
    pub old: Option<(u32, String)>,
    pub new: Option<(u32, String)>,
    pub subject: String,
    pub interdiff: Option<FileDiff>,
}

fn malformed(line: &str) -> Error {
    Error::Other(format!("Couldn't read git range-diff's output: {}", printable(line)))
}

/// Splits off the first whitespace-separated token, returning it and the rest after it (with
/// the whitespace that followed it still in front).
fn token(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start_matches(' ');
    if s.is_empty() {
        return None;
    }
    let end = s.find(' ').unwrap_or(s.len());
    Some((&s[..end], &s[end..]))
}

/// `<n>:` and its hex, or `-:` (or `-`) and a run of dashes for a missing side.
fn side(index: &str, hex: &str, line: &str) -> Result<Option<(u32, String)>> {
    let index = index.strip_suffix(':').unwrap_or(index);
    if index == "-" {
        return if !hex.is_empty() && hex.bytes().all(|b| b == b'-') { Ok(None) } else { Err(malformed(line)) };
    }
    let n: u32 = index.parse().map_err(|_| malformed(line))?;
    let hex_ok = (4..=64).contains(&hex.len()) && hex.bytes().all(|b| b.is_ascii_hexdigit());
    if n == 0 || !hex_ok {
        return Err(malformed(line));
    }
    Ok(Some((n, hex.to_owned())))
}

fn header(line: &str) -> Result<RawPair> {
    let bad = || malformed(line);
    let (a_idx, rest) = token(line).ok_or_else(bad)?;
    let (a_hex, rest) = token(rest).ok_or_else(bad)?;
    let (status, rest) = token(rest).ok_or_else(bad)?;
    let (b_idx, rest) = token(rest).ok_or_else(bad)?;
    let (b_hex, rest) = token(rest).ok_or_else(bad)?;
    let (old, new) = (side(a_idx, a_hex, line)?, side(b_idx, b_hex, line)?);
    let status = match status {
        "=" => Status::Unchanged,
        "!" => Status::Changed,
        "<" => Status::Removed,
        ">" => Status::Added,
        _ => return Err(bad()),
    };
    let fits = match status {
        Status::Unchanged | Status::Changed => old.is_some() && new.is_some(),
        Status::Removed => old.is_some() && new.is_none(),
        Status::Added => old.is_none() && new.is_some(),
    };
    if !fits {
        return Err(bad());
    }
    let subject = rest.strip_prefix(' ').unwrap_or(rest).to_owned();
    Ok(RawPair { status, old, new, subject, interdiff: None })
}

/// Adds one body line (its 4-space indent already removed) to `hunks`.
fn body_line(hunks: &mut Vec<Hunk>, rest: &str) {
    if rest.starts_with("@@") {
        hunks.push(Hunk { header: rest.to_owned(), lines: Vec::new() });
        return;
    }
    let mut chars = rest.chars();
    let (origin, text) = match chars.next() {
        Some(c @ ('-' | '+' | ' ')) => (c, chars.as_str()),
        // Anything else is shown whole as context.
        Some(_) => (' ', rest),
        None => (' ', ""),
    };
    if hunks.is_empty() {
        hunks.push(Hunk { header: String::new(), lines: Vec::new() });
    }
    let line = DiffLine { origin, old: None, new: None, text: text.to_owned() };
    hunks.last_mut().expect("a hunk was just pushed").lines.push(line);
}

fn interdiff(hunks: Vec<Hunk>) -> FileDiff {
    FileDiff { binary: false, hunks, highlight: None, intraline: Default::default(), whole: None }
}

/// The pairs in git's order. Empty output (both ranges empty) is no pairs.
pub fn pairs(stdout: &str) -> Result<Vec<RawPair>> {
    let mut out: Vec<RawPair> = Vec::new();
    let mut hunks: Vec<Hunk> = Vec::new();
    for line in stdout.lines() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("    ") {
            if out.is_empty() {
                return Err(malformed(line));
            }
            body_line(&mut hunks, rest);
            continue;
        }
        if let Some(last) = out.last_mut()
            && !hunks.is_empty()
        {
            last.interdiff = Some(interdiff(std::mem::take(&mut hunks)));
        }
        out.push(header(line)?);
    }
    if let Some(last) = out.last_mut()
        && !hunks.is_empty()
    {
        last.interdiff = Some(interdiff(hunks));
    }
    Ok(out)
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;
