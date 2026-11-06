//! Paths a repository's own files name (a `.git` file, a `commondir`, a linked worktree's
//! record, a link in a git dir, a nested repository's config), and whether following one would
//! reach another machine. The spelling is checked first, then mapped drive letters, then every
//! link on the way; nothing is touched before the spelling check. A place on the same network
//! root as the place already in use (`home`) is allowed: reaching it costs nothing new.
//!
//! Network spellings on Windows: UNC with either separator (`\\server\share`, `//server/share`),
//! the prefixed UNC forms (`\\?\UNC\`, `\\.\UNC\`, `\??\UNC\`), `\\wsl$` and `\\wsl.localhost`
//! (touching one starts the distro), and any other device-namespace path that isn't a drive
//! letter or `Volume{guid}` (its target can't be told from the spelling, so it is never
//! allowed). Then a drive letter `GetDriveTypeW` calls remote (only ever asked of a `X:\` root,
//! never of a share's, whose query reaches its server). On macOS and Linux: the automounter roots
//! `/net/<host>` and `/Network/Servers/<host>` (with their `/System/Volumes/Data/` forms), where a
//! lookup mounts the host, matched in any case (macOS's root volume ignores case by default);
//! `//server/share` is a local name there.

use std::path::{Component, Path, PathBuf};

/// The network place a path is on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NetRoot {
    /// A share, lowercased; `wsl.localhost` is kept as `wsl$` (the same distros).
    Share { server: String, share: String },
    /// A drive letter mapped to a share, uppercase.
    MappedDrive(char),
    /// An automounted host (`/net/<host>`, `/Network/Servers/<host>`), lowercased.
    Automount(String),
    /// A device-namespace path whose target can't be told: never allowed.
    Device,
}

/// The most links one check follows before refusing.
const MAX_LINKS: usize = 32;
/// The most path parts one check processes before refusing.
const MAX_PARTS: usize = 4096;

/// The network root `text` names by its spelling alone (a drive letter is never mapped).
pub(crate) fn spelled_root_for(text: &str, windows: bool) -> Option<NetRoot> {
    root_for(text, windows, &|_| false)
}

/// The network root `text` names, with `mapped` telling whether a drive letter is a mapped
/// network drive (asked only for a drive letter's root).
pub(crate) fn root_for(text: &str, windows: bool, mapped: &dyn Fn(char) -> bool) -> Option<NetRoot> {
    if windows { windows_root(text, mapped) } else { unix_root(text) }
}

/// `root_for` on this OS, asking Windows about drive letters.
pub(crate) fn root_of(path: &Path) -> Option<NetRoot> {
    root_for(&path.to_string_lossy(), cfg!(windows), &crate::volume::mapped_drive)
}

/// Whether a path on `named` may be followed from a place on `home`: a local one always, a
/// network one only on the same root, a device path never.
pub(crate) fn may_follow_for(named: Option<&NetRoot>, home: Option<&NetRoot>) -> bool {
    match named {
        None => true,
        Some(NetRoot::Device) => false,
        Some(r) => home == Some(r),
    }
}

fn windows_root(text: &str, mapped: &dyn Fn(char) -> bool) -> Option<NetRoot> {
    let norm = text.replace('/', "\\");
    // Verbatim and device prefixes count only when written with backslashes, as Win32 reads them.
    if [r"\\?\", r"\\.\", r"\??\"].iter().any(|p| text.starts_with(p)) {
        let rest = &norm[4..];
        if rest.get(..4).is_some_and(|u| u.eq_ignore_ascii_case(r"UNC\")) {
            return Some(share_of(&rest[4..]));
        }
        if let Some(letter) = drive_of(rest).filter(|_| rest.len() == 2 || rest[2..].starts_with('\\')) {
            return drive_root(letter, mapped);
        }
        if rest.get(..7).is_some_and(|v| v.eq_ignore_ascii_case("Volume{")) {
            return None;
        }
        return Some(NetRoot::Device);
    }
    if let Some(rest) = norm.strip_prefix(r"\\") {
        return Some(share_of(rest));
    }
    drive_of(&norm).and_then(|letter| drive_root(letter, mapped))
}

/// The share `rest` (what follows `\\`) names: the server up to the next `\`, then the share.
fn share_of(rest: &str) -> NetRoot {
    let mut parts = rest.splitn(3, '\\');
    let server = parts.next().unwrap_or("").to_lowercase();
    let share = parts.next().unwrap_or("").to_lowercase();
    if server.is_empty() {
        return NetRoot::Device;
    }
    let server = if server == "wsl.localhost" { "wsl$".to_owned() } else { server };
    NetRoot::Share { server, share }
}

/// The drive letter `text` starts with (`C:`), if any.
fn drive_of(text: &str) -> Option<char> {
    let b = text.as_bytes();
    (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':').then(|| char::from(b[0]).to_ascii_uppercase())
}

fn drive_root(letter: char, mapped: &dyn Fn(char) -> bool) -> Option<NetRoot> {
    mapped(letter).then_some(NetRoot::MappedDrive(letter))
}

fn unix_root(text: &str) -> Option<NetRoot> {
    if !text.starts_with('/') {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for p in text.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    // macOS's root volume ignores case by default, so `/NET/host` reaches the automounter too.
    let is = |part: &str, name: &str| part.eq_ignore_ascii_case(name);
    let rest = match parts.as_slice() {
        [s, v, d, rest @ ..] if is(s, "System") && is(v, "Volumes") && is(d, "Data") => rest,
        all => all,
    };
    let host = match rest {
        [n, host, ..] if is(n, "net") => host,
        [n, s, host, ..] if is(n, "Network") && is(s, "Servers") => host,
        _ => return None,
    };
    Some(NetRoot::Automount(host.to_ascii_lowercase()))
}

/// Whether `named`, a path a repository's files name, may be touched from `home`, the place
/// already in use: its spelling and every link on the way lead to a local place or one on
/// `home`'s network root. The spelling is checked before anything is touched; then each part
/// past what `named` shares with `home` is looked at without following it, and a link's target
/// goes through the same checks (at most `MAX_LINKS`).
pub(crate) fn may_follow(named: &Path, home: &Path) -> bool {
    let home_root = root_of(home);
    if !may_follow_for(root_of(named).as_ref(), home_root.as_ref()) {
        return false;
    }
    walk(named, home, home_root.as_ref())
}

/// One part of a path being walked.
enum Part {
    /// A prefix and root (`C:\`, `\\server\share\`, `/`): starts over from there.
    Root(PathBuf),
    Name(std::ffi::OsString),
    Up,
}

/// `p`'s parts, with `..` applied lexically on Windows (as Win32 does before any I/O, never
/// above the root).
fn parts(p: &Path) -> Vec<Part> {
    let mut out: Vec<Part> = Vec::new();
    let mut root = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Prefix(_) | Component::RootDir => root.push(c.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir if cfg!(windows) => match out.last() {
                Some(Part::Name(_)) => {
                    out.pop();
                }
                // Never above the root.
                None if !root.as_os_str().is_empty() => {}
                // A relative path's `..` leaves the folder it is joined to.
                _ => out.push(Part::Up),
            },
            Component::ParentDir => out.push(Part::Up),
            Component::Normal(n) => out.push(Part::Name(n.to_os_string())),
        }
    }
    if !root.as_os_str().is_empty() {
        out.insert(0, Part::Root(root));
    }
    out
}

fn walk(named: &Path, home: &Path, home_root: Option<&NetRoot>) -> bool {
    let mut todo: Vec<Part> = parts(named);
    todo.reverse();
    let mut at = PathBuf::new();
    let (mut links, mut seen) = (0, 0);
    while let Some(part) = todo.pop() {
        seen += 1;
        if seen > MAX_PARTS {
            return false;
        }
        let name = match part {
            Part::Root(r) => {
                at = r;
                continue;
            }
            Part::Up => {
                // `at` is free of links (or in use already), so this is where `..` leads.
                at.pop();
                continue;
            }
            Part::Name(n) => n,
        };
        let next = at.join(&name);
        // Already in use: nothing new is reached.
        if home.starts_with(&next) {
            at = next;
            continue;
        }
        // `..` can lead into an automounter's root on Unix.
        if !may_follow_for(spelled_root_for(&next.to_string_lossy(), cfg!(windows)).as_ref(), home_root) {
            return false;
        }
        let Ok(meta) = std::fs::symlink_metadata(&next) else {
            // Nothing further exists to be reached.
            return true;
        };
        if !meta.file_type().is_symlink() {
            at = next;
            continue;
        }
        links += 1;
        if links > MAX_LINKS {
            return false;
        }
        let Ok(target) = std::fs::read_link(&next) else { return false };
        if !may_follow_for(root_of(&target).as_ref(), home_root) {
            return false;
        }
        // An absolute target starts over from its root; a relative one goes on from `at`.
        let mut spliced = parts(&target);
        spliced.reverse();
        todo.extend(spliced);
    }
    true
}

#[cfg(test)]
#[path = "net_path_tests.rs"]
mod tests;
