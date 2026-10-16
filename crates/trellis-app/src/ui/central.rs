//! The central area: which view fills it (welcome, conflict, blame, file history, the pull
//! request panel, diff, the rebase and Git Flow panels, or the graph), and the Esc chain that
//! closes them.

use super::UiCache;
use super::theme::color;
use super::{blame, common, conflict, diff, gitflow_panel, graph, history, pull_request, rebase};
use crate::msg::BlameMsg;
use crate::msg::ConflictMsg;
use crate::msg::DiffMsg;
use crate::msg::HistoryMsg;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::msg::RebaseMsg;
use crate::msg::RepoMsg;
use crate::msg::SelectionMsg;
use crate::pull_request::PullRequestMsg;
use crate::state::AppState;
use eframe::egui::{self, Key};

/// Esc closes the innermost thing open: Preferences, the conflict view, the blame view, the
/// file history, the diff (a line selection first), the pull request panel, the interactive
/// rebase panel (its message editor first), the Git Flow panel, then a multi-selection, and
/// finally the search bar. Dialogs (a credential prompt included) and the command palette handle
/// Esc themselves. In a text field Esc only leaves the field, and in the terminal it goes to the
/// shell.
pub(crate) fn escape_key(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    if !ctx.input(|i| i.key_pressed(Key::Escape))
        || st.modal.is_some()
        || st.askpass.showing()
        || st.palette.is_some()
        || st.terminal.focused
        || st.view.was_typing
    {
        return;
    }
    if st.prefs.is_some() {
        msgs.push(Msg::Prefs(PrefsMsg::ClosePrefs));
    } else if st.conflict.is_some() {
        msgs.push(Msg::Conflict(ConflictMsg::CloseConflict));
    } else if st.blame.is_some() {
        msgs.push(Msg::Blame(BlameMsg::Close));
    } else if st.history.is_some() {
        msgs.push(Msg::History(HistoryMsg::CloseFileHistory));
    } else if st.diff.is_some() {
        // Esc first drops a line selection, then closes the diff.
        msgs.push(if st.view.diff_lines.is_empty() { Msg::Repo(RepoMsg::CloseDiff) } else { Msg::Diff(DiffMsg::ClearLineSelection) });
    } else if st.pull_request.is_some() {
        msgs.push(Msg::PullRequest(PullRequestMsg::Close));
    } else if let Some(p) = &st.rebase {
        // Esc first closes the message editor, then the panel.
        msgs.push(if p.editing.is_some() { Msg::Rebase(RebaseMsg::RebaseCancelMessage) } else { Msg::Rebase(RebaseMsg::CloseRebase) });
    } else if st.gitflow_panel {
        msgs.push(Msg::GitFlow(crate::gitflow::GitFlowMsg::ClosePanel));
    } else if matches!(st.selection, crate::state::Selection::Commits(_)) {
        msgs.push(Msg::Selection(SelectionMsg::CollapseSelection));
    } else {
        st.view.show_search = false;
    }
}

/// The central panel: the welcome screen without a repository, else the innermost view open
/// (conflict, blame, file history, pull request panel, diff, rebase panel, Git Flow panel), else
/// the graph.
pub(super) fn show(ctx: &egui::Context, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    egui::CentralPanel::default().frame(egui::Frame::new().fill(color::bg())).show(ctx, |ui| {
        if st.snap.is_none() {
            common::welcome(ui, st, msgs);
        } else if st.conflict.is_some() {
            conflict::show(ui, st, msgs);
        } else if st.blame.is_some() {
            blame::show(ui, st, msgs);
        } else if st.history.is_some() {
            history::show(ui, st, cache, msgs);
        } else if st.pull_request.is_some() {
            pull_request::show(ui, st, cache, msgs);
        } else if st.diff.is_some() {
            diff::show(ui, st, &mut cache.diff_rows, msgs);
        } else if st.rebase.is_some() {
            rebase::show(ui, st, cache, msgs);
        } else if st.gitflow_panel {
            gitflow_panel::show(ui, st, msgs);
        } else {
            graph::show(ui, st, cache, msgs);
        }
    });
}
