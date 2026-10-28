//! Taking a deleted file back out of the system trash for Undo. Only an item that was at the
//! file's path and holds exactly the bytes Glowfly kept is taken out (the newest such item), and
//! never over anything at the path.
//!
//! - Windows: the Recycle Bin is listed (the `trash` crate) and the item's `$R` file linked back
//!   to its path, which never replaces anything (the shell's own restore runs without UI and so
//!   would overwrite a file saved there meanwhile); then its `$R` bytes and `$I` record go.
//! - Linux and the BSDs (freedesktop): the trash folders the `trash` crate knows (inside Flatpak
//!   also the host's home Trash, which Glowfly moves files into itself) are read, and the file is
//!   linked back to its path (copied when the trash is on another drive), then removed from the
//!   trash with its `.trashinfo`.
//! - macOS: the Trash keeps no record of where a file came from that Glowfly can read, so nothing
//!   is taken out ([`Untrashed::Unsupported`]) and Undo writes its own copy instead.
// The finding helpers serve Windows and freedesktop only; they are tested on every system.
#![cfg_attr(target_os = "macos", allow(dead_code))]

use std::path::{Path, PathBuf};

/// How taking a file back out of the trash went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Untrashed {
    /// The file is back at its path and no longer in the trash.
    Restored,
    /// Nothing in the trash was at that path with those bytes (emptied, restored by hand, or its
    /// bytes couldn't be read to check).
    NotFound,
    /// This system's trash can't be read back (macOS).
    Unsupported,
    /// The trash holds the file but couldn't give it back; what went wrong.
    Failed(String),
}

/// Takes the newest item in the trash that was at the absolute path given, and whose bytes the
/// check accepts (it is given a readable path to them), back to that path; never over anything
/// there. Production code passes [`os_untrash`]; tests pass a stand-in.
pub type Untrasher = fn(&Path, &dyn Fn(&Path) -> bool) -> Untrashed;

/// [`Untrasher`] for the system trash (see the module docs for each system).
pub fn os_untrash(full: &Path, ours: &dyn Fn(&Path) -> bool) -> Untrashed {
    // As in `os_trash`: tests use stand-ins unless asked for by hand.
    if cfg!(test) && std::env::var_os("GLOWFLY_REAL_TRASH").is_none() {
        return Untrashed::Failed("tests never use the real trash".into());
    }
    #[cfg(windows)]
    {
        win_untrash(full, ours)
    }
    #[cfg(target_os = "macos")]
    {
        let _ = (full, ours);
        Untrashed::Unsupported
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        xdg_untrash(full, ours)
    }
}

/// One item in the trash: where it was, when it went (any key that sorts by time), where its
/// bytes are now, and the system's handle for it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Candidate<T> {
    pub original: PathBuf,
    pub deleted: i64,
    pub bytes: PathBuf,
    pub item: T,
}

/// Whether two absolute paths name the same place: component by component, and on Windows
/// without regard to case, separators or a `\\?\` prefix.
pub(crate) fn same_place(a: &Path, b: &Path, windows: bool) -> bool {
    if !windows {
        return a == b;
    }
    let norm = |p: &Path| {
        let s = p.to_string_lossy().replace('/', "\\");
        let s = s.strip_prefix(r"\\?\UNC\").map(|r| format!(r"\\{r}")).unwrap_or(s);
        let s = s.strip_prefix(r"\\?\").map(str::to_owned).unwrap_or(s);
        s.trim_end_matches('\\').to_lowercase()
    };
    norm(a) == norm(b)
}

/// The newest of `items` that was at one of `places` (one path, as given and with its parent
/// folder resolved, since the trash may record either) and whose bytes `ours` accepts.
pub(crate) fn newest_ours<T>(
    mut items: Vec<Candidate<T>>,
    places: &[PathBuf],
    windows: bool,
    ours: &dyn Fn(&Path) -> bool,
) -> Option<Candidate<T>> {
    items.retain(|c| places.iter().any(|p| same_place(&c.original, p, windows)));
    items.sort_by_key(|c| std::cmp::Reverse(c.deleted));
    items.into_iter().find(|c| ours(&c.bytes))
}

/// `full`, and `full` with its parent folder resolved (links, short names) when that differs.
fn places(full: &Path) -> Vec<PathBuf> {
    let mut out = vec![full.to_path_buf()];
    if let (Some(parent), Some(name)) = (full.parent(), full.file_name())
        && let Ok(real) = std::fs::canonicalize(parent)
        && real.join(name) != full
    {
        out.push(real.join(name));
    }
    out
}

/// The `$I…` record next to a Recycle Bin item's `$R…` bytes; `None` for any other name.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn recycle_record(bytes: &Path) -> Option<PathBuf> {
    let name = bytes.file_name()?.to_str()?;
    let rest = name.strip_prefix("$R")?;
    Some(bytes.with_file_name(format!("$I{rest}")))
}

/// The trash's list, or why it couldn't be read. The `trash` crate panics on some malformed
/// `.trashinfo` files; that becomes a failure, so Undo writes Glowfly's copy instead.
fn listed() -> Result<Vec<trash::TrashItem>, String> {
    #[cfg(not(target_os = "macos"))]
    {
        match std::panic::catch_unwind(trash::os_limited::list) {
            Ok(Ok(items)) => Ok(items),
            Ok(Err(e)) => Err(super::detail_of(&e)),
            Err(_) => Err("the trash holds a record Glowfly can't read".into()),
        }
    }
    #[cfg(target_os = "macos")]
    {
        Err("this system's trash can't be read back".into())
    }
}

/// The Recycle Bin item is linked back to its path (never replacing anything, unlike the shell's
/// restore, which runs without UI and so overwrites), then its `$R` bytes and `$I` record go.
#[cfg(windows)]
fn win_untrash(full: &Path, ours: &dyn Fn(&Path) -> bool) -> Untrashed {
    let items = match listed() {
        Ok(items) => items,
        Err(e) => return Untrashed::Failed(e),
    };
    // A Recycle Bin item's parsing name is the path of its bytes in the volume's bin folder.
    let candidates = items
        .into_iter()
        .map(|i| Candidate { original: i.original_path(), deleted: i.time_deleted, bytes: PathBuf::from(&i.id), item: () })
        .collect();
    let Some(found) = newest_ours(candidates, &places(full), true, ours) else { return Untrashed::NotFound };
    let Some(record) = recycle_record(&found.bytes) else {
        return Untrashed::Failed(format!("{} isn't a Recycle Bin item Glowfly knows", found.bytes.display()));
    };
    match super::home::take_out(&found.bytes, &record, full) {
        Ok(()) => Untrashed::Restored,
        Err(e) => Untrashed::Failed(e.to_string()),
    }
}

/// The freedesktop items: the crate's list, plus inside Flatpak the host's home Trash.
#[cfg(not(any(windows, target_os = "macos")))]
fn xdg_untrash(full: &Path, ours: &dyn Fn(&Path) -> bool) -> Untrashed {
    let mut candidates: Vec<Candidate<PathBuf>> = match listed() {
        Ok(items) => items
            .into_iter()
            .map(|i| {
                let info = PathBuf::from(&i.id);
                Candidate { original: i.original_path(), deleted: i.time_deleted, bytes: super::home::bytes_of_info(&info), item: info }
            })
            .collect(),
        Err(e) => return Untrashed::Failed(e),
    };
    if crate::flatpak::in_flatpak()
        && let Some(dir) = super::home::home_trash_dir(crate::flatpak::host_var)
    {
        for c in super::home::list_home(&dir) {
            if !candidates.iter().any(|k| k.item == c.item) {
                candidates.push(c);
            }
        }
    }
    let Some(found) = newest_ours(candidates, &places(full), false, ours) else { return Untrashed::NotFound };
    match super::home::take_out(&found.bytes, &found.item, full) {
        Ok(()) => Untrashed::Restored,
        Err(e) => Untrashed::Failed(e.to_string()),
    }
}
