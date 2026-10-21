//! Reading and writing theme files in `<config dir>/themes`. Called only by the app shell at
//! startup and by the runner; the parsing itself is in `themes.rs`.

use super::themes::{ThemeCatalog, ThemeFile, ThemeProblem, parse};
use std::path::{Path, PathBuf};

/// Theme files larger than this are refused without being read.
pub const MAX_FILE_BYTES: u64 = 256 * 1024;

/// `<config dir>/themes`; None when there is no config folder.
pub fn themes_dir() -> Option<PathBuf> {
    super::config_dir().map(|d| d.join("themes"))
}

/// Every theme file in the themes folder.
pub fn load_all() -> ThemeCatalog {
    match themes_dir() {
        Some(d) => load_dir(&d),
        None => ThemeCatalog::default(),
    }
}

/// Every `*.json` file directly in `dir` (the extension in any case; links to files are
/// followed). A missing folder is an empty catalog, and is not created.
pub fn load_dir(dir: &Path) -> ThemeCatalog {
    let mut cat = ThemeCatalog { dir: Some(dir.to_owned()), ..ThemeCatalog::default() };
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return cat,
        Err(e) => {
            cat.error = Some(format!("Could not list {}: {e}", crate::native_path::shown(dir)));
            return cat;
        }
    };
    for entry in entries {
        let path = match entry {
            Ok(entry) => entry.path(),
            Err(e) => {
                cat.skipped.push(format!("an entry could not be read: {e}"));
                continue;
            }
        };
        if !path.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case("json")) {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            cat.skipped.push(format!("skipped {}: its name isn't UTF-8", crate::native_path::shown(&path)));
            continue;
        };
        if let Some(file) = load_file(id, &path) {
            cat.files.push(file);
        }
    }
    cat.files.sort_by(|a, b| (a.name.to_lowercase(), &a.id).cmp(&(b.name.to_lowercase(), &b.id)));
    cat
}

/// One theme file, or None when `path` isn't a regular file (a folder named `x.json`).
fn load_file(id: &str, path: &Path) -> Option<ThemeFile> {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return Some(ThemeFile::failed(id, path, ThemeProblem::Unreadable(e.to_string()))),
    };
    if !meta.is_file() {
        return None;
    }
    if meta.len() > MAX_FILE_BYTES {
        return Some(ThemeFile::failed(id, path, ThemeProblem::TooLarge));
    }
    Some(match std::fs::read(path) {
        Ok(bytes) => parse(id, path, &bytes),
        Err(e) => ThemeFile::failed(id, path, ThemeProblem::Unreadable(e.to_string())),
    })
}

/// Writes `json` to a new file in `dir` (created if missing): `my-theme.json`, or
/// `my-theme-2.json`, `-3`… when that name is taken. Returns its id and path.
pub fn create(dir: &Path, json: &str) -> std::io::Result<(String, PathBuf)> {
    std::fs::create_dir_all(dir)?;
    // Lowercase names only, so a volume that ignores case agrees about which are taken.
    for n in 1..=999 {
        let stem = if n == 1 { "my-theme".to_owned() } else { format!("my-theme-{n}") };
        let path = dir.join(format!("{stem}.json"));
        // Any entry takes the name, a dangling link too (writing would follow it).
        match path.symlink_metadata() {
            Ok(_) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        crate::persist::atomic_write(&path, json.as_bytes())?;
        return Ok((stem, path));
    }
    Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "every my-theme-N.json name is taken"))
}
