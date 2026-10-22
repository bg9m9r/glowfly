//! `update`: the only place application state changes. Each handler takes the current state
//! and a message, mutates the state, and returns the effects to run. Nothing here blocks or
//! touches disk, network or the window, which keeps it fully testable.

mod app;
mod askpass;
mod askpass_store;
mod auto_stash;
mod avatars;
mod blame;
mod branch_colour;
mod branches;
mod build_status;
mod clone;
mod clone_service;
mod commit;
mod compare_wip;
mod conflict;
mod conflict_check;
mod conflict_files;
mod custom_commands;
mod custom_commands_run;
mod dashboard;
mod deep_link;
mod deep_link_open;
mod dialog;
mod diff;
mod diff_gaps;
mod diff_preview;
mod files;
mod fold_keys;
mod forge;
mod forge_filter_edit;
mod forge_filters;
mod fork_pr;
mod git_env;
mod git_job;
mod gitflow;
mod gitflow_finish;
mod gitflow_resume;
mod graph_columns;
mod graph_filter;
mod highlight;
mod history;
mod history_find;
mod history_index;
mod history_view;
mod inactive;
mod init;
mod issue;
mod issue_comment;
mod issue_create;
mod job_errors;
mod job_stop;
mod jobs;
mod journal;
mod journal_lost;
mod keys;
mod layout;
mod lfs;
mod line_counts;
mod load_more;
mod logs;
mod logs_copy;
mod logs_export;
mod logs_runs;
mod logs_tabs;
mod notify;
mod onboarding;
mod op_preview;
mod op_progress;
mod ops;
mod palette;
mod palette_keys;
mod partial;
mod partial_content;
mod partial_size;
mod patch;
mod pick;
mod pick_done;
mod pr_details;
mod pr_repo_pick;
mod prefs;
mod prefs_leave;
mod prefs_nav;
mod prefs_save;
mod profile_rules;
mod profile_setup;
mod profiles;
mod publish;
mod pull_request;
mod push_pr;
mod range_diff;
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
mod repo_colour;
mod repo_scan;
mod repo_sync;
mod repos;
pub(crate) mod revert;
mod search;
mod search_deeper;
mod search_index;
mod search_paths;
mod selection;
mod selection_focus;
mod shallow;
mod shortcuts;
mod sign_in;
mod signatures;
mod sparse;
mod ssh;
mod ssh_pick;
mod ssh_service;
mod ssh_use;
mod ssh_windows;
mod stack;
mod stack_prs;
mod stack_push;
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
mod tag_checkout;
mod tags;
mod terminal;
mod terminal_header;
mod terminal_schemes;
mod terminal_tabs;
mod terminal_view;
mod themes;
mod tool_compare;
mod tool_git;
mod tools;
mod trello;
mod trust;
mod trust_inherit;
mod undo;
mod updates;
mod watch;
mod wip_list;
mod work_files;
mod work_files_delete;
mod worktree_counts;
mod worktree_manage;
mod worktree_move;
mod worktrees;

#[cfg(test)]
mod test_mods;
#[cfg(test)]
mod tests;

use crate::effect::OsEffect;
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
    // Leaving a Preferences page with unsaved edits asks first (update/prefs_leave.rs).
    let Some(msg) = prefs_leave::hold(st, msg) else {
        st.dialog.track(st.modal.as_ref());
        return fx;
    };
    // A failure logged from here on is announced by a toast; one pushed by now isn't this one's.
    let toast_mark = st.toast_seq;
    match msg {
        // Each area's messages go to its handler, which matches them exhaustively.
        Msg::Repo(m) => repo::handle(st, m, now, &mut fx),
        Msg::Recent(m) => recent::handle(st, m, now, &mut fx),
        Msg::Repos(m) => repos::handle(st, m, now, &mut fx),
        Msg::Load(m) => refresh::handle(st, m, now, &mut fx),
        Msg::Watch(m) => watch::handle(st, m, now, &mut fx),
        Msg::Selection(m) => selection::handle(st, m, now, &mut fx),
        Msg::Graph(m) => graph_filter::handle(st, m, &mut fx),
        Msg::Search(m) => search::handle(st, m, &mut fx),
        Msg::Avatars(m) => avatars::handle(st, m, &mut fx),
        Msg::Diff(m) => diff::handle(st, m, &mut fx),
        Msg::Conflict(m) => conflict::handle(st, m, &mut fx),
        Msg::History(m) => history::handle(st, m, now, &mut fx),
        Msg::Blame(m) => blame::handle(st, m, now, &mut fx),
        Msg::Rebase(m) => rebase::handle(st, m, now, &mut fx),
        Msg::Stack(m) => stack_push::handle(st, m, now, &mut fx),
        Msg::Tabs(m) => tabs::handle(st, m, &mut fx),
        Msg::Stage(m) => ops::stage(st, m, now, &mut fx),
        Msg::Commit(m) => commit::handle(st, m, now, &mut fx),
        Msg::Job(m) => ops::job(st, m, now, &mut fx),
        Msg::Stash(m) => stash::handle(st, m, &mut fx),
        Msg::AutoStash(m) => auto_stash::handle(st, m, now, &mut fx),
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
        Msg::CustomCommand(m) => custom_commands::handle(st, m, now, &mut fx),
        Msg::ConflictCheck(m) => conflict_check::handle(st, m, now, &mut fx),
        Msg::OpPreview(m) => op_preview::handle(st, m, now, &mut fx),
        Msg::Journal(m) => journal::handle(st, m, now, &mut fx),
        Msg::RangeDiff(m) => range_diff::handle(st, m, now, &mut fx),
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
        Msg::Logs(m) => logs::handle(st, m, now, &mut fx),
        Msg::Onboarding(m) => onboarding::handle(st, m, now, &mut fx),
        Msg::DeepLink(m) => deep_link::handle(st, m, now, &mut fx),
        Msg::Dashboard(m) => dashboard::handle(st, m, now, &mut fx),
        Msg::Dialog(m) => dialog::handle(st, m),
        Msg::App(m) => app::handle(st, m, &mut fx),
        Msg::Updates(m) => updates::handle(st, m, now, &mut fx),
        Msg::Partial(m) => partial::handle(st, m, &mut fx),
        Msg::Shortcut(a) => shortcuts::handle(st, a, now, &mut fx),

        // Cross-cutting messages, handled here (see `Msg`).
        Msg::Tick(t) => {
            ops::auto_fetch(st, t, &mut fx);
            watch::poll(st, t, &mut fx);
            build_status::tick(st, t, &mut fx);
            pull_request::checks_tick(st, t, &mut fx);
            updates::tick(st, t, &mut fx);
            prefs_save::tick(st, t);
            load_more::tick(st, t, &mut fx);
        }
        Msg::Scoped { workdir, msg } => {
            if st.workdir().is_some_and(|w| w == workdir) {
                fx.extend(update(st, *msg, now));
            } else {
                inactive::inactive(st, &workdir, *msg, now);
            }
        }
        Msg::Confirm { title, body, button, then } => st.modal = Some(crate::state::Modal::Confirm { title, body, button, then }),
        Msg::ConfirmDanger { title, body, button, ack, then } => {
            st.modal = Some(crate::state::Modal::ConfirmDanger { title, body, button, ack, acked: false, then });
        }
        Msg::ConfirmAccepted => {
            // A step that needs the acknowledgement waits, open, until it is ticked.
            if !st.modal.as_ref().is_some_and(crate::state::Modal::confirm_ready) {
                tracing::debug!("confirmation not ready (no confirm open, or its acknowledgement isn't ticked)");
            } else if let Some(
                crate::state::Modal::Confirm { then, .. }
                | crate::state::Modal::ConfirmDanger { then, .. }
                | crate::state::Modal::ConfirmChoice { then, .. },
            ) = st.modal.take()
            {
                fx.extend(update(st, *then, now));
            }
        }
        Msg::ConfirmAlt => {
            if matches!(st.modal, Some(crate::state::Modal::ConfirmChoice { .. }))
                && let Some(crate::state::Modal::ConfirmChoice { alt_then, .. }) = st.modal.take()
            {
                fx.extend(update(st, *alt_then, now));
            }
        }
        Msg::ToastAction(id) => notify::toast_action(st, id, false, now, &mut fx),
        Msg::ToastDetails(id) => notify::toast_action(st, id, true, now, &mut fx),
        Msg::Notify(m) => notify::handle(st, m, now, &mut fx),
        Msg::CloseModal => dialog::close(st),
        Msg::Copy(text) => fx.push(Effect::Ui(UiEffect::CopyText(text))),
        Msg::OpenUrl(url) => fx.push(Effect::Os(OsEffect::OpenUrl(url))),
        Msg::OpenTerminal => tools::open_terminal(st, now, &mut fx),
        Msg::OpenFolder(p) => fx.push(Effect::Os(OsEffect::OpenFolder(p))),
        Msg::Logged { ok, text } => st.log(ok, text),
        Msg::ClearLog => logs::clear(st, &mut fx),
    }
    prefs_leave::settle(st);
    // Records a dialog that just opened (to tell later whether it was edited) and forgets a
    // closed one.
    st.dialog.track(st.modal.as_ref());
    // An expanded commit panel lasts only for the tool view it was expanded in.
    crate::state::layout::drop_stale_expansion(st);
    logs_runs::stamp(st, now);
    logs_tabs::announce(st, toast_mark, now);
    logs::flush(st, &mut fx);
    fx
}
