//! The Trash in the home folder (freedesktop): where it is, what its `.trashinfo` files hold,
//! which mount a path is on, which trash folder on a drive can be used, and moving a file into
//! the home Trash (copying it when it is on another drive). Pure or std-only, so it compiles and
//! is tested on every system, though only Linux and the BSDs use it (`xdg.rs`, `home_trash`).
#![cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_encode};
use std::borrow::Cow;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};

/// What a `.trashinfo` file's `Path=` leaves as it is: letters, digits, the separator and the
/// unreserved marks.
const PATH_KEEPS: &AsciiSet = &NON_ALPHANUMERIC.remove(b'/').remove(b'-').remove(b'_').remove(b'.').remove(b'~');

/// The home Trash: `$XDG_DATA_HOME/Trash`, else `$HOME/.local/share/Trash`. Empty values count
/// as unset, as in `no_home_trash`.
pub(super) fn home_trash_dir(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let set = |k: &str| var(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    match set("XDG_DATA_HOME") {
        Some(data) => Some(data.join("Trash")),
        None => set("HOME").map(|home| home.join(".local").join("share").join("Trash")),
    }
}

/// A path's bytes for a `.trashinfo` file: the raw bytes where the system has them, else the
/// path as text.
pub(super) fn path_bytes(p: &Path) -> Cow<'_, [u8]> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Cow::Borrowed(p.as_os_str().as_bytes())
    }
    #[cfg(not(unix))]
    {
        match p.to_string_lossy() {
            Cow::Borrowed(s) => Cow::Borrowed(s.as_bytes()),
            Cow::Owned(s) => Cow::Owned(s.into_bytes()),
        }
    }
}

/// The contents of the `.trashinfo` file for the item that was at `path_bytes`, deleted at
/// `date` (`YYYY-MM-DDThh:mm:ss`, local time).
pub(super) fn trashinfo(path_bytes: &[u8], date: &str) -> String {
    format!("[Trash Info]\nPath={}\nDeletionDate={date}\n", percent_encode(path_bytes, PATH_KEEPS))
}

/// The mount points in a mount table (`/proc/self/mounts`, `/etc/mtab`): the second field of
/// each line, with the octal escapes for space, tab, newline and backslash decoded.
pub(super) fn parse_mounts(text: &str) -> Vec<PathBuf> {
    text.lines().filter_map(|line| line.split_whitespace().nth(1)).map(|field| PathBuf::from(unescape(field))).collect()
}

fn unescape(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut rest = field;
    while let Some(i) = rest.find('\\') {
        out.push_str(&rest[..i]);
        let code = rest.get(i + 1..i + 4);
        let ch = match code {
            Some("040") => Some(' '),
            Some("011") => Some('\t'),
            Some("012") => Some('\n'),
            Some("134") => Some('\\'),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[i + 4..];
            }
            None => {
                out.push('\\');
                rest = &rest[i + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The mount `path` is on: the longest mount point that contains it, component by component
/// (`/mnt/data` doesn't contain `/mnt/database`).
pub(super) fn topdir_of<'a>(path: &Path, mounts: &'a [PathBuf]) -> Option<&'a Path> {
    mounts.iter().filter(|m| path.starts_with(m)).max_by_key(|m| m.components().count()).map(PathBuf::as_path)
}

/// The shared trash folder of a drive, `$topdir/.Trash`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AdminTrash {
    Missing,
    /// A symbolic link, not a folder, or a folder without the sticky bit: never used.
    Invalid,
    /// Valid, but without a folder for this user.
    NoUserFolder,
    /// `$topdir/.Trash/$uid` exists.
    UserFolder {
        writable: bool,
    },
}

/// This user's own trash folder on a drive, `$topdir/.Trash-$uid`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UserTrash {
    /// Not there yet; it can be made when the drive's top folder may be changed.
    Missing {
        topdir_writable: bool,
    },
    Folder {
        writable: bool,
    },
    NotAFolder,
}

/// What a drive's trash folders look like.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct VolumeTrashFacts {
    pub admin: AdminTrash,
    pub user: UserTrash,
}

/// Whether a file on a drive with `f` can go to the drive's own trash. A `.Trash/$uid` that is
/// there is the only choice, writable or not (when moving into it fails, `.Trash-$uid` isn't
/// tried); otherwise `.Trash-$uid` must be a writable folder, or creatable.
pub(super) fn volume_trash_usable(f: &VolumeTrashFacts) -> bool {
    match f.admin {
        AdminTrash::UserFolder { writable } => writable,
        _ => matches!(f.user, UserTrash::Folder { writable: true } | UserTrash::Missing { topdir_writable: true }),
    }
}

/// Moves the regular file `src` (an absolute path) into the trash folder `trash` with its
/// `.trashinfo` (deleted at `date`), under a name not taken yet ("a.txt", "a.txt.2", …).
/// `rename` is `std::fs::rename` outside tests. Across drives the file is copied (keeping its
/// permissions) and the original removed. On any failure the trash and `src` are left as they
/// were. Returns where the file went.
pub(super) fn move_into(src: &Path, trash: &Path, date: &str, rename: fn(&Path, &Path) -> io::Result<()>) -> io::Result<PathBuf> {
    let meta = fs::symlink_metadata(src)?;
    if !meta.is_file() {
        return Err(io::Error::new(ErrorKind::InvalidInput, format!("{} is not a regular file", src.display())));
    }
    let name = src.file_name().ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "the path has no file name"))?;
    let (files, info) = (trash.join("files"), trash.join("info"));
    make_dir(&files)?;
    make_dir(&info)?;
    for n in 1..=10_000u32 {
        let mut item = name.to_os_string();
        if n > 1 {
            item.push(format!(".{n}"));
        }
        let mut info_name = item.clone();
        info_name.push(".trashinfo");
        let info_file = info.join(info_name);
        // The info file claims the name, as other programs using the trash expect.
        let mut f = match OpenOptions::new().write(true).create_new(true).open(&info_file) {
            Ok(f) => f,
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        };
        let target = files.join(&item);
        if target.symlink_metadata().is_ok() {
            drop(f);
            let _ = fs::remove_file(&info_file);
            continue;
        }
        let written = f.write_all(trashinfo(&path_bytes(src), date).as_bytes()).and_then(|()| f.sync_all());
        drop(f);
        return match written.and_then(|()| place(src, &target, &meta, rename)) {
            Ok(()) => Ok(target),
            Err(e) => {
                let _ = fs::remove_file(&info_file);
                Err(e)
            }
        };
    }
    Err(io::Error::new(ErrorKind::AlreadyExists, "the Trash already holds too many files with this name"))
}

/// Puts `src` at `target`: renamed, or copied and then removed when it is on another drive.
/// A failed copy removes the partial `target` and leaves `src`.
fn place(src: &Path, target: &Path, meta: &fs::Metadata, rename: fn(&Path, &Path) -> io::Result<()>) -> io::Result<()> {
    match rename(src, target) {
        Err(e) if e.kind() == ErrorKind::CrossesDevices => {}
        r => return r,
    }
    let mut out = OpenOptions::new().write(true).create_new(true).open(target)?;
    let copied = File::open(src)
        .and_then(|mut input| io::copy(&mut input, &mut out))
        .and_then(|_| out.sync_all())
        .and_then(|()| fs::set_permissions(target, meta.permissions()));
    drop(out);
    match copied.and_then(|()| fs::remove_file(src)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(target);
            Err(e)
        }
    }
}

/// Creates `dir` and its parents, private to this user where the system has modes.
fn make_dir(dir: &Path) -> io::Result<()> {
    let mut b = fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    b.create(dir)
}
