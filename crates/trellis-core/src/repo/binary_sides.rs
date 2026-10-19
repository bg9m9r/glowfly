//! A binary diff's two sides as the viewer shows them: each side's size and, for an image
//! small enough, its decoded image. The sides come from `tool_sides::diff_sides`, so renames,
//! deletions and untracked files resolve the same way the viewer and the external diff tool see
//! them (a submodule has no sides and gives an error).

use super::DiffTarget;
use super::tool_sides::{SideSource, diff_sides};
use crate::error::Result;
use crate::lfs::pointer;
use crate::preview_image::{self, PreviewImage};
use git2::Repository;
use std::path::Path;

/// The largest file the preview decodes; a larger one shows its size only.
pub const PREVIEW_MAX_BYTES: u64 = 32 << 20;

/// One side of a binary file.
#[derive(Clone, Debug)]
pub struct BinarySide {
    /// Its size in bytes.
    pub size: u64,
    /// The decoded image, for an image path that decoded; `None` otherwise.
    pub image: Option<PreviewImage>,
}

/// Both sides; `None` where the file doesn't exist (an added file's old side, a deleted file's
/// new side).
#[derive(Clone, Debug)]
pub struct BinarySides {
    pub old: Option<BinarySide>,
    pub new: Option<BinarySide>,
}

/// The sizes and previews of the binary file `target` names, decoding images up to
/// `PREVIEW_MAX_BYTES`. Blocking: call from a worker thread.
pub fn binary_sides(workdir: &Path, target: &DiffTarget) -> Result<BinarySides> {
    binary_sides_with(workdir, target, PREVIEW_MAX_BYTES)
}

/// `binary_sides` decoding only files of at most `max_bytes`.
pub(crate) fn binary_sides_with(workdir: &Path, target: &DiffTarget, max_bytes: u64) -> Result<BinarySides> {
    let sides = diff_sides(workdir, target)?;
    let repo = Repository::open(workdir)?;
    Ok(BinarySides { old: side(&repo, workdir, &sides.old, max_bytes)?, new: side(&repo, workdir, &sides.new, max_bytes)? })
}

/// One side's size and preview; `None` for a side with no file.
fn side(repo: &Repository, workdir: &Path, source: &SideSource, max_bytes: u64) -> Result<Option<BinarySide>> {
    Ok(match source {
        SideSource::Empty => None,
        SideSource::Blob { id, path } => {
            let (size, _) = repo.odb()?.read_header(*id)?;
            let size = size as u64;
            let image = if wants_preview(path, size, max_bytes) { preview(repo.find_blob(*id)?.content()) } else { None };
            Some(BinarySide { size, image })
        }
        SideSource::Working(p) => {
            let file = workdir.join(p);
            let Ok(meta) = std::fs::metadata(&file) else { return Ok(None) };
            let size = meta.len();
            let image = if wants_preview(p, size, max_bytes) { std::fs::read(&file).ok().and_then(|b| preview(&b)) } else { None };
            Some(BinarySide { size, image })
        }
    })
}

fn wants_preview(path: &str, size: u64, max_bytes: u64) -> bool {
    preview_image::is_image_path(path) && size <= max_bytes
}

/// `bytes` decoded, unless they are a Git LFS pointer (which has its own summary).
fn preview(bytes: &[u8]) -> Option<PreviewImage> {
    if pointer::parse(bytes).is_some() {
        return None;
    }
    preview_image::decode(bytes)
}

#[cfg(test)]
#[path = "binary_sides_tests.rs"]
mod tests;
