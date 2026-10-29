//! The repositories nested in a swept work tree (submodules, and repositories cloned or made
//! inside it): each folder's own ignore rules decide what below it is walked, and a fingerprint
//! of its HEAD and refs is compared every sweep, since its git folder is never walked. A commit,
//! checkout or ref change there is reported as `<folder>/.git/HEAD`, the path the watcher would
//! classify for it. At most `MAX_NESTED` are tracked; past that the rest are walked as plain
//! folders, by the outer rules.

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

/// Starts tracking the repository in `rel` (relative to `workdir`), unless it is tracked already
/// or `MAX_NESTED` are.
pub(super) fn add(workdir: &Path, nested: &mut BTreeMap<PathBuf, Nested>, rel: &Path) {
    if nested.contains_key(rel) || nested.len() >= MAX_NESTED {
        return;
    }
    let abs = workdir.join(rel);
    nested.insert(rel.to_path_buf(), Nested { ignores: Ignores::open(&abs), refs: refs_of(&abs) });
}

/// Stops tracking the repositories in `rel` and below it.
pub(super) fn drop_under(nested: &mut BTreeMap<PathBuf, Nested>, rel: &Path) {
    nested.retain(|k, _| !k.starts_with(rel));
}

/// The innermost tracked repository holding `rel` (a folder below its own), with `rel` relative
/// to it.
pub(super) fn owner<'a, 'r>(nested: &'a mut BTreeMap<PathBuf, Nested>, rel: &'r Path) -> Option<(&'a mut Nested, &'r Path)> {
    let key = nested.keys().filter(|k| rel != k.as_path() && rel.starts_with(k)).max_by_key(|k| k.components().count())?.clone();
    let inner = rel.strip_prefix(&key).ok()?;
    Some((nested.get_mut(&key)?, inner))
}

/// Compares every tracked repository's HEAD and refs, naming `<folder>/.git/HEAD` in `found`
/// for each that moved, and has their ignore rules and index read again before they are next
/// asked (a `.gitignore` or the index may have changed since).
pub(super) fn compare(workdir: &Path, nested: &mut BTreeMap<PathBuf, Nested>, found: &mut Vec<PathBuf>) {
    for (rel, n) in nested.iter_mut() {
        n.ignores.clear();
        n.ignores.index_changed();
        let abs = workdir.join(rel);
        let refs = refs_of(&abs);
        if refs != n.refs {
            n.refs = refs;
            found.push(abs.join(".git").join("HEAD"));
        }
    }
}

/// A fingerprint of the HEAD and refs of the repository whose work tree is `abs` (its `.git` a
/// folder, or a file naming the git dir); None when it doesn't open.
fn refs_of(abs: &Path) -> Option<u64> {
    let repo = git2::Repository::open_ext(abs, git2::RepositoryOpenFlags::NO_SEARCH, std::iter::empty::<&std::ffi::OsStr>()).ok()?;
    let mut h = DefaultHasher::new();
    match repo.find_reference("HEAD") {
        Ok(head) => {
            head.symbolic_target_bytes().hash(&mut h);
            head.target().hash(&mut h);
        }
        Err(_) => 0u8.hash(&mut h),
    }
    for r in repo.references().ok()?.flatten() {
        r.name_bytes().hash(&mut h);
        r.target().hash(&mut h);
        r.symbolic_target_bytes().hash(&mut h);
    }
    Some(h.finish())
}
