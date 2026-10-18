//! The patch file dialogs, saving a patch, reading one for the Apply patch dialog and the text
//! of a commit's patch for the clipboard (`Effect::Patch`). Applying a patch is an ordinary
//! `RunGit` job.

use crate::msg::Msg;
use crate::patch::{PatchEffect, PatchMsg};
use trellis_core::patch_ops;

const PATCH_FILES: (&str, &[&str]) = ("Patch files", &["patch", "diff", "mbox"]);

/// Runs `effect` to completion (a dialog waits for the user) and reports its result.
pub(super) fn run(effect: PatchEffect) -> Msg {
    let (filter, extensions) = PATCH_FILES;
    Msg::Patch(match effect {
        PatchEffect::PickSave { workdir, what, name } => {
            // No starting folder: the system dialog remembers the last one, and asks before
            // replacing a file.
            let dest = rfd::FileDialog::new().set_title("Save patch").set_file_name(&name).add_filter(filter, extensions).save_file();
            PatchMsg::SaveTo { workdir, what, dest }
        }
        PatchEffect::Export { workdir, what, dest } => {
            let result = patch_ops::save(&workdir, &what, &dest).map_err(|e| e.to_string());
            PatchMsg::Exported { dest, result }
        }
        PatchEffect::Copy { workdir, oids } => PatchMsg::Copied(patch_ops::patch_text(&workdir, &oids).map_err(|e| e.to_string())),
        PatchEffect::Inspect { workdir, path } => {
            let result = patch_ops::inspect(&path).map_err(|e| e.to_string());
            PatchMsg::Inspected { workdir, path, result }
        }
        PatchEffect::PickOpen { workdir } => {
            let path =
                rfd::FileDialog::new().set_title("Apply patch").add_filter(filter, extensions).add_filter("All files", &["*"]).pick_file();
            PatchMsg::ApplyFrom { workdir, path }
        }
    })
}
