//! The working-tree status as git reads it (`git status --porcelain=v2 -z`), for repositories
//! set up so that git is much faster than libgit2: fsmonitor and the untracked cache only help
//! git itself.

use super::{FileEntry, rpath};
use crate::gitcli::{NO_PROMPTS, READ_TIMEOUT, args, output_bytes};
use std::path::Path;

/// (unstaged, staged) as `status::libgit2_status` reports them, or None when git failed.
pub(super) fn status(workdir: &Path) -> Option<(Vec<FileEntry>, Vec<FileEntry>)> {
    // Without optional locks git doesn't rewrite the index, which the file watcher would report
    // as a change, reading the status again.
    let cmd = args(&["--no-optional-locks", "status", "--porcelain=v2", "-z", "--untracked-files=all"]);
    // Paths are git's bytes, which needn't be UTF-8: kept undecoded until `rpath` reads them.
    let (out, stdout) = output_bytes(workdir, &cmd, NO_PROMPTS, Some(READ_TIMEOUT));
    if !out.ok {
        return None;
    }
    Some(parse(&stdout))
}

/// `status` of only the work-tree paths `paths` (each taken literally, a folder covering
/// everything below it), or None when their command line would be too long
/// (`status_paths::CLI_PATH_BYTES`) or git failed.
pub(super) fn status_of(workdir: &Path, paths: &[String]) -> Option<(Vec<FileEntry>, Vec<FileEntry>)> {
    if paths.iter().map(|p| p.len() + 1).sum::<usize>() > super::status_paths::CLI_PATH_BYTES {
        return None;
    }
    // Literal pathspecs, so a name like `[ab].txt` isn't a pattern matching `a.txt`.
    let mut cmd = args(&["--no-optional-locks", "--literal-pathspecs", "status", "--porcelain=v2", "-z", "--untracked-files=all", "--"]);
    cmd.extend(paths.iter().cloned());
    let (out, stdout) = output_bytes(workdir, &cmd, NO_PROMPTS, Some(READ_TIMEOUT));
    out.ok.then(|| parse(&stdout))
}

/// Parses porcelain v2 records separated by NULs. Paths are taken whole, exactly as git wrote
/// them: a name may start or end with spaces.
pub(super) fn parse(out: &[u8]) -> (Vec<FileEntry>, Vec<FileEntry>) {
    let (mut unstaged, mut staged) = (Vec::new(), Vec::new());
    let mut records = out.split(|&b| b == 0);
    while let Some(rec) = records.next() {
        match rec.first() {
            // "1 XY sub mH mI mW hH hI path"
            Some(b'1') => {
                if let [_, xy, .., path] = fields(rec, 9)[..] {
                    changed(xy, path, &mut unstaged, &mut staged);
                }
            }
            // "2 XY sub mH mI mW hH hI Xscore path", then the original path as its own record.
            Some(b'2') => {
                if let [_, xy, .., path] = fields(rec, 10)[..] {
                    changed(xy, path, &mut unstaged, &mut staged);
                }
                records.next();
            }
            // "u XY sub m1 m2 m3 mW h1 h2 h3 path"
            Some(b'u') => {
                if let Some(path) = fields(rec, 11).get(10) {
                    unstaged.push(rpath::entry(path, '!'));
                }
            }
            // "? path": everything after the one space is the name.
            Some(b'?') => {
                if let Some(path) = rec.strip_prefix(b"? ") {
                    unstaged.push(rpath::entry(path, '?'));
                }
            }
            _ => {}
        }
    }
    unstaged.sort_by(|a, b| a.path.cmp(&b.path));
    staged.sort_by(|a, b| a.path.cmp(&b.path));
    (unstaged, staged)
}

/// The first `n` space-separated fields of `rec`, the last holding the rest.
fn fields(rec: &[u8], n: usize) -> Vec<&[u8]> {
    rec.splitn(n, |&b| b == b' ').collect()
}

fn changed(xy: &[u8], path: &[u8], unstaged: &mut Vec<FileEntry>, staged: &mut Vec<FileEntry>) {
    let (x, y) = (char::from(xy.first().copied().unwrap_or(b'.')), char::from(xy.get(1).copied().unwrap_or(b'.')));
    let index = match x {
        'A' | 'C' => Some('A'),
        'M' | 'D' | 'R' | 'T' => Some(x),
        _ => None,
    };
    let worktree = match y {
        'M' | 'D' | 'R' | 'T' => Some(y),
        // Added with intent to add: not in the index yet.
        'A' => Some('?'),
        _ => None,
    };
    if let Some(c) = index {
        staged.push(rpath::entry(path, c));
    }
    if let Some(c) = worktree {
        unstaged.push(rpath::entry(path, c));
    }
}
