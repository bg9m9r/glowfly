//! Moves the folders earlier versions kept their files in to where Glowfly keeps them, once, at
//! start, when Glowfly's own folder isn't there yet:
//!
//! - the settings folder: the one the app used while it was called Trellis (the same layout,
//!   `%APPDATA%\Trellis`, `~/Library/Application Support/Trellis`, `<XDG config>/trellis`), else
//!   older ones (the app was called Riffle before that, and every platform but Windows once used
//!   `<XDG config>/Trellis`); only the newest that exists is moved;
//! - the cache and logs folders Trellis used (`%LOCALAPPDATA%\Trellis` holds both on Windows).
//!
//! A folder is renamed, or copied and then removed where the two are on different volumes.

use super::paths::{AppName, GLOWFLY, Os, TRELLIS, abs_var, cache_dir_named, config_dir_named, log_dir_named};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Settings folders earlier versions used, most recent first. Only the first that exists is moved.
pub(crate) fn old_config_dirs_for(os: Os, env: impl Fn(&str) -> Option<OsString>) -> Vec<PathBuf> {
    let new = config_dir_named(os, GLOWFLY, &env);
    let mut dirs: Vec<PathBuf> = config_dir_named(os, TRELLIS, &env).into_iter().collect();
    let mut bases = Vec::new();
    match os {
        Os::Windows => bases.extend(abs_var(&env, "APPDATA")),
        Os::Mac | Os::Xdg => {
            // Old builds took $XDG_CONFIG_HOME when set, else ~/.config; check both on macOS,
            // where the variable may have been set in one shell and not another.
            bases.extend(abs_var(&env, "XDG_CONFIG_HOME"));
            if os == Os::Mac || bases.is_empty() {
                bases.extend(abs_var(&env, "HOME").map(|h| h.join(".config")));
            }
        }
    }
    for name in [TRELLIS.title, "Riffle"] {
        for base in &bases {
            let dir = base.join(name);
            // On Windows `%APPDATA%\Trellis` is already first.
            if Some(&dir) != new.as_ref() && !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
    }
    dirs
}

/// The cache and logs folders `old` used, each with where Glowfly keeps it. The cache comes
/// first: on Windows it holds the logs, which then move with it.
pub(crate) fn local_dirs_for(os: Os, old: AppName, env: impl Fn(&str) -> Option<OsString>) -> Vec<(PathBuf, PathBuf)> {
    [(cache_dir_named(os, old, &env), cache_dir_named(os, GLOWFLY, &env)), (log_dir_named(os, old, &env), log_dir_named(os, GLOWFLY, &env))]
        .into_iter()
        .filter_map(|(o, n)| Some((o?, n?)))
        .collect()
}

/// Moves the settings, cache and logs folders from where an earlier version kept them (see the
/// module docs). Run before anything writes to Glowfly's own folders, logging included, or the
/// new folder would exist and nothing would move. Every move is tried; the first error is
/// returned.
pub fn migrate_old_dirs() -> std::io::Result<()> {
    let env = |k: &str| std::env::var_os(k);
    migrate_all(Os::CURRENT, env)
}

pub(crate) fn migrate_all(os: Os, env: impl Fn(&str) -> Option<OsString>) -> std::io::Result<()> {
    let mut first = None;
    if let Some(new) = config_dir_named(os, GLOWFLY, &env)
        && let Err(e) = migrate_first(&old_config_dirs_for(os, &env), &new)
    {
        first = Some(e);
    }
    for (old, new) in local_dirs_for(os, TRELLIS, &env) {
        if let Err(e) = move_dir_if_new_missing(&old, &new) {
            first.get_or_insert(e);
        }
    }
    first.map_or(Ok(()), Err)
}

pub(crate) fn migrate_first(old: &[PathBuf], new: &Path) -> std::io::Result<()> {
    match old.iter().find(|d| d.is_dir()) {
        Some(dir) => move_dir_if_new_missing(dir, new),
        None => Ok(()),
    }
}

pub(crate) fn move_dir_if_new_missing(old: &Path, new: &Path) -> std::io::Result<()> {
    // `exists` is also true when `new` only differs from `old` in case on a case-insensitive
    // disk (`~/.config/Trellis` vs `trellis`), which correctly leaves the folder alone.
    if !old.is_dir() || new.exists() {
        return Ok(());
    }
    if let Some(parent) = new.parent() {
        std::fs::create_dir_all(parent)?;
    }
    move_dir_with(old, new, |a, b| std::fs::rename(a, b))
}

/// Moves the folder `old` to `new` with `rename`, or, where that fails (the two are on different
/// volumes, EXDEV), by copying it and then removing the original. A copy that fails is removed
/// again, leaving `old` as it was.
fn move_dir_with(old: &Path, new: &Path, rename: impl Fn(&Path, &Path) -> std::io::Result<()>) -> std::io::Result<()> {
    let Err(e) = rename(old, new) else { return Ok(()) };
    if let Err(copy) = copy_tree(old, new) {
        let _ = std::fs::remove_dir_all(new);
        return Err(std::io::Error::new(copy.kind(), format!("{e}; copying instead failed too: {copy}")));
    }
    std::fs::remove_dir_all(old)
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "migrate_tests.rs"]
mod tests;
