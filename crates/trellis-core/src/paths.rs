//! One folder, many spellings: a Windows short 8.3 name (`C:\Users\RUNNER~1`) and its long
//! form, a link or junction and its target, macOS's `/var` and `/private/var`, `Repo` and `repo`
//! on a volume that ignores case. libgit2 and git report the resolved form, while a typed path,
//! `%TEMP%` or a recent-list entry may use any other.
//!
//! Two questions, two answers, and nothing else in Trellis compares folders:
//!
//! - **Is it the same folder?** Asked of the disk: `FolderId` (device and inode on Unix, volume
//!   serial and file id on Windows) through `same_folder`, or a `FolderId` cached when a
//!   repository loaded (`Snapshot::folder`). Whether case matters depends on the volume (and on
//!   Windows on the folder), never on the OS, so nothing here folds case or Unicode forms.
//! - **Is it spelled the same?** `same_spelling`, pure and lexical, for paths that don't exist or
//!   haven't been looked at yet. It ignores only what can never name another folder: separator
//!   style, `.` parts, trailing separators, the `\\?\` and `\\.\` forms of a drive, and the case
//!   of a drive letter or a UNC server and share.

use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};

mod encode;
mod identity;
pub use encode::{decode, encode};
pub use identity::FolderId;

/// `path` as it really is on disk: links, junctions and short 8.3 names followed, in its plain
/// form (no `\\?\` prefix on Windows). A path that doesn't exist yet (a clone's destination,
/// say) has its nearest existing parent resolved and the rest kept as typed, with `..` applied
/// to the parts that don't exist. A relative path, or one nothing of which resolves, is returned
/// as it is: a relative path is never resolved against the process's working folder.
pub fn resolve(path: &Path) -> PathBuf {
    if !path.is_absolute() {
        return path.to_path_buf();
    }
    let parts: Vec<Component> = path.components().collect();
    for n in (1..=parts.len()).rev() {
        // A bare drive prefix (`C:`) names that drive's current folder, not its root.
        if matches!(parts[n - 1], Component::Prefix(_)) {
            continue;
        }
        let base: PathBuf = parts[..n].iter().collect();
        if let Ok(real) = std::fs::canonicalize(&base) {
            let mut out = plain(&real);
            for part in &parts[n..] {
                match part {
                    Component::Normal(name) => out.push(name),
                    // The part before it doesn't exist, so it can't be a link: dropping it is
                    // what the file system will do once it is created.
                    Component::ParentDir => {
                        out.pop();
                    }
                    _ => {}
                }
            }
            return out;
        }
    }
    path.to_path_buf()
}

/// The system's temporary folder, resolved (`std::env::temp_dir` may give its short 8.3 name on
/// Windows, or `/var/...` for `/private/var/...` on macOS).
pub fn temp_dir() -> PathBuf {
    resolve(&std::env::temp_dir())
}

/// `path` without the verbatim `\\?\` prefix `canonicalize` adds on Windows, when the plain form
/// names the same file (a drive or a UNC share, and no part that Windows would rewrite: a
/// trailing dot or space, or a device name such as `NUL`); unchanged otherwise.
pub fn plain(path: &Path) -> PathBuf {
    use std::path::Prefix;
    let mut parts = path.components();
    let Some(Component::Prefix(p)) = parts.next() else { return path.to_path_buf() };
    let head: OsString = match p.kind() {
        Prefix::VerbatimDisk(d) => format!("{}:", d as char).into(),
        // Built from the parts as they are, so a name that isn't valid Unicode survives.
        Prefix::VerbatimUNC(server, share) => [OsStr::new(r"\\"), server, OsStr::new(r"\"), share].into_iter().collect(),
        _ => return path.to_path_buf(),
    };
    if parts.clone().any(|c| matches!(c, Component::Normal(n) if !plain_safe(n))) {
        return path.to_path_buf();
    }
    let mut out = PathBuf::from(head);
    out.extend(parts);
    out
}

/// Whether a path part means the same without the `\\?\` prefix: Win32 path parsing strips a
/// trailing dot or space and turns a device name (`CON`, `nul.txt`, `COM¹`) into the device.
fn plain_safe(name: &OsStr) -> bool {
    let Some(name) = name.to_str() else { return true };
    if name.ends_with('.') || name.ends_with(' ') {
        return false;
    }
    let stem = name.split('.').next().unwrap_or(name).trim_end().to_ascii_uppercase();
    let numbered =
        |d: &str| stem.strip_prefix(d).is_some_and(|n| (n.len() == 1 && n.as_bytes()[0].is_ascii_digit()) || matches!(n, "¹" | "²" | "³"));
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$") || numbered("COM") || numbered("LPT");
    !device
}

/// Whether `a` and `b` are spelled as the same path (see the module docs for what is ignored).
/// Pure: use it only for paths that don't exist, or as a first check before `same_folder` or a
/// cached `FolderId`.
pub fn same_spelling(a: &Path, b: &Path) -> bool {
    spelling(a) == spelling(b)
}

/// Whether `path` is `root` or inside it, by spelling (as `same_spelling`), comparing whole
/// parts. Pure.
pub fn is_within(path: &Path, root: &Path) -> bool {
    let (p, r) = (spelling(path), spelling(root));
    !r.is_empty() && p.len() >= r.len() && p[..r.len()] == r[..]
}

/// Whether `a` and `b` are the same folder (or file) on disk: by `FolderId` when both have one,
/// and otherwise by the spelling of their resolved forms (a path that doesn't exist, or a volume
/// that gives no file ids). Touches the disk: worker threads only.
pub fn same_folder(a: &Path, b: &Path) -> bool {
    if same_spelling(a, b) {
        return true;
    }
    match (FolderId::of(a), FolderId::of(b)) {
        (Some(x), Some(y)) => x == y,
        _ => same_spelling(&resolve(a), &resolve(b)),
    }
}

/// The rest of typed folder text after a leading `~` that means the home folder: `~` alone, or
/// followed by a separator (either one on Windows). None for any other text (`~user` too).
fn home_relative(text: &str) -> Option<&str> {
    let rest = text.strip_prefix('~')?;
    if rest.is_empty() {
        return Some("");
    }
    let mut chars = rest.chars();
    let sep = chars.next()?;
    (sep == '/' || (cfg!(windows) && sep == '\\')).then_some(chars.as_str())
}

/// Typed folder text as a path: trimmed, with a leading `~` (see `home_relative`) replaced by
/// `home`. Pure; the caller looks the home folder up (`settings::home_dir`).
pub fn expand_home(text: &str, home: Option<&Path>) -> PathBuf {
    let text = text.trim();
    match (home_relative(text), home) {
        (Some(""), Some(home)) => home.to_path_buf(),
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(text),
    }
}

/// Whether typed folder text names a folder by its full path, or from the home folder (`~/…`).
/// A relative path is refused rather than resolved against the process's working folder.
pub fn typed_is_absolute(text: &str) -> bool {
    let text = text.trim();
    home_relative(text).is_some() || Path::new(text).is_absolute()
}

/// The parts of `path` that `same_spelling` compares.
fn spelling(path: &Path) -> Vec<OsString> {
    path.components()
        .filter(|c| !matches!(c, Component::CurDir))
        .map(|c| match c {
            Component::Prefix(p) => prefix_spelling(p.kind()),
            other => other.as_os_str().to_owned(),
        })
        .collect()
}

/// A Windows path prefix in one spelling per place: drive letters and UNC servers and shares
/// are case-insensitive whatever the volume, and `\\?\C:`, `\\.\C:` and `C:` are one drive.
/// `\\wsl$` and `\\wsl.localhost` are two names for the same WSL file server.
fn prefix_spelling(p: std::path::Prefix) -> OsString {
    use std::path::Prefix;
    let drive = |d: u8| OsString::from(format!("{}:", (d as char).to_ascii_lowercase()));
    match p {
        Prefix::Disk(d) | Prefix::VerbatimDisk(d) => drive(d),
        Prefix::DeviceNS(n) if matches!(n.as_encoded_bytes(), [d, b':'] if d.is_ascii_alphabetic()) => drive(n.as_encoded_bytes()[0]),
        Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
            let server = server.to_string_lossy().to_ascii_lowercase();
            let server = if server == "wsl.localhost" { "wsl$".to_owned() } else { server };
            OsString::from(format!(r"\\{}\{}", server, share.to_string_lossy().to_ascii_lowercase()))
        }
        Prefix::Verbatim(x) => [OsStr::new(r"\\?\"), x].into_iter().collect(),
        Prefix::DeviceNS(x) => [OsStr::new(r"\\.\"), x].into_iter().collect(),
    }
}

#[cfg(test)]
mod tests;
