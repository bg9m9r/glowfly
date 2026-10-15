//! The working-tree status as git reads it (`git status --porcelain=v2 -z`), for repositories
//! set up so that git is much faster than libgit2: fsmonitor and the untracked cache only help
//! git itself.

use super::FileEntry;
use crate::gitcli::{args, read};
use std::path::Path;

/// (unstaged, staged) as `status::libgit2_status` reports them, or None when git failed.
pub(super) fn status(workdir: &Path) -> Option<(Vec<FileEntry>, Vec<FileEntry>)> {
    // Without optional locks git doesn't rewrite the index, which the file watcher would report
    // as a change, reading the status again.
    let cmd = args(&["--no-optional-locks", "status", "--porcelain=v2", "-z", "--untracked-files=all"]);
    let out = read(workdir, &cmd);
    if !out.ok {
        return None;
    }
    Some(parse(&out.stdout))
}

/// Parses porcelain v2 records separated by NULs.
pub(super) fn parse(text: &str) -> (Vec<FileEntry>, Vec<FileEntry>) {
    let (mut unstaged, mut staged) = (Vec::new(), Vec::new());
    let mut records = text.split('\0');
    while let Some(rec) = records.next() {
        let (kind, rest) = rec.split_at(rec.len().min(1));
        match kind {
            // "1 XY sub mH mI mW hH hI path"
            "1" => {
                if let [xy, .., path] = rec.splitn(9, ' ').skip(1).collect::<Vec<_>>()[..] {
                    changed(xy, path, &mut unstaged, &mut staged);
                }
            }
            // "2 XY sub mH mI mW hH hI Xscore path", then the original path as its own record.
            "2" => {
                if let [xy, .., path] = rec.splitn(10, ' ').skip(1).collect::<Vec<_>>()[..] {
                    changed(xy, path, &mut unstaged, &mut staged);
                }
                records.next();
            }
            // "u XY sub m1 m2 m3 mW h1 h2 h3 path"
            "u" => {
                if let Some(path) = rec.splitn(11, ' ').nth(10) {
                    unstaged.push(FileEntry { path: path.to_owned(), status: '!' });
                }
            }
            "?" => unstaged.push(FileEntry { path: rest.trim_start_matches(' ').to_owned(), status: '?' }),
            _ => {}
        }
    }
    unstaged.sort_by(|a, b| a.path.cmp(&b.path));
    staged.sort_by(|a, b| a.path.cmp(&b.path));
    (unstaged, staged)
}

fn changed(xy: &str, path: &str, unstaged: &mut Vec<FileEntry>, staged: &mut Vec<FileEntry>) {
    let mut chars = xy.chars();
    let (x, y) = (chars.next().unwrap_or('.'), chars.next().unwrap_or('.'));
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
        staged.push(FileEntry { path: path.to_owned(), status: c });
    }
    if let Some(c) = worktree {
        unstaged.push(FileEntry { path: path.to_owned(), status: c });
    }
}
