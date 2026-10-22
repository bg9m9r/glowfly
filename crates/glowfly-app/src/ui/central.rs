//! The central area: which view fills it (the dashboard, welcome, conflict, blame, file history,
//! the pull request panel or the issue panel, diff, the rebase panel, the Range-diff view, the
//! operation journal, the Git Flow panel, or the graph), and the Esc chain that closes them.

use super::UiCache;
use super::theme::color;
use super::{blame, common, conflict, dashboard, diff, gitflow_panel, graph, history, issue, journal, pull_request, range_diff, rebase};
use crate::msg::BlameMsg;
use crate::msg::ConflictMsg;
use crate::msg::DiffMsg;
use crate::msg::HistoryMsg;
use crate::msg::IssueMsg;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::msg::RebaseMsg;
use crate::msg::RepoMsg;
use crate::msg::SelectionMsg;
use crate::pull_request::PullRequestMsg;
use crate::state::AppState;
use eframe::egui::{self, Key};

/// Esc closes the innermost thing open: a maximised terminal panel (restored first, while the
/// keyboard is outside the shell), Preferences, the tour, the dashboard, the conflict view, the blame view, a
/// pull request's unposted code comment (cancelled, with a confirmation once edited), the
/// file history (a line selection first), the diff (a line selection first), the pull request panel, the issue panel, the interactive
/// rebase panel (its message editor first), the Range-diff view, the operation journal, the Git
/// Flow panel, then a multi-selection or a
/// commit compared with the working directory, and
/// finally the search bar. Dialogs (a credential prompt and the welcome included), the command
/// palette and an open menu of the menu bar or title bar handle Esc themselves. In a text field Esc only leaves the field, and in the terminal it goes to the
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
        || st.terminal.focused
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
    if crate::state::terminal::maximized_shown(st) {
        msgs.push(Msg::Terminal(crate::terminal::TerminalMsg::ToggleMaximize));
        return;
    }
    if st.prefs.is_some() {
        msgs.push(Msg::Prefs(PrefsMsg::ClosePrefs));
    } else if st.onboarding.tour.is_some() {
        msgs.push(Msg::Onboarding(crate::onboarding::OnboardingMsg::CloseTour));
    } else if st.dashboard.open {
        msgs.push(Msg::Dashboard(crate::msg::DashboardMsg::Close));
    } else if st.conflict.is_some() {
        msgs.push(Msg::Conflict(ConflictMsg::CloseConflict));
    } else if st.blame.is_some() {
        msgs.push(Msg::Blame(BlameMsg::Close));
    } else if crate::state::pull_request_line_comment::esc_cancels(st) {
        msgs.push(Msg::PullRequest(PullRequestMsg::LineComment(crate::pull_request::line_comment::LineCommentMsg::Cancel)));
    } else if st.history.is_some() && !st.view.diff_lines.is_empty() {
        // In File history too, Esc first drops a line selection of the entry's diff.
        msgs.push(Msg::Diff(DiffMsg::ClearLineSelection));
    } else if st.history.is_some() {
        msgs.push(Msg::History(HistoryMsg::CloseFileHistory));
    } else if st.diff_or_download() {
        // Esc first drops a line selection, then closes the diff.
        msgs.push(if st.view.diff_lines.is_empty() { Msg::Repo(RepoMsg::CloseDiff) } else { Msg::Diff(DiffMsg::ClearLineSelection) });
    } else if st.pull_request.is_some() {
        msgs.push(Msg::PullRequest(PullRequestMsg::Close));
    } else if st.issue.is_some() {
        msgs.push(Msg::Issue(IssueMsg::Close));
    } else if let Some(p) = &st.rebase {
        // Esc first closes the message editor, then the panel.
        msgs.push(if p.editing.is_some() { Msg::Rebase(RebaseMsg::RebaseCancelMessage) } else { Msg::Rebase(RebaseMsg::CloseRebase) });
    } else if st.range_diff.is_some() {
        msgs.push(Msg::RangeDiff(crate::range_diff::RangeDiffMsg::Close));
    } else if st.journal.is_some() {
        msgs.push(Msg::Journal(crate::journal::JournalMsg::Close));
    } else if st.gitflow_panel {
        msgs.push(Msg::GitFlow(crate::gitflow::GitFlowMsg::ClosePanel));
    } else if matches!(st.selection, crate::state::Selection::Commits(_) | crate::state::Selection::VsWorkdir(_)) {
        msgs.push(Msg::Selection(SelectionMsg::CollapseSelection));
    } else {
        st.view.show_search = false;
    }
}

/// The central panel: the dashboard while open, else the welcome screen without a repository, else the innermost view open
/// (conflict, blame, file history, pull request panel, issue panel, diff, rebase panel, Range-diff view, operation journal, Git
/// Flow panel), else
/// the graph. Returns where the panel was drawn (the tour outlines it).
pub(super) fn show(ctx: &egui::Context, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) -> egui::Rect {
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(color::bg()))
        .show(ctx, |ui| {
            // `AppState::graph_shown` and `state::layout::tool_view` follow this order: keep the
            // three in step.
            if st.dashboard.open {
                dashboard::show(ui, st, msgs);
            } else if st.snap.is_none() {
                common::welcome(ui, st, msgs);
            } else if st.conflict.is_some() {
                conflict::show(ui, st, msgs);
            } else if st.blame.is_some() {
                blame::show(ui, st, cache, msgs);
            } else if st.history.is_some() {
                history::show(ui, st, cache, msgs);
            } else if st.pull_request.is_some() {
                pull_request::show(ui, st, cache, msgs);
            } else if st.issue.is_some() {
                issue::show(ui, st, &mut cache.markdown, msgs);
            } else if st.diff_or_download() {
                diff::show(ui, st, cache, msgs);
            } else if st.rebase.is_some() {
                rebase::show(ui, st, cache, msgs);
            } else if st.range_diff.is_some() {
                range_diff::show(ui, st, msgs);
            } else if st.journal.is_some() {
                journal::show(ui, st, msgs);
            } else if st.gitflow_panel {
                gitflow_panel::show(ui, st, msgs);
            } else {
                graph::show(ui, st, cache, msgs);
            }
        })
        .response
        .rect
}
