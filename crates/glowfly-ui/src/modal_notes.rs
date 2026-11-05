//! The dialogs whose views only read (`Modal::form_copy` gives none): the confirmations, the
//! branch deletion, the trust prompt, a conflict check's report, an operation's preview, the
//! glowfly:// link dialogs, About and its notices, the release notes, fetching or pulling across
//! repositories and the home Trash offer, drawn from the dialog itself. Three hold a little
//! state the user changes, sent back as an edit when it changed: the danger confirmation's
//! acknowledgement box (`EditMsg::ConfirmAck`), the auto-stash question's Always stash box
//! (`EditMsg::AutoStashAlways`) and Generate SSH key's fields (`EditMsg::SshKeygen`, its
//! passphrases staying in `ViewState::secrets`).

use super::confirm_dialog;
use super::modal_forms::DialogEnv;
use crate::dialog_frame::{Cx, Tone};
use eframe::egui;
use glowfly_model::msg::{EditMsg, Msg};
use glowfly_model::state::{Modal, ViewState};

/// Draws the dialog `shown`; whether it should close.
pub(super) fn draw(ui: &mut egui::Ui, cx: Cx, shown: &Modal, env: &DialogEnv, view: &mut ViewState, msgs: &mut Vec<Msg>) -> bool {
    let (snap, idle) = (env.snap, env.idle);
    match shown {
        Modal::Confirm { title, body, button, .. } => {
            let texts = confirm_dialog::Texts { title, body, button, details: None };
            confirm_dialog::show(ui, cx, texts, Tone::Normal, None, None, msgs)
        }
        Modal::ConfirmDetails { title, body, details, button, .. } => {
            let texts = confirm_dialog::Texts { title, body, button, details: Some(details) };
            confirm_dialog::show(ui, cx, texts, Tone::Normal, None, None, msgs)
        }
        Modal::ConfirmDanger { title, body, button, ack, acked, .. } => {
            let mut ticked = *acked;
            let ack = ack.as_deref().map(|label| (label, &mut ticked));
            let texts = confirm_dialog::Texts { title, body, button, details: None };
            let close = confirm_dialog::show(ui, cx, texts, Tone::Danger, ack, None, msgs);
            if ticked != *acked {
                msgs.push(Msg::Edit(EditMsg::ConfirmAck(ticked)));
            }
            close
        }
        Modal::ConfirmChoice { title, body, button, alt, .. } => {
            let texts = confirm_dialog::Texts { title, body, button, details: None };
            confirm_dialog::show(ui, cx, texts, Tone::Normal, None, Some(alt), msgs)
        }
        Modal::DeleteBranch(form) => crate::branch_delete_dialog::show(ui, cx, form, msgs),
        Modal::Trust(p) => crate::trust_dialog::show(ui, cx, p, msgs),
        Modal::ConflictReport(r) => {
            let remote = glowfly_model::state::conflict_check::is_remote(&r.request.target);
            let on_head = snap.is_some_and(|s| glowfly_model::state::conflict_check::for_head(r, s));
            let merge_into = snap.and_then(|s| s.merge_into.as_deref());
            let gates = crate::conflict_check::Gates { remote, idle, merge_into };
            crate::conflict_check::report_modal(ui, cx, r, gates, on_head, msgs)
        }
        Modal::OpPreview(d) => crate::op_preview_dialog::show(ui, cx, d, &env.preview_env, msgs),
        Modal::DeepLink(d) => crate::deep_link_dialog::show(ui, cx, d, msgs),
        Modal::About => {
            let copy = crate::menu_bar::ThisCopy { version: env.version, build: env.build };
            crate::menu_bar::about(ui, cx, env.git, copy, msgs)
        }
        Modal::Notices => crate::menu_bar::notices(ui, cx, msgs),
        Modal::ReleaseNotes(d) => crate::release_notes::show(ui, cx, d, msgs),
        Modal::SyncRepos => crate::repo_sync::show(ui, cx, env.repo_sync, msgs),
        Modal::TrashOffer(o) => crate::trash_offer_dialog::show(ui, cx, o, msgs),
        Modal::AutoStash(ask) => {
            let mut always = ask.always;
            let close = crate::auto_stash_dialog::show(ui, cx, ask, &mut always, msgs);
            if always != ask.always {
                msgs.push(Msg::Edit(EditMsg::AutoStashAlways(always)));
            }
            close
        }
        Modal::SshKeygen => {
            let Some(p) = env.prefs else { return true };
            let mut f = p.ssh.keygen_fields();
            let close = crate::prefs::ssh::keygen_dialog::show(ui, cx, &p.ssh, &mut f, &mut view.secrets, msgs);
            if f != p.ssh.keygen_fields() {
                msgs.push(Msg::Edit(EditMsg::SshKeygen(f)));
            }
            close
        }
        // Forms: drawn from the frame's copy by `modal_forms.rs`.
        Modal::NewBranch { .. }
        | Modal::NewTag(_)
        | Modal::AnnotateTag(_)
        | Modal::NewWorktree(_)
        | Modal::RemoveWorktree(_)
        | Modal::DeleteRemoteBranch(_)
        | Modal::LockWorktree(_)
        | Modal::MoveWorktree(_)
        | Modal::AddSubmodule(_)
        | Modal::EditSubmodule(_)
        | Modal::RemoveSubmodule(_)
        | Modal::Clone(_)
        | Modal::Init(_)
        | Modal::CreatePr { .. }
        | Modal::Publish(_)
        | Modal::RenameBranch(_)
        | Modal::RenameStash(_)
        | Modal::StashPush(_)
        | Modal::SetUpstream { .. }
        | Modal::NewFile(_)
        | Modal::RenameFile { .. }
        | Modal::Shortcuts { .. }
        | Modal::GitFlowStart(_)
        | Modal::GitFlowFinish(_)
        | Modal::Remote(_)
        | Modal::AddFork(_)
        | Modal::CreateFork(_)
        | Modal::ForgeFilter(_)
        | Modal::CreateIssue(_)
        | Modal::RewordCommit(_)
        | Modal::UndoCreated(_)
        | Modal::ApplyPatch(_)
        | Modal::SavePatch(_)
        | Modal::FindRepos { .. }
        | Modal::RenameTab(_)
        | Modal::RepoGroup(_)
        | Modal::AddToGroup(_)
        | Modal::Mainline(_) => false,
    }
}
