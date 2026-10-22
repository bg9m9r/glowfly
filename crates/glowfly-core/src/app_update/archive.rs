//! Reading a portable update's archive: the Windows zip or the Linux and macOS tar.gz that
//! scripts/package-*.{ps1,sh} make, `glowfly-<tag>-<triple>/{program, doc files}`.
//!
//! Only fixed names are taken (the program and the doc files, exact case), from the archive's
//! top level or, when every entry shares one root folder, from inside it; deeper entries are
//! ignored. Nothing here builds a path from a name read in the archive: the names it returns
//! are always one of the wanted ones. A wanted name that isn't a regular file, or appears
//! twice, is refused, and sizes are capped. Zip entries that aren't wanted are never
//! decompressed; a tar.gz has to be streamed past them, so every tar entry counts toward the
//! total.

use super::{MAX_DOWNLOAD, TargetOs, split_extension};
use std::io::{Cursor, Read};

/// The most an archive may unpack to.
pub const MAX_EXTRACT: u64 = 1 << 30;

/// The files beside the program that an update replaces when they are already there.
pub const DOC_FILES: [&str; 5] = ["LICENSE-MIT", "LICENSE-APACHE", "README.md", "THIRD-PARTY-NOTICES.md", "CHANGELOG.md"];

const WANTED_WINDOWS: [&str; 6] = ["glowfly.exe", DOC_FILES[0], DOC_FILES[1], DOC_FILES[2], DOC_FILES[3], DOC_FILES[4]];
const WANTED_UNIX: [&str; 6] = ["glowfly", DOC_FILES[0], DOC_FILES[1], DOC_FILES[2], DOC_FILES[3], DOC_FILES[4]];

/// A file taken from the archive; `name` is one of the wanted names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// The names taken from an archive for `os`: the program first, then [`DOC_FILES`].
pub fn wanted(os: TargetOs) -> &'static [&'static str] {
    if os == TargetOs::Windows { &WANTED_WINDOWS } else { &WANTED_UNIX }
}

/// The program's name in an archive for `os`.
pub fn program_name(os: TargetOs) -> &'static str {
    wanted(os)[0]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    File,
    Dir,
    Other,
}

/// An entry seen in the archive: its name (zip's `\` already made `/`), kind, and its bytes
/// when it was read.
struct Seen {
    name: String,
    kind: Kind,
    bytes: Option<Vec<u8>>,
}

/// `name` without leading `./` parts.
fn trim_dot(name: &str) -> &str {
    let mut n = name;
    while let Some(rest) = n.strip_prefix("./") {
        n = rest;
    }
    n
}

/// The one folder every name starts with, when there is one: `root/...` for each name (the
/// root's own entry `root/` too). None when any name is at the top level, absolute or on a
/// drive.
pub fn common_root<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let mut root: Option<&str> = None;
    for name in names {
        let (first, _) = trim_dot(name).split_once('/')?;
        // A drive (`C:`) is no folder.
        if first.is_empty() || first == "." || first == ".." || first.contains(':') {
            return None;
        }
        match root {
            None => root = Some(first),
            Some(r) if r == first => {}
            Some(_) => return None,
        }
    }
    root.map(str::to_owned)
}

/// The name of `raw` relative to `root` (or the top level), when it is a single plain part
/// there: never empty, `.`, `..` or containing a separator.
pub fn entry_name(raw: &str, root: Option<&str>) -> Option<String> {
    let n = trim_dot(raw);
    let rest = match root {
        Some(r) => n.strip_prefix(r)?.strip_prefix('/')?,
        None => n,
    };
    let ok = !rest.is_empty() && rest != "." && rest != ".." && !rest.contains(['/', '\\']);
    ok.then(|| rest.to_owned())
}

/// Whether `raw` could be a wanted entry, at the top level or one folder down.
fn candidate(raw: &str, wanted: &[&str]) -> bool {
    let n = trim_dot(raw);
    let first = n.split_once('/').map(|(f, _)| f);
    [None, first].into_iter().any(|root| entry_name(n, root).is_some_and(|e| wanted.contains(&e.as_str())))
}

/// The wanted files of the archive `bytes`, whose kind `file_name`'s extension says.
pub fn read(file_name: &str, bytes: &[u8], wanted: &[&str]) -> Result<Vec<Entry>, String> {
    let lower = file_name.to_ascii_lowercase();
    let bad = |e: &dyn std::fmt::Display| format!("Couldn't read the archive {file_name}: {e}");
    let seen = match split_extension(&lower).1 {
        ".zip" => read_zip(bytes, wanted).map_err(|e| e.unwrap_or_else(|e| bad(&e)))?,
        ".tar.gz" => read_tar_gz(bytes, wanted).map_err(|e| e.unwrap_or_else(|e| bad(&e)))?,
        _ => return Err(bad(&"it isn't a zip or tar.gz file")),
    };
    select(seen, wanted)
}

/// A failure: Ok(text) for one of ours, Err(error) for the archive's own.
type Fail = Result<String, Box<dyn std::error::Error>>;

fn too_large() -> String {
    format!("The archive is larger than {} MB when unpacked.", MAX_EXTRACT / (1024 * 1024))
}

/// Reads one wanted entry of at most [`MAX_DOWNLOAD`] bytes, adding it to `total`.
fn take(mut r: impl Read, size: u64, total: &mut u64) -> Result<Vec<u8>, Fail> {
    if size > MAX_DOWNLOAD || total.saturating_add(size) > MAX_EXTRACT {
        return Err(Ok(too_large()));
    }
    let mut bytes = Vec::new();
    r.by_ref().take(MAX_DOWNLOAD + 1).read_to_end(&mut bytes).map_err(|e| Err(e.into()))?;
    *total = total.saturating_add(bytes.len() as u64);
    if bytes.len() as u64 > MAX_DOWNLOAD || *total > MAX_EXTRACT {
        return Err(Ok(too_large()));
    }
    Ok(bytes)
}

fn read_zip(bytes: &[u8], wanted: &[&str]) -> Result<Vec<Seen>, Fail> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| Err(e.into()))?;
    let mut seen = Vec::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        // Raw: only the entry's header, nothing decompressed.
        let (name, kind, size) = {
            let f = zip.by_index_raw(i).map_err(|e| Err(e.into()))?;
            let name = String::from_utf8_lossy(f.name_raw()).replace('\\', "/");
            let kind = if f.is_dir() || name.ends_with('/') {
                Kind::Dir
            } else if f.is_file() {
                Kind::File
            } else {
                Kind::Other
            };
            (name, kind, f.size())
        };
        let bytes = if kind == Kind::File && candidate(&name, wanted) {
            Some(take(zip.by_index(i).map_err(|e| Err(e.into()))?, size, &mut total)?)
        } else {
            None
        };
        seen.push(Seen { name, kind, bytes });
    }
    Ok(seen)
}

fn read_tar_gz(bytes: &[u8], wanted: &[&str]) -> Result<Vec<Seen>, Fail> {
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
    let mut seen = Vec::new();
    let mut total = 0u64;
    for entry in tar.entries().map_err(|e| Err(e.into()))? {
        let entry = entry.map_err(|e| Err(e.into()))?;
        // In tar, `\` is an ordinary name character.
        let name = String::from_utf8_lossy(&entry.path_bytes()).into_owned();
        let ty = entry.header().entry_type();
        let kind = if ty.is_dir() {
            Kind::Dir
        } else if ty.is_file() {
            Kind::File
        } else {
            Kind::Other
        };
        let size = entry.size();
        let bytes = if kind == Kind::File && candidate(&name, wanted) {
            Some(take(entry, size, &mut total)?)
        } else {
            // Streamed past, so it counts toward what is unpacked.
            total = total.saturating_add(size);
            if total > MAX_EXTRACT {
                return Err(Ok(too_large()));
            }
            None
        };
        seen.push(Seen { name, kind, bytes });
    }
    Ok(seen)
}

/// The wanted entries of `seen`, relative to its root folder when it has one.
fn select(seen: Vec<Seen>, wanted: &[&str]) -> Result<Vec<Entry>, String> {
    let root = common_root(seen.iter().map(|s| s.name.as_str()));
    let mut out: Vec<Entry> = Vec::new();
    for s in seen {
        if s.kind == Kind::Dir {
            continue;
        }
        let Some(name) = entry_name(&s.name, root.as_deref()).filter(|n| wanted.contains(&n.as_str())) else { continue };
        if out.iter().any(|e| e.name == name) {
            return Err(format!("The archive has two copies of {name}."));
        }
        match (s.kind, s.bytes) {
            (Kind::File, Some(bytes)) => out.push(Entry { name, bytes }),
            _ => return Err(format!("{name} in the archive isn't a regular file.")),
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "archive_tests.rs"]
mod tests;
