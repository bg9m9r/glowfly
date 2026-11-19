//! The hooks git would run from a hooks folder (`active_hooks`), and hook names compared as
//! the file systems git meets compare them (`is_hook_name`, `hook_base`).

use super::{HOOK_NAMES, follow};
use std::path::{Path, PathBuf};

/// The hooks in `dir` that git would run, sorted. A missing folder has none.
///
/// A security check, so it overmatches: a listed file counts when its name is a hook name under
/// any case rule a file system might use (see [`is_hook_name`]), and each hook name is also
/// looked up directly, so a file system that matches names in some other way (a Linux FAT or
/// casefold mount, a Windows short name) can't hide one.
///
/// A link is followed (a link to a script runs the script) only where `net_path::may_follow`
/// allows from `dir`; one that leads elsewhere counts as run without being looked at.
pub(super) fn active_hooks(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let runs = |p: &Path| match std::fs::symlink_metadata(p) {
        Ok(m) if m.file_type().is_symlink() => follow::far(p, dir) || std::fs::metadata(p).is_ok_and(|m| m.is_file() && runnable(&m)),
        Ok(m) => m.is_file() && runnable(&m),
        Err(_) => false,
    };
    let mut hooks: Vec<(String, PathBuf)> = read
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_owned();
            (is_hook_name(&name) && runs(&e.path())).then(|| (name, e.path()))
        })
        .collect();
    let exe = if cfg!(windows) { ".exe" } else { "" };
    for name in HOOK_NAMES {
        let found = [name.to_string(), format!("{name}{exe}")].into_iter().map(|n| dir.join(n)).find(|p| runs(p));
        if let Some(path) = found
            && !hooks.iter().any(|(h, _)| hook_base(h) == hook_base(name))
        {
            hooks.push(((*name).to_owned(), path));
        }
    }
    hooks.sort();
    hooks
}

/// Whether git may run a file called `file` in a hooks folder: a hook name in any letter case,
/// on every OS (Windows and macOS ignore case by default, and so do FAT, exFAT, CIFS and
/// casefold folders on Linux), with the code points HFS+ ignores left out. Git for Windows also
/// runs `<hook>.exe`.
pub(crate) fn is_hook_name(file: &str) -> bool {
    HOOK_NAMES.iter().any(|h| hook_base(file) == hook_base(h))
}

/// `name` as a case-insensitive file system may compare it: HFS+ ignorable code points
/// dropped, uppercased character by character as NTFS does (so "ı" and "ſ" become "I" and "S"),
/// then lowercased (so the Kelvin sign is "k"), and `.exe` dropped on Windows.
pub(crate) fn hook_base(name: &str) -> String {
    let kept: String = name.chars().filter(|c| !crate::names::is_hfs_ignorable(*c)).collect();
    let upper: String = kept.chars().flat_map(char::to_uppercase).collect();
    let key = upper.to_lowercase();
    match key.strip_suffix(".exe") {
        Some(base) if cfg!(windows) => base.to_owned(),
        _ => key,
    }
}

/// Git skips hooks without an execute bit on Unix; on Windows it runs any file.
#[cfg(unix)]
fn runnable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn runnable(_meta: &std::fs::Metadata) -> bool {
    true
}
