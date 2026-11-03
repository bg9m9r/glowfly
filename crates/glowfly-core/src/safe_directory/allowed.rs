//! Folders git opens though libgit2 refuses them for their owner, allowed for this process only.
//! libgit2 has no per-path switch for its owner check, but it honours `safe.directory` in the
//! configs outside the repository. At start (`init`) Glowfly puts a folder of its own first in
//! libgit2's search path for the global config (`.gitconfig`). The folder holds no `.gitconfig`
//! until a folder is allowed, so libgit2 reads the user's own as before. Once one is allowed
//! (`allow`), Glowfly writes a `.gitconfig` there that includes the user's and lists the allowed
//! folders. The global level is the user's own file and libgit2 never checks its owner, so the
//! wrapper changes nothing else: the ProgramData and system configs are found, and their owner
//! checked, exactly as before. Nothing reaches the user's git settings, and nothing is allowed
//! that git itself doesn't open (`verdict`).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// The search-path separator libgit2 uses (`GIT_PATH_LIST_SEPARATOR`).
const SEPARATOR: char = if cfg!(windows) { ';' } else { ':' };

/// The global config's name in each folder of the search path.
const GLOBAL_FILE: &str = ".gitconfig";

/// Stale lists of other processes are removed after this long without a write.
const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 3600);

struct Allowed {
    /// The `.gitconfig` libgit2 looks for first.
    file: PathBuf,
    /// The folders libgit2 searched for the global config before `init`, in order.
    homes: Vec<PathBuf>,
    entries: Mutex<Vec<String>>,
}

static ALLOWED: OnceLock<Allowed> = OnceLock::new();

/// Points libgit2's global config search at this process's own folder under the cache folder,
/// ahead of the usual ones. Call once at start, before libgit2 is used and before any thread
/// that could use it starts; without it `allow` allows nothing.
pub fn init() -> Result<(), String> {
    let Some(base) = crate::settings::cache_dir().map(|c| c.join("allowed-by-git")) else { return Ok(()) };
    remove_stale(&base);
    let dir = base.join(std::process::id().to_string());
    let file = dir.join(GLOBAL_FILE);
    let _ = std::fs::remove_file(&file);
    std::fs::create_dir_all(&dir).map_err(|e| format!("couldn't create {}: {e}", dir.display()))?;
    let text = dir.to_str().filter(|d| !d.contains(SEPARATOR) && !d.contains("$PATH"));
    let Some(text) = text else { return Err(format!("{} can't go in libgit2's search path", dir.display())) };
    // SAFETY: called once on the main thread at start, before libgit2 is used. The threads
    // started before it (logging, the system font loader) never call libgit2, so nothing reads
    // the search path while it changes.
    let homes = unsafe { git2::opts::get_search_path(git2::ConfigLevel::Global) }.map_err(|e| e.to_string())?;
    let homes = homes.to_string_lossy().split(SEPARATOR).filter(|d| !d.is_empty()).map(PathBuf::from).collect();
    // SAFETY: as above.
    unsafe { git2::opts::set_search_path(git2::ConfigLevel::Global, format!("{text}{SEPARATOR}$PATH")) }
        .map_err(|e| format!("couldn't point libgit2 at {}: {e}", dir.display()))?;
    let _ = ALLOWED.set(Allowed { file, homes, entries: Mutex::new(Vec::new()) });
    Ok(())
}

/// Whether `value` can go in the list: no control character, which could end its line and
/// write config of its own.
pub fn listable(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_control)
}

/// Lets libgit2 open the folders `values` (in `safe.directory` spelling) for the rest of this
/// process. False, with nothing listed, when `init` didn't run, a value isn't `listable` or the
/// list couldn't be written.
pub fn allow(values: &[String]) -> bool {
    if !values.iter().all(|v| listable(v)) {
        return false;
    }
    let Some(a) = ALLOWED.get() else { return false };
    let mut entries = a.entries.lock().unwrap_or_else(|e| e.into_inner());
    let added: Vec<&String> = values.iter().filter(|v| !entries.contains(v)).collect();
    if added.is_empty() && a.file.exists() {
        return true;
    }
    let mut next = entries.clone();
    next.extend(added.into_iter().cloned());
    if write(a, &next) {
        *entries = next;
        true
    } else {
        false
    }
}

/// Takes `values` off the list again (a folder that turned out not to be the one git vetted).
pub fn forget(values: &[String]) {
    let Some(a) = ALLOWED.get() else { return };
    let mut entries = a.entries.lock().unwrap_or_else(|e| e.into_inner());
    entries.retain(|e| !values.contains(e));
    if !write(a, &entries) {
        // A list that can't be rewritten mustn't keep a folder: without it, nothing is allowed.
        let _ = std::fs::remove_file(&a.file);
        entries.clear();
    }
}

fn write(a: &Allowed, entries: &[String]) -> bool {
    let opts = crate::persist::WriteOptions { refuse_links: true, private: true, ..Default::default() };
    let text = file_text(user_config(&a.homes).as_deref(), entries);
    match crate::persist::write_file(&a.file, text.as_bytes(), opts) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!("couldn't write {}: {e}", a.file.display());
            false
        }
    }
}

/// The global config libgit2 would read without the list: the first `.gitconfig` in `homes`,
/// else where the first would be (included all the same: a file made later is read then).
fn user_config(homes: &[PathBuf]) -> Option<PathBuf> {
    let files = homes.iter().map(|h| h.join(GLOBAL_FILE));
    files.clone().find(|f| f.exists()).or_else(|| files.into_iter().next())
}

/// The `.gitconfig` that includes `include` and lists `entries` under `safe.directory`. Every
/// entry must be `listable`.
pub(crate) fn file_text(include: Option<&Path>, entries: &[String]) -> String {
    let mut text = String::from("# Written by Glowfly for this run only: folders git opens. Deleted at the next start.\n");
    if let Some(p) = include.and_then(Path::to_str).filter(|p| listable(p)) {
        let p = if cfg!(windows) { p.replace('\\', "/") } else { p.to_owned() };
        text.push_str(&format!("[include]\n\tpath = {}\n", quoted(&p)));
    }
    let entries: Vec<&String> = entries.iter().filter(|e| listable(e)).collect();
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
        let written = std::fs::metadata(path.join(GLOBAL_FILE)).or_else(|_| d.metadata()).and_then(|m| m.modified());
        if written.is_ok_and(|t| now.duration_since(t).is_ok_and(|age| age > STALE_AFTER)) {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_includes_the_users_config_and_names_each_folder() {
        assert_eq!(file_text(None, &[]).lines().count(), 1, "only the comment");
        let real = if cfg!(windows) { Path::new(r"C:\Users\ada\.gitconfig") } else { Path::new("/home/ada/.gitconfig") };
        let entries = vec!["C:/work/app".to_owned(), "%(prefix)///nas/team/app".to_owned(), r#"/srv/a"b\c"#.to_owned()];
        let text = file_text(Some(real), &entries);
        let path = if cfg!(windows) { "C:/Users/ada/.gitconfig" } else { "/home/ada/.gitconfig" };
        assert!(text.contains(&format!("[include]\n\tpath = \"{path}\"\n")), "{text}");
        assert!(text.contains("[safe]\n\tdirectory = \"C:/work/app\"\n\tdirectory = \"%(prefix)///nas/team/app\"\n"), "{text}");
        assert!(text.contains(r#"directory = "/srv/a\"b\\c""#), "escaped: {text}");
    }

    #[test]
    fn a_control_character_never_reaches_the_list() {
        let crafted = "/srv/shared/x\n[safe]\n\tdirectory = *".to_owned();
        assert!(!listable(&crafted) && !listable("/srv/a\rb") && !listable("/srv/a\u{7f}b") && !listable(""));
        assert!(listable("/srv/shared/x") && listable("%(prefix)///nas/team/app"));
        assert!(!allow(&["/srv/shared/x".to_owned(), crafted.clone()]), "refused whole");
        let text = file_text(None, &["/srv/ok".to_owned(), crafted]);
        assert!(!text.contains('*') && text.lines().filter(|l| l.contains("directory")).count() == 1, "{text}");
    }

    #[test]
    fn the_users_config_is_the_first_that_exists() {
        let base = crate::paths::temp_dir().join(format!("glowfly-allowed-homes-{}", std::process::id()));
        let (a, b) = (base.join("a"), base.join("b"));
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(b.join(GLOBAL_FILE), "").unwrap();
        assert_eq!(user_config(&[a.clone(), b.clone()]), Some(b.join(GLOBAL_FILE)));
        std::fs::remove_file(b.join(GLOBAL_FILE)).unwrap();
        assert_eq!(user_config(&[a.clone(), b]), Some(a.join(GLOBAL_FILE)), "none yet: the first");
        assert_eq!(user_config(&[]), None);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn nothing_is_allowed_without_init() {
        // The test process never points libgit2 elsewhere (`init` is for the app's start).
        assert!(!allow(&["C:/work/app".to_owned()]));
    }
}
