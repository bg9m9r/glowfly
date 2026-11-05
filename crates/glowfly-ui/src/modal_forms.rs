//! The dialogs whose views write their form (`Modal::form_copy`): each is drawn from the
//! frame's copy, which `modals.rs` sends back as `EditMsg::Dialog` when it changed. What every
//! dialog reads besides its own state is gathered once in `DialogEnv`. The dialogs that only
//! read (questions, reports, notes) are drawn by `modal_notes.rs`.

use crate::dialog_frame::Cx;
use chrono::NaiveDate;
use eframe::egui;
use glowfly_core::forge::Connection;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;
use glowfly_core::settings::{RepoForgeFilters, RepoGroup};
use glowfly_model::keymap::Keymap;
use glowfly_model::msg::Msg;
use glowfly_model::state::git_env::GitState;
use glowfly_model::state::menu_bar::BuildInfo;
use glowfly_model::state::prefs::PrefsState;
use glowfly_model::state::repo_scan::{FinderRow, RepoScan};
use glowfly_model::state::repo_sync::RepoSync;
use glowfly_model::state::{AppState, ForgeState, Modal};
use std::path::PathBuf;

/// What the dialogs read from the rest of the state, gathered before one is drawn.
pub(crate) struct DialogEnv<'a> {
    pub snap: Option<&'a Snapshot>,
    pub fs: Option<&'a ForgeState>,
    /// No job is running.
    pub idle: bool,
    pub stage_renames: bool,
    pub push_tags: bool,
    pub connections: &'a [Connection],
    pub groups: &'a [RepoGroup],
    pub keys: &'a Keymap,
    pub git: &'a GitState,
    pub version: &'static str,
    pub build: &'a BuildInfo,
    pub t: Lang,
    pub repo_filters: RepoForgeFilters,
    /// Find repositories' results (which are open or favorites) and project folders, worked out
    /// only while it is open.
    pub finder: Option<(Vec<FinderRow>, Vec<PathBuf>)>,
    pub repo_scan: &'a RepoScan,
    pub repo_sync: Option<&'a RepoSync>,
    pub preview_env: crate::op_preview_dialog::Env<'a>,
    pub today: NaiveDate,
    pub sparse_blocked: Option<String>,
    /// Preferences, for the Generate SSH key dialog's page (None: it closes).
    pub prefs: Option<&'a PrefsState>,
}

impl<'a> DialogEnv<'a> {
    /// The environment for drawing `shown`.
    pub fn new(st: &'a AppState, shown: &Modal) -> Self {
        let snap = st.snap.as_ref();
        DialogEnv {
            snap,
            fs: st.session.forge.as_ref(),
            idle: st.job.is_none(),
            stage_renames: st.settings.stage_renames,
            push_tags: st.settings.push_new_tags,
            connections: &st.settings.connections,
            groups: &st.settings.repo_groups,
            keys: &st.keymap,
            git: &st.git,
            version: st.updates.shown,
            build: &st.updates.build,
            t: st.settings.lang(),
            repo_filters: snap.map(|s| st.settings.forge_filters_for(&s.real_workdir)).unwrap_or_default(),
            finder: matches!(shown, Modal::FindRepos { .. }).then(|| {
                (glowfly_model::state::repo_scan::finder_rows(st), glowfly_model::state::repo_scan::project_folders(&st.settings))
            }),
            repo_scan: &st.repo_scan,
            repo_sync: st.repo_sync.as_ref(),
            preview_env: crate::op_preview_dialog::Env {
                snap,
                settings: &st.settings,
                now: st.cues.now,
                offset: st.cues.utc_offset,
                idle: st.job.is_none(),
            },
            today: glowfly_core::clone_ops::local_date(st.cues.now, st.cues.utc_offset),
            sparse_blocked: st.git.blocked(glowfly_core::git_version::Feature::SparseCheckout),
            prefs: st.prefs.as_ref(),
        }
    }
}

/// Draws the form `modal` (the frame's copy, edited in place); whether the dialog should close.
pub(crate) fn draw(ui: &mut egui::Ui, cx: Cx, modal: &mut Modal, env: &DialogEnv, msgs: &mut Vec<Msg>) -> bool {
    let (snap, idle, connections) = (env.snap, env.idle, env.connections);
    match modal {
        Modal::NewBranch { start, name, checkout, track, touched } => {
            // Not under a credential prompt or the discard question: its fields need the keyboard.
            let form = crate::branch_dialogs::NewBranchForm { start: start.as_deref(), name, checkout, track: track.as_ref(), touched };
            crate::branch_dialogs::new_branch(ui, cx, form, snap, msgs)
        }
        Modal::NewTag(form) => crate::tag_dialog::new_tag(ui, cx, form, snap, env.push_tags, msgs),
        Modal::AnnotateTag(form) => crate::tag_dialog::annotate_tag(ui, cx, form, snap, msgs),
        Modal::NewWorktree(form) => crate::worktree_dialog::new_worktree(ui, cx, form, snap, msgs),
        Modal::DeleteRemoteBranch(form) => crate::remote_branch_delete_dialog::show(ui, cx, form, msgs),
        Modal::RemoveWorktree(form) => crate::worktree_manage_dialog::remove(ui, cx, form, idle, msgs),
        Modal::LockWorktree(form) => crate::worktree_manage_dialog::lock(ui, cx, form, idle, msgs),
        Modal::MoveWorktree(form) => crate::worktree_move_dialog::move_worktree(ui, cx, form, snap, idle, msgs),
        Modal::AddSubmodule(form) => crate::submodule_dialogs::add(ui, cx, form, snap, idle, msgs),
        Modal::EditSubmodule(form) => crate::submodule_dialogs::edit(ui, cx, form, idle, msgs),
        Modal::RemoveSubmodule(form) => crate::submodule_dialogs::remove(ui, cx, form, idle, msgs),
        Modal::Clone(form) => crate::clone_dialog::show(ui, cx, form, env.today, env.sparse_blocked.as_deref(), connections, msgs),
        Modal::Init(form) => crate::init_dialog::show(ui, cx, form, chrono::Datelike::year(&env.today), msgs),
        Modal::CreatePr { source, target, targets, title, description, draft, items, repos } => {
            let Some(fs) = env.fs.filter(|f| f.host.is_some()) else { return true };
            let form = crate::pull_request::create_dialog::PrForm { source, target, targets, title, description, draft, items, repos };
            crate::pull_request::create_dialog::show(ui, cx, form, fs, snap, idle, msgs)
        }
        Modal::Publish(f) => crate::publish::form(ui, cx, f, idle, msgs),
        Modal::RenameBranch(form) => crate::branch_dialogs::rename(ui, cx, form, snap, msgs),
        Modal::RenameStash(form) => crate::stash_dialog::rename(ui, cx, form, idle, msgs),
        Modal::StashPush(form) => crate::stash_push_dialog::show(ui, cx, form, snap, idle, msgs),
        Modal::SetUpstream { branch, remote, name, touched } => {
            crate::branch_dialogs::upstream(ui, cx, branch, (remote, name, touched), snap, msgs)
        }
        Modal::NewFile(form) => crate::file_dialogs::new_file(ui, cx, form, snap, msgs),
        Modal::RenameFile { old, form } => crate::file_dialogs::rename_file(ui, cx, old, form, snap, env.stage_renames, msgs),
        Modal::Remote(form) => {
            let fork_source = snap.and_then(|s| glowfly_model::state::forks::fork_source(s, connections));
            crate::remotes::dialog::show(ui, cx, form, snap, fork_source, idle, msgs)
        }
        Modal::AddFork(p) => crate::remotes::fork_pick::show(ui, cx, p, snap, connections, idle, msgs),
        Modal::CreateFork(f) => crate::remotes::fork_create::show(ui, cx, f, snap, connections, idle, msgs),
        Modal::GitFlowStart(form) => crate::gitflow_dialog::start(ui, cx, form, snap, msgs),
        Modal::GitFlowFinish(form) => crate::gitflow_finish_dialog::finish(ui, cx, form, snap, idle, msgs),
        Modal::Shortcuts { filter } => crate::shortcuts::sheet(ui, cx, filter, env.keys),
        Modal::ForgeFilter(form) => crate::forge_filter_dialog::show(ui, cx, form, env.fs, &env.repo_filters, msgs),
        Modal::CreateIssue(form) => crate::issue::create_dialog::show(ui, cx, form, env.fs, msgs),
        Modal::RewordCommit(form) => crate::rebase::reword_dialog::show(ui, cx, form, snap, idle, msgs),
        Modal::UndoCreated(form) => crate::undo_dialog::created_branch(ui, cx, form, msgs),
        Modal::ApplyPatch(form) => crate::patch_apply_dialog::show(ui, cx, form, snap, idle, msgs),
        Modal::SavePatch(form) => crate::patch_save_dialog::show(ui, cx, form, snap, idle, msgs),
        Modal::FindRepos { filter } => {
            let (rows, folders) = env.finder.as_ref().map(|(r, f)| (&r[..], &f[..])).unwrap_or_default();
            let lists = crate::repo_finder::FinderLists { rows, scan: env.repo_scan, folders };
            crate::repo_finder::show(ui, cx, filter, lists, msgs)
        }
        Modal::RenameTab(form) => crate::tab_dialog::rename(ui, cx, form, msgs),
        Modal::RepoGroup(form) => crate::repo_group_dialog::show(ui, cx, form, msgs),
        Modal::AddToGroup(form) => crate::repo_group_picker::show(ui, cx, form, env.groups, msgs),
        Modal::Mainline(form) => crate::mainline_dialog::show(ui, cx, form, idle, msgs),
        // Not forms: drawn from the dialog itself by `modal_notes.rs` (`form_copy` gives none).
        Modal::Confirm { .. }
        | Modal::ConfirmDetails { .. }
        | Modal::ConfirmDanger { .. }
        | Modal::ConfirmChoice { .. }
        | Modal::DeleteBranch(_)
        | Modal::Trust(_)
        | Modal::ConflictReport(_)
        | Modal::OpPreview(_)
        | Modal::DeepLink(_)
        | Modal::About
        | Modal::Notices
        | Modal::ReleaseNotes(_)
        | Modal::SyncRepos
        | Modal::TrashOffer(_)
        | Modal::AutoStash(_)
        | Modal::SshKeygen => false,
    }
}
