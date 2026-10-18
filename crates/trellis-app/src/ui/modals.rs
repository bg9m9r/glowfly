//! Dialogs: confirmation, new branch, new tag and annotate tag (drawn by `tag_dialog.rs`), create
//! worktree (drawn by `worktree_dialog.rs`), remove and lock worktree (drawn by
//! `worktree_manage_dialog.rs`), add and remove submodule (drawn by `submodule_dialogs.rs`),
//! clone (drawn by `clone_dialog.rs` and `clone_service.rs`), init (drawn by
//! `init_dialog.rs`), create pull request, publish (drawn by `publish.rs`), renaming a
//! branch or setting its upstream (drawn by `branch_dialogs.rs`), renaming a stash (drawn by
//! `stash_dialog.rs`), stashing with a message and a choice of what goes in (drawn by
//! `stash_push_dialog.rs`), creating or renaming a working-tree file (drawn by
//! `file_dialogs.rs`), starting a Git Flow branch (drawn by `gitflow_dialog.rs`), finishing one
//! (drawn by `gitflow_finish_dialog.rs`), adding or editing a remote (drawn by
//! `remotes/dialog.rs`), adding a fork as a remote and forking on the hosting service (drawn by
//! `remotes/fork_pick.rs` and `remotes/fork_create.rs`), the keyboard shortcut sheet (drawn by
//! `shortcuts/sheet.rs`), the repository trust prompt (drawn by `trust_dialog.rs`), a conflict
//! check's report (drawn by `conflict_check.rs`), a pull request or issue filter (drawn by
//! `forge_filter_dialog.rs`), the trellis:// link dialogs (drawn by `deep_link_dialog.rs`),
//! creating an issue (drawn by `issue/create_dialog.rs`), rewording a commit from the graph
//! (drawn by `rebase/reword_dialog.rs`), the About dialog (drawn by `menu_bar/about.rs`), the
//! release notes (drawn by `release_notes.rs`), undoing a checkout that created a branch
//! (drawn by `undo_dialog.rs`), applying a patch file (drawn by `patch_apply_dialog.rs`) and
//! creating a patch from uncommitted changes (drawn by `patch_save_dialog.rs`).

use super::common::{capitalize, issue_chip, parse_keys};
use super::theme::color;
use crate::msg::ForgeMsg;
use crate::msg::Msg;
use crate::state::branches;
use crate::state::push_pr::{self, HostBranch};
use crate::state::{AppState, ForgeState, Modal};
use eframe::egui::{self, RichText};
use trellis_core::forge::{self, NewPullRequest, RefStyle};
use trellis_core::repo::Snapshot;

pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    // Under a credential prompt, Esc and clicks belong to the prompt.
    let prompting = st.askpass.showing();
    let today = trellis_core::clone_ops::local_date(st.view.now, st.view.utc_offset);
    let sparse_blocked = st.git.blocked(trellis_core::git_version::Feature::SparseCheckout);
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
                if super::branch_dialogs::new_branch(ui, start.as_deref(), name, checkout, snap, !prompting, msgs) {
                    close = true;
                }
            }
            Modal::NewTag(form) => {
                if super::tag_dialog::new_tag(ui, form, snap, push_tags, !prompting, msgs) {
                    close = true;
                }
            }
            Modal::AnnotateTag(form) => {
                if super::tag_dialog::annotate_tag(ui, form, snap, !prompting, msgs) {
                    close = true;
                }
            }
            Modal::NewWorktree(form) => {
                if super::worktree_dialog::new_worktree(ui, form, snap, !prompting, msgs) {
                    close = true;
                }
            }
            Modal::RemoveWorktree(form) => {
                if super::worktree_manage_dialog::remove(ui, form, idle, msgs) {
                    close = true;
                }
            }
            Modal::LockWorktree(form) => {
                if super::worktree_manage_dialog::lock(ui, form, !prompting, idle, msgs) {
                    close = true;
                }
            }
            Modal::MoveWorktree(form) => {
                if super::worktree_move_dialog::move_worktree(ui, form, snap, !prompting, idle, msgs) {
                    close = true;
                }
            }
            Modal::AddSubmodule(form) => {
                if super::submodule_dialogs::add(ui, form, snap, !prompting, idle, msgs) {
                    close = true;
                }
            }
            Modal::EditSubmodule(form) => {
                if super::submodule_dialogs::edit(ui, form, !prompting, idle, msgs) {
                    close = true;
                }
            }
            Modal::RemoveSubmodule(form) => {
                if super::submodule_dialogs::remove(ui, form, idle, msgs) {
                    close = true;
                }
            }
            Modal::Clone(form) => {
                if super::clone_dialog::show(ui, form, today, sparse_blocked.as_deref(), connections, !prompting, msgs) {
                    close = true;
                }
            }
            Modal::Init(form) => {
                if super::init_dialog::show(ui, form, chrono::Datelike::year(&today), !prompting, msgs) {
                    close = true;
                }
            }
            Modal::CreatePr { source, target, targets, title, description, draft, items } => {
                let Some(fs) = fs.filter(|f| f.host.is_some()) else {
                    close = true;
                    return;
                };
                let form = PrForm { source, target, targets, title, description, draft, items };
                if create_pr(ui, form, fs, snap, idle, msgs) {
                    close = true;
                }
            }
            Modal::Publish(f) => {
                if super::publish::form(ui, f, idle, msgs) {
                    close = true;
                }
            }
            Modal::RenameBranch { old, name } => {
                if super::branch_dialogs::rename(ui, old, name, snap, !prompting, msgs) {
                    close = true;
                }
            }
            Modal::RenameStash(form) => {
                if super::stash_dialog::rename(ui, form, !prompting, idle, msgs) {
                    close = true;
                }
            }
            Modal::StashPush(form) => {
                if super::stash_push_dialog::show(ui, form, snap, !prompting, idle, msgs) {
                    close = true;
                }
            }
            Modal::SetUpstream { branch, remote, name } => {
                if super::branch_dialogs::upstream(ui, branch, remote, name, snap, msgs) {
                    close = true;
                }
            }
            Modal::NewFile(form) => {
                if super::file_dialogs::new_file(ui, form, snap, !prompting, msgs) {
                    close = true;
                }
            }
            Modal::RenameFile { old, form } => {
                if super::file_dialogs::rename_file(ui, old, form, snap, stage_renames, !prompting, msgs) {
                    close = true;
                }
            }
            Modal::Remote(form) => {
                let fork_source = snap.and_then(|s| crate::state::forks::fork_source(s, connections));
                if super::remotes::dialog::show(ui, form, snap, fork_source, !prompting, idle, msgs) {
                    close = true;
                }
            }
            Modal::AddFork(p) => {
                if super::remotes::fork_pick::show(ui, p, snap, connections, !prompting, idle, msgs) {
                    close = true;
                }
            }
            Modal::CreateFork(f) => {
                if super::remotes::fork_create::show(ui, f, snap, connections, !prompting, idle, msgs) {
                    close = true;
                }
            }
            Modal::GitFlowStart(form) => {
                if super::gitflow_dialog::start(ui, form, snap, !prompting, msgs) {
                    close = true;
                }
            }
            Modal::GitFlowFinish(form) => {
                if super::gitflow_finish_dialog::finish(ui, form, snap, idle, !prompting, msgs) {
                    close = true;
                }
            }
            Modal::Shortcuts { filter } => {
                if super::shortcuts::sheet(ui, filter, !prompting, keys) {
                    close = true;
                }
            }
            Modal::Trust(p) => {
                if super::trust_dialog::show(ui, p, msgs) {
                    close = true;
                }
            }
            Modal::ConflictReport(r) => {
                let remote = snap.is_some_and(|s| crate::state::conflict_check::is_remote(s, &r.request.target));
                let on_head = snap.is_some_and(|s| crate::state::conflict_check::for_head(r, s));
                if super::conflict_check::report_modal(ui, r, remote, on_head, idle, msgs) {
                    close = true;
                }
            }
            Modal::ForgeFilter(form) => {
                if super::forge_filter_dialog::show(ui, form, fs, &repo_filters, !prompting, msgs) {
                    close = true;
                }
            }
            Modal::DeepLink(d) => {
                if super::deep_link_dialog::show(ui, d, msgs) {
                    close = true;
                }
            }
            Modal::CreateIssue(form) => {
                if super::issue::create_dialog::show(ui, form, fs, !prompting, msgs) {
                    close = true;
                }
            }
            Modal::RewordCommit(form) => {
                if super::rebase::reword_dialog::show(ui, form, snap, !prompting, idle, msgs) {
                    close = true;
                }
            }
            Modal::About => {
                if super::menu_bar::about(ui, t, git, crate::state::menu_bar::VERSION, msgs) {
                    close = true;
                }
            }
            Modal::ReleaseNotes(d) => {
                if super::release_notes::show(ui, t, d, msgs) {
                    close = true;
                }
            }
            Modal::UndoCreated(form) => {
                if super::undo_dialog::created_branch(ui, form, msgs) {
                    close = true;
                }
            }
            Modal::ApplyPatch(form) => {
                if super::patch_apply_dialog::show(ui, form, snap, idle, msgs) {
                    close = true;
                }
            }
            Modal::SavePatch(form) => {
                if super::patch_save_dialog::show(ui, form, snap, idle, msgs) {
                    close = true;
                }
            }
        }
    });
    if close || (!prompting && resp.should_close()) {
        // The dialog closes before what it asked for runs, so a dialog that opens (the trust
        // prompt holding the job it started) isn't closed with it.
        msgs.insert(first, Msg::CloseModal);
    }
}

struct PrForm<'a> {
    source: &'a mut String,
    target: &'a mut String,
    targets: &'a mut Vec<String>,
    title: &'a mut String,
    description: &'a mut String,
    draft: &'a mut bool,
    items: &'a mut String,
}

/// What the create dialog says about `source` not being (all) on the code host's remote, and
/// the local branch it offers to push, if any.
fn push_note(snap: &Snapshot, fs: &ForgeState, source: &str, pushed: bool) -> Option<(String, Option<String>)> {
    let remote = &fs.remote;
    match push_pr::dialog_push(snap, fs, source) {
        Some((b, HostBranch::Missing)) => Some((format!("{source} isn't on {remote} yet."), Some(b.name.clone()))),
        Some((b, HostBranch::Ahead(n))) => {
            Some((format!("{} has {} that aren't on {remote}.", b.name, branches::commits(n)), Some(b.name.clone())))
        }
        Some((_, HostBranch::Blocked(why))) => Some((why, None)),
        Some((_, HostBranch::Pushed)) => None,
        None if !pushed => Some((format!("{source} isn't on {remote}. Fetch, or push it from where it lives."), None)),
        None => None,
    }
}

/// The create pull/merge request form; returns true when the dialog should close.
fn create_pr(ui: &mut egui::Ui, f: PrForm, fs: &ForgeState, snap: Option<&Snapshot>, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    let Some(host) = fs.host.as_ref() else { return true };
    let mut close = false;
    let noun = fs.pr_noun();
    ui.heading(format!("Create {noun}"));
    ui.label(RichText::new(format!("{} · {}", host.provider.name(), host.display())).color(color::weak()));
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(f.source.as_str()).strong());
        ui.label("into");
        egui::ComboBox::from_id_salt("pr-target").selected_text(f.target.as_str()).width(200.0).show_ui(ui, |ui| {
            for t in f.targets.iter() {
                ui.selectable_value(f.target, t.clone(), t);
            }
        });
    });
    let pushed = snap.is_some_and(|s| s.remotes.iter().any(|(r, b)| *r == fs.remote && b.iter().any(|(n, _)| n == f.source)));
    if let Some((text, local)) = snap.and_then(|s| push_note(s, fs, f.source, pushed)) {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(text).color(color::amber()));
            // The dialog stays open: it is the one shown again once the push lands.
            if let Some(local) = local
                && ui.add_enabled(idle, egui::Button::new(format!("Push {local}"))).clicked()
            {
                msgs.push(Msg::Forge(ForgeMsg::PushAndStartPr { branch: local, target: Some(f.target.clone()) }));
            }
        });
    }
    ui.add_space(6.0);
    ui.add(egui::TextEdit::singleline(f.title).hint_text("Title").desired_width(f32::INFINITY));
    ui.add(egui::TextEdit::multiline(f.description).hint_text("Description (Markdown)").desired_rows(5).desired_width(f32::INFINITY));
    let keys = parse_keys(f.items);
    if fs.tracker.is_some() {
        ui.horizontal(|ui| {
            ui.label(format!("{}s", capitalize(fs.issue_noun())));
            let hint = match fs.style() {
                Some(RefStyle::Jira) => "e.g. PROJ-12, PROJ-14",
                Some(RefStyle::Trello) => "e.g. AbCd1234 or a card link",
                _ => "e.g. 123, 456",
            };
            ui.add(egui::TextEdit::singleline(f.items).hint_text(hint).desired_width(220.0));
        });
        for k in &keys {
            issue_chip(ui, fs, k, msgs);
        }
    }
    ui.checkbox(f.draft, "Create as draft");
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let valid = pushed && !f.title.trim().is_empty() && !f.target.is_empty() && f.target != f.source;
        if ui.add_enabled(valid, egui::Button::new(RichText::new(format!("Create {noun}")).strong())).clicked() {
            // Trackers linked by mention get "Closes #12"-style lines; Azure Boards items are
            // linked through the API.
            let mut body = f.description.trim().to_owned();
            let links: Vec<String> = fs
                .tracker
                .as_ref()
                .map(|t| keys.iter().filter_map(|k| forge::link_line(host.provider, t.provider, k)).collect())
                .unwrap_or_default();
            if !links.is_empty() {
                if !body.is_empty() {
                    body.push_str("\n\n");
                }
                body.push_str(&links.join("\n"));
            }
            msgs.push(Msg::Forge(ForgeMsg::CreatePr(NewPullRequest {
                source: f.source.clone(),
                target: f.target.clone(),
                title: f.title.trim().to_owned(),
                description: body,
                draft: *f.draft,
                issues: keys.clone(),
            })));
            close = true;
        }
        if ui.button("Cancel").clicked() {
            close = true;
        }
    });
    close
}
