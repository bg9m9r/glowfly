//! A binary diff's two sides as the viewer shows them: each side's size and, for an image
//! small enough, its decoded image; any other file of at most `HEX_MAX_BYTES` brings its bytes
//! for the hex view. The sides come from `tool_sides::diff_sides`, so renames,
//! deletions and untracked files resolve the same way the viewer and the external diff tool see
//! them (a submodule has no sides and gives an error).

use super::DiffTarget;
use super::tool_sides::{SideSource, diff_sides};
use crate::error::Result;
use crate::lfs::pointer;
use crate::preview_image::{self, PreviewImage};
use git2::Repository;
use std::io::Read;
use std::path::Path;

/// The largest file the preview decodes; a larger one shows its size only.
pub const PREVIEW_MAX_BYTES: u64 = 32 << 20;

/// The largest file whose bytes come along for the hex view; a larger one shows its size only.
pub const HEX_MAX_BYTES: u64 = 64 << 10;

/// One side of a binary file.
#[derive(Clone, Debug)]
pub struct BinarySide {
    /// Its size in bytes.
    pub size: u64,
    /// The decoded image, for an image path that decoded; `None` otherwise.
    pub image: Option<PreviewImage>,
    /// The file's bytes for the hex view: `Some` for a file that is no image and at most
    /// `HEX_MAX_BYTES` long (nor a Git LFS pointer), `None` otherwise.
    pub bytes: Option<Vec<u8>>,
}

/// Both sides; `None` where the file doesn't exist (an added file's old side, a deleted file's
/// new side).
#[derive(Clone, Debug)]
pub struct BinarySides {
    pub old: Option<BinarySide>,
    pub new: Option<BinarySide>,
}

/// The sizes, previews and hex bytes of the binary file `target` names, decoding images up to
/// `PREVIEW_MAX_BYTES` and reading other files up to `HEX_MAX_BYTES`. Blocking: call from a
/// worker thread.
pub fn binary_sides(workdir: &Path, target: &DiffTarget) -> Result<BinarySides> {
    binary_sides_with(workdir, target, Caps { image: PREVIEW_MAX_BYTES, hex: HEX_MAX_BYTES })
}

/// The largest image decoded and the largest other file read for the hex view.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Caps {
    pub image: u64,
    pub hex: u64,
}

/// `binary_sides` within `caps`.
pub(crate) fn binary_sides_with(workdir: &Path, target: &DiffTarget, caps: Caps) -> Result<BinarySides> {
    let sides = diff_sides(workdir, target)?;
    let repo = Repository::open(workdir)?;
    let mut out = BinarySides { old: side(&repo, workdir, &sides.old, caps)?, new: side(&repo, workdir, &sides.new, caps)? };
    // The hex view compares both sides: one side without its bytes leaves neither.
    if out.hex().is_none() {
        out.old.iter_mut().chain(out.new.iter_mut()).for_each(|s| s.bytes = None);
    }
    Ok(out)
}

/// Both sides' bytes for the hex view, `None` for a side without the file.
pub type HexBytes<'a> = (Option<&'a [u8]>, Option<&'a [u8]>);

impl BinarySides {
    /// Both sides' bytes for the hex view, when every side that has the file brought them.
    pub fn hex(&self) -> Option<HexBytes<'_>> {
        fn of(s: &Option<BinarySide>) -> Option<Option<&[u8]>> {
            match s {
                None => Some(None),
                Some(s) => s.bytes.as_deref().map(Some),
            }
        }
        let (old, new) = (of(&self.old)?, of(&self.new)?);
        (old.is_some() || new.is_some()).then_some((old, new))
    }
}

/// One side's size, preview and hex bytes; `None` for a side with no file. The contents are
/// read only when the size is within a cap.
fn side(repo: &Repository, workdir: &Path, source: &SideSource, caps: Caps) -> Result<Option<BinarySide>> {
    let (path, size) = match source {
        SideSource::Empty => return Ok(None),
        SideSource::Blob { id, path, .. } => (path.as_str(), repo.odb()?.read_header(*id)?.0 as u64),
        SideSource::Working(p) => match std::fs::metadata(workdir.join(p)) {
            Ok(meta) => (p.as_str(), meta.len()),
            Err(_) => return Ok(None),
        },
    };
    let is_image = preview_image::is_image_path(path);
    let cap = if is_image { caps.image } else { caps.hex };
    let bytes = if size <= cap { contents(repo, workdir, source, cap)?.filter(|b| pointer::parse(b).is_none()) } else { None };
    Ok(Some(match is_image {
        true => BinarySide { size, image: bytes.as_deref().and_then(preview_image::decode), bytes: None },
        false => BinarySide { size, image: None, bytes },
    }))
}

/// The side's contents, `None` for a working file that can't be read or grew past `cap` since
/// its size was read (it keeps the size-only view).
fn contents(repo: &Repository, workdir: &Path, source: &SideSource, cap: u64) -> Result<Option<Vec<u8>>> {
    Ok(match source {
        SideSource::Blob { id, .. } => Some(repo.find_blob(*id)?.content().to_vec()),
        SideSource::Working(p) => {
            let mut out = Vec::new();
            let read = std::fs::File::open(workdir.join(p)).and_then(|f| f.take(cap + 1).read_to_end(&mut out));
            read.ok().filter(|_| out.len() as u64 <= cap).map(|_| out)
        }
        SideSource::Empty => None,
    })
}

#[cfg(test)]
#[path = "binary_sides_tests.rs"]
mod tests;
