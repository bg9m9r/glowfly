//! A fingerprint of every config file git reads for a repository's config (system, global, XDG,
//! the repository's own and what they include), so a read that only depends on config (the
//! identity) is skipped while none changed. Only meaningful within one run.
//!
//! The files are the ones `git config` reads (git-config(1), FILES and ENVIRONMENT). Missing
//! files are part of the stamp too: creating one is a change. Files up to `HASHED_UP_TO` are
//! hashed by their bytes (no trust in timestamps); a larger one counts by size and time only when
//! it was last modified long enough ago (`repo::stamp_trustworthy`), else the stamp never matches.
//! So does a stamp whose include list was cut short or whose repository config includes a file on
//! another network place (never touched).

use crate::{config_includes, net_path, paths, repo, settings, watch};
use std::collections::hash_map::DefaultHasher;
use std::ffi::OsString;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::SystemTime;

#[cfg(test)]
#[path = "config_stamp_tests.rs"]
mod tests;

/// Files up to this size are fingerprinted by their bytes.
const HASHED_UP_TO: u64 = 64 * 1024;

/// An opaque fingerprint of the config files of one repository at one moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigStamp(u64);

/// The stamp of the config files git reads for the repository whose git dir is `git_dir`.
pub fn of_repo(git_dir: &Path) -> ConfigStamp {
    of_files(git_dir, outside_files())
}

/// The config files outside the repository. The environment and the git install don't change
/// while Glowfly runs, so they are worked out once.
fn outside_files() -> &'static [PathBuf] {
    static FILES: OnceLock<Vec<PathBuf>> = OnceLock::new();
    FILES.get_or_init(|| {
        // Git for Windows reads `%PROGRAMDATA%\Git\config` before the system file.
        let programdata = std::env::var_os("PROGRAMDATA").filter(|_| cfg!(windows)).map(|d| PathBuf::from(d).join("Git").join("config"));
        outside_files_with(
            |k| std::env::var_os(k),
            settings::home_dir().as_deref(),
            crate::flatpak::child_config_home(),
            git2::Config::find_system().ok(),
            programdata,
        )
    })
}

/// Whether a `GIT_CONFIG_NOSYSTEM`-style variable is on: set to anything but a false spelling.
fn truthy(v: &OsString) -> bool {
    let s = v.to_string_lossy();
    !matches!(s.trim().to_ascii_lowercase().as_str(), "" | "0" | "false" | "no" | "off")
}

/// The config files `git config` reads from outside the repository, given the environment:
/// the system files, then the global ones, then `GIT_CONFIG`. Candidates are listed even when
/// missing. `child_xdg` is the host's XDG folder inside Flatpak; `system` and `programdata` the
/// install's own system files. Pure.
pub(crate) fn outside_files_with(
    env: impl Fn(&str) -> Option<OsString>,
    home: Option<&Path>,
    child_xdg: Option<PathBuf>,
    system: Option<PathBuf>,
    programdata: Option<PathBuf>,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !env("GIT_CONFIG_NOSYSTEM").is_some_and(|v| truthy(&v)) {
        out.extend(programdata);
        match env("GIT_CONFIG_SYSTEM") {
            Some(path) => out.push(PathBuf::from(path)),
            None => out.extend(system),
        }
    }
    match env("GIT_CONFIG_GLOBAL") {
        Some(path) => out.push(PathBuf::from(path)),
        None => {
            let xdg = child_xdg
                .or_else(|| env("XDG_CONFIG_HOME").map(PathBuf::from).filter(|p| p.is_absolute()))
                .or_else(|| home.map(|h| h.join(".config")));
            out.extend(xdg.map(|x| x.join("git").join("config")));
            out.extend(home.map(|h| h.join(".gitconfig")));
        }
    }
    // Honoured by `git config` itself, as if `--file` were given.
    out.extend(env("GIT_CONFIG").map(PathBuf::from));
    out
}

/// The stamp of `git_dir`'s own config files, `outside` (and what they include).
pub(crate) fn of_files(git_dir: &Path, outside: &[PathBuf]) -> ConfigStamp {
    let now = SystemTime::now();
    let common = watch::common_dir(git_dir, git_dir);
    let roots = config_includes::repo_roots(git_dir, common.as_deref());
    let cx = config_includes::Context::of_repo(git_dir);
    let mut files: Vec<PathBuf> = outside.to_vec();
    let far = config_includes::included(outside, &cx);
    files.extend(far.files);
    let mut never = far.cut_short;
    let mut on_branch = far.on_branch;
    match config_includes::included_within(&roots, &cx, &|p| net_path::may_follow(p, git_dir)) {
        Ok(near) => {
            files.extend(roots);
            files.extend(near.files);
            never |= near.cut_short;
            on_branch |= near.on_branch;
        }
        // A file on another network place: never touched, never trusted.
        Err(_) => never = true,
    }
    if on_branch {
        files.push(git_dir.join("HEAD"));
    }
    let mut listed: Vec<PathBuf> = Vec::new();
    for f in files {
        if !listed.iter().any(|l| paths::same_spelling(l, &f)) {
            listed.push(f);
        }
    }
    let mut h = DefaultHasher::new();
    for f in &listed {
        f.hash(&mut h);
        file_stamp(f, now).hash(&mut h);
    }
    if never {
        nanos(now).hash(&mut h);
    }
    ConfigStamp(h.finish())
}

/// A fingerprint of one file at the time the stamp is taken.
#[derive(Hash)]
enum FileStamp {
    Missing,
    /// A small file's length and the hash of its bytes.
    Bytes(u64, u64),
    /// A larger file's length and modification time, old enough to trust.
    Settled(u64, SystemTime),
    /// A larger file modified too recently to trust: differs from every other stamp.
    Unsettled(u128),
}

fn nanos(t: SystemTime) -> u128 {
    t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default()
}

fn file_stamp(p: &Path, taken_at: SystemTime) -> FileStamp {
    // Only a regular file is ever opened (links followed): a FIFO would block.
    let Ok(m) = std::fs::metadata(p) else { return FileStamp::Missing };
    if !m.is_file() {
        return FileStamp::Missing;
    }
    if m.len() <= HASHED_UP_TO
        && let Ok(f) = std::fs::File::open(p)
    {
        let mut bytes = Vec::new();
        if f.take(HASHED_UP_TO + 1).read_to_end(&mut bytes).is_ok() && bytes.len() as u64 <= HASHED_UP_TO {
            let mut h = DefaultHasher::new();
            bytes.hash(&mut h);
            return FileStamp::Bytes(bytes.len() as u64, h.finish());
        }
    }
    match m.modified() {
        Ok(t) if repo::stamp_trustworthy(Some(t), taken_at) => FileStamp::Settled(m.len(), t),
        _ => FileStamp::Unsettled(nanos(taken_at)),
    }
}
