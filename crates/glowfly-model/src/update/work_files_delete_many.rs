//! Deleting several picked files of Unstaged or Staged Files at once: one danger confirmation
//! naming the count, where the files go (the Recycle Bin / Trash, or for good with a kept copy)
//! and how many leave the staging area too; the run it agreed to; and the result: one undo
//! entry for every file that went, a toast, or a log line naming the file it stopped at. A pick
//! Glowfly can't act on safely, or one gone from the working tree, blocks the whole delete; a
//! single file left takes the ordinary single-file flow. The trash-or-permanent choice is fixed
//! when the confirmation opens and travels inside it.

use crate::effect::{DeleteEach, DeleteMode, Effect, FilesEffect};
use crate::keymap::BindingId;
use crate::msg::{Msg, WorkFileMsg};
use crate::state::files::{TRASH, delete_gone, delete_unsafe, unstages_on_delete};
use crate::state::path_hazards::hazard_of;
use crate::state::wip_lists::files;
use crate::state::{AppState, Modal};
use glowfly_core::error::WriteFailed;
use glowfly_core::i18n::Lang;
use glowfly_core::undo::Action;
use std::time::Instant;

/// Opens the confirmation for deleting `paths`, the ones still in Unstaged or Staged Files (in
/// the order given, which is the list's), or refuses when one of them has a hazard or is gone
/// from the working tree.
pub(super) fn ask(st: &mut AppState, paths: Vec<String>, now: Instant, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    let Some(snap) = &st.snap else { return };
    let (unstaged, staged) = (files(snap, false), files(snap, true));
    let listed = |p: &String| unstaged.iter().chain(&staged).any(|f| f.path == *p);
    let paths: Vec<String> = paths.into_iter().filter(listed).collect();
    if let Some((path, h)) = paths.iter().find_map(|p| hazard_of(st, p).map(|h| (p, h))) {
        return st.log(false, delete_unsafe(t, path, &h.reason(t)));
    }
    if let Some(path) = paths.iter().find(|p| snap.missing_from_workdir(p)) {
        return st.log(false, delete_gone(t, path));
    }
    match paths.as_slice() {
        [] => {}
        [one] => {
            let msg = Msg::WorkFile(WorkFileMsg::DeleteFile { path: one.clone(), confirmed: false });
            fx.extend(super::update_sealed(st, msg, now));
        }
        _ => {
            let mode = DeleteMode::for_settings(&st.settings);
            let files: Vec<DeleteEach> = paths
                .iter()
                .map(|p| {
                    let unstage = unstages_on_delete(snap, p);
                    // A staged-only tracked file's content is in git; the others' only on disk.
                    let promise = unstage || unstaged.iter().any(|f| f.path == *p);
                    DeleteEach { path: p.clone(), unstage, promise }
                })
                .collect();
            let hint = st.keymap.paren(BindingId::Undo);
            st.modal = Some(confirmation(t, files, mode, &hint));
        }
    }
}

/// The danger confirmation for deleting `files` the way `mode` says; `hint` is the Undo key.
fn confirmation(t: Lang, files: Vec<DeleteEach>, mode: DeleteMode, hint: &str) -> Modal {
    let n = files.len();
    let trash = mode == DeleteMode::Trash;
    let mut body = if trash {
        t.trn("details.wip.delete_files.body_trash", n, &[("trash", &TRASH), ("hint", &hint)])
    } else {
        t.trn("details.wip.delete_files.body_permanent", n, &[("hint", &hint)])
    };
    let unstages = files.iter().filter(|f| f.unstage).count();
    if unstages > 0 {
        body.push(' ');
        body.push_str(&t.trn("details.wip.delete_files.unstages", unstages, &[]));
    }
    let title = t.trn("details.wip.delete_files.title", n, &[]);
    let button = if trash {
        t.trn("details.wip.delete_files.button_trash", n, &[("trash", &TRASH)])
    } else {
        t.trn("details.wip.delete_files.button_permanent", n, &[])
    };
    Modal::danger(title, body, button, Msg::WorkFile(WorkFileMsg::DeletePathsConfirmed { files, mode }))
}

/// Runs the delete the confirmation agreed to; the picks end here.
pub(super) fn confirmed(st: &mut AppState, files: Vec<DeleteEach>, mode: DeleteMode, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    st.cues.wip_picks = None;
    let _ = super::file_trust::push(st, FilesEffect::DeletePaths { workdir, files, mode }, fx);
}

/// The runner deleted `done` files (`no_undo` of them with nothing Undo can put back), then
/// stopped at `failed` (if any): records the one undo entry, says how it went and reads the
/// status again.
pub(super) fn done(
    st: &mut AppState,
    mode: DeleteMode,
    (done, no_undo): (usize, usize),
    failed: Option<(String, WriteFailed)>,
    undo: Option<Action>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    if let Some(a) = undo {
        st.session.undo.record(a);
    }
    let t = st.settings.lang();
    let hint = st.keymap.paren(BindingId::Undo);
    match failed {
        None => st.toast(true, done_text(t, mode, done, no_undo, &hint), now),
        Some((path, e)) => {
            let why = crate::state::error_words::typed::write(t, &e);
            st.log(false, stopped_text(t, done, &path, &why, &hint));
        }
    }
    super::watch::settled(st, now);
    super::repo::refresh_status(st, fx);
}

/// The toast after `done` files went the way `mode` says, `no_undo` of them beyond Undo's reach.
fn done_text(t: Lang, mode: DeleteMode, done: usize, no_undo: usize, hint: &str) -> String {
    let trash = matches!(mode, DeleteMode::Trash | DeleteMode::HomeTrash);
    let mut text = if trash {
        t.trn("details.wip.delete_files.done_trash", done, &[("trash", &TRASH), ("hint", &hint)])
    } else {
        t.trn("details.wip.delete_files.done_permanent", done, &[("hint", &hint)])
    };
    if no_undo > 0 {
        let note = if trash {
            t.trn("details.wip.delete_files.no_undo", no_undo, &[("trash", &TRASH)])
        } else {
            t.trn("details.wip.delete_files.no_undo_permanent", no_undo, &[])
        };
        text.push(' ');
        text.push_str(&note);
    }
    text
}

/// The log line for a multi-file delete that deleted `done` files and then stopped at `path`
/// (`why`); `hint` is the Undo key, or "" for a tab that isn't active.
pub(super) fn stopped_text(t: Lang, done: usize, path: &str, why: &str, hint: &str) -> String {
    if done == 0 {
        return t.trf("details.wip.delete_files.failed", &[("path", &path), ("why", &why)]);
    }
    t.trn("details.wip.delete_files.partial", done, &[("path", &path), ("why", &why), ("hint", &hint)])
}
