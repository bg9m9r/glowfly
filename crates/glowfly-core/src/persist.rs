//! Every file Trellis writes goes through here: its own files (settings, recent repositories,
//! open tabs, logs), files written for the user (saved patches), and working-tree files it puts
//! back itself (undo and redo of a discard or restore). [`remove_tree`] deletes a folder Trellis
//! owns the removal of (a submodule's stored module) without following links. Logs grow by
//! [`append`] (or a handle from [`open_append`]) instead of being replaced.
//!
//! A file is written to a temporary sibling, flushed to disk, then renamed over the target, so a
//! crash or power cut leaves either the old file or the new one, never half of each. On Unix the
//! folder is flushed too, so the rename itself survives a power cut. The new file keeps the old
//! one's permissions (and, on Windows, its read-only attribute). A symbolic link at the target
//! is followed to the file it names and stays a link, or, for working-tree files, refused.
//! Windows refuses a rename while another program (a virus scanner, an indexer, a sync client)
//! briefly holds the file, so that is retried. Temporary files a crash left behind are removed
//! by the next write of the same file. [`LatestOnly`] keeps saves that run on separate threads
//! from landing out of order.

mod replace;

use replace::{clean_stale_temps, replace, set_mode, sync_dir};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// How [`write_file`] treats what is already at the target.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WriteOptions {
    /// Refuse when the target is a symbolic link, instead of following it to the file it names.
    /// Working-tree files set this: a link in a repository can point anywhere.
    pub refuse_links: bool,
    /// Set (`Some(true)`) or clear (`Some(false)`) the executable bits on Unix; `None` keeps the
    /// old file's. Nowhere else has them. Failing to set them (FAT and exFAT drives, some
    /// network shares) doesn't fail the write.
    pub executable: Option<bool>,
    /// Only this user may read the file: on Unix it is created with mode 0600 before any byte
    /// is written (whatever the old file's mode was, and `executable` is ignored), and a folder
    /// this write creates gets 0700. Elsewhere nothing changes (Windows profile folders are
    /// already private to their user).
    pub private: bool,
}

/// Replaces `path` with `bytes` atomically, creating its folder if needed. A symbolic link at
/// `path` is followed and stays a link.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    write_file(path, bytes, WriteOptions::default())
}

/// Replaces `path` with `bytes` atomically, creating its folder if needed, as `opts` says.
pub fn write_file(path: &Path, bytes: &[u8], opts: WriteOptions) -> std::io::Result<()> {
    let target = if opts.refuse_links {
        if is_link(path) {
            return Err(std::io::Error::other(format!("{} is a symbolic link; Trellis doesn't write through links", path.display())));
        }
        path.to_owned()
    } else {
        link_target(path)?
    };
    let dir = target.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let new_dir = opts.private && std::fs::symlink_metadata(dir).is_err();
    std::fs::create_dir_all(dir)?;
    if new_dir {
        make_private_dir(dir);
    }
    let old = std::fs::metadata(&target).ok().filter(|m| m.is_file());
    let tmp = temp_sibling(&target);
    let written = (|| {
        let mut open = std::fs::OpenOptions::new();
        open.write(true).create_new(true);
        #[cfg(unix)]
        if opts.private {
            use std::os::unix::fs::OpenOptionsExt;
            open.mode(0o600);
        }
        let mut f = open.open(&tmp)?;
        f.write_all(bytes)?;
        if !opts.private {
            set_mode(&f, old.as_ref(), opts.executable);
        }
        f.sync_all()
    })();
    let result = written.and_then(|()| replace(&tmp, &target, old.as_ref()));
    if result.is_err() {
        let _ = replace::remove(&tmp);
        return result;
    }
    sync_dir(dir);
    if let Some(name) = target.file_name() {
        clean_stale_temps(dir, &name.to_string_lossy());
    }
    Ok(())
}

/// Opens `path` for appending, creating it and its folder if needed. A symbolic link at `path`
/// is followed. Only for append-only logs: each write lands at the end of the file, even when
/// another process appends to it too, and nothing is flushed to disk, so a crash can lose or tear
/// the last line. Readers must skip a bad last line.
pub fn open_append(path: &Path) -> std::io::Result<std::fs::File> {
    let target = link_target(path)?;
    if let Some(dir) = target.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::OpenOptions::new().create(true).append(true).open(&target)
}

/// Appends `bytes` to the log at `path` in one write (see [`open_append`] for what a crash can
/// leave).
pub fn append(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    open_append(path)?.write_all(bytes)
}

/// Gives a folder that a private write just created mode 0700 on Unix (best effort).
fn make_private_dir(dir: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// Removes the file at `path` (not following a link). A missing file is fine. On Windows a
/// read-only file is removed too, and a file another program briefly holds is retried.
pub fn remove_file(path: &Path) -> std::io::Result<()> {
    match replace::remove(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        r => r,
    }
}

/// Removes the folder (or file) at `path` and everything in it, depth first, never following a
/// link: a symbolic link or junction inside is removed as a link, and its target is left alone.
/// Files go through [`remove_file`] (read-only ones too, as pack files under `.git` are on
/// Windows). A missing path is fine.
pub fn remove_tree(path: &Path) -> std::io::Result<()> {
    let meta = match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        r => r?,
    };
    if meta.file_type().is_symlink() {
        return remove_link(path, &meta);
    }
    if !meta.is_dir() {
        return remove_file(path);
    }
    for entry in std::fs::read_dir(path)? {
        remove_tree(&entry?.path())?;
    }
    std::fs::remove_dir(path)
}

/// Removes the link at `path` itself. Windows removes a link to a folder (or a junction) as a
/// folder, which leaves its target as it is.
fn remove_link(path: &Path, meta: &std::fs::Metadata) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileTypeExt;
        if meta.file_type().is_symlink_dir() {
            return std::fs::remove_dir(path);
        }
    }
    #[cfg(not(windows))]
    let _ = meta;
    remove_file(path)
}

fn is_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

/// Where a chain of symbolic links starting at `path` ends (possibly a file that doesn't exist
/// yet); `path` itself when it isn't a link. Relative targets are relative to the link's folder.
fn link_target(path: &Path) -> std::io::Result<PathBuf> {
    let mut p = path.to_owned();
    for _ in 0..40 {
        if !is_link(&p) {
            return Ok(p);
        }
        let to = std::fs::read_link(&p)?;
        p = if to.is_absolute() { to } else { p.parent().unwrap_or(Path::new("")).join(to) };
    }
    Err(std::io::Error::other(format!("{} is a chain of too many symbolic links", path.display())))
}

/// A temporary name next to `path` that no other writer (thread or process) is using:
/// `.<name>.<process id>.<counter>.tmp`.
fn temp_sibling(path: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!(".{name}.{}.{n}.tmp", std::process::id()))
}

/// Whether `candidate` is a name [`temp_sibling`] makes for a file called `name`.
fn is_temp_of(name: &str, candidate: &str) -> bool {
    let Some(rest) = candidate.strip_prefix('.').and_then(|r| r.strip_prefix(name)).and_then(|r| r.strip_suffix(".tmp")) else {
        return false;
    };
    let mut parts = rest.split('.');
    let digits = |s: Option<&str>| s.is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()));
    parts.next() == Some("") && digits(parts.next()) && digits(parts.next()) && parts.next().is_none()
}

/// The Unix permission bits a replacement gets: the old file's (`old`), or those a new file is
/// created with (`created`, the umask applied), with the executable bits set or cleared as
/// `executable` says. Setting them adds `x` wherever `r` is set, as git does.
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) fn wanted_mode(old: Option<u32>, created: u32, executable: Option<bool>) -> u32 {
    let mode = old.unwrap_or(created) & 0o7777;
    match executable {
        None => mode,
        Some(true) => mode | ((mode & 0o444) >> 2),
        Some(false) => mode & !0o111,
    }
}

/// Whether a file with permission bits `have` needs a chmod to get `want`. Skipping one that
/// isn't needed matters on file systems that refuse every chmod (FAT, exFAT).
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) fn needs_chmod(have: u32, want: u32) -> bool {
    have & 0o7777 != want & 0o7777
}

/// Moves `path` aside to the first free `<name>.bak`, `<name>.bak-2`, `<name>.bak-3`, … and
/// returns the new name.
pub fn move_aside(path: &Path) -> std::io::Result<PathBuf> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let backup = (1..)
        .map(|i| path.with_file_name(if i == 1 { format!("{name}.bak") } else { format!("{name}.bak-{i}") }))
        .find(|p| p.symlink_metadata().is_err())
        .expect("an unused backup name");
    std::fs::rename(path, &backup)?;
    Ok(backup)
}

/// Orders saves of one file that run on separate threads: each save takes a ticket when it is
/// requested, and a save whose ticket is older than one already written is skipped, so the
/// newest snapshot always wins.
#[derive(Default)]
pub struct LatestOnly {
    issued: AtomicU64,
    written: Mutex<u64>,
}

impl LatestOnly {
    /// Takes the next ticket. Call this where saves are requested, in request order.
    pub fn ticket(&self) -> u64 {
        self.issued.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Runs `write` unless a newer ticket has already been written; `None` when skipped.
    /// Writes are one at a time.
    pub fn write<T>(&self, ticket: u64, write: impl FnOnce() -> T) -> Option<T> {
        let mut written = self.written.lock().unwrap_or_else(|e| e.into_inner());
        if ticket <= *written {
            return None;
        }
        let out = write();
        *written = ticket;
        Some(out)
    }
}

#[cfg(test)]
mod tests;
