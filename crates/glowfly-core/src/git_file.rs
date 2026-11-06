//! The git dirs a repository's own files name (a `.git` file's `gitdir:`, a `commondir`, a
//! linked worktree's `gitdir` record), read bounded (regular files only) and followed only where
//! `net_path::may_follow` allows from `home`, the place already in use. Also whether another
//! repository (a nested one, a submodule) can be opened by libgit2 without it reaching a refused
//! place: the links in its git dir, the files its config includes, and its `core.worktree`,
//! `core.excludesFile` and `core.attributesFile`.

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

fn text_of(file: &Path) -> Option<String> {
    read_file_bounded(file, SMALL_FILE).and_then(|b| String::from_utf8(b).ok())
}

/// `named` joined to `base` unless absolute.
fn joined(base: &Path, named: &str) -> PathBuf {
    let named = Path::new(named);
    if named.is_absolute() { named.to_path_buf() } else { base.join(named) }
}

/// The common dir `git_dir`'s `commondir` file names (relative to `git_dir` unless absolute),
/// resolved only once it is allowed from `home`. None without one.
pub(crate) fn common_dir_of(git_dir: &Path, home: &Path) -> Named {
    let file = git_dir.join("commondir");
    if !may_follow(&file, git_dir) {
        return Named::Refused(file);
    }
    let Some(text) = text_of(&file) else { return Named::None };
    if text.trim().is_empty() {
        return Named::None;
    }
    let named = joined(git_dir, text.trim_end_matches(['\n', '\r']));
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
        let line = text_of(&dot).and_then(|t| t.lines().next().and_then(|l| l.strip_prefix("gitdir:")).map(|g| g.trim().to_owned()));
        let Some(line) = line.filter(|l| !l.is_empty()) else { return Found::None };
        let named = joined(folder, &line);
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
/// a link directly in its git dir, common dir or their `info/`, a file its config includes, or
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
/// `admin`, as its `gitdir` record names it: absolute, or relative to `admin` (`git worktree add
/// --relative-paths`). Refused when it may not be followed from `home`.
pub(crate) fn worktree_folder(admin: &Path, home: &Path) -> Named {
    let text = read_small_in(&admin.join("gitdir"), admin).and_then(|b| String::from_utf8(b).ok());
    let Some(line) = text.as_deref().and_then(|t| t.lines().next()).map(str::trim).filter(|l| !l.is_empty()) else {
        return Named::None;
    };
    let named = joined(admin, line);
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
    /// The folder (or its `.git`) itself is a link leading there: libgit2's submodule list
    /// stats it through the link.
    pub link: bool,
}

/// The gitlinks in `repo`'s index whose folder, or the repository in it, names a place that
/// may not be followed from the work tree.
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
        let Ok(path) = String::from_utf8(e.path) else { continue };
        // One part at a time, so the folder uses the platform's separators.
        let folder = path.split('/').fold(workdir.to_path_buf(), |p, c| p.join(c));
        let dot = folder.join(".git");
        if !may_follow(&folder, workdir) {
            out.push(Offsite { path, link: true });
        } else if let Found::Refused(p) = other_repo(&folder, workdir) {
            out.push(Offsite { path, link: p == dot });
        }
    }
    out
}

/// `o` for a diff that reaches `repo`'s work tree, with every submodule left out when one of its
/// gitlinks names a place that may not be followed (`offsite_gitlinks`): libgit2's diff opens
/// each submodule folder's repository to compare it, as its status does.
pub(crate) fn skip_offsite_submodules<'o>(repo: &Repository, o: &'o mut DiffOptions) -> &'o mut DiffOptions {
    if !offsite_gitlinks(repo).is_empty() {
        o.ignore_submodules(true);
    }
    o
}

#[cfg(test)]
#[path = "git_file_tests.rs"]
mod tests;
