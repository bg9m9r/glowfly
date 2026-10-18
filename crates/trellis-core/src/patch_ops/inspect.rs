//! What a patch file holds, read without git: how many emailed commits (a format-patch series)
//! and which files it touches, for the Apply patch dialog.

use crate::error::{Error, Result};
use std::path::Path;

/// What a patch file holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PatchSummary {
    /// Emailed commits in it (0 for a plain diff).
    pub commits: usize,
    /// The first commit's subject, without its "[PATCH…]" tag.
    pub first_subject: Option<String>,
    /// The files it touches (their new names), in order and without duplicates.
    pub files: Vec<String>,
}

/// The largest patch file read.
pub const MAX_INSPECT: u64 = 64 << 20;

/// Reads the patch file at `path`, refusing one larger than [`MAX_INSPECT`].
pub fn inspect(path: &Path) -> Result<PatchSummary> {
    if std::fs::metadata(path)?.len() > MAX_INSPECT {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
        return Err(Error::Other(format!("{name} is larger than 64 MB; Trellis doesn't open patches that large.")));
    }
    Ok(summarize(&std::fs::read(path)?))
}

/// What the patch `bytes` hold (see [`PatchSummary`]); text that isn't UTF-8 is read lossily.
pub fn summarize(bytes: &[u8]) -> PatchSummary {
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text.lines().collect();
    let header = |l: &str| ["From:", "Date:", "Subject:"].iter().any(|h| l.starts_with(h));
    let mut s = PatchSummary::default();
    // A single email without the mbox "From " line.
    if lines.first().is_some_and(|l| header(l)) {
        s.commits = 1;
    }
    let add = |files: &mut Vec<String>, f: String| {
        if !f.is_empty() && !files.contains(&f) {
            files.push(f);
        }
    };
    let mut in_git = false;
    for (i, line) in lines.iter().enumerate() {
        let next = lines.get(i + 1).copied().unwrap_or("");
        if line.starts_with("From ") && header(next) {
            s.commits += 1;
            in_git = false;
        } else if let Some(rest) = line.strip_prefix("Subject: ").filter(|_| s.commits == 1 && s.first_subject.is_none()) {
            // A folded header goes on in lines that start with white space.
            let mut subject = rest.trim().to_owned();
            for more in lines[i + 1..].iter().take_while(|l| l.starts_with([' ', '\t'])) {
                subject.push(' ');
                subject.push_str(more.trim());
            }
            s.first_subject = Some(strip_tag(&subject).to_owned());
        } else if let Some(rest) = line.strip_prefix("diff --git ") {
            in_git = true;
            add(&mut s.files, git_target(rest));
        } else if line.starts_with("diff ") || line.starts_with("Index: ") {
            in_git = false;
        } else if !in_git && line.starts_with("--- ") && next.starts_with("+++ ") {
            let new = plain_path(&next[4..]);
            add(&mut s.files, if new == "/dev/null" { plain_path(&line[4..]) } else { new });
        }
    }
    s
}

/// `subject` without a leading "[PATCH…] " tag.
fn strip_tag(subject: &str) -> &str {
    match subject.strip_prefix("[PATCH").and_then(|r| r.split_once(']')) {
        Some((_, rest)) => rest.trim_start(),
        None => subject,
    }
}

/// The new path of a `diff --git a/X b/Y` line (`rest` is after "diff --git "): Y, unquoted,
/// without its "b/".
fn git_target(rest: &str) -> String {
    let strip = |p: String| p.strip_prefix("b/").map(str::to_owned).unwrap_or(p);
    if rest.starts_with('"') {
        let Some((_, after)) = unquote(rest) else { return String::new() };
        let after = after.trim_start();
        return strip(if after.starts_with('"') { unquote(after).map(|(b, _)| b).unwrap_or_default() } else { after.to_owned() });
    }
    if let Some(i) = rest.find(" \"b/") {
        return strip(unquote(&rest[i + 1..]).map(|(b, _)| b).unwrap_or_default());
    }
    // Both unquoted: the split where both sides name the same path, else the last " b/".
    let splits: Vec<usize> = rest.match_indices(" b/").map(|(i, _)| i).collect();
    let same = splits.iter().find(|&&i| rest[..i].strip_prefix("a/") == Some(&rest[i + 3..]));
    match same.or(splits.last()) {
        Some(&i) => rest[i + 3..].to_owned(),
        None => rest.split_once(' ').map(|(_, b)| b.to_owned()).unwrap_or_default(),
    }
}

/// The path of a `--- ` or `+++ ` line's rest: unquoted, cut at a tab, without "a/" or "b/".
fn plain_path(rest: &str) -> String {
    let p = if rest.starts_with('"') {
        unquote(rest).map(|(p, _)| p).unwrap_or_default()
    } else {
        rest.split('\t').next().unwrap_or("").trim_end().to_owned()
    };
    match p.strip_prefix("b/").or_else(|| p.strip_prefix("a/")) {
        Some(stripped) => stripped.to_owned(),
        None => p,
    }
}

/// The C-quoted string `s` starts with (git's quoting of unusual names), and what follows it.
fn unquote(s: &str) -> Option<(String, &str)> {
    let body = s.strip_prefix('"')?;
    let bytes = body.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => return Some((String::from_utf8_lossy(&out).into_owned(), &body[i + 1..])),
            b'\\' if i + 1 < bytes.len() => {
                let c = bytes[i + 1];
                i += 2;
                match c {
                    b'0'..=b'7' => {
                        let digits: Vec<u8> = bytes[i - 1..].iter().take(3).take_while(|d| (b'0'..=b'7').contains(d)).copied().collect();
                        out.push(digits.iter().fold(0u32, |n, d| n * 8 + u32::from(d - b'0')) as u8);
                        i += digits.len() - 1;
                    }
                    b'n' => out.push(b'\n'),
                    b't' => out.push(b'\t'),
                    b'r' => out.push(b'\r'),
                    b'a' => out.push(7),
                    b'b' => out.push(8),
                    b'f' => out.push(12),
                    b'v' => out.push(11),
                    other => out.push(other),
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "inspect_tests.rs"]
mod tests;
