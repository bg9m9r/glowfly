//! Taking a deleted file back out of the system trash for Undo. Only an item that was at the
//! file's path and holds exactly the bytes Glowfly kept is taken out (the newest such item), and
//! never over anything at the path.
//!
//! - Windows: the Recycle Bin is listed and the item restored through the shell (the `trash`
//!   crate), which keeps its dates and attributes.
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

/// The newest of `items` that was at `full` and whose bytes `ours` accepts.
pub(crate) fn newest_ours<T>(
    mut items: Vec<Candidate<T>>,
    full: &Path,
    windows: bool,
    ours: &dyn Fn(&Path) -> bool,
) -> Option<Candidate<T>> {
    items.retain(|c| same_place(&c.original, full, windows));
    items.sort_by_key(|c| std::cmp::Reverse(c.deleted));
    items.into_iter().find(|c| ours(&c.bytes))
}

#[cfg(windows)]
fn win_untrash(full: &Path, ours: &dyn Fn(&Path) -> bool) -> Untrashed {
    let items = match trash::os_limited::list() {
        Ok(items) => items,
        Err(e) => return Untrashed::Failed(super::detail_of(&e)),
    };
    // A Recycle Bin item's parsing name is the path of its bytes in the volume's bin folder.
    let candidates = items
        .into_iter()
        .map(|i| Candidate { original: i.original_path(), deleted: i.time_deleted, bytes: PathBuf::from(&i.id), item: i })
        .collect();
    let Some(found) = newest_ours(candidates, full, true, ours) else { return Untrashed::NotFound };
    if full.symlink_metadata().is_ok() {
        return Untrashed::Failed(format!("{} is in the way", full.display()));
    }
    match trash::os_limited::restore_all([found.item]) {
        Ok(()) => Untrashed::Restored,
        Err(e) => Untrashed::Failed(super::detail_of(&e)),
    }
}

/// The freedesktop items: the crate's list, plus inside Flatpak the host's home Trash.
#[cfg(not(any(windows, target_os = "macos")))]
fn xdg_untrash(full: &Path, ours: &dyn Fn(&Path) -> bool) -> Untrashed {
    let mut candidates: Vec<Candidate<PathBuf>> = match trash::os_limited::list() {
        Ok(items) => items
            .into_iter()
            .map(|i| {
                let info = PathBuf::from(&i.id);
                Candidate { original: i.original_path(), deleted: i.time_deleted, bytes: super::home::bytes_of_info(&info), item: info }
            })
            .collect(),
        Err(e) => return Untrashed::Failed(super::detail_of(&e)),
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
    let Some(found) = newest_ours(candidates, full, false, ours) else { return Untrashed::NotFound };
    match super::home::take_out(&found.bytes, &found.item, full) {
        Ok(()) => Untrashed::Restored,
        Err(e) => Untrashed::Failed(e.to_string()),
    }
}
