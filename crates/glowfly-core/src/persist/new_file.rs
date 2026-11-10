//! Writing a file under a name nothing has yet: the streamed write of `write_streamed`, finished
//! by a move that refuses a taken name instead of replacing it.

use super::{WriteOptions, make_private_dir, replace, set_mode, sync_dir, temp_sibling};
use std::io;
use std::path::{Path, PathBuf};

/// How many names [`write_streamed_new`] tries, the first one included.
pub(super) const NAME_TRIES: usize = 5;

/// Writes a new file: `fill` writes the content into a temporary sibling of `first`, which is
/// then moved to `first` by a move the file system refuses when the name is taken
/// (`rename_noclobber`). When `first` is taken, `next` is asked for another name in the same
/// folder, at most [`NAME_TRIES`] names in all. Returns the path the file got.
///
/// An error from `fill`, a failed move or running out of names removes the temporary file and
/// replaces nothing; running out of names is `ErrorKind::AlreadyExists`. `opts.refuse_links`
/// doesn't apply: nothing is followed, and a link at a name counts as taken. `private` and
/// `executable` work as in [`write_streamed`](super::write_streamed).
pub fn write_streamed_new(
    first: &Path,
    opts: WriteOptions,
    mut next: impl FnMut() -> PathBuf,
    fill: impl FnOnce(&mut std::fs::File) -> io::Result<()>,
) -> io::Result<PathBuf> {
    let dir = first.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let new_dir = opts.private && std::fs::symlink_metadata(dir).is_err();
    std::fs::create_dir_all(dir)?;
    if new_dir {
        make_private_dir(dir);
    }
    let tmp = temp_sibling(first);
    let written = (|| {
        let mut open = std::fs::OpenOptions::new();
        open.write(true).create_new(true);
        #[cfg(unix)]
        if opts.private {
            use std::os::unix::fs::OpenOptionsExt;
            open.mode(0o600);
        }
        let mut f = open.open(&tmp)?;
        fill(&mut f)?;
        if !opts.private {
            set_mode(&f, None, opts.executable);
        }
        f.sync_all()
    })();
    let fail = |e: io::Error| {
        let _ = replace::remove(&tmp);
        Err(e)
    };
    if let Err(e) = written {
        return fail(e);
    }
    for attempt in 0..NAME_TRIES {
        let target = if attempt == 0 { first.to_owned() } else { next() };
        if target.parent() != Some(dir) {
            let why = format!("{} isn't in {}", target.display(), dir.display());
            return fail(io::Error::new(io::ErrorKind::InvalidInput, why));
        }
        match move_new(&tmp, &target) {
            Ok(()) => {
                sync_dir(dir);
                return Ok(target);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return fail(e),
        }
    }
    fail(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("{} and the next free names were taken while it was written", first.display()),
    ))
}

/// Moves `from` to the free name `to` (never replacing a file there), retrying while another
/// program (a virus scanner on a fresh installer) briefly holds either on Windows.
fn move_new(from: &Path, to: &Path) -> io::Result<()> {
    let mut attempt = 0;
    loop {
        match crate::repo::noclobber::rename_noclobber(from, to) {
            Err(e) if replace::retryable(&e) && attempt < replace::ATTEMPTS => {
                attempt += 1;
                replace::pause(attempt);
            }
            r => return r,
        }
    }
}

#[cfg(test)]
#[path = "new_file_tests.rs"]
mod tests;
