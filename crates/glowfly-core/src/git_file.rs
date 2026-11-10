//! The git dirs a repository's own files name (a `.git` file's `gitdir:`, a `commondir`, a
//! linked worktree's `gitdir` record), read bounded (regular files only) and followed only where
//! `net_path::may_follow` allows from `home`, the place already in use. Also whether another
//! repository (a nested one, a submodule) can be opened by libgit2 without it reaching a refused
//! place: the links in its git dir, the files its config includes, its `core.worktree`,
//! `core.excludesFile` and `core.attributesFile`, and its object store with the alternates it
//! names (up to libgit2's depth of 5, `alternates.rs`). The linked worktrees under a common dir
//! are listed here too (`admin.rs`), as libgit2's own list follows links in their admin dirs.

mod admin;
mod alternates;

pub(crate) use admin::{admin_locked, worktree_admins};

use crate::config_includes::{self, read_file_bounded};
use crate::net_path::may_follow;
use git2::{ConfigLevel, DiffOptions, Oid, Repository};
use std::path::{Path, PathBuf};

/// The most bytes read of a `.git` file, `commondir`, HEAD, ref or worktree record.
pub(crate) const SMALL_FILE: u64 = 4096;

/// A path one file names.
#[derive(Debug, PartialEq)]
pub(crate) enum Named {
    /// The file is missing, unreadable or names nothing.
    None,
    At(PathBuf),
    /// It names a place that may not be followed (the path given back).
    Refused(PathBuf),
}

/// A repository's git dir and common dir (the git dir itself without a `commondir`).
#[derive(Debug, PartialEq)]
pub(crate) struct RepoDirs {
    pub git_dir: PathBuf,
    pub common: PathBuf,
}

/// The repository a folder holds.
#[derive(Debug, PartialEq)]
pub(crate) enum Found {
    /// No `.git`, or one that names nothing.
    None,
    Repo(RepoDirs),
    /// Its files name a place that may not be followed (the first one found).
    Refused(PathBuf),
}

/// `file`'s bytes (a regular file of at most `SMALL_FILE`) when `may_follow(file, base)`, with
/// `base` the allowed folder holding it.
pub(crate) fn read_small_in(file: &Path, base: &Path) -> Option<Vec<u8>> {
    if !may_follow(file, base) {
        return None;
    }
    read_file_bounded(file, SMALL_FILE)
}

/// `bytes` up to the first NUL: libgit2 reads a `.git` file, `commondir`, a worktree record and
/// an alternates file as a C string, so nothing past a NUL is ever used.
fn up_to_nul(bytes: &[u8]) -> &[u8] {
    bytes.iter().position(|b| *b == 0).map_or(bytes, |n| &bytes[..n])
}

/// libgit2's `git__isspace`: ASCII whitespace with VT, which `u8::is_ascii_whitespace` leaves out.
fn git_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\x0b' | b'\x0c' | b'\r')
}

/// A file naming one path, as libgit2 reads it.
#[derive(Debug, PartialEq)]
enum Value {
    Missing,
    /// Possibly empty.
    Text(String),
    /// libgit2 would use a value this can't check the same way: one still holding a LF or CR
    /// (libgit2 takes every line as one path), one that isn't UTF-8, or a file that can't be read
    /// whole (not a regular file, or over `SMALL_FILE`). Never followed.
    Unsure,
}

/// `file` read as libgit2 reads a `.git` file, `commondir` or worktree record
/// (`git_futils_readbuffer`, then `git_str_rtrim`): trailing ASCII whitespace (`git__isspace`)
/// trimmed from the whole file first, then cut at the first NUL with nothing trimmed again (so
/// `x<SP><NUL>` is `x<SP>`), every line kept.
fn value_of(file: &Path) -> Value {
    let Some(bytes) = read_file_bounded(file, SMALL_FILE) else {
        return if std::fs::symlink_metadata(file).is_ok() { Value::Unsure } else { Value::Missing };
    };
    value_in(&bytes)
}

fn value_in(bytes: &[u8]) -> Value {
    let trimmed = &bytes[..bytes.iter().rposition(|c| !git_space(*c)).map_or(0, |n| n + 1)];
    match std::str::from_utf8(up_to_nul(trimmed)) {
        Ok(t) if !t.contains(['\n', '\r']) && !t.ends_with(unicode_space) => Value::Text(t.to_owned()),
        _ => Value::Unsure,
    }
}

/// A non-ASCII space: on Unix libgit2 trims with `isspace()`, which a locale other than C may
/// widen, so a value starting or ending with one is never followed.
fn unicode_space(c: char) -> bool {
    !c.is_ascii() && c.is_whitespace()
}

/// A `commondir` or worktree record's value as libgit2 resolves it: absolute as it is, and
/// joined to `base` only when it starts with `./` or `../` (`git_fs_path_is_relative`, `/`
/// only). None for any other value, which libgit2 resolves against the process's own folder.
fn joined_as_libgit2(base: &Path, named: &str) -> Option<PathBuf> {
    if Path::new(named).is_absolute() {
        Some(PathBuf::from(named))
    } else if named.starts_with("./") || named.starts_with("../") {
        Some(base.join(named))
    } else {
        None
    }
}

/// `named` joined to `base` unless absolute.
fn joined(base: &Path, named: &str) -> PathBuf {
    let named = Path::new(named);
    if named.is_absolute() { named.to_path_buf() } else { base.join(named) }
}

/// The common dir `git_dir`'s `commondir` file names (absolute, or relative to `git_dir` when it
/// starts with `./` or `../`), resolved only once it is allowed from `home`. None without one.
pub(crate) fn common_dir_of(git_dir: &Path, home: &Path) -> Named {
    let file = git_dir.join("commondir");
    if !may_follow(&file, git_dir) {
        return Named::Refused(file);
    }
    // libgit2's `lookup_commondir`: the whole trimmed file (an empty one is resolved against the
    // process's own folder).
    let text = match value_of(&file) {
        Value::Missing => return Named::None,
        Value::Text(t) => t,
        Value::Unsure => return Named::Refused(file),
    };
    let Some(named) = joined_as_libgit2(git_dir, &text) else { return Named::Refused(file) };
    if !may_follow(&named, home) {
        return Named::Refused(named);
    }
    Named::At(crate::paths::resolve(&named))
}

/// The git dir and common dir of the repository whose work tree is `folder`: its `.git` folder,
/// or the one its `.git` file names (relative to `folder` unless absolute). Cheap: the `.git`
/// entry and `commondir`, nothing opened.
pub(crate) fn dirs_of(folder: &Path, home: &Path) -> Found {
    let dot = folder.join(".git");
    // Also covers a `.git` that is a link, and a folder that is one.
    if !may_follow(&dot, home) {
        return Found::Refused(dot);
    }
    let Ok(meta) = std::fs::metadata(&dot) else { return Found::None };
    let git_dir = if meta.is_dir() {
        dot
    } else {
        // libgit2's `read_gitfile`: the whole trimmed file after `gitdir:` and any ASCII
        // whitespace, joined to the folder unless absolute.
        let text = match value_of(&dot) {
            Value::Missing => return Found::None,
            Value::Text(t) => t,
            Value::Unsure => return Found::Refused(dot),
        };
        let Some(named) = text.strip_prefix("gitdir:").map(|g| g.trim_start_matches(|c: char| c.is_ascii() && git_space(c as u8))) else {
            return Found::None;
        };
        // Nothing after `gitdir:`: libgit2 counts the file as malformed.
        if named.is_empty() {
            return Found::None;
        }
        if named.starts_with(unicode_space) {
            return Found::Refused(dot);
        }
        let named = joined(folder, named);
        if !may_follow(&named, home) {
            return Found::Refused(named);
        }
        named
    };
    match common_dir_of(&git_dir, home) {
        Named::At(common) => Found::Repo(RepoDirs { git_dir, common }),
        Named::None => Found::Repo(RepoDirs { common: git_dir.clone(), git_dir }),
        Named::Refused(p) => Found::Refused(p),
    }
}

/// `dirs_of`, and whether libgit2 can open the repository without reaching a refused place.
pub(crate) fn other_repo(folder: &Path, home: &Path) -> Found {
    match dirs_of(folder, home) {
        Found::Repo(d) => match openable(&d, folder, home) {
            Ok(()) => Found::Repo(d),
            Err(p) => Found::Refused(p),
        },
        other => other,
    }
}

/// The first place libgit2 opening the repository of `d` would reach that may not be followed:
/// a link directly in its git dir, common dir or their `info/`, a link in its object store or a
/// store its alternates name (followed to libgit2's depth of 5), a file its config includes, or
/// its `core.worktree`, `core.excludesFile` or `core.attributesFile`.
fn openable(d: &RepoDirs, folder: &Path, home: &Path) -> Result<(), PathBuf> {
    let mut dirs = vec![d.git_dir.clone()];
    if d.common != d.git_dir {
        dirs.push(d.common.clone());
    }
    for dir in &dirs {
        for at in [dir.clone(), dir.join("info")] {
            let Ok(read) = std::fs::read_dir(&at) else { continue };
            for e in read.flatten() {
                // The listing's own type: a link is never followed to tell.
                if e.file_type().is_ok_and(|t| t.is_symlink()) && !may_follow(&e.path(), &at) {
                    return Err(e.path());
                }
            }
        }
    }
    // Read when a status compares the submodule's HEAD tree.
    alternates::reachable(&d.common, home)?;
    let roots = config_includes::repo_roots(&d.git_dir, Some(&d.common));
    config_includes::included_within(&roots, &config_includes::Context::of_repo(&d.git_dir), &|p| may_follow(p, home))?;
    // The includes are allowed, so libgit2 may follow them; an unreadable config fails the
    // open by itself.
    let Ok(mut cfg) = git2::Config::new() else { return Ok(()) };
    for (file, level) in [(d.common.join("config"), ConfigLevel::Local), (d.git_dir.join("config.worktree"), ConfigLevel::Worktree)] {
        if file.is_file() {
            let _ = cfg.add_file(&file, level, false);
        }
    }
    let value = |key: &str, base: &Path| cfg.get_path(key).ok().map(|p| if p.is_absolute() { p } else { base.join(p) });
    let named = [value("core.worktree", &d.git_dir), value("core.excludesFile", folder), value("core.attributesFile", folder)];
    match named.into_iter().flatten().find(|p| !may_follow(p, home)) {
        Some(p) => Err(p),
        None => Ok(()),
    }
}

/// The `.git` file of the linked worktree whose admin dir (`<common>/worktrees/<id>`) is
/// `admin`, as its `gitdir` record names it: absolute, or relative to `admin` when it starts with
/// `./` or `../` (`git worktree add --relative-paths`). Refused when it may not be followed from
/// `home`, or when libgit2 would read it differently (`Value::Unsure`, another relative one).
pub(crate) fn worktree_folder(admin: &Path, home: &Path) -> Named {
    let file = admin.join("gitdir");
    if !may_follow(&file, admin) {
        return Named::None;
    }
    // libgit2's `git_worktree__read_link`: the whole trimmed record.
    let text = match value_of(&file) {
        Value::Missing => return Named::None,
        Value::Text(t) => t,
        Value::Unsure => return Named::Refused(file),
    };
    // An empty record is an absolute path to libgit2 (`dirname("")`): refused below too.
    let Some(named) = joined_as_libgit2(admin, &text) else { return Named::Refused(file) };
    if may_follow(&named, home) { Named::At(named) } else { Named::Refused(named) }
}

/// The commit the HEAD file in linked worktree `name`'s admin dir (`<common>/worktrees/<name>`)
/// names: a ref inside the repository (`refs/` and below, no `..`) or a commit id. Read from the
/// admin dir alone, so a worktree whose folder may not be followed is never touched.
pub(crate) fn worktree_head(repo: &Repository, name: &str) -> Option<Oid> {
    let admin = repo.commondir().join("worktrees").join(name);
    let text = read_small_in(&admin.join("HEAD"), &admin)?;
    let text = std::str::from_utf8(&text).ok()?.trim();
    let oid = match text.strip_prefix("ref:").map(str::trim) {
        Some(r) if r.starts_with("refs/") && !r.split('/').any(|p| p == "..") => repo.refname_to_id(r).ok()?,
        Some(_) => return None,
        None => Oid::from_str(text).ok()?,
    };
    repo.find_object(oid, None).ok()?.peel_to_commit().ok().map(|c| c.id())
}

/// A gitlink whose folder or repository names a refused place.
#[derive(Debug, PartialEq)]
pub(crate) struct Offsite {
    /// Repository-relative, with `/`.
    pub path: String,
    /// The folder (or its `.git`) itself is a link leading there (libgit2's submodule list
    /// stats it through the link), or the gitlink's name can't be looked at.
    pub link: bool,
}

/// The gitlinks in `repo`'s index whose folder, or the repository in it, names a place that
/// may not be followed from the work tree, and (as `link`) the ones whose name can't be looked
/// at. None in a bare repository (libgit2 looks no submodule up there) or without a readable
/// index (libgit2's status then fails by itself).
pub(crate) fn offsite_gitlinks(repo: &Repository) -> Vec<Offsite> {
    let (Some(workdir), Ok(mut index)) = (repo.workdir(), repo.index()) else { return Vec::new() };
    let _ = index.read(false);
    let mut out = Vec::new();
    let mut last: Option<Vec<u8>> = None;
    for e in index.iter() {
        // A conflicted gitlink has an entry per side.
        if e.mode != 0o160000 || last.as_ref() == Some(&e.path) {
            continue;
        }
        last = Some(e.path.clone());
        // libgit2 joins the name's bytes to the work tree as they are. A name that can't be
        // joined the same way here (not UTF-8, or not a plain relative path inside the work
        // tree) can't be looked at, so it counts as one libgit2 must be kept from.
        let path = match String::from_utf8(e.path) {
            Ok(p) => p,
            Err(e) => {
                out.push(Offsite { path: String::from_utf8_lossy(e.as_bytes()).into_owned(), link: true });
                continue;
            }
        };
        let Ok(folder) = crate::repo::work_path(workdir, &path) else {
            out.push(Offsite { path, link: true });
            continue;
        };
        let dot = folder.join(".git");
        if !may_follow(&folder, workdir) {
            out.push(Offsite { path, link: true });
        } else if let Found::Refused(p) = other_repo(&folder, workdir) {
            out.push(Offsite { path, link: p == dot });
        }
    }
    out
}

/// Whether the repository of `d`, found in `folder`, has `folder` as the work tree libgit2 would
/// give it (`load_workdir`): no linked-worktree record or `commondir` in its git dir, and every
/// `core.worktree` its own config files set (as written; relative to the git dir) resolves to
/// `folder`. False when that can't be told. Called once `other_repo` allowed the repository, so
/// the config files' includes are ones libgit2 may follow.
pub(crate) fn work_tree_is(d: &RepoDirs, folder: &Path) -> bool {
    // libgit2 takes a git dir with a `gitdir` file as a linked worktree's and uses the folder
    // that record names.
    if d.common != d.git_dir || d.git_dir.join("gitdir").symlink_metadata().is_ok() {
        return false;
    }
    let Ok(mut cfg) = git2::Config::new() else { return false };
    for (file, level) in [(d.common.join("config"), ConfigLevel::Local), (d.git_dir.join("config.worktree"), ConfigLevel::Worktree)] {
        if file.is_file() && cfg.add_file(&file, level, false).is_err() {
            return false;
        }
    }
    let Ok(entries) = cfg.entries(Some(r"^core\.worktree$")) else { return false };
    let real = crate::paths::resolve(folder);
    let mut same = true;
    let read = entries.for_each(|e| {
        same &= crate::config_entry::utf8(e).is_some_and(|v| !v.is_empty() && crate::paths::resolve(&joined(&d.git_dir, v)) == real);
    });
    read.is_ok() && same
}

/// Whether libgit2 is kept from `repo`'s submodules: one of its gitlinks names a place that may
/// not be followed (`offsite_gitlinks`), or its `.gitmodules` or that of a checked-out submodule
/// at any depth is one libgit2 would read differently from git (`gitmodules::unread_anywhere`;
/// libgit2's submodule code reads those files itself).
pub(crate) fn submodules_unread(repo: &Repository) -> bool {
    !offsite_gitlinks(repo).is_empty() || crate::gitmodules::unread_anywhere(repo)
}

/// `o` for a diff that reaches `repo`'s work tree, with every submodule left out when libgit2
/// is kept from them (`submodules_unread`): its diff reads `.gitmodules` and opens each
/// submodule folder's repository to compare it, as its status does.
pub(crate) fn skip_offsite_submodules<'o>(repo: &Repository, o: &'o mut DiffOptions) -> &'o mut DiffOptions {
    if submodules_unread(repo) {
        o.ignore_submodules(true);
    }
    o
}

#[cfg(test)]
#[path = "git_file_tests.rs"]
mod tests;
