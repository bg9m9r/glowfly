//! Dialogs: confirmation, new branch, new tag and annotate tag (drawn by `tag_dialog.rs`), create
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
//! `conflict_check.rs`), a pull request or issue filter (drawn by `forge_filter_dialog.rs`), the
//! trellis:// link dialogs (drawn by `deep_link_dialog.rs`), creating an issue (drawn by
//! `issue/create_dialog.rs`), rewording a commit from the graph (drawn by
//! `rebase/reword_dialog.rs`), the About dialog (drawn by `menu_bar/about.rs`), the release notes
//! (drawn by `release_notes.rs`), undoing a checkout that created a branch (drawn by
//! `undo_dialog.rs`), applying a patch file (drawn by `patch_apply_dialog.rs`), creating a patch
//! from uncommitted changes (drawn by `patch_save_dialog.rs`), finding repositories in project
//! folders (drawn by `repo_finder.rs`), fetching or pulling across repositories (drawn by
//! `repo_sync.rs`), renaming a tab (drawn by `tab_dialog.rs`), choosing a merge's mainline parent
//! (drawn by `mainline_dialog.rs`) and the home Trash offer (drawn by `trash_offer_dialog.rs`).

use crate::msg::Msg;
use crate::state::{AppState, Modal};
use eframe::egui;

pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    // Under a credential prompt, Esc and clicks belong to the prompt.
    let prompting = st.askpass.showing();
    let today = trellis_core::clone_ops::local_date(st.view.now, st.view.utc_offset);
    let sparse_blocked = st.git.blocked(trellis_core::git_version::Feature::SparseCheckout);
    // Worked out before the dialog borrows the state: which results are open or favorites.
    let finder = matches!(st.modal, Some(Modal::FindRepos { .. }))
        .then(|| (crate::state::repo_scan::finder_rows(st), crate::state::repo_scan::project_folders(&st.settings)));
    let repo_scan = &st.repo_scan;
    let repo_sync = st.repo_sync.as_ref();
    let Some(modal) = st.modal.as_mut() else { return };
    let snap = st.snap.as_ref();
    let fs = st.forge.as_ref();
    let idle = st.job.is_none();
    let stage_renames = st.settings.stage_renames;
    let push_tags = st.settings.push_new_tags;
    let connections = &st.settings.connections;
    let keys = &st.keymap;
    let git = &st.git;
    let t = st.settings.lang();
    let repo_filters = snap.map(|s| st.settings.forge_filters_for(&s.real_workdir)).unwrap_or_default();
    let mut close = false;
    let first = msgs.len();
    let resp = egui::Modal::new(egui::Id::new("trellis-modal")).show(ctx, |ui| {
        ui.set_width(460.0);
        match modal {
            Modal::Confirm { title, body, button, .. } => {
                ui.heading(title.as_str());
                ui.add_space(6.0);
                ui.label(body.as_str());
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button(button.as_str()).clicked() {
                        msgs.push(Msg::ConfirmAccepted);
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Modal::NewBranch { start, name, checkout } => {
                // Not under a credential prompt: its fields need the keyboard.
                close = super::branch_dialogs::new_branch(ui, start.as_deref(), name, checkout, snap, !prompting, msgs);
            }
            Modal::NewTag(form) => close = super::tag_dialog::new_tag(ui, form, snap, push_tags, !prompting, msgs),
            Modal::AnnotateTag(form) => close = super::tag_dialog::annotate_tag(ui, form, snap, !prompting, msgs),
            Modal::NewWorktree(form) => close = super::worktree_dialog::new_worktree(ui, form, snap, !prompting, msgs),
            Modal::RemoveWorktree(form) => close = super::worktree_manage_dialog::remove(ui, form, idle, msgs),
            Modal::LockWorktree(form) => close = super::worktree_manage_dialog::lock(ui, form, !prompting, idle, msgs),
            Modal::MoveWorktree(form) => close = super::worktree_move_dialog::move_worktree(ui, form, snap, !prompting, idle, msgs),
            Modal::AddSubmodule(form) => close = super::submodule_dialogs::add(ui, form, snap, !prompting, idle, msgs),
            Modal::EditSubmodule(form) => close = super::submodule_dialogs::edit(ui, form, !prompting, idle, msgs),
            Modal::RemoveSubmodule(form) => close = super::submodule_dialogs::remove(ui, form, idle, msgs),
            Modal::Clone(form) => {
                close = super::clone_dialog::show(ui, form, today, sparse_blocked.as_deref(), connections, !prompting, msgs)
            }
            Modal::Init(form) => close = super::init_dialog::show(ui, form, chrono::Datelike::year(&today), !prompting, msgs),
            Modal::CreatePr { source, target, targets, title, description, draft, items } => {
                let Some(fs) = fs.filter(|f| f.host.is_some()) else {
                    close = true;
                    return;
                };
                let form = super::create_pr_dialog::PrForm { source, target, targets, title, description, draft, items };
                close = super::create_pr_dialog::show(ui, form, fs, snap, idle, msgs);
            }
            Modal::Publish(f) => close = super::publish::form(ui, f, idle, msgs),
            Modal::RenameBranch { old, name } => close = super::branch_dialogs::rename(ui, old, name, snap, !prompting, msgs),
            Modal::RenameStash(form) => close = super::stash_dialog::rename(ui, form, !prompting, idle, msgs),
            Modal::StashPush(form) => close = super::stash_push_dialog::show(ui, form, snap, !prompting, idle, msgs),
            Modal::SetUpstream { branch, remote, name } => close = super::branch_dialogs::upstream(ui, branch, remote, name, snap, msgs),
            Modal::NewFile(form) => close = super::file_dialogs::new_file(ui, form, snap, !prompting, msgs),
            Modal::RenameFile { old, form } => {
                close = super::file_dialogs::rename_file(ui, old, form, snap, stage_renames, !prompting, msgs)
            }
            Modal::Remote(form) => {
                let fork_source = snap.and_then(|s| crate::state::forks::fork_source(s, connections));
                close = super::remotes::dialog::show(ui, form, snap, fork_source, !prompting, idle, msgs);
            }
            Modal::AddFork(p) => close = super::remotes::fork_pick::show(ui, p, snap, connections, !prompting, idle, msgs),
            Modal::CreateFork(f) => close = super::remotes::fork_create::show(ui, f, snap, connections, !prompting, idle, msgs),
            Modal::GitFlowStart(form) => close = super::gitflow_dialog::start(ui, form, snap, !prompting, msgs),
            Modal::GitFlowFinish(form) => close = super::gitflow_finish_dialog::finish(ui, form, snap, idle, !prompting, msgs),
            Modal::Shortcuts { filter } => close = super::shortcuts::sheet(ui, filter, !prompting, keys),
            Modal::Trust(p) => close = super::trust_dialog::show(ui, p, msgs),
            Modal::ConflictReport(r) => {
                let remote = snap.is_some_and(|s| crate::state::conflict_check::is_remote(s, &r.request.target));
                let on_head = snap.is_some_and(|s| crate::state::conflict_check::for_head(r, s));
                close = super::conflict_check::report_modal(ui, r, remote, on_head, idle, msgs);
            }
            Modal::ForgeFilter(form) => close = super::forge_filter_dialog::show(ui, form, fs, &repo_filters, !prompting, msgs),
            Modal::DeepLink(d) => close = super::deep_link_dialog::show(ui, d, msgs),
            Modal::CreateIssue(form) => close = super::issue::create_dialog::show(ui, form, fs, !prompting, msgs),
            Modal::RewordCommit(form) => close = super::rebase::reword_dialog::show(ui, form, snap, !prompting, idle, msgs),
            Modal::About => close = super::menu_bar::about(ui, t, git, crate::state::menu_bar::VERSION, msgs),
            Modal::ReleaseNotes(d) => close = super::release_notes::show(ui, t, d, msgs),
            Modal::UndoCreated(form) => close = super::undo_dialog::created_branch(ui, form, msgs),
            Modal::ApplyPatch(form) => close = super::patch_apply_dialog::show(ui, form, snap, idle, msgs),
            Modal::SavePatch(form) => close = super::patch_save_dialog::show(ui, form, snap, idle, msgs),
            Modal::FindRepos { filter } => {
                let (rows, folders) = finder.as_ref().map(|(r, f)| (&r[..], &f[..])).unwrap_or_default();
                let lists = super::repo_finder::FinderLists { rows, scan: repo_scan, folders };
                close = super::repo_finder::show(ui, t, filter, lists, !prompting, msgs);
            }
            Modal::SyncRepos => close = super::repo_sync::show(ui, repo_sync),
            Modal::RenameTab(form) => close = super::tab_dialog::rename(ui, form, !prompting, msgs),
            Modal::Mainline(form) => close = super::mainline_dialog::show(ui, form, idle, msgs),
            Modal::TrashOffer(o) => close = super::trash_offer_dialog::show(ui, o, msgs),
        }
    });
    if close || (!prompting && resp.should_close()) {
        // The dialog closes before what it asked for runs, so a dialog that opens (the trust
        // prompt holding the job it started) isn't closed with it.
        msgs.insert(first, Msg::CloseModal);
    }
}
