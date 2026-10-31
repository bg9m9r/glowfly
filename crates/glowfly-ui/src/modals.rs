//! Dialogs, every one drawn through `dialog_frame` (header with its close button, which sends
//! `Dismiss::CloseButton` like Esc, scrolling body, footer in the fixed order, Enter and the
//! default button's focus, and for less familiar operations a side pane explaining them) in
//! `theme::dialog`: confirmation (drawn by `confirm_dialog.rs`, also in its danger form and with an alternative answer), new branch, new tag and annotate tag (drawn by `tag_dialog.rs`), create
//! worktree (drawn by `worktree_dialog.rs`), remove and lock worktree (drawn by
//! `worktree_manage_dialog.rs`), add and remove submodule (drawn by `submodule_dialogs.rs`), clone
//! (drawn by `clone_dialog.rs` and `clone_service.rs`), init (drawn by `init_dialog.rs`), create
//! pull request (drawn by `create_pr_dialog.rs`), publish (drawn by `publish.rs`), renaming a
//! branch or setting its upstream (drawn by `branch_dialogs.rs`), renaming a stash (drawn by
//! `stash_dialog.rs`), stashing with a message and a choice of what goes in (drawn by
//! `stash_push_dialog.rs`), creating or renaming a working-tree file (drawn by `file_dialogs.rs`),
//! starting a Git Flow branch (drawn by `gitflow_dialog.rs`), finishing one (drawn by
//! `gitflow_finish_dialog.rs`), adding or editing a remote (drawn by `remotes/dialog.rs`), adding a
//! fork as a remote and forking on the hosting service (drawn by `remotes/fork_pick.rs` and
//! `remotes/fork_create.rs`), the keyboard shortcut sheet (drawn by `shortcuts/sheet.rs`), the
//! repository trust prompt (drawn by `trust_dialog.rs`), a conflict check's report (drawn by
//! `conflict_check.rs`), what an operation would do before it runs (drawn by
//! `op_preview_dialog.rs`), a pull request or issue filter (drawn by `forge_filter_dialog.rs`), the
//! glowfly:// link dialogs (drawn by `deep_link_dialog.rs`), creating an issue (drawn by
//! `issue/create_dialog.rs`), rewording a commit from the graph (drawn by
//! `rebase/reword_dialog.rs`), the About dialog and its third-party notices (drawn by `menu_bar/about.rs` and
//! `menu_bar/notices.rs`), the release notes
//! (drawn by `release_notes.rs`), undoing a checkout that created a branch (drawn by
//! `undo_dialog.rs`), applying a patch file (drawn by `patch_apply_dialog.rs`), creating a patch
//! from uncommitted changes (drawn by `patch_save_dialog.rs`), finding repositories in project
//! folders (drawn by `repo_finder.rs`), fetching or pulling across repositories (drawn by
//! `repo_sync.rs`), renaming a tab (drawn by `tab_dialog.rs`), choosing a merge's mainline parent
//! (drawn by `mainline_dialog.rs`), the home Trash offer (drawn by `trash_offer_dialog.rs`),
//! stashing uncommitted changes before an operation (drawn by `auto_stash_dialog.rs`) and
//! generating an SSH key (drawn by `ssh_keygen_dialog.rs`). An edited form kept under a question
//! (state/dialog_guard.rs) is drawn first, beneath the question.

use crate::msg::{DialogMsg, Dismiss, Msg};
use crate::state::{AppState, Modal};
use crate::ui::dialog_frame::{Cx, Tone};
use eframe::egui;

#[path = "confirm_dialog.rs"]
mod confirm_dialog;

pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    // An edited form kept under a question (state/dialog_guard.rs) is drawn first, beneath it:
    // dimmed, without the keyboard, and what it sends is dropped.
    let over = match st.dialog.take_under() {
        Some((form, opened)) => {
            let top = st.modal.replace(form);
            draw(ctx, st, &mut Vec::new(), Layer::Under);
            let form = std::mem::replace(&mut st.modal, top);
            if let Some(form) = form {
                st.dialog.put_under((form, opened));
            }
            true
        }
        None => false,
    };
    draw(ctx, st, msgs, if over { Layer::Over } else { Layer::Only });
}

/// Where the dialog drawn stacks.
#[derive(Clone, Copy, PartialEq)]
enum Layer {
    /// The one dialog.
    Only,
    /// The form kept under a question.
    Under,
    /// The question over a form kept underneath.
    Over,
}

fn draw(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>, layer: Layer) {
    // Under a credential prompt, Esc and clicks belong to the prompt.
    let prompting = st.askpass.showing();
    // Under the "Discard your changes?" question, the form's fields don't take the focus.
    let asking = st.dialog.asking && layer != Layer::Under;
    let keyboard = !prompting && !asking && layer != Layer::Under;
    // The form underneath keeps the usual id, so its fields keep their state when it is back.
    let area = if layer == Layer::Over { "glowfly-modal-over" } else { "glowfly-modal" };
    let today = glowfly_core::clone_ops::local_date(st.view.now, st.view.utc_offset);
    let sparse_blocked = st.git.blocked(glowfly_core::git_version::Feature::SparseCheckout);
    // Worked out before the dialog borrows the state: which results are open or favorites.
    let finder = matches!(st.modal, Some(Modal::FindRepos { .. }))
        .then(|| (crate::state::repo_scan::finder_rows(st), crate::state::repo_scan::project_folders(&st.settings)));
    let repo_scan = &st.repo_scan;
    let repo_sync = st.repo_sync.as_ref();
    // The Generate SSH key dialog's fields live on the SSH page (a borrow apart from the modal's).
    let ssh_page = st.prefs.as_mut().map(|p| &mut p.ssh);
    let preview_env = super::op_preview_dialog::Env {
        snap: st.snap.as_ref(),
        settings: &st.settings,
        now: st.view.now,
        offset: st.view.utc_offset,
        idle: st.job.is_none(),
    };
    let Some(modal) = st.modal.as_mut() else { return };
    let snap = st.snap.as_ref();
    let fs = st.session.forge.as_ref();
    let idle = st.job.is_none();
    let stage_renames = st.settings.stage_renames;
    let push_tags = st.settings.push_new_tags;
    let connections = &st.settings.connections;
    let keys = &st.keymap;
    let git = &st.git;
    let version = st.updates.shown;
    let t = st.settings.lang();
    let repo_filters = snap.map(|s| st.settings.forge_filters_for(&s.real_workdir)).unwrap_or_default();
    let mut close = false;
    let first = msgs.len();
    let resp = super::theme::show_modal(ctx, super::theme::dialog(ctx, egui::Id::new(area)), |ui| {
        // A dialog's key in egui's memory: the first frame of each kind takes the focus.
        let cx = Cx { t, keyboard, key: egui::Id::new("glowfly-dialog").with(std::mem::discriminant(&*modal)) };
        match modal {
            Modal::Confirm { title, body, button, .. } => {
                let texts = confirm_dialog::Texts { title, body, button, details: None };
                close = confirm_dialog::show(ui, cx, texts, Tone::Normal, None, None, msgs)
            }
            Modal::ConfirmDetails { title, body, details, button, .. } => {
                let texts = confirm_dialog::Texts { title, body, button, details: Some(details) };
                close = confirm_dialog::show(ui, cx, texts, Tone::Normal, None, None, msgs)
            }
            Modal::ConfirmDanger { title, body, button, ack, acked, .. } => {
                let ack = ack.as_deref().map(|label| (label, acked));
                let texts = confirm_dialog::Texts { title, body, button, details: None };
                close = confirm_dialog::show(ui, cx, texts, Tone::Danger, ack, None, msgs);
            }
            Modal::ConfirmChoice { title, body, button, alt, .. } => {
                let texts = confirm_dialog::Texts { title, body, button, details: None };
                close = confirm_dialog::show(ui, cx, texts, Tone::Normal, None, Some(alt), msgs)
            }
            Modal::NewBranch { start, name, checkout, track, touched } => {
                // Not under a credential prompt or the discard question: its fields need the keyboard.
                let form = super::branch_dialogs::NewBranchForm { start: start.as_deref(), name, checkout, track: track.as_ref(), touched };
                close = super::branch_dialogs::new_branch(ui, cx, form, snap, msgs);
            }
            Modal::NewTag(form) => close = super::tag_dialog::new_tag(ui, cx, form, snap, push_tags, msgs),
            Modal::AnnotateTag(form) => close = super::tag_dialog::annotate_tag(ui, cx, form, snap, msgs),
            Modal::NewWorktree(form) => close = super::worktree_dialog::new_worktree(ui, cx, form, snap, msgs),
            Modal::DeleteBranch(form) => close = super::branch_delete_dialog::show(ui, cx, form, msgs),
            Modal::DeleteRemoteBranch(form) => close = super::remote_branch_delete_dialog::show(ui, cx, form, msgs),
            Modal::RemoveWorktree(form) => close = super::worktree_manage_dialog::remove(ui, cx, form, idle, msgs),
            Modal::LockWorktree(form) => close = super::worktree_manage_dialog::lock(ui, cx, form, idle, msgs),
            Modal::MoveWorktree(form) => close = super::worktree_move_dialog::move_worktree(ui, cx, form, snap, idle, msgs),
            Modal::AddSubmodule(form) => close = super::submodule_dialogs::add(ui, cx, form, snap, idle, msgs),
            Modal::EditSubmodule(form) => close = super::submodule_dialogs::edit(ui, cx, form, idle, msgs),
            Modal::RemoveSubmodule(form) => close = super::submodule_dialogs::remove(ui, cx, form, idle, msgs),
            Modal::Clone(form) => close = super::clone_dialog::show(ui, cx, form, today, sparse_blocked.as_deref(), connections, msgs),
            Modal::Init(form) => close = super::init_dialog::show(ui, cx, form, chrono::Datelike::year(&today), msgs),
            Modal::CreatePr { source, target, targets, title, description, draft, items, repos } => {
                let Some(fs) = fs.filter(|f| f.host.is_some()) else {
                    close = true;
                    return;
                };
                let form = super::create_pr_dialog::PrForm { source, target, targets, title, description, draft, items, repos };
                close = super::create_pr_dialog::show(ui, cx, form, fs, snap, idle, msgs);
            }
            Modal::Publish(f) => close = super::publish::form(ui, cx, f, idle, msgs),
            Modal::RenameBranch(form) => close = super::branch_dialogs::rename(ui, cx, form, snap, msgs),
            Modal::RenameStash(form) => close = super::stash_dialog::rename(ui, cx, form, idle, msgs),
            Modal::StashPush(form) => close = super::stash_push_dialog::show(ui, cx, form, snap, idle, msgs),
            Modal::SetUpstream { branch, remote, name, touched } => {
                close = super::branch_dialogs::upstream(ui, cx, branch, (remote, name, touched), snap, msgs)
            }
            Modal::NewFile(form) => close = super::file_dialogs::new_file(ui, cx, form, snap, msgs),
            Modal::RenameFile { old, form } => close = super::file_dialogs::rename_file(ui, cx, old, form, snap, stage_renames, msgs),
            Modal::Remote(form) => {
                let fork_source = snap.and_then(|s| crate::state::forks::fork_source(s, connections));
                close = super::remotes::dialog::show(ui, cx, form, snap, fork_source, idle, msgs);
            }
            Modal::AddFork(p) => close = super::remotes::fork_pick::show(ui, cx, p, snap, connections, idle, msgs),
            Modal::CreateFork(f) => close = super::remotes::fork_create::show(ui, cx, f, snap, connections, idle, msgs),
            Modal::GitFlowStart(form) => close = super::gitflow_dialog::start(ui, cx, form, snap, msgs),
            Modal::GitFlowFinish(form) => close = super::gitflow_finish_dialog::finish(ui, cx, form, snap, idle, msgs),
            Modal::Shortcuts { filter } => close = super::shortcuts::sheet(ui, cx, filter, keys),
            Modal::Trust(p) => close = super::trust_dialog::show(ui, cx, p, msgs),
            Modal::ConflictReport(r) => {
                let remote = snap.is_some_and(|s| crate::state::conflict_check::is_remote(s, &r.request.target));
                let on_head = snap.is_some_and(|s| crate::state::conflict_check::for_head(r, s));
                close = super::conflict_check::report_modal(ui, cx, r, remote, on_head, idle, msgs);
            }
            Modal::OpPreview(d) => close = super::op_preview_dialog::show(ui, cx, d, &preview_env, msgs),
            Modal::ForgeFilter(form) => close = super::forge_filter_dialog::show(ui, cx, form, fs, &repo_filters, msgs),
            Modal::DeepLink(d) => close = super::deep_link_dialog::show(ui, cx, d, msgs),
            Modal::CreateIssue(form) => close = super::issue::create_dialog::show(ui, cx, form, fs, msgs),
            Modal::RewordCommit(form) => close = super::rebase::reword_dialog::show(ui, cx, form, snap, idle, msgs),
            Modal::About => close = super::menu_bar::about(ui, cx, git, version, msgs),
            Modal::Notices => close = super::menu_bar::notices(ui, cx, msgs),
            Modal::ReleaseNotes(d) => close = super::release_notes::show(ui, cx, d, msgs),
            Modal::UndoCreated(form) => close = super::undo_dialog::created_branch(ui, cx, form, msgs),
            Modal::ApplyPatch(form) => close = super::patch_apply_dialog::show(ui, cx, form, snap, idle, msgs),
            Modal::SavePatch(form) => close = super::patch_save_dialog::show(ui, cx, form, snap, idle, msgs),
            Modal::FindRepos { filter } => {
                let (rows, folders) = finder.as_ref().map(|(r, f)| (&r[..], &f[..])).unwrap_or_default();
                let lists = super::repo_finder::FinderLists { rows, scan: repo_scan, folders };
                close = super::repo_finder::show(ui, cx, filter, lists, msgs);
            }
            Modal::SyncRepos => close = super::repo_sync::show(ui, cx, repo_sync, msgs),
            Modal::RenameTab(form) => close = super::tab_dialog::rename(ui, cx, form, msgs),
            Modal::RepoGroup(form) => close = super::repo_group_dialog::show(ui, cx, form, msgs),
            Modal::Mainline(form) => close = super::mainline_dialog::show(ui, cx, form, idle, msgs),
            Modal::TrashOffer(o) => close = super::trash_offer_dialog::show(ui, cx, o, msgs),
            Modal::AutoStash(ask) => close = super::auto_stash_dialog::show(ui, cx, ask, msgs),
            Modal::SshKeygen => {
                close = match ssh_page {
                    Some(p) => super::ssh_keygen_dialog::show(ui, cx, p, msgs),
                    None => true,
                }
            }
        }
    });
    // Taken every frame, so a click that closed this dialog never reaches the next one.
    let closed = super::dialog_frame::take_closed(&resp);
    if layer == Layer::Under {
        return;
    }
    if close {
        // The dialog closes before what it asked for runs, so a dialog that opens (the trust
        // prompt holding the job it started) isn't closed with it.
        msgs.insert(first, Msg::CloseModal);
    } else if asking {
        // Drawn after the form, so it stacks on top and the form beneath takes no input.
        super::dialog_guard::discard_question(ctx, t, msgs);
    } else if !prompting && let Some(how) = if closed { Some(Dismiss::CloseButton) } else { super::dialog_guard::dismissal(&resp) } {
        msgs.insert(first, Msg::Dialog(DialogMsg::Dismiss(how)));
    }
}
