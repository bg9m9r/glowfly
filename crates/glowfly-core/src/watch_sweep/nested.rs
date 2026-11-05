//! The repositories nested in a swept work tree (submodules, and repositories cloned or made
//! inside it): each folder's own ignore rules decide what below it is walked, and what the parent's
//! status reads of it is compared every sweep, since its git folder is never walked: its HEAD,
//! the loose ref of the branch HEAD names and the packed refs' stamp, a few file reads with no
//! repository opened. A commit or checkout there is reported as `<folder>/.git/HEAD`, the path
//! the watcher would classify for it. At most `MAX_NESTED` are tracked; past that the rest are
//! walked as plain folders, by the outer rules. So is one whose files (its `.git` file,
//! `commondir`, a link in its git dir, its config) name a network place other than the outer
//! work tree's (`git_file::other_repo`): its repository is never opened, nor its refs read.

use crate::git_file::{self, Found, read_small_in};
use crate::watch::Ignores;
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// The most nested repositories tracked (each keeps a repository open for its ignore rules).
pub(super) const MAX_NESTED: usize = 256;

/// One nested repository.
pub(super) struct Nested {
    pub(super) ignores: Ignores,
    refs: Option<u64>,
}

/// Starts tracking the repository in `rel` (relative to `workdir`), unless it is tracked already,
/// `MAX_NESTED` are, or its files name a place that may not be followed.
pub(super) fn add(workdir: &Path, nested: &mut BTreeMap<PathBuf, Nested>, rel: &Path) {
    if nested.contains_key(rel) || nested.len() >= MAX_NESTED {
        return;
    }
    let abs = workdir.join(rel);
    if let Found::Refused(p) = git_file::other_repo(&abs, workdir) {
        tracing::debug!(folder = %abs.display(), named = %p.display(), "nested repository names a network place: walked as a plain folder");
        return;
    }
    nested.insert(rel.to_path_buf(), Nested { ignores: Ignores::open(&abs), refs: refs_of(&abs, workdir) });
}

/// Stops tracking the repositories in `rel` and below it.
pub(super) fn drop_under(nested: &mut BTreeMap<PathBuf, Nested>, rel: &Path) {
    nested.retain(|k, _| !k.starts_with(rel));
}

/// The innermost tracked repository holding `rel` (a folder below its own), with `rel` relative
/// to it: one lookup per parent folder of `rel`, deepest first.
pub(super) fn owner<'a, 'r>(nested: &'a mut BTreeMap<PathBuf, Nested>, rel: &'r Path) -> Option<(&'a mut Nested, &'r Path)> {
    if nested.is_empty() {
        return None;
    }
    let key = rel.ancestors().skip(1).take_while(|a| !a.as_os_str().is_empty()).find(|a| nested.contains_key(*a))?;
    let inner = rel.strip_prefix(key).ok()?;
    Some((nested.get_mut(key)?, inner))
}

/// Compares every tracked repository's HEAD and refs, naming `<folder>/.git/HEAD` in `found`
/// for each that moved, and has their ignore rules and index read again before they are next
/// asked (a `.gitignore` or the index may have changed since). One whose `.git` now names a
/// place that may not be followed is named once and no longer tracked.
pub(super) fn compare(workdir: &Path, nested: &mut BTreeMap<PathBuf, Nested>, found: &mut Vec<PathBuf>) {
    let mut gone = Vec::new();
    for (rel, n) in nested.iter_mut() {
        let abs = workdir.join(rel);
        if let Found::Refused(p) = git_file::dirs_of(&abs, workdir) {
            tracing::debug!(folder = %abs.display(), named = %p.display(), "nested repository now names a network place: no longer tracked");
            found.push(abs.join(".git").join("HEAD"));
            gone.push(rel.clone());
            continue;
        }
        n.ignores.clear();
        n.ignores.index_changed();
        let refs = refs_of(&abs, workdir);
        if refs != n.refs {
            n.refs = refs;
            found.push(abs.join(".git").join("HEAD"));
        }
    }
    nested.retain(|k, _| !gone.contains(k));
}

/// A fingerprint of what the repository whose work tree is `abs` has checked out: its HEAD, the
/// loose ref of the branch HEAD names and the packed refs' size and time. None when `abs` holds
/// no repository (no `.git` naming a git dir with objects, or a HEAD that isn't a small regular
/// file), or its files name a place that may not be followed from `home` (`git_file`). Every
/// file is read bounded, regular files only, and only where `net_path` allows: their names come
/// from the nested repository's own content. The packed refs are stamped without following a
/// link.
pub(super) fn refs_of(abs: &Path, home: &Path) -> Option<u64> {
    let Found::Repo(d) = git_file::dirs_of(abs, home) else { return None };
    let objects = d.common.join("objects");
    if !crate::net_path::may_follow(&objects, &d.common) || !objects.is_dir() {
        return None;
    }
    let head = read_small_in(&d.git_dir.join("HEAD"), &d.git_dir)?;
    let mut h = DefaultHasher::new();
    head.hash(&mut h);
    if let Some(target) = head.strip_prefix(b"ref:").map(<[u8]>::trim_ascii).filter(|t| ref_name_ok(t)) {
        read_small_in(&d.common.join(String::from_utf8_lossy(target).as_ref()), &d.common).hash(&mut h);
    }
    std::fs::symlink_metadata(d.common.join("packed-refs")).ok().map(|m| (m.len(), m.modified().ok())).hash(&mut h);
    Some(h.finish())
}

/// Whether a symbolic ref's target names a ref inside the git dir: `refs/` and below, no `..`
/// part, no backslash or NUL (so it can't name a file elsewhere).
pub(super) fn ref_name_ok(target: &[u8]) -> bool {
    target.starts_with(b"refs/") && !target.iter().any(|&b| b == b'\\' || b == 0) && !target.split(|&b| b == b'/').any(|p| p == b"..")
}
