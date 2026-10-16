//! `update`: the only place application state changes. Each handler takes the current state
//! and a message, mutates the state, and returns the effects to run. Nothing here blocks or
//! touches disk, network or the window, which keeps it fully testable.

mod askpass;
mod avatars;
mod blame;
mod branches;
mod build_status;
mod clone;
mod clone_service;
mod commit;
mod conflict;
mod conflict_check;
mod diff;
mod files;
mod forge;
mod git_env;
mod gitflow;
mod gitflow_finish;
mod graph_columns;
mod graph_filter;
mod highlight;
mod history;
mod inactive;
mod init;
mod jobs;
mod keys;
mod lfs;
mod notify;
mod ops;
mod palette;
mod patch;
mod prefs;
mod profile_setup;
mod profiles;
mod publish;
mod pull_request;
mod push_pr;
mod rebase;
mod recent;
mod refresh;
mod remote_forks;
mod remotes;
mod repo;
mod search;
mod search_paths;
mod selection;
mod shallow;
mod shortcuts;
mod signatures;
mod sparse;
mod ssh;
mod ssh_service;
mod ssh_use;
mod stash;
mod stash_push;
mod submodule_add;
mod submodule_remove;
mod submodules;
mod tabs;
mod tag_annotate;
mod tags;
mod terminal;
mod tool_compare;
mod tools;
mod trello;
mod trust;
mod undo;
mod watch;
mod work_files;
mod work_files_delete;
mod worktree_manage;
mod worktrees;

#[cfg(test)]
mod askpass_gpg_tests;
#[cfg(test)]
mod askpass_tests;
#[cfg(test)]
mod blame_tests;
#[cfg(test)]
mod branch_tests;
#[cfg(test)]
mod build_status_tests;
#[cfg(test)]
mod cherry_pick_tests;
#[cfg(test)]
mod clone_service_tests;
#[cfg(test)]
mod clone_sparse_tests;
#[cfg(test)]
mod clone_tests;
#[cfg(test)]
mod commit_tests;
#[cfg(test)]
mod conflict_check_tests;
#[cfg(test)]
mod conflict_tests;
#[cfg(test)]
mod desktop_notify_tests;
#[cfg(test)]
mod diff_nav_tests;
#[cfg(test)]
mod diff_pairs_tests;
#[cfg(test)]
mod diff_tests;
#[cfg(test)]
mod editor_tests;
#[cfg(test)]
mod files_open_tests;
#[cfg(test)]
mod files_tests;
#[cfg(test)]
mod files_undo_tests;
#[cfg(test)]
mod fold_tests;
#[cfg(test)]
mod force_push_tests;
#[cfg(test)]
mod forge_tests;
#[cfg(test)]
mod git_env_tests;
#[cfg(test)]
mod gitflow_finish_tests;
#[cfg(test)]
mod gitflow_panel_tests;
#[cfg(test)]
mod gitflow_tests;
#[cfg(test)]
mod graph_columns_tests;
#[cfg(test)]
mod graph_filter_tests;
#[cfg(test)]
mod highlight_context_tests;
#[cfg(test)]
mod highlight_header_tests;
#[cfg(test)]
mod highlight_tests;
#[cfg(test)]
mod history_tests;
#[cfg(test)]
mod init_tests;
#[cfg(test)]
mod keys_tests;
#[cfg(test)]
mod language_override_tests;
#[cfg(test)]
mod lfs_fetch_tests;
#[cfg(test)]
mod lfs_tests;
#[cfg(test)]
mod lfs_track_tests;
#[cfg(test)]
mod lines_tests;
#[cfg(test)]
mod load_more_tests;
#[cfg(test)]
mod notify_tests;
#[cfg(test)]
mod palette_blame_tests;
#[cfg(test)]
mod palette_checkout_tests;
#[cfg(test)]
mod palette_more_tests;
#[cfg(test)]
mod palette_pick_tests;
#[cfg(test)]
mod palette_profile_tests;
#[cfg(test)]
mod palette_recent_tests;
#[cfg(test)]
mod palette_stash_tests;
#[cfg(test)]
mod palette_tests;
#[cfg(test)]
mod patch_tests;
#[cfg(test)]
mod path_alias_tests;
#[cfg(test)]
mod path_case_tests;
#[cfg(test)]
mod path_hazards_tests;
#[cfg(test)]
mod perf_tests;
#[cfg(test)]
mod pr_checkout_tests;
#[cfg(test)]
mod profile_setup_tests;
#[cfg(test)]
mod profiles_tests;
#[cfg(test)]
mod publish_tests;
#[cfg(test)]
mod push_pr_state_tests;
#[cfg(test)]
mod push_pr_tests;
#[cfg(test)]
mod rebase_e2e_tests;
#[cfg(test)]
mod rebase_tests;
#[cfg(test)]
mod recent_tests;
#[cfg(test)]
mod refresh_tests;
#[cfg(test)]
mod remote_fork_tests;
#[cfg(test)]
mod remote_tests;
#[cfg(test)]
mod revert_tests;
#[cfg(test)]
mod search_paths_tests;
#[cfg(test)]
mod search_tests;
#[cfg(test)]
mod selection_tests;
#[cfg(test)]
mod shallow_tests;
#[cfg(test)]
mod shortcuts_tests;
#[cfg(test)]
mod signature_tests;
#[cfg(test)]
mod sparse_tests;
#[cfg(test)]
mod ssh_service_tests;
#[cfg(test)]
mod ssh_tests;
#[cfg(test)]
mod ssh_use_agent_tests;
#[cfg(test)]
mod ssh_use_tests;
#[cfg(test)]
mod stash_push_tests;
#[cfg(test)]
mod stash_rename_tests;
#[cfg(test)]
mod stash_tests;
#[cfg(test)]
mod submodule_add_tests;
#[cfg(test)]
mod submodule_remove_tests;
#[cfg(test)]
mod submodule_tests;
#[cfg(test)]
mod tabs_tests;
#[cfg(test)]
mod tag_annotate_tests;
#[cfg(test)]
mod tag_remote_tests;
#[cfg(test)]
mod tag_tests;
#[cfg(test)]
mod terminal_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tool_compare_tests;
#[cfg(test)]
mod tools_tests;
#[cfg(test)]
mod trello_tests;
#[cfg(test)]
mod trust_held_tab_tests;
#[cfg(test)]
mod trust_held_tests;
#[cfg(test)]
mod trust_tests;
#[cfg(test)]
mod undo_tests;
#[cfg(test)]
mod watch_poll_tests;
#[cfg(test)]
mod watch_tests;
#[cfg(test)]
mod whitespace_tests;
#[cfg(test)]
mod whole_file_tests;
#[cfg(test)]
mod work_files_discard_tests;
#[cfg(test)]
mod work_files_stage_tests;
#[cfg(test)]
mod work_files_tests;
#[cfg(test)]
mod work_files_trash_tests;
#[cfg(test)]
mod work_files_unstage_tests;
#[cfg(test)]
mod worktree_manage_tests;
#[cfg(test)]
mod worktree_tests;
#[cfg(test)]
mod worktree_wip_tests;
#[cfg(test)]
mod wrap_tests;

use crate::effect::{Effect, UiEffect};
use crate::msg::Msg;
use crate::state::AppState;
use std::time::Instant;

pub fn update(st: &mut AppState, msg: Msg, now: Instant) -> Vec<Effect> {
    let mut fx = Vec::new();
    // A row whose path could reach the wrong file: refused before any handler runs.
    if let Some(why) = crate::state::path_hazards::refusal(st, &msg) {
        st.toast(false, why, now);
        return fx;
    }
    match msg {
        // Each area's messages go to its handler, which matches them exhaustively.
        Msg::Repo(m) => repo::handle(st, m, now, &mut fx),
        Msg::Recent(m) => recent::handle(st, m, &mut fx),
        Msg::Load(m) => refresh::handle(st, m, now, &mut fx),
        Msg::Watch(m) => watch::handle(st, m, now, &mut fx),
        Msg::Selection(m) => selection::handle(st, m, &mut fx),
        Msg::Graph(m) => graph_filter::handle(st, m, &mut fx),
        Msg::Search(m) => search::handle(st, m, &mut fx),
        Msg::Avatars(m) => avatars::handle(st, m, &mut fx),
        Msg::Diff(m) => diff::handle(st, m, &mut fx),
        Msg::Conflict(m) => conflict::handle(st, m, &mut fx),
        Msg::History(m) => history::handle(st, m, now, &mut fx),
        Msg::Blame(m) => blame::handle(st, m, now, &mut fx),
        Msg::Rebase(m) => rebase::handle(st, m, now, &mut fx),
        Msg::Tabs(m) => tabs::handle(st, m, &mut fx),
        Msg::Stage(m) => ops::stage(st, m, now, &mut fx),
        Msg::Commit(m) => commit::handle(st, m, now, &mut fx),
        Msg::Job(m) => ops::job(st, m, now, &mut fx),
        Msg::Stash(m) => stash::handle(st, m, &mut fx),
        Msg::Files(m) => files::handle(st, m, now, &mut fx),
        Msg::WorkFile(m) => work_files::handle(st, m, now, &mut fx),
        Msg::Branch(m) => branches::handle(st, m, &mut fx),
        Msg::Clone(m) => clone::handle(st, m, now, &mut fx),
        Msg::Init(m) => init::handle(st, m, now, &mut fx),
        Msg::Tag(m) => tags::handle(st, m, &mut fx),
        Msg::Remote(m) => remotes::handle(st, m, &mut fx),
        Msg::Worktree(m) => worktrees::handle(st, m, now, &mut fx),
        Msg::Submodule(m) => submodules::handle(st, m, now, &mut fx),
        Msg::Terminal(m) => terminal::handle(st, m, &mut fx),
        Msg::Patch(m) => patch::handle(st, m, now, &mut fx),
        Msg::Tools(m) => tools::handle(st, m, now, &mut fx),
        Msg::ConflictCheck(m) => conflict_check::handle(st, m, now, &mut fx),
        Msg::PullRequest(m) => pull_request::handle(st, m, now, &mut fx),
        Msg::Forge(m) => forge::handle(st, m, now, &mut fx),
        Msg::Publish(m) => publish::handle(st, m, &mut fx),
        Msg::Credentials(m) => askpass::handle(st, m, now, &mut fx),
        Msg::Prefs(m) => prefs::handle(st, m, now, &mut fx),
        Msg::Keys(m) => keys::handle(st, m, &mut fx),
        Msg::Lfs(m) => lfs::handle(st, m, now, &mut fx),
        Msg::GitFlow(m) => gitflow::handle(st, m, now, &mut fx),
        Msg::Sparse(m) => sparse::handle(st, m, now, &mut fx),
        Msg::Ssh(m) => ssh::handle(st, m, now, &mut fx),
        Msg::Profile(m) => profiles::handle(st, m, now, &mut fx),
        Msg::Palette(m) => palette::handle(st, m, now, &mut fx),
        Msg::Trust(m) => trust::handle(st, m, now, &mut fx),
        Msg::GitEnv(m) => git_env::handle(st, m, now, &mut fx),
        Msg::Shortcut(a) => shortcuts::handle(st, a, now, &mut fx),

        // Cross-cutting messages, handled here (see `Msg`).
        Msg::Tick(t) => {
            ops::auto_fetch(st, t, &mut fx);
            watch::poll(st, t, &mut fx);
            build_status::tick(st, t, &mut fx);
        }
        Msg::Scoped { workdir, msg } => {
            if st.workdir().is_some_and(|w| w == workdir) {
                fx.extend(update(st, *msg, now));
            } else {
                inactive::inactive(st, &workdir, *msg, now);
            }
        }
        Msg::Confirm { title, body, button, then } => st.modal = Some(crate::state::Modal::Confirm { title, body, button, then }),
        Msg::ConfirmAccepted => {
            if let Some(crate::state::Modal::Confirm { then, .. }) = st.modal.take() {
                fx.extend(update(st, *then, now));
            }
        }
        Msg::ToastAction(id) => notify::toast_action(st, id, now, &mut fx),
        Msg::CloseModal => {
            // Closing the trust prompt without a decision drops the job waiting for it.
            if matches!(st.modal, Some(crate::state::Modal::Trust(_))) {
                trust::dismissed(st);
            }
            st.modal = None;
        }
        Msg::Copy(text) => fx.push(Effect::Ui(UiEffect::CopyText(text))),
        Msg::OpenUrl(url) => fx.push(Effect::OpenUrl(url)),
        Msg::OpenTerminal => tools::open_terminal(st, now, &mut fx),
        Msg::OpenFolder(p) => fx.push(Effect::OpenFolder(p)),
        Msg::Logged { ok, text } => st.log(ok, text),
        Msg::ClearLog => st.log.clear(),
    }
    fx
}
