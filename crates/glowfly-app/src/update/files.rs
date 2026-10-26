//! File actions from file menus: restoring a file as it was in a commit (after a confirmation,
//! recorded for undo with the working copy it replaced), opening a working-tree file in its
//! default program or showing it in the file manager, and creating, renaming and deleting
//! working-tree files (in `work_files.rs`).

use crate::effect::Effect;
use crate::effect::OsEffect;
use crate::msg::FilesMsg;
use crate::msg::Msg;
use crate::state::trust::Standing;
use crate::state::{AppState, Modal, Selection};
use git2::Oid;
use glowfly_core::opens::{Os, opens_as_program};
use glowfly_core::repo::RestoredFrom;
use std::path::Path;
use std::time::Instant;

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// " (Ctrl+Z)" after "Undo", or nothing when Undo has no key.
fn undo_hint(st: &AppState) -> String {
    st.keymap.paren(crate::keymap::BindingId::Undo)
}

pub(super) fn handle(st: &mut AppState, msg: FilesMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        FilesMsg::RestoreFile { oid, path, deleted, confirmed: false, .. } => ask_restore(st, oid, path, deleted),
        FilesMsg::RestoreFile { oid, path, confirmed: true, overwrite, .. } => {
            // `deleted` only shapes the wording; core decides which version to write.
            if let Some(workdir) = st.workdir() {
                fx.push(Effect::RestoreFile { workdir, oid, path, overwrite });
            }
        }
        FilesMsg::RestoreWouldOverwrite { path, oid, reason } => {
            // Status said the file was clean, but the disk disagrees (an ignored file, edits git
            // skips, or a stale status). Ask again, this time saying what will be lost.
            st.modal = Some(Modal::danger(
                "Restore file",
                format!(
                    "{reason}.\n\nRestoring the version from commit {} replaces it; Undo{} puts its current contents back.",
                    short(oid),
                    undo_hint(st)
                ),
                "Restore anyway",
                Msg::Files(FilesMsg::RestoreFile { oid, path, deleted: false, confirmed: true, overwrite: true }),
            ));
            super::repo::refresh_status(st, fx);
        }
        FilesMsg::FileRestored { path, oid, result, undo } => {
            let undoable = undo.is_some();
            if let Some(a) = undo {
                st.undo.record(a);
            }
            match result {
                Ok(from) => {
                    let text = match from {
                        RestoredFrom::Commit => {
                            format!("Restored {path} from {}. The change is unstaged in your working tree.", short(oid))
                        }
                        RestoredFrom::Parent(_) => format!("Restored {path} as it was before {}.", short(oid)),
                    };
                    st.toast(true, text, now);
                    // The file is back, so Open and Show become available without reselecting,
                    // in the commit detail, a comparison and the file history.
                    if let Some(d) = st.detail.as_mut() {
                        d.absent.remove(&path);
                    }
                    if let Some(h) = st.session.history.view.as_mut() {
                        h.absent.remove(&path);
                        if h.path == path {
                            h.on_disk = true;
                        }
                    }
                    if let Selection::Commits(m) = &mut st.selection {
                        if let Some(Ok(c)) = &mut m.changes {
                            c.absent.remove(&path);
                        }
                        for c in m.stacked.iter_mut().flatten().flatten() {
                            c.absent.remove(&path);
                        }
                    }
                }
                Err(e) => {
                    let key = st.keymap.hint(crate::keymap::BindingId::Undo).map(str::to_owned);
                    st.log(false, crate::msg::restore_failed(&path, &e, undoable, key.as_deref()));
                }
            }
            super::watch::settled(st, now);
            super::repo::refresh_status(st, fx);
        }
        FilesMsg::OpenWorkingFile(p) => match opens_as_program(Path::new(&p), None, Os::current(), None) {
            Some(kind) => ask_open(st, p, kind),
            None => open(st, p, false, fx),
        },
        FilesMsg::OpenConfirmed(p) => open(st, p, true, fx),
        FilesMsg::OpenNeedsConfirm { workdir, path, kind } => {
            if st.workdir().is_some_and(|w| w == workdir) {
                ask_open(st, path, kind);
            } else {
                st.log(false, crate::msg::not_opened(&path, kind));
            }
        }
        FilesMsg::ShowWorkingFile(p) => {
            if let Some(w) = st.workdir() {
                fx.push(Effect::Os(OsEffect::RevealPath(w.join(&p))));
            }
        }
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
        format!("Your uncommitted changes to {path} will be overwritten. Undo{} puts them back.", undo_hint(st))
    } else {
        "The change is left unstaged; discarding it brings back the current version.".to_owned()
    };
    let then = Msg::Files(FilesMsg::RestoreFile { oid, path, deleted, confirmed: true, overwrite: dirty });
    let body = format!("{source}\n\n{note}");
    // Overwriting uncommitted changes is a danger step; a clean restore is not.
    st.modal = Some(if dirty {
        Modal::danger("Restore file", body, "Restore", then)
    } else {
        Modal::Confirm { title: "Restore file".into(), body, button: "Restore".into(), then: Box::new(then) }
    });
}

fn open(st: &AppState, path: String, confirmed: bool, fx: &mut Vec<Effect>) {
    if let Some(workdir) = st.workdir() {
        fx.push(Effect::Os(OsEffect::OpenPath { workdir, path, confirmed }));
    }
}

/// Opening `path` would run it (it is `kind`): files from a repository carry no download
/// warning, so the system wouldn't ask. Ask here, unless the user chose not to trust this
/// repository, which refuses it outright. Either way the editor is offered for reading it.
fn ask_open(st: &mut AppState, path: String, kind: &'static str) {
    let editor = crate::state::tools::OPEN_IN_EDITOR;
    match st.trust_standing() {
        Some(Standing::NotTrusted) => st.log(
            false,
            format!(
                "Did not open {path}: it is {kind}, and opening it would run it. You chose not to trust this repository. To read it, use {editor}."
            ),
        ),
        standing => {
            let trust = match standing {
                Some(Standing::Trusted) => "",
                Some(Standing::Changed) => " This repository gained settings or hooks that run programs since you trusted it.",
                _ => " You haven't said whether you trust this repository.",
            };
            st.modal = Some(Modal::Confirm {
                title: "Run this file?".into(),
                body: format!(
                    "{path} is {kind}. Opening it with its default app runs it, with your permissions. Files from a repository get no download warning from your system, so only run it if you trust where it came from.{trust}\n\nTo read or edit it instead, use {editor}."
                ),
                button: "Run".into(),
                then: Box::new(Msg::Files(FilesMsg::OpenConfirmed(path))),
            });
        }
    }
}
