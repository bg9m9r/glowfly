//! `update`: the only place application state changes. Each handler takes the current state
//! and a message, mutates the state, and returns the effects to run. Nothing here blocks or
//! touches disk, network or the window, which keeps it fully testable.

mod askpass;
mod avatars;
mod branches;
mod clone;
mod commit;
mod conflict;
mod diff;
mod files;
mod forge;
mod gitflow;
mod gitflow_finish;
mod graph_filter;
mod highlight;
mod history;
mod lfs;
mod ops;
mod palette;
mod prefs;
mod publish;
mod rebase;
mod refresh;
mod remotes;
mod repo;
mod selection;
mod shortcuts;
mod stash;
mod submodules;
mod tabs;
mod tags;
mod terminal;
mod undo;
mod watch;
mod work_files;
mod work_files_delete;
mod worktree_manage;
mod worktrees;

#[cfg(test)]
mod askpass_tests;
#[cfg(test)]
mod branch_tests;
#[cfg(test)]
mod cherry_pick_tests;
#[cfg(test)]
mod clone_tests;
#[cfg(test)]
mod commit_tests;
#[cfg(test)]
mod conflict_tests;
#[cfg(test)]
mod diff_nav_tests;
#[cfg(test)]
mod diff_pairs_tests;
#[cfg(test)]
mod diff_tests;
#[cfg(test)]
mod editor_tests;
#[cfg(test)]
mod files_tests;
#[cfg(test)]
mod fold_tests;
#[cfg(test)]
mod force_push_tests;
#[cfg(test)]
mod forge_tests;
#[cfg(test)]
mod gitflow_finish_tests;
#[cfg(test)]
mod gitflow_tests;
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
mod language_override_tests;
#[cfg(test)]
mod lfs_tests;
#[cfg(test)]
mod lfs_track_tests;
#[cfg(test)]
mod lines_tests;
#[cfg(test)]
mod palette_checkout_tests;
#[cfg(test)]
mod palette_more_tests;
#[cfg(test)]
mod palette_pick_tests;
#[cfg(test)]
mod palette_recent_tests;
#[cfg(test)]
mod palette_stash_tests;
#[cfg(test)]
mod palette_tests;
#[cfg(test)]
mod perf_tests;
#[cfg(test)]
mod pr_checkout_tests;
#[cfg(test)]
mod publish_tests;
#[cfg(test)]
mod rebase_e2e_tests;
#[cfg(test)]
mod rebase_tests;
#[cfg(test)]
mod refresh_tests;
#[cfg(test)]
mod remote_tests;
#[cfg(test)]
mod revert_tests;
#[cfg(test)]
mod selection_tests;
#[cfg(test)]
mod shortcuts_tests;
#[cfg(test)]
mod stash_tests;
#[cfg(test)]
mod submodule_tests;
#[cfg(test)]
mod tabs_tests;
#[cfg(test)]
mod tag_remote_tests;
#[cfg(test)]
mod tag_tests;
#[cfg(test)]
mod terminal_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod undo_tests;
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

pub use repo::commit_matches;

use crate::effect::{Effect, UiEffect};
use crate::msg::{Inactive, Msg};
use crate::state::AppState;
use std::path::Path;
use std::time::Instant;

pub fn update(st: &mut AppState, msg: Msg, now: Instant) -> Vec<Effect> {
    let mut fx = Vec::new();
    match msg {
        // Each area's messages go to its handler, which matches them exhaustively.
        Msg::Repo(m) => repo::handle(st, m, now, &mut fx),
        Msg::Load(m) => refresh::handle(st, m, now, &mut fx),
        Msg::Watch(m) => watch::handle(st, m, now, &mut fx),
        Msg::Selection(m) => selection::handle(st, m, &mut fx),
        Msg::Graph(m) => graph_filter::handle(st, m, &mut fx),
        Msg::Avatars(m) => avatars::handle(st, m, &mut fx),
        Msg::Diff(m) => diff::handle(st, m, &mut fx),
        Msg::Conflict(m) => conflict::handle(st, m, &mut fx),
        Msg::History(m) => history::handle(st, m, now, &mut fx),
        Msg::Rebase(m) => rebase::handle(st, m, now, &mut fx),
        Msg::Tabs(m) => tabs::handle(st, m, &mut fx),
        Msg::Stage(m) => ops::stage(st, m, now, &mut fx),
        Msg::Commit(m) => commit::handle(st, m, now, &mut fx),
        Msg::Job(m) => ops::job(st, m, now, &mut fx),
        Msg::Files(m) => files::handle(st, m, now, &mut fx),
        Msg::WorkFile(m) => work_files::handle(st, m, now, &mut fx),
        Msg::Branch(m) => branches::handle(st, m, &mut fx),
        Msg::Clone(m) => clone::handle(st, m, &mut fx),
        Msg::Tag(m) => tags::handle(st, m, &mut fx),
        Msg::Remote(m) => remotes::handle(st, m, &mut fx),
        Msg::Worktree(m) => worktrees::handle(st, m, now, &mut fx),
        Msg::Submodule(m) => submodules::handle(st, m, &mut fx),
        Msg::Terminal(m) => terminal::handle(st, m, &mut fx),
        Msg::Forge(m) => forge::handle(st, m, now, &mut fx),
        Msg::Publish(m) => publish::handle(st, m, &mut fx),
        Msg::Credentials(m) => askpass::handle(st, m, now, &mut fx),
        Msg::Prefs(m) => prefs::handle(st, m, now, &mut fx),
        Msg::Lfs(m) => lfs::handle(st, m, now, &mut fx),
        Msg::GitFlow(m) => gitflow::handle(st, m, now, &mut fx),
        Msg::Palette(m) => palette::handle(st, m, now, &mut fx),
        Msg::Shortcut(a) => shortcuts::handle(st, a, now, &mut fx),

        // Cross-cutting messages, handled here (see `Msg`).
        Msg::Tick(t) => ops::auto_fetch(st, t, &mut fx),
        Msg::Scoped { workdir, msg } => {
            if st.workdir().is_some_and(|w| w == workdir) {
                fx.extend(update(st, *msg, now));
            } else {
                inactive(st, &workdir, *msg, now);
            }
        }
        Msg::Confirm { title, body, button, then } => st.modal = Some(crate::state::Modal::Confirm { title, body, button, then }),
        Msg::ConfirmAccepted => {
            if let Some(crate::state::Modal::Confirm { then, .. }) = st.modal.take() {
                fx.extend(update(st, *then, now));
            }
        }
        Msg::CloseModal => st.modal = None,
        Msg::Copy(text) => fx.push(Effect::Ui(UiEffect::CopyText(text))),
        Msg::OpenUrl(url) => fx.push(Effect::OpenUrl(url)),
        Msg::OpenTerminal => {
            if let Some(w) = st.workdir() {
                fx.push(Effect::OpenTerminal(w));
            }
        }
        Msg::OpenFolder(p) => fx.push(Effect::OpenFolder(p)),
        Msg::Logged { ok, text } => st.log(ok, text),
        Msg::ClearLog => st.log.clear(),
    }
    fx
}

/// A result for the repository at `workdir`, whose tab is no longer active: only what its area
/// says still matters (`Msg::on_inactive`) is applied.
fn inactive(st: &mut AppState, workdir: &Path, msg: Msg, now: Instant) {
    match msg.on_inactive() {
        Some(Inactive::Log(text)) => st.log(false, text),
        Some(Inactive::Undo(action)) => {
            if let Some(p) = st.tabs.iter_mut().find(|t| t.is(workdir)).and_then(|t| t.parked.as_mut()) {
                p.undo.record(action);
            }
        }
        Some(Inactive::WorkFile { op, result }) => work_files::inactive_result(st, op, result, now),
        None => tracing::debug!("dropped a result for {}, which is no longer the active tab", workdir.display()),
    }
}
