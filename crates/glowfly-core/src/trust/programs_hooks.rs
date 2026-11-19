//! Scripts a hook runs from the working tree. A hook such as `exec ./scripts/check.sh` or
//! `node tools/lint.js` runs a file the branch carries: a checkout or pull that changes it
//! changes what the hook does without changing the hook. `scan` reads each hook's text with the
//! word scanner the config programs use (`programs.rs`), with its rules and `follow::far` guard,
//! so the trust scan follows those files as it follows a config program's. A bare word
//! (`npx lint-staged`, `git diff`, `npm test`) is no path here (`path_of`'s `hook`), and
//! `$(dirname "$0")` is read as the hooks folder (husky's `. "$(dirname "$0")/h"`).
//!
//! Bounded: at most `MAX_WORDS` words of one hook, lines up to `MAX_LINE_BYTES`, and
//! `MAX_FILES` files in all. Past any of them, or for a hook too large to keep its text
//! (`MAX_TEXT_BYTES`), the whole working tree is followed.
//! A hook with a NUL byte in it is a compiled program: it has no words to read.

use super::{candidates, path_of};
use std::path::{Path, PathBuf};

/// The most bytes of a hook whose text is read for the files it runs.
pub(in crate::trust) const MAX_TEXT_BYTES: usize = 1024 * 1024;

/// The most words read from one hook.
const MAX_WORDS: usize = 4096;

/// The longest line read; the word scanner's work grows with a line's words.
const MAX_LINE_BYTES: usize = 4096;

/// The most files followed for what hooks run.
const MAX_FILES: usize = 64;

/// The ways a hook script names its own folder.
const OWN_FOLDER: &[&str] = &[
    "$(dirname \"$0\")",
    "$(dirname -- \"$0\")",
    "$(dirname $0)",
    "$(dirname -- $0)",
    "`dirname \"$0\"`",
    "`dirname -- \"$0\"`",
    "`dirname $0`",
];

/// The files in the working tree (absolute paths, maybe missing) that the hooks in `hooks_dir`
/// with these `texts` run: each hook's bytes, or None for one read but too large to keep. A file
/// at a place `net_path::may_follow` refuses is left out and noted in `offsite`; one whose
/// bytes name no file here is named in `unread`.
pub(in crate::trust) fn scan(
    workdir: &Path,
    hooks_dir: &Path,
    texts: &[Option<Vec<u8>>],
    offsite: &mut Vec<String>,
    unread: &mut Vec<String>,
) -> Vec<PathBuf> {
    let home = crate::settings::home_dir();
    let folder = folder_text(workdir, hooks_dir);
    let mut out: Vec<PathBuf> = Vec::new();
    let mut whole = false;
    'hooks: for text in texts {
        let Some(bytes) = text else {
            whole = true;
            continue;
        };
        if bytes.contains(&0) {
            continue;
        }
        let mut text = super::super::keys::text_of(bytes).into_owned();
        if let Some(folder) = &folder {
            for form in OWN_FOLDER {
                text = text.replace(form, folder);
            }
        }
        let mut words = 0;
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            if line.len() > MAX_LINE_BYTES {
                whole = true;
                continue 'hooks;
            }
            for word in candidates(line) {
                words += 1;
                if words > MAX_WORDS {
                    whole = true;
                    continue 'hooks;
                }
                for p in path_of(workdir, home.as_deref(), &word, true, unread) {
                    if super::super::follow::note_far(&p, workdir, offsite) || out.contains(&p) {
                        continue;
                    }
                    if out.len() >= MAX_FILES {
                        whole = true;
                        break 'hooks;
                    }
                    out.push(p);
                }
            }
        }
    }
    if whole {
        out.push(workdir.to_path_buf());
    }
    out
}

/// `hooks_dir` as a hook's words would spell it: relative to the working tree when it is there,
/// `/`-separated. None when it has a character the word scanner reads as shell syntax.
fn folder_text(workdir: &Path, hooks_dir: &Path) -> Option<String> {
    let rel = hooks_dir.strip_prefix(workdir).ok().filter(|r| !r.as_os_str().is_empty());
    let text = rel.unwrap_or(hooks_dir).to_str()?.replace('\\', "/");
    let plain =
        !text.is_empty() && !text.starts_with('~') && !text.contains(|c: char| c.is_whitespace() || "\"'`$;|&()<>=,*?[]{}!#".contains(c));
    plain.then_some(text)
}

#[cfg(test)]
#[path = "programs_hooks_tests.rs"]
mod tests;
