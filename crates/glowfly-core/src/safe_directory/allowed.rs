//! Folders git opens though libgit2 refuses them for their owner, allowed for this process only.
//! libgit2 has no per-path switch for its owner check, but it honours `safe.directory` in every
//! config it loads besides the repository's own. At start (`init`) Glowfly puts a folder of its
//! own first in libgit2's search path for the ProgramData config. The folder holds no `config`
//! until a folder is allowed, so libgit2 reads the real ProgramData config as before. Once one
//! is allowed (`allow`), Glowfly writes a `config` there that includes the real one and lists
//! the allowed folders. Nothing reaches the user's git settings, and nothing is allowed
//! that git itself doesn't open (`verdict`).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// The search-path separator libgit2 uses (`GIT_PATH_LIST_SEPARATOR`).
const SEPARATOR: char = if cfg!(windows) { ';' } else { ':' };

/// Stale lists of other processes are removed after this long without a write.
const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 3600);

struct Allowed {
    /// The `config` libgit2 looks for first.
    file: PathBuf,
    /// The ProgramData config libgit2 reads without `init` (Windows only), included first; a
    /// missing file is skipped.
    include: Option<PathBuf>,
    entries: Mutex<Vec<String>>,
}

static ALLOWED: OnceLock<Allowed> = OnceLock::new();

/// Points libgit2's ProgramData config search at this process's own folder under the cache
/// folder, ahead of the usual one. Call once at start, before any other thread exists and
/// before libgit2 is used; without it `allow` allows nothing.
pub fn init() -> Result<(), String> {
    let Some(base) = crate::settings::cache_dir().map(|c| c.join("allowed-by-git")) else { return Ok(()) };
    remove_stale(&base);
    let dir = base.join(std::process::id().to_string());
    let file = dir.join("config");
    let _ = std::fs::remove_file(&file);
    std::fs::create_dir_all(&dir).map_err(|e| format!("couldn't create {}: {e}", dir.display()))?;
    let text = dir.to_str().filter(|d| !d.contains(SEPARATOR) && !d.contains("$PATH"));
    let Some(text) = text else { return Err(format!("{} can't go in libgit2's search path", dir.display())) };
    let include = programdata_config();
    // SAFETY: called once on the main thread at start, before any other thread exists and before
    // libgit2 is used, so nothing reads the search path while it changes.
    unsafe { git2::opts::set_search_path(git2::ConfigLevel::ProgramData, format!("{text}{SEPARATOR}$PATH")) }
        .map_err(|e| format!("couldn't point libgit2 at {}: {e}", dir.display()))?;
    let _ = ALLOWED.set(Allowed { file, include, entries: Mutex::new(Vec::new()) });
    Ok(())
}

/// The ProgramData config libgit2 reads by default: `%PROGRAMDATA%\Git\config` on Windows,
/// none elsewhere.
fn programdata_config() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    std::env::var_os("PROGRAMDATA").map(PathBuf::from).filter(|p| p.is_absolute()).map(|p| p.join("Git").join("config"))
}

/// Lets libgit2 open the folders `values` (in `safe.directory` spelling) for the rest of this
/// process. False when `init` didn't run or the list couldn't be written.
pub fn allow(values: &[String]) -> bool {
    let Some(a) = ALLOWED.get() else { return false };
    let mut entries = a.entries.lock().unwrap_or_else(|e| e.into_inner());
    let before = entries.len();
    for v in values {
        if !entries.contains(v) {
            entries.push(v.clone());
        }
    }
    if entries.len() == before && a.file.exists() {
        return true;
    }
    match crate::persist::atomic_write(&a.file, file_text(a.include.as_deref(), &entries).as_bytes()) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!("couldn't write {}: {e}", a.file.display());
            false
        }
    }
}

/// The `config` that includes `include` and lists `entries` under `safe.directory`.
pub(crate) fn file_text(include: Option<&Path>, entries: &[String]) -> String {
    let mut text = String::from("# Written by Glowfly for this run only: folders git opens. Deleted at the next start.\n");
    if let Some(p) = include {
        let p = p.to_string_lossy();
        let p = if cfg!(windows) { p.replace('\\', "/") } else { p.into_owned() };
        text.push_str(&format!("[include]\n\tpath = {}\n", quoted(&p)));
    }
    if !entries.is_empty() {
        text.push_str("[safe]\n");
        for e in entries {
            text.push_str(&format!("\tdirectory = {}\n", quoted(e)));
        }
    }
    text
}

/// `value` as a quoted config value: backslashes and quotes escaped.
fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Removes lists other processes left behind and haven't written to for `STALE_AFTER`.
fn remove_stale(base: &Path) {
    let Ok(dirs) = std::fs::read_dir(base) else { return };
    let now = std::time::SystemTime::now();
    for d in dirs.flatten() {
        let path = d.path();
        let written = std::fs::metadata(path.join("config")).or_else(|_| d.metadata()).and_then(|m| m.modified());
        if written.is_ok_and(|t| now.duration_since(t).is_ok_and(|age| age > STALE_AFTER)) {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_includes_the_real_config_and_names_each_folder() {
        assert_eq!(file_text(None, &[]).lines().count(), 1, "only the comment");
        let real = if cfg!(windows) { Path::new(r"C:\ProgramData\Git\config") } else { Path::new("/etc/pd/config") };
        let entries = vec!["C:/work/app".to_owned(), "%(prefix)///nas/team/app".to_owned(), r#"/srv/a"b\c"#.to_owned()];
        let text = file_text(Some(real), &entries);
        let path = if cfg!(windows) { "C:/ProgramData/Git/config" } else { "/etc/pd/config" };
        assert!(text.contains(&format!("[include]\n\tpath = \"{path}\"\n")), "{text}");
        assert!(text.contains("[safe]\n\tdirectory = \"C:/work/app\"\n\tdirectory = \"%(prefix)///nas/team/app\"\n"), "{text}");
        assert!(text.contains(r#"directory = "/srv/a\"b\\c""#), "escaped: {text}");
    }

    #[test]
    fn nothing_is_allowed_without_init() {
        // The test process never points libgit2 elsewhere (`init` is for the app's start).
        assert!(!allow(&["C:/work/app".to_owned()]));
    }
}
