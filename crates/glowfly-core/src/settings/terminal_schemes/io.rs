//! Reading and writing terminal schemes in `<config dir>/themes/terminal`, and reading a file to
//! import. Called only by the runner and with the themes folder at load; parsing is in the
//! parent module.

use super::{ImportError, ImportReport, Scheme, SchemeCatalog, SchemeFile, import, parse_stored, scheme_json};
use crate::settings::themes::{MAX_FILE_BYTES, ThemeProblem};
use std::path::{Path, PathBuf};

/// Files to import larger than this are refused without being read (a settings.json with many
/// profiles stays well under it).
pub const MAX_IMPORT_BYTES: u64 = 1024 * 1024;

/// `<themes dir>/terminal`.
pub fn schemes_dir(themes_dir: &Path) -> PathBuf {
    themes_dir.join("terminal")
}

/// Every `*.json` file directly in `dir` (the extension in any case). A missing folder is an
/// empty catalog.
pub fn load_dir(dir: &Path) -> SchemeCatalog {
    let mut cat = SchemeCatalog::default();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return cat,
        Err(e) => {
            cat.error = Some(format!("Could not list {}: {e}", crate::native_path::shown(dir)));
            return cat;
        }
    };
    for path in entries.flatten().map(|e| e.path()) {
        if !path.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case("json")) {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else { continue };
        let file = match std::fs::metadata(&path) {
            Ok(m) if !m.is_file() => continue,
            Ok(m) if m.len() > MAX_FILE_BYTES => SchemeFile::failed(id, &path, ThemeProblem::TooLarge),
            Ok(_) => match std::fs::read(&path) {
                Ok(bytes) => parse_stored(id, &path, &bytes),
                Err(e) => SchemeFile::failed(id, &path, ThemeProblem::Unreadable(e.to_string())),
            },
            Err(e) => SchemeFile::failed(id, &path, ThemeProblem::Unreadable(e.to_string())),
        };
        cat.files.push(file);
    }
    cat.files.sort_by(|a, b| (a.name.to_lowercase(), &a.id).cmp(&(b.name.to_lowercase(), &b.id)));
    cat
}

/// Reads the file at `path` and writes each scheme in it to `dir` (created if missing).
pub fn import_path(path: &Path, dir: &Path) -> Result<ImportReport, ImportError> {
    let meta = std::fs::metadata(path).map_err(|e| ImportError::Unreadable(e.to_string()))?;
    if !meta.is_file() {
        return Err(ImportError::Unreadable("it is not a file".into()));
    }
    if meta.len() > MAX_IMPORT_BYTES {
        return Err(ImportError::TooLarge);
    }
    let bytes = std::fs::read(path).map_err(|e| ImportError::Unreadable(e.to_string()))?;
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let found = import(&stem, &bytes)?;
    let mut report = ImportReport { saved: Vec::new(), skipped: found.skipped };
    let mut first_error = None;
    for s in &found.schemes {
        match save(dir, s) {
            Ok(id) => report.saved.push((id, s.name.clone())),
            Err(e) => {
                report.skipped.push(format!("scheme \"{}\" could not be saved: {e}", s.name));
                first_error.get_or_insert(e.to_string());
            }
        }
    }
    if report.saved.is_empty() {
        return Err(ImportError::Save(first_error.unwrap_or_default()));
    }
    Ok(report)
}

/// Writes `s` to a new file in `dir` named from it (`campbell.json`, else `campbell-2.json`…),
/// returning its id.
pub fn save(dir: &Path, s: &Scheme) -> std::io::Result<String> {
    std::fs::create_dir_all(dir)?;
    let base = stem_for(&s.name);
    for n in 1..=999 {
        let stem = if n == 1 { base.clone() } else { format!("{base}-{n}") };
        let path = dir.join(format!("{stem}.json"));
        // Any entry takes the name, a dangling link too (writing would follow it).
        match path.symlink_metadata() {
            Ok(_) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        crate::persist::atomic_write(&path, scheme_json(s).as_bytes())?;
        return Ok(stem);
    }
    Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, format!("every {base}-N.json name is taken")))
}

/// A lowercase ASCII file stem for a scheme name (`One Half Dark` is `one-half-dark`), safe on
/// every OS; `scheme` when nothing is left.
pub(super) fn stem_for(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out: String = out.trim_end_matches('-').chars().take(60).collect();
    let out = out.trim_end_matches('-');
    if out.is_empty() { "scheme".to_owned() } else { crate::names::portable(out, "-scheme") }
}
