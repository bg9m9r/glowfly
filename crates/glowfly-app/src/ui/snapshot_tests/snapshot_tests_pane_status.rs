//! Panes that are loading or have failed (`pane_status.rs`): blame loading and failed, the file
//! history's list and File view failing (and a version that can't be shown), a pull request's
//! files failing, and a comparison's file list failing inline in the details panel.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_compare::selected;
use super::snapshot_tests_history::with_file_view;
use super::snapshot_tests_pull_request::with_pr;
use crate::msg::{BlameMsg, HistoryMsg, Msg, SelectionMsg};
use crate::pull_request::PullRequestMsg;
use crate::state::blame::BlameView;
use crate::state::history::{FileHistoryView, TextError, TextState};
use crate::state::pull_request::{Files, PrTab};
use crate::state::{AppState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;

const BLAME_FAILED: &str = "Couldn't blame this file";

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// Clicks the button labelled `label` and returns what it pushed.
fn click(h: &mut H, label: &str) -> Vec<Msg> {
    h.state_mut().1.clear();
    h.get_by_label(label).click();
    h.run();
    std::mem::take(&mut h.state_mut().1)
}

#[test]
fn blame_loading() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "pane-blame-loading");
    st.session.blame.view = Some(BlameView::loading("src/app.rs".into(), None));
    let mut h = harness(st, true);
    h.run_steps(3);
    assert!(h.query_by_label("Loading the blame…").is_some());
    h.check_snapshot("blame_loading");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn blame_failed() {
    let raw = "not found: src/x.rs is not in commit abc1234";
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "pane-blame-failed");
    st.session.blame.view = Some(BlameView::failed("src/x.rs".into(), None, raw));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("blame_failed");
    assert!(h.query_by_label(BLAME_FAILED).is_some());
    assert!(h.query_by_label("Src/x.rs is not in commit abc1234").is_some());
    assert!(h.query_by_label_contains("not found:").is_none(), "no kind prefix on screen");
    assert!(matches!(&click(&mut h, "Try again")[..], [Msg::Blame(BlameMsg::Retry)]));
    let copied = click(&mut h, "Copy details");
    assert!(matches!(&copied[..], [Msg::Copy(t)] if t.starts_with(BLAME_FAILED) && t.ends_with(raw)), "the raw error");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn blame_held_light() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "pane-blame-held");
    st.session.blame.view = Some(BlameView::failed("src/app.rs".into(), None, crate::state::blame::HELD));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("blame_held_light");
    assert!(h.query_by_label("Blame is paused until you trust this repository").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn history_list_failed() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "pane-history-failed");
    let mut view = FileHistoryView::loading("src/app.rs".into(), None);
    view.loading = false;
    view.error = Some("network error: the repository is on a drive that went away".into());
    st.session.history.view = Some(view);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("history_list_failed");
    assert!(h.query_by_label(super::history_footer::FAILED_FILE).is_some());
    assert!(matches!(&click(&mut h, "Try again")[..], [Msg::History(HistoryMsg::RetryFileHistory)]));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn history_text_failed_and_unavailable() {
    let (mut st, dir) = with_file_view("pane-history-text", ThemeChoice::Dark);
    let text = st.session.history.view.as_mut().unwrap().text.as_mut().unwrap();
    text.state = TextState::Failed(TextError { message: "could not read lib.rs: the object is missing".into(), retry: true });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("history_text_failed");
    assert!(h.query_by_label(super::history_file::FAILED_TITLE).is_some());
    assert!(matches!(&click(&mut h, "Try again")[..], [Msg::History(HistoryMsg::RetryFileText)]));

    let text = h.state_mut().0.session.history.view.as_mut().unwrap().text.as_mut().unwrap();
    text.state = TextState::Failed(TextError { message: "lib.rs is too large to show (9 MB)".into(), retry: false });
    h.run();
    h.check_snapshot("history_text_unavailable");
    assert!(h.query_by_label(super::history_file::UNAVAILABLE_TITLE).is_some());
    assert!(h.query_by_label("lib.rs is too large to show (9 MB)").is_some());
    assert!(h.query_by_label("Try again").is_none(), "a fixed reason");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pr_files_failed() {
    let (mut st, dir) = with_pr(ThemeChoice::Dark, "pane-pr-files", true);
    let v = st.session.pull_request.view.as_mut().unwrap();
    v.tab = PrTab::Files;
    v.files = Files::Failed("unexpected response: the merge base couldn't be found".into());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("pr_files_failed");
    assert!(h.query_by_label("The merge base couldn't be found").is_some());
    let msgs = click(&mut h, "Try again");
    assert!(matches!(&msgs[..], [Msg::PullRequest(PullRequestMsg::ReloadFiles)]));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn details_multi_failed() {
    let (mut st, dir) = selected(ThemeChoice::Dark, "pane-multi-failed", &["Add login page", "Search results page"], false);
    if let Selection::Commits(m) = &mut st.selection {
        m.changes = Some(Err("could not find the merge base of these commits".into()));
    }
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("details_multi_failed");
    assert!(h.query_by_label("Couldn't compare these commits").is_some());
    let msgs = click(&mut h, "Try again");
    assert!(matches!(&msgs[..], [Msg::Selection(SelectionMsg::RetryCompare)]));
    drop(h);
    remove_fixture(&dir);
}
