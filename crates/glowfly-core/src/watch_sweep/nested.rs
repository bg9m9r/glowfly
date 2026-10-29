//! The repositories nested in a swept work tree (submodules, and repositories cloned or made
//! inside it): each folder's own ignore rules decide what below it is walked, and what the parent's
//! status reads of it is compared every sweep, since its git folder is never walked: its HEAD,
//! the loose ref of the branch HEAD names and the packed refs' stamp, a few file reads with no
//! repository opened. A commit or checkout there is reported as `<folder>/.git/HEAD`, the path
//! the watcher would classify for it. At most `MAX_NESTED` are tracked; past that the rest are
//! walked as plain folders, by the outer rules.

use crate::config_includes::read_file_bounded;
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

/// The most bytes read of a nested repository's HEAD, ref or `.git` file.
const SMALL_FILE: u64 = 4096;

/// A fingerprint of what the repository whose work tree is `abs` has checked out: its HEAD, the
/// loose ref of the branch HEAD names and the packed refs' size and time. None when `abs` holds
/// no repository (no `.git` naming a git dir with objects, or a HEAD that isn't a small regular
/// file). Every file is read bounded, regular files only: their names come from the nested
/// repository's own content.
pub(super) fn refs_of(abs: &Path) -> Option<u64> {
    let git_dir = git_dir_of(abs)?;
    let common = crate::watch::common_dir(&git_dir).unwrap_or_else(|| git_dir.clone());
    if !common.join("objects").is_dir() {
        return None;
    }
    let head = read_file_bounded(&git_dir.join("HEAD"), SMALL_FILE)?;
    let mut h = DefaultHasher::new();
    head.hash(&mut h);
    if let Some(target) = head.strip_prefix(b"ref:").map(<[u8]>::trim_ascii).filter(|t| ref_name_ok(t)) {
        read_file_bounded(&common.join(String::from_utf8_lossy(target).as_ref()), SMALL_FILE).hash(&mut h);
    }
    std::fs::metadata(common.join("packed-refs")).ok().map(|m| (m.len(), m.modified().ok())).hash(&mut h);
    Some(h.finish())
}

/// The git dir of the repository whose work tree is `abs`: its `.git` folder, or the folder its
/// `.git` file names (relative to `abs` unless absolute).
fn git_dir_of(abs: &Path) -> Option<PathBuf> {
    let dot_git = abs.join(".git");
    if std::fs::metadata(&dot_git).ok()?.is_dir() {
        return Some(dot_git);
    }
    let text = read_file_bounded(&dot_git, SMALL_FILE)?;
    let text = String::from_utf8(text).ok()?;
    let named = Path::new(text.lines().next()?.strip_prefix("gitdir:")?.trim());
    Some(if named.is_absolute() { named.to_path_buf() } else { abs.join(named) })
}

/// Whether a symbolic ref's target names a ref inside the git dir: `refs/` and below, no `..`
/// part, no backslash or NUL (so it can't name a file elsewhere).
pub(super) fn ref_name_ok(target: &[u8]) -> bool {
    target.starts_with(b"refs/") && !target.iter().any(|&b| b == b'\\' || b == 0) && !target.split(|&b| b == b'/').any(|p| p == b"..")
}
