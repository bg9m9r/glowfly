//! Reverting a target's whole change to one file in the working tree: its content goes back
//! to the old side (renames and mode changes stay). An unchanged working file is written
//! directly; one edited since is merged (`revert_merge`). An added file is removed, a deleted
//! one recreated.

use super::lines::{Patched, replace_patch};
use super::revert::{ALREADY_REVERTED, Ctx, Reverted, deleted_exists, missing, raw};
use super::revert_merge::merge_into_workdir;
use crate::error::{Error, Result};
use crate::persist::{self, WriteOptions};
use git2::{Delta, DiffDelta, ErrorCode, Patch};

/// Reverts the whole of `delta` (whose text patch is `patch`; `None` for a binary file).
pub(super) fn file_part(ctx: &Ctx, delta: &DiffDelta<'_>, patch: Option<Patch<'_>>) -> Result<Reverted> {
    let repo = ctx.repo;
    let (old, new) = (delta.old_file(), delta.new_file());
    let binary = patch.as_ref().is_none_or(|p| p.delta().flags().is_binary());
    let refuse_binary = || Error::Unsupported("binary files can only be reverted by Restore".into());
    // The working file in repository form, to tell whether it changed since the commit.
    let ours = if ctx.exists() { Some(repo.blob_path(ctx.file)?) } else { None };
    match delta.status() {
        Delta::Deleted => {
            if ours.is_some() {
                return Err(deleted_exists());
            }
            if binary {
                return Err(refuse_binary());
            }
            let blob = repo.find_blob(old.id())?;
            let mode = raw(old.mode());
            match replace_patch(ctx.work, Patched::Create(mode), b"", blob.content()) {
                Some(bytes) => ctx.apply(&bytes)?,
                // An empty file: no patch can create it.
                None => persist::write_file(
                    ctx.file,
                    b"",
                    WriteOptions { refuse_links: true, executable: Some(mode == 0o100755), ..Default::default() },
                )?,
            }
            Ok(Reverted::Applied)
        }
        Delta::Added => {
            let ours = ours.ok_or_else(missing)?;
            if ours == new.id() {
                persist::remove_file(ctx.file)?;
                return Ok(Reverted::Removed);
            }
            if binary {
                return Err(refuse_binary());
            }
            merge_into_workdir(ctx, ctx.mode(delta)?, new.id(), b"", true)
        }
        _ => {
            if old.id() == new.id() || (!binary && patch.as_ref().is_some_and(|p| p.num_hunks() == 0)) {
                return Err(Error::Other("only the file mode changed; there is no content to revert".into()));
            }
            let ours = ours.ok_or_else(missing)?;
            if binary {
                return Err(refuse_binary());
            }
            let mode = ctx.mode(delta)?;
            let old_blob = repo.find_blob(old.id())?;
            if ours != new.id() {
                return merge_into_workdir(ctx, mode, new.id(), old_blob.content(), false);
            }
            let new_blob = repo.find_blob(new.id())?;
            let bytes = replace_patch(ctx.work, Patched::Existing(mode), new_blob.content(), old_blob.content())
                .ok_or_else(|| Error::Other(ALREADY_REVERTED.into()))?;
            match ctx.apply(&bytes) {
                Err(e) if e.code() == ErrorCode::ApplyFail => {
                    Err(Error::Other("the file changed while reverting; nothing was changed".into()))
                }
                r => {
                    r?;
                    Ok(Reverted::Applied)
                }
            }
        }
    }
}
