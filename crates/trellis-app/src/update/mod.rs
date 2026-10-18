//! `update`: the only place application state changes. Each handler takes the current state
//! and a message, mutates the state, and returns the effects to run. Nothing here blocks or
//! touches disk, network or the window, which keeps it fully testable.

mod app;
mod askpass;
mod askpass_store;
mod avatars;
mod blame;
mod branches;
mod build_status;
mod clone;
mod clone_service;
mod commit;
mod compare_wip;
mod conflict;
mod conflict_check;
mod conflict_files;
mod dashboard;
mod deep_link;
mod deep_link_open;
mod diff;
mod diff_gaps;
mod files;
mod fold_keys;
mod forge;
mod forge_filter_edit;
mod forge_filters;
mod git_env;
mod gitflow;
mod gitflow_finish;
mod graph_columns;
mod graph_filter;
mod highlight;
mod history;
mod history_find;
mod history_view;
mod inactive;
mod init;
mod issue;
mod issue_create;
mod jobs;
mod keys;
mod layout;
mod lfs;
mod logs;
mod notify;
mod onboarding;
mod ops;
mod palette;
mod patch;
mod pick;
mod pr_details;
mod prefs;
mod profile_setup;
mod profiles;
mod publish;
mod pull_request;
mod push_pr;
mod rebase;
mod rebase_quick;
mod rebase_remaining;
mod rebase_resume;
mod recent;
mod refresh;
mod refresh_paths;
mod release_notes;
mod remote_forks;
mod remotes;
mod repo;
mod repo_scan;
mod repo_sync;
mod repos;
pub(crate) mod revert;
mod search;
mod search_paths;
mod selection;
mod shallow;
mod shortcuts;
mod sign_in;
mod signatures;
mod sparse;
mod ssh;
mod ssh_service;
mod ssh_use;
mod stash;
mod stash_push;
mod status_tip;
mod submodule_add;
mod submodule_batch;
mod submodule_edit;
mod submodule_remove;
mod submodules;
mod tab_groups;
mod tab_label;
mod tabs;
mod tabs_close;
mod tag_annotate;
mod tags;
mod terminal;
mod terminal_tabs;
mod terminal_view;
mod themes;
mod tool_compare;
mod tools;
mod trello;
mod trust;
mod undo;
mod updates;
mod watch;
mod work_files;
mod work_files_delete;
mod worktree_manage;
mod worktree_move;
mod worktrees;

#[cfg(test)]
mod test_mods;
#[cfg(test)]
mod tests;

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
        Msg::Recent(m) => recent::handle(st, m, now, &mut fx),
        Msg::Repos(m) => repos::handle(st, m, now, &mut fx),
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
        Msg::Layout(m) => layout::handle(st, m, now, &mut fx),
        Msg::Patch(m) => patch::handle(st, m, now, &mut fx),
        Msg::Tools(m) => tools::handle(st, m, now, &mut fx),
        Msg::ConflictCheck(m) => conflict_check::handle(st, m, now, &mut fx),
        Msg::PullRequest(m) => pull_request::handle(st, m, now, &mut fx),
        Msg::Issue(m) => issue::handle(st, m, now, &mut fx),
        Msg::Forge(m) => forge::handle(st, m, now, &mut fx),
        Msg::ForgeFilter(m) => forge_filters::handle(st, m, &mut fx),
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
        Msg::Theme(m) => themes::handle(st, m, now, &mut fx),
        Msg::Logs(m) => logs::handle(st, m, &mut fx),
        Msg::Onboarding(m) => onboarding::handle(st, m, now, &mut fx),
        Msg::DeepLink(m) => deep_link::handle(st, m, now, &mut fx),
        Msg::Dashboard(m) => dashboard::handle(st, m, now, &mut fx),
        Msg::App(m) => app::handle(st, m, &mut fx),
        Msg::Updates(m) => updates::handle(st, m, now, &mut fx),
        Msg::Shortcut(a) => shortcuts::handle(st, a, now, &mut fx),

        // Cross-cutting messages, handled here (see `Msg`).
        Msg::Tick(t) => {
            ops::auto_fetch(st, t, &mut fx);
            watch::poll(st, t, &mut fx);
            build_status::tick(st, t, &mut fx);
            updates::tick(st, t, &mut fx);
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
            if matches!(st.modal, Some(crate::state::Modal::DeepLink(_))) {
                deep_link::modal_closed(st);
            }
            st.modal = None;
        }
        Msg::Copy(text) => fx.push(Effect::Ui(UiEffect::CopyText(text))),
        Msg::OpenUrl(url) => fx.push(Effect::OpenUrl(url)),
        Msg::OpenTerminal => tools::open_terminal(st, now, &mut fx),
        Msg::OpenFolder(p) => fx.push(Effect::OpenFolder(p)),
        Msg::Logged { ok, text } => st.log(ok, text),
        Msg::ClearLog => logs::clear(st, &mut fx),
    }
    logs::flush(st, &mut fx);
    fx
}
