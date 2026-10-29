//! The paths the patches `git am` applies would write, read from the mails themselves (no git
//! runs): when one of them is a place hooks run from that follows the working tree, the hooks
//! it writes run in that same `git am` (`post-applypatch`) or in the next git command, so the
//! job is held (incoming.rs).

use super::Follow;
use super::follow::touches;
use super::incoming::Unchecked;
use std::path::Path;

/// The first place in `follows` that a patch in the mailbox at `file` writes, or `Err` when the
/// file can't be read. Paths lose their first part, as git's default `-p1` strips them.
pub(super) fn touched_in(file: &Path, follows: &[Follow]) -> Result<Option<String>, Unchecked> {
    let bytes = std::fs::read(file).map_err(|e| Unchecked::Unreadable(format!("{}: {e}", file.display())))?;
    Ok(paths(&String::from_utf8_lossy(&bytes)).into_iter().find(|p| touches(p, follows)))
}

/// Every path the diffs in `text` name, old and new, without git's `a/` and `b/` prefixes.
/// Where a line can be split more than one way, every reading is kept: more paths are
/// compared, never fewer.
pub(super) fn paths(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines().map(|l| l.trim_end_matches('\r')) {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            out.extend(git_header(rest));
        } else if let Some(rest) = line.strip_prefix("--- ").or_else(|| line.strip_prefix("+++ ")) {
            let p = unquote(rest.split('\t').next().unwrap_or(rest));
            if p != "/dev/null" {
                out.push(strip_one(&p));
            }
        } else if let Some(rest) = ["rename from ", "rename to ", "copy from ", "copy to "].iter().find_map(|k| line.strip_prefix(k)) {
            out.push(unquote(rest));
        }
    }
    out.retain(|p| !p.is_empty());
    out.sort();
    out.dedup();
    out
}

/// The paths in a `diff --git a/<old> b/<new>` header. Quoted names are read as git writes
/// them; an unquoted one is split at each ` b/` that could end the old name.
fn git_header(rest: &str) -> Vec<String> {
    if rest.starts_with('"') || rest.ends_with('"') {
        return quoted_pair(rest).into_iter().map(|p| strip_one(&p)).collect();
    }
    let mut out = Vec::new();
    for (i, _) in rest.match_indices(" b/") {
        let (old, new) = (&rest[..i], &rest[i + 1..]);
        if strip_one(old) == strip_one(new) {
            return vec![strip_one(old)];
        }
        out.extend([strip_one(old), strip_one(new)]);
    }
    if out.is_empty() {
        out.push(strip_one(rest));
    }
    out
}

/// Two names of which at least one is C-quoted.
fn quoted_pair(rest: &str) -> Vec<String> {
    if let Some(q) = rest.strip_prefix('"') {
        let Some(end) = closing(q) else { return vec![unquote(rest)] };
        return vec![unquote(&rest[..end + 2]), unquote(rest[end + 2..].trim_start())];
    }
    let at = rest.find(" \"").unwrap_or(rest.len());
    vec![rest[..at].to_owned(), unquote(rest[at..].trim_start())]
}

/// The index in `q` (after an opening quote) of the closing quote.
fn closing(q: &str) -> Option<usize> {
    let mut escaped = false;
    for (i, c) in q.char_indices() {
        match c {
            '\\' if !escaped => escaped = true,
            '"' if !escaped => return Some(i),
            _ => escaped = false,
        }
    }
    None
}

/// A name as git C-quotes it (`"a/\303\251"`), or as written when it isn't quoted.
fn unquote(s: &str) -> String {
    let Some(inner) = s.strip_prefix('"').and_then(|r| r.strip_suffix('"')) else { return s.to_owned() };
    let mut bytes = Vec::new();
    let mut it = inner.bytes().peekable();
    while let Some(b) = it.next() {
        if b != b'\\' {
            bytes.push(b);
            continue;
        }
        match it.next() {
            Some(b'n') => bytes.push(b'\n'),
            Some(b't') => bytes.push(b'\t'),
            Some(d @ b'0'..=b'7') => {
                let mut v = u32::from(d - b'0');
                for _ in 0..2 {
                    if let Some(n) = it.peek().copied().filter(u8::is_ascii_digit) {
                        v = v * 8 + u32::from(n - b'0');
                        it.next();
                    }
                }
                bytes.push(v as u8);
            }
            Some(other) => bytes.push(other),
            None => {}
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// `p` without its first part (git's default `-p1`: `a/x` is `x`).
fn strip_one(p: &str) -> String {
    p.split_once('/').map_or_else(|| p.to_owned(), |(_, rest)| rest.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_path_a_patch_names_is_read() {
        let mail = "From x\nSubject: [PATCH] hooks\n\n---\n\
            diff --git a/.githooks/post-applypatch b/.githooks/post-applypatch\nnew file mode 100755\n\
            --- /dev/null\n+++ b/.githooks/post-applypatch\n@@ -0,0 +1 @@\n+echo hi\n\
            diff --git a/x y b/z w\nrename from x y\nrename to z w\n\
            diff --git \"a/\\303\\251\" \"b/\\303\\251\"\nGIT binary patch\n";
        let got = paths(mail);
        for want in [".githooks/post-applypatch", "x y", "z w", "é"] {
            assert!(got.contains(&want.to_owned()), "{want} in {got:?}");
        }
    }

    #[test]
    fn a_patch_reaches_a_folder_under_any_spelling() {
        let follows = [Follow { path: ".githooks".into(), tracked: None, hooks: true }];
        assert!(touches(".GITHOOKS/post-applypatch", &follows));
        assert!(touches(".githooks./x", &follows));
        assert!(!touches("src/main.rs", &follows));
    }
}
