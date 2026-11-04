//! The central area: which view fills it (the dashboard, Help > Getting started, welcome, conflict, blame, file history,
//! the pull request panel or the issue panel, diff, the rebase panel, the Range-diff view, the
//! operation journal, the Git Flow panel, or the graph), and the Esc chain that closes them.

use super::UiCache;
use super::theme::color;
use super::{
    blame, common, conflict, dashboard, diff, getting_started, gitflow_panel, graph, history, issue, journal, pull_request, range_diff,
    rebase,
};
use eframe::egui::{self, Key};
use glowfly_model::msg::BlameMsg;
use glowfly_model::msg::ConflictMsg;
use glowfly_model::msg::DiffMsg;
use glowfly_model::msg::HistoryMsg;
use glowfly_model::msg::IssueMsg;
use glowfly_model::msg::Msg;
use glowfly_model::msg::PrefsMsg;
use glowfly_model::msg::RebaseMsg;
use glowfly_model::msg::RepoMsg;
use glowfly_model::msg::SelectionMsg;
use glowfly_model::pull_request::PullRequestMsg;
use glowfly_model::state::AppState;
use glowfly_model::state::layout::Pane;

/// Esc closes the innermost thing open: a maximised terminal panel (restored first, while the
/// keyboard is outside the shell), Preferences (on the Save / Revert bar's buttons, the
/// page's unsaved edits instead), the tour, the dashboard, Getting started, the conflict view, the blame view, the
/// text selected in a diff's code, a pull request's unposted code comment (cancelled, with a confirmation once edited), the
/// file history (a line selection first), the diff (a line selection first), the pull request panel, the issue panel, the interactive
/// rebase panel (its message editor first), the Range-diff view, the operation journal, the Git
/// Flow panel, then a multi-selection or a
/// commit compared with the working directory, and
/// finally the search bar. Dialogs (a credential prompt and the welcome included), the command
/// palette and an open menu of the menu bar or title bar handle Esc themselves. With the graph
/// shown, Esc on a focused control that isn't a text field only gives the graph the focus back. In a text field Esc only leaves the field, and in the terminal it goes to the
/// shell.
pub(crate) fn escape_key(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    if !ctx.input(|i| i.key_pressed(Key::Escape))
        // That Esc cancels the tab drag (`tabs.rs`) and nothing else.
        || super::tabs::dragging(ctx)
        // That Esc closes the open menu (`menu_bar`) and nothing else.
        || super::menu_bar::any_open(ctx)
        || st.modal.is_some()
        || st.askpass.showing()
        || st.palette.is_some()
        || st.view.terminal.focused
        || st.view.was_typing
        || (st.onboarding.welcome.is_some() && st.prefs.is_none())
    {
        return;
    }
    // That Esc closes the open popup (the diff header's View or file actions menu) only. egui
    // closes a popup it draws on Esc too; closing it here also clears one no longer drawn.
    if ctx.memory(|m| m.any_popup_open()) {
        ctx.memory_mut(|m| m.close_popup());
        return;
    }
    // On a focused button, list or other control (not a text field) it only returns the focus
    // to the graph, when the graph is shown.
    if super::pane_focus::escape_to_graph(ctx, st) {
        return;
    }
    if glowfly_model::state::terminal::maximized_shown(st) {
        msgs.push(Msg::Terminal(glowfly_model::terminal::TerminalMsg::ToggleMaximize));
        return;
    }
    if let Some(p) = st.prefs.as_ref() {
        // Nothing while the page's save is written. On the Save / Revert bar's buttons it
        // reverts the page; anywhere else it closes Preferences, which asks first about
        // unsaved edits (update/prefs_leave.rs).
        let edited = glowfly_model::state::prefs_save::unsaved(p, &st.settings, st.snap.as_ref());
        if !glowfly_model::state::prefs_save::saving(p) {
            msgs.push(Msg::Prefs(if edited && st.view.prefs_bar_focused { PrefsMsg::RevertPage } else { PrefsMsg::ClosePrefs }));
        }
    } else if st.onboarding.tour.is_some() {
        msgs.push(Msg::Onboarding(glowfly_model::onboarding::OnboardingMsg::CloseTour));
    } else if st.dashboard.open {
        msgs.push(Msg::Dashboard(glowfly_model::msg::DashboardMsg::Close));
    } else if st.onboarding.getting_started.is_some() {
        msgs.push(Msg::Onboarding(glowfly_model::onboarding::OnboardingMsg::CloseGettingStarted));
    } else if st.session.conflict.view.is_some() {
        msgs.push(Msg::Conflict(ConflictMsg::CloseConflict));
    } else if st.session.blame.view.is_some() {
        msgs.push(Msg::Blame(BlameMsg::Close));
    } else if st.view.diff_text.is_some() && (st.diff_or_download() || st.session.history.view.is_some()) {
        // In a diff, Esc first drops the text selected in its code, then its lines, then the rest.
        msgs.push(Msg::Diff(DiffMsg::ClearTextSelection));
    } else if glowfly_model::state::pull_request::line_comment::esc_cancels(st) {
        msgs.push(Msg::PullRequest(PullRequestMsg::LineComment(glowfly_model::pull_request::line_comment::LineCommentMsg::Cancel)));
    } else if st.session.history.view.is_some() && !st.cues.diff_lines.is_empty() {
        // In File history too, Esc first drops a line selection of the entry's diff.
        msgs.push(Msg::Diff(DiffMsg::ClearLineSelection));
    } else if st.session.history.view.is_some() {
        msgs.push(Msg::History(HistoryMsg::CloseFileHistory));
    } else if st.diff_or_download() {
        // Esc first drops a line selection, then closes the diff.
        msgs.push(if st.cues.diff_lines.is_empty() { Msg::Repo(RepoMsg::CloseDiff) } else { Msg::Diff(DiffMsg::ClearLineSelection) });
    } else if st.session.pull_request.view.is_some() {
        msgs.push(Msg::PullRequest(PullRequestMsg::Close));
    } else if st.session.issue.view.is_some() {
        msgs.push(Msg::Issue(IssueMsg::Close));
    } else if let Some(p) = &st.session.rebase.view {
        // Esc first closes the message editor, then the panel.
        msgs.push(if p.editing.is_some() { Msg::Rebase(RebaseMsg::RebaseCancelMessage) } else { Msg::Rebase(RebaseMsg::CloseRebase) });
    } else if st.session.range_diff.view.is_some() {
        msgs.push(Msg::RangeDiff(glowfly_model::range_diff::RangeDiffMsg::Close));
    } else if st.session.journal.view.is_some() {
        msgs.push(Msg::Journal(glowfly_model::journal::JournalMsg::Close));
    } else if st.session.gitflow_panel {
        msgs.push(Msg::GitFlow(glowfly_model::gitflow::GitFlowMsg::ClosePanel));
    } else if matches!(st.session.selection, glowfly_model::state::Selection::Commits(_) | glowfly_model::state::Selection::VsWorkdir(_)) {
        msgs.push(Msg::Selection(SelectionMsg::CollapseSelection));
    } else {
        st.view.show_search = false;
    }
}

/// The central panel: the dashboard while open, else Getting started while open, else the welcome screen without a repository, else the innermost view open
/// (conflict, blame, file history, pull request panel, issue panel, diff, rebase panel, Range-diff view, operation journal, Git
/// Flow panel), else
/// the graph. Returns where the panel was drawn (the tour outlines it).
pub(super) fn show(ctx: &egui::Context, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) -> egui::Rect {
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(color::bg()))
        .show(ctx, |ui| {
            super::pane_focus::drawn_at(ui.ctx(), Pane::Central, ui.max_rect());
            // Any central view but the graph (which has its own) is one focus target for the
            // central view's and the diff's pane keys.
            let name = st.settings.lang().tr("central.focus.name");
            let (rect, other) = (ui.max_rect(), !st.graph_shown() && st.snap.is_some() && !st.dashboard.open);
            let target = other.then(|| super::pane_focus::central(ui, st, rect, name));
            // `AppState::graph_shown` and `state::layout::tool_view` follow this order: keep the
            // three in step.
            if st.dashboard.open {
                dashboard::show(ui, st, msgs);
            } else if st.onboarding.getting_started.is_some() {
                getting_started::show(ui, st, msgs);
            } else if st.snap.is_none() {
                common::welcome(ui, st, msgs);
            } else if st.session.conflict.view.is_some() {
                conflict::show(ui, st, msgs);
            } else if st.session.blame.view.is_some() {
                blame::show(ui, st, cache, msgs);
            } else if st.session.history.view.is_some() {
                history::show(ui, st, cache, msgs);
            } else if st.session.pull_request.view.is_some() {
                pull_request::show(ui, st, cache, msgs);
            } else if st.session.issue.view.is_some() {
                issue::show(ui, st, &mut cache.markdown, msgs);
            } else if st.diff_or_download() {
                diff::show(ui, st, cache, msgs);
            } else if st.session.rebase.view.is_some() {
                rebase::show(ui, st, cache, msgs);
            } else if st.session.range_diff.view.is_some() {
                range_diff::show(ui, st, msgs);
            } else if st.session.journal.view.is_some() {
                journal::show(ui, st, msgs);
            } else if st.session.gitflow_panel {
                gitflow_panel::show(ui, st, msgs);
            } else {
                graph::show(ui, st, cache, msgs);
            }
            if let Some(t) = target {
                t.ring(ui);
            }
        })
        .response
        .rect
}
