//! The views' edits (`EditMsg`), each set back on the state it was drawn from: an edit for a
//! tab no longer active, a File view version no longer shown, or a rename, terminal session,
//! blame, welcome, dialog, Preferences, palette step, pull request, issue, rebase editor or
//! conflict file that has since gone is dropped, and only the fields the view writes are taken
//! (the matches, the template, the read and save, the posting state stay update's).
//!
//! A dialog's form is the exception: it is taken back whole. That is safe because the shell
//! runs the runner's results before it draws the frame and the frame's edits ahead of its
//! other messages (`edits_first`), so nothing can change the form between the view's copy and
//! its edit. A form kept under a question (`DialogGuard::under`) is never touched.

use crate::effect::Effect;
use crate::msg::EditMsg;
use crate::state::history::FileHistoryView;
use crate::state::{AppState, Modal};

pub(super) fn handle(st: &mut AppState, msg: EditMsg, fx: &mut Vec<Effect>) {
    match msg {
        EditMsg::Commit { tab, fields } => {
            if st.active_tab_id() == Some(tab) {
                st.session.draft.apply(fields);
            }
        }
        EditMsg::Welcome(f) => {
            if let Some(w) = &mut st.onboarding.welcome {
                w.apply(f);
            }
        }
        EditMsg::TerminalRename(r) => {
            if let Some(open) = st.terminal.renaming.as_mut().filter(|open| open.id == r.id) {
                *open = r;
            }
        }
        EditMsg::TerminalSelection { id, selection } => {
            if let Some(s) = st.terminal.sessions.iter_mut().find(|s| s.id == id) {
                s.selection = selection;
            }
        }
        EditMsg::Blame(f) => {
            if let Some(v) = st.session.blame.view.as_mut().filter(|v| v.path == f.path && v.at == f.at) {
                v.apply(*f);
            }
        }
        EditMsg::FileNav { oid, path, nav } => {
            let shown = |v: &&mut FileHistoryView| v.shown().is_some_and(|(o, p, _)| o == oid && p == path);
            if let Some(v) = st.session.history.view.as_mut().filter(shown) {
                v.nav.apply_view(*nav);
            }
        }
        EditMsg::Settings(s) => {
            st.settings = *s;
            super::prefs::settings_edited(st, fx);
        }
        EditMsg::Dialog(f) => {
            if let Some(m) = st.modal.as_mut() {
                m.take_form(f.into_inner());
            }
        }
        EditMsg::ConfirmAck(on) => {
            if let Some(Modal::ConfirmDanger { acked, .. }) = st.modal.as_mut() {
                *acked = on;
            }
        }
        EditMsg::AutoStashAlways(on) => {
            if let Some(Modal::AutoStash(a)) = st.modal.as_mut() {
                a.always = on;
            }
        }
        EditMsg::SshKeygen(f) => {
            if let Some(p) = st.prefs.as_mut() {
                p.ssh.apply_keygen(f);
            }
        }
        EditMsg::Prefs(f) => {
            if let Some(p) = st.prefs.as_mut() {
                p.apply(*f);
            }
        }
        EditMsg::PaletteQuery { generation, query } => {
            if generation != st.palette_gen {
                return;
            }
            // The sub-list is ranked for it after the update (`palette::settle`).
            if let Some(p) = st.palette.as_mut().filter(|p| p.query != query) {
                p.query = query;
                p.selected = 0;
            }
        }
        EditMsg::PullRequest(f) => {
            if let Some(v) = st.session.pull_request.view.as_mut().filter(|v| v.pr.id == f.id) {
                v.apply(*f);
            }
        }
        EditMsg::IssueComment { key, fields } => {
            if let Some(v) = st.session.issue.view.as_mut().filter(|v| v.key == key) {
                v.comment.apply(fields);
            }
        }
        EditMsg::RebaseDraft { editing, draft } => {
            if let Some(p) = st.session.rebase.view.as_mut().filter(|p| p.editing == Some(editing)) {
                p.draft = draft;
            }
        }
        EditMsg::ConflictOutput { path, buffer } => {
            let open = st.session.conflict.view.as_mut().filter(|v| v.path == path);
            if let Some(e) = open.and_then(|v| v.edit.as_mut()) {
                e.buffer = buffer;
            }
        }
    }
}
