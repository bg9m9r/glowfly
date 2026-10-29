//! The config files git pulls in through `include.path` and `includeIf.<condition>.path`, so the
//! watcher and the repository pool notice edits to them as they do to the main config files.
//!
//! Resolved the way git does: a relative path from the folder of the file that includes it, `~/`
//! from the home folder; `gitdir:` and `gitdir/i:` (a pattern from `~/`, from the including
//! file's folder for `./`, otherwise anywhere below `**/`, everything inside for a trailing `/`)
//! matched against the git dir as given and resolved; `onbranch:` against the checked-out branch.
//! `hasconfig:remote.*.url:` needs every remote of the whole config, so it counts as met: a file
//! watched or stamped for nothing costs a reload at most. Any other condition, `~user/` and
//! `%(prefix)/` paths are left out, as git leaves out what it can't evaluate.
//!
//! Bounded: at most `MAX_DEPTH` levels below the main files (git's own limit, past which git
//! refuses the config), `MAX_FILES` files and `MAX_FILE_BYTES` read from each, and a file is
//! read once however many times it is included, so a cycle ends. A file that doesn't exist is
//! still listed (its creation is a change), and so is one too large to read.

use std::path::{Path, PathBuf};

mod parse;
#[cfg(test)]
mod tests;
mod wildmatch;

/// The deepest include read (git refuses a config nested deeper).
pub const MAX_DEPTH: usize = 10;
/// The most included files listed.
pub const MAX_FILES: usize = 64;
/// Files larger than this are listed but not read.
pub const MAX_FILE_BYTES: u64 = 1 << 20;

/// What conditions are evaluated against.
#[derive(Clone, Debug, Default)]
pub struct Context {
    /// The repository's git dir (a linked worktree's own); None meets no `gitdir:` condition.
    pub git_dir: Option<PathBuf>,
    /// The checked-out branch's short name; None (detached, or no repository) meets no
    /// `onbranch:` condition.
    pub branch: Option<Vec<u8>>,
    /// Where `~/` points.
    pub home: Option<PathBuf>,
}

impl Context {
    /// For the repository whose git dir is `git_dir`, with the user's home folder.
    pub fn of_repo(git_dir: &Path) -> Context {
        Context { git_dir: Some(git_dir.to_path_buf()), branch: head_branch(git_dir), home: crate::settings::home_dir() }
    }
}

/// What `included` found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Included {
    /// The included files, resolved (`paths::resolve`), in the order git reads them, each once.
    pub files: Vec<PathBuf>,
    /// An `onbranch:` condition was read: switching branches can change `files`.
    pub on_branch: bool,
}

/// The files `roots` include, directly or through each other, for `cx`. The roots themselves
/// are never listed.
pub fn included(roots: &[PathBuf], cx: &Context) -> Included {
    let mut walk = Walk { cx, seen: roots.iter().map(|r| crate::paths::resolve(r)).collect(), out: Included::default() };
    for root in roots {
        walk.read(&crate::paths::resolve(root), 0);
    }
    walk.out
}

/// The files the repository's own config files (the common dir's `config`, the worktree's
/// `config.worktree`) include. `common`: a linked worktree's common git dir.
pub fn of_repo(git_dir: &Path, common: Option<&Path>) -> Included {
    included(&repo_roots(git_dir, common), &Context::of_repo(git_dir))
}

/// The repository's own config files: the common dir's `config` and the worktree's
/// `config.worktree`.
pub fn repo_roots(git_dir: &Path, common: Option<&Path>) -> Vec<PathBuf> {
    vec![common.unwrap_or(git_dir).join("config"), git_dir.join("config.worktree")]
}

struct Walk<'a> {
    cx: &'a Context,
    /// Every file listed or a root, so each is read once.
    seen: Vec<PathBuf>,
    out: Included,
}

impl Walk<'_> {
    /// Lists what `file` (at `depth` below the roots) includes, and reads those in turn.
    fn read(&mut self, file: &Path, depth: usize) {
        let Some(text) = read_small(file) else { return };
        for entry in parse::includes(&text) {
            if let Some(c) = &entry.condition
                && !self.met(c, file)
            {
                continue;
            }
            let Some(path) = self.resolve(&entry.path, file) else { continue };
            if self.seen.contains(&path) {
                continue;
            }
            if self.out.files.len() >= MAX_FILES {
                return;
            }
            self.seen.push(path.clone());
            self.out.files.push(path.clone());
            if depth + 1 < MAX_DEPTH {
                self.read(&path, depth + 1);
            }
        }
    }

    /// Where an include's `value`, written in `file`, points; None when it can't be told.
    fn resolve(&self, value: &[u8], file: &Path) -> Option<PathBuf> {
        let named = path_of(&home_expanded(value, self.cx.home.as_deref())?);
        let joined = if named.is_absolute() { named } else { file.parent()?.join(named) };
        Some(crate::paths::resolve(&joined))
    }

    fn met(&mut self, condition: &[u8], file: &Path) -> bool {
        if let Some(p) = condition.strip_prefix(b"gitdir:") {
            self.gitdir(p, false, file)
        } else if let Some(p) = condition.strip_prefix(b"gitdir/i:") {
            self.gitdir(p, true, file)
        } else if let Some(p) = condition.strip_prefix(b"onbranch:") {
            self.out.on_branch = true;
            let Some(branch) = &self.cx.branch else { return false };
            wildmatch::matches(&folder_pattern(p.to_vec()), branch, false)
        } else {
            condition.starts_with(b"hasconfig:remote.*.url:")
        }
    }

    fn gitdir(&self, pattern: &[u8], fold: bool, file: &Path) -> bool {
        let Some(git_dir) = &self.cx.git_dir else { return false };
        let Some(mut p) = home_expanded(pattern, self.cx.home.as_deref()) else { return false };
        if p.starts_with(b"./") {
            let Some(dir) = file.parent() else { return false };
            let mut from = escaped(&text_of(dir));
            from.extend_from_slice(&p[1..]);
            p = from;
        } else if !absolute(&p) {
            p = [b"**/".as_slice(), &p].concat();
        }
        let p = folder_pattern(p);
        let given = text_of(git_dir);
        let real = text_of(&crate::paths::resolve(git_dir));
        wildmatch::matches(&p, &given, fold) || (real != given && wildmatch::matches(&p, &real, fold))
    }
}

/// `file`'s bytes when it is a file no larger than `MAX_FILE_BYTES`.
fn read_small(file: &Path) -> Option<Vec<u8>> {
    let meta = std::fs::metadata(file).ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return None;
    }
    std::fs::read(file).ok()
}

/// A pattern naming a folder (a trailing `/`) also matches everything inside it.
fn folder_pattern(mut p: Vec<u8>) -> Vec<u8> {
    if p.ends_with(b"/") {
        p.extend_from_slice(b"**");
    }
    p
}

/// `value` with a leading `~/` (or a lone `~`) taken from `home`; None for `~user/`,
/// `%(prefix)/` and a `~` with no home.
fn home_expanded(value: &[u8], home: Option<&Path>) -> Option<Vec<u8>> {
    if value.starts_with(b"%(prefix)/") {
        return None;
    }
    let Some(rest) = value.strip_prefix(b"~") else { return Some(value.to_vec()) };
    if !rest.is_empty() && !rest.starts_with(b"/") {
        return None;
    }
    let mut out = text_of(home?);
    while out.ends_with(b"/") {
        out.pop();
    }
    out.extend_from_slice(rest);
    Some(out)
}

/// Whether git takes `p` as a full path: a leading slash, or on Windows a drive or backslash.
fn absolute(p: &[u8]) -> bool {
    p.starts_with(b"/") || (cfg!(windows) && (p.starts_with(b"\\") || (p.len() >= 2 && p[0].is_ascii_alphabetic() && p[1] == b':')))
}

/// `p` with the pattern characters escaped, so it matches only itself.
fn escaped(p: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(p.len());
    for &b in p {
        if matches!(b, b'*' | b'?' | b'[' | b'\\') {
            out.push(b'\\');
        }
        out.push(b);
    }
    out
}

/// A path as git spells it for matching: its bytes, with forward slashes on Windows and no
/// trailing slash (libgit2 gives the git dir with one).
fn text_of(p: &Path) -> Vec<u8> {
    let mut b = p.as_os_str().as_encoded_bytes().to_vec();
    if cfg!(windows) {
        for c in &mut b {
            if *c == b'\\' {
                *c = b'/';
            }
        }
    }
    while b.len() > 1 && b.ends_with(b"/") {
        b.pop();
    }
    b
}

/// The path a config value names (UTF-8 on Windows, as Git for Windows reads it; bytes elsewhere).
fn path_of(b: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        PathBuf::from(std::ffi::OsStr::from_bytes(b))
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(String::from_utf8_lossy(b).into_owned())
    }
}

/// The short name of the branch `git_dir`'s HEAD names (born or not); None when detached.
pub fn head_branch(git_dir: &Path) -> Option<Vec<u8>> {
    let head = std::fs::read(git_dir.join("HEAD")).ok()?;
    let target = head.strip_prefix(b"ref:")?.trim_ascii();
    Some(target.strip_prefix(b"refs/heads/")?.to_vec())
}
