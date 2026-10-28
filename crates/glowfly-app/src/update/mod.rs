//! `update`: the only place application state changes. Each handler takes the current state
//! and a message, mutates the state, and returns the effects to run. Nothing here blocks or
//! touches disk, network or the window, which keeps it fully testable.

mod app;
mod askpass;
mod askpass_store;
mod auto_stash;
mod auto_stash_restack;
mod auto_stash_saved;
mod avatars;
mod blame;
mod branch_colour;
mod branch_delete;
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
mod large_graph;
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
mod pr_checkout;
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
mod refresh_index;
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
mod search_loaded;
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
mod stage_folder;
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
mod terminal_busy;
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
mod trust_seal;
mod undo;
mod updates;
mod updates_restart;
mod watch;
mod wip_list;
pub(crate) mod work_files;
mod work_files_delete;
mod worktree_counts;
mod worktree_manage;
mod worktree_move;
mod worktrees;

#[cfg(test)]
mod tests;

use crate::effect::OsEffect;
use crate::effect::{Effect, UiEffect};
use crate::msg::Msg;
use crate::state::AppState;
use std::time::Instant;

/// `trust_seal::wrap`, for tests.
#[cfg(test)]
pub(crate) use trust_seal::wrap as trust_seal_wrap;

/// `update_sealed` with the effects taken out of their trust checks (`Effect::unsealed`), so
/// tests see what was asked for; `update_sealed` keeps them.
#[cfg(test)]
pub fn update(st: &mut AppState, msg: Msg, now: Instant) -> Vec<Effect> {
    update_sealed(st, msg, now).into_iter().map(Effect::unsealed).collect()
}

/// Handles `msg` and returns the effects to run, each that must be checked against the
/// repository's trust first carrying its check (`Effect::Checked`, update/trust_seal.rs).
pub fn update_sealed(st: &mut AppState, msg: Msg, now: Instant) -> Vec<Effect> {
    // Only the seals recorded from here on are this call's: an outer call (a handler that
    // updates with another message) wraps its own, and those a handler called on its own (in a
    // test) left belong to no effect here.
    let mark = st.trust.seals.len();
    let mut fx = update_plain(st, msg, now);
    trust_seal::wrap(st, &mut fx, mark);
    fx
}

fn update_plain(st: &mut AppState, msg: Msg, now: Instant) -> Vec<Effect> {
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
    route(st, msg, now, &mut fx);
    prefs_leave::settle(st);
    app::settle(st, &mut fx);
    // A form kept under a question that has closed comes back (state/dialog_guard.rs).
    crate::state::dialog_guard::restore(&mut st.modal, &mut st.dialog);
    // Records a dialog that just opened (to tell later whether it was edited) and forgets a
    // closed one.
    st.dialog.track(st.modal.as_ref());
    // An expanded commit panel lasts only for the tool view it was expanded in.
    crate::state::layout::drop_stale_expansion(st);
    logs_runs::stamp(st, now);
    logs_tabs::announce(st, toast_mark, now);
    logs::flush(st, &mut fx);
    large_graph::settle(st, &mut fx);
    fx
}

/// `handler_call!(f, st, m, now, fx; <args>)`: calls an area's handler with the arguments its
/// row in msg/areas.rs names (`now, fx`, `fx` or none).
macro_rules! handler_call {
    ($f:path, $st:ident, $m:ident, $now:ident, $fx:ident; now, fx) => {
        $f($st, $m, $now, $fx)
    };
    ($f:path, $st:ident, $m:ident, $now:ident, $fx:ident; fx) => {
        $f($st, $m, $fx)
    };
    ($f:path, $st:ident, $m:ident, $now:ident, $fx:ident;) => {
        $f($st, $m)
    };
}

/// Declares `fn $name($st, $msg, $now, $fx)`: each area's messages go to its handler (from the
/// rows of msg/areas.rs), then the arms given for the rest.
macro_rules! area_router {
    (
        { fn $name:ident($st:ident, $msg:ident, $now:ident, $fx:ident) { $($extra:tt)* } }
        $( $(#[$doc:meta])* $V:ident($T:ty) $area:literal => $module:ident :: $func:ident ( $($arg:ident),* ); )*
    ) => {
        fn $name($st: &mut AppState, $msg: Msg, $now: Instant, $fx: &mut Vec<Effect>) {
            match $msg {
                $( Msg::$V(m) => handler_call!($module::$func, $st, m, $now, $fx; $($arg),*), )*
                $($extra)*
            }
        }
    };
}

// Each area's messages go to its handler, which matches them exhaustively; the shortcut keys
// and the cross-cutting messages (see `Msg`) are handled here.
crate::msg::msg_areas! { area_router! {
    fn route(st, msg, now, fx) {
        Msg::Shortcut(a) => shortcuts::handle(st, a, now, fx),

        // Cross-cutting messages, handled here (see `Msg`).
        Msg::Tick(t) => {
            ops::auto_fetch(st, t, fx);
            watch::poll(st, t, fx);
            build_status::tick(st, t, fx);
            pull_request::checks_tick(st, t, fx);
            updates::tick(st, t, fx);
            prefs_save::tick(st, t);
            load_more::tick(st, t, fx);
            search_loaded::tick(st, t, fx);
            history_index::tick(st, t, fx);
        }
        Msg::Scoped { workdir, msg } => {
            if st.workdir().is_some_and(|w| w == workdir) {
                fx.extend(update_sealed(st, *msg, now));
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
                | crate::state::Modal::ConfirmDetails { then, .. }
                | crate::state::Modal::ConfirmDanger { then, .. }
                | crate::state::Modal::ConfirmChoice { then, .. },
            ) = st.modal.take()
            {
                fx.extend(update_sealed(st, *then, now));
            }
        }
        Msg::ConfirmAlt => {
            if matches!(st.modal, Some(crate::state::Modal::ConfirmChoice { .. }))
                && let Some(crate::state::Modal::ConfirmChoice { alt_then, .. }) = st.modal.take()
            {
                fx.extend(update_sealed(st, *alt_then, now));
            }
        }
        Msg::ToastAction(id) => notify::toast_action(st, id, false, now, fx),
        Msg::ToastDetails(id) => notify::toast_action(st, id, true, now, fx),
        Msg::CloseModal => dialog::close(st),
        Msg::Copy(text) => fx.push(Effect::Ui(UiEffect::CopyText(text))),
        Msg::OpenUrl(url) => fx.push(Effect::Os(OsEffect::OpenUrl(url))),
        Msg::OpenTerminal => tools::open_terminal(st, now, fx),
        Msg::OpenFolder(p) => fx.push(Effect::Os(OsEffect::OpenFolder(p))),
        Msg::Logged { ok, text } => st.log(ok, text),
        Msg::ClearLog => logs::clear(st, fx),
    }
} }
