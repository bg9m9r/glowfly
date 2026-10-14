//! File actions from file menus: restoring a file as it was in a commit (after a confirmation),
//! opening a working-tree file in its default program or showing it in the file manager, and
//! creating, renaming and deleting working-tree files (in `work_files.rs`).

use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::{AppState, Modal, Selection};
use git2::Oid;
use std::time::Instant;
use trellis_core::repo::RestoredFrom;

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

pub(super) fn handle(st: &mut AppState, msg: Msg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        Msg::RestoreFile { oid, path, deleted, confirmed: false, .. } => ask_restore(st, oid, path, deleted),
        Msg::RestoreFile { oid, path, confirmed: true, overwrite, .. } => {
            // `deleted` only shapes the wording; core decides which version to write.
            if let Some(workdir) = st.workdir() {
                fx.push(Effect::RestoreFile { workdir, oid, path, overwrite });
            }
        }
        Msg::RestoreWouldOverwrite { path, oid, reason } => {
            // Status said the file was clean, but the disk disagrees (an ignored file, edits git
            // skips, or a stale status). Ask again, this time saying what will be lost.
            st.modal = Some(Modal::Confirm {
                title: "Restore file".into(),
                body: format!(
                    "{reason}.\n\nRestoring the version from commit {} replaces it, and its current contents can't be recovered.",
                    short(oid)
                ),
                button: "Restore anyway".into(),
                then: Box::new(Msg::RestoreFile { oid, path, deleted: false, confirmed: true, overwrite: true }),
            });
            super::repo::refresh_status(st, fx);
        }
        Msg::FileRestored { path, oid, result } => {
            match result {
                Ok(from) => {
                    let text = match from {
                        RestoredFrom::Commit => {
                            format!("Restored {path} from {}. The change is unstaged in your working tree.", short(oid))
                        }
                        RestoredFrom::Parent(_) => format!("Restored {path} as it was before {}.", short(oid)),
                    };
                    st.toast(true, text, now);
                    // The file is back, so Open and Show become available without reselecting.
                    if let Some(d) = st.detail.as_mut() {
                        d.absent.remove(&path);
                    }
                    if let Selection::Commits(m) = &mut st.selection
                        && let Some(Ok(c)) = &mut m.changes
                    {
                        c.absent.remove(&path);
                    }
                }
                Err(e) => st.log(false, format!("Could not restore {path}: {e}")),
            }
            super::watch::settled(st, now);
            super::repo::refresh_status(st, fx);
        }
        Msg::NewFileDialog(_)
        | Msg::CreateFile(_)
        | Msg::RenameFileDialog(_)
        | Msg::RenameFile { .. }
        | Msg::SetRenameStages(_)
        | Msg::DeleteFile { .. }
        | Msg::DiscardUntracked(_)
        | Msg::DeleteConfirmed(_)
        | Msg::TrashRefused { .. }
        | Msg::WorkFileDone { .. } => super::work_files::handle(st, msg, now, fx),
        Msg::OpenWorkingFile(p) => {
            if let Some(w) = st.workdir() {
                fx.push(Effect::OpenPath(w.join(&p)));
            }
        }
        Msg::ShowWorkingFile(p) => {
            if let Some(w) = st.workdir() {
                fx.push(Effect::RevealPath(w.join(&p)));
            }
        }
        _ => {}
    }
}

/// Opens the confirmation for restoring `path` from `oid`, warning when status shows uncommitted
/// changes. Status can miss some (ignored files, edits git skips), so without that warning the
/// restore runs with `overwrite: false` and core checks the disk before writing. A conflicted
/// path is refused instead.
fn ask_restore(st: &mut AppState, oid: Oid, path: String, deleted: bool) {
    let Some(snap) = &st.snap else { return };
    if snap.unstaged.iter().any(|f| f.path == path && f.status == '!') {
        st.log(false, format!("Resolve the conflict in {path} before restoring it."));
        return;
    }
    let dirty = snap.unstaged.iter().chain(&snap.staged).any(|f| f.path == path);
    let sha = short(oid);
    let source = if deleted {
        format!("{path} will be written back to your working tree as it was before commit {sha} deleted it.")
    } else {
        format!("{path} will be replaced in your working tree by the version from commit {sha}.")
    };
    let note = if dirty {
        format!("Your uncommitted changes to {path} will be overwritten and cannot be recovered.")
    } else {
        "The change is left unstaged; discarding it brings back the current version.".to_owned()
    };
    st.modal = Some(Modal::Confirm {
        title: "Restore file".into(),
        body: format!("{source}\n\n{note}"),
        button: "Restore".into(),
        then: Box::new(Msg::RestoreFile { oid, path, deleted, confirmed: true, overwrite: dirty }),
    });
}
