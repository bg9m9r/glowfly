//! A partial clone's missing contents (the diff, File view and blame panes while they download,
//! wait for the trust prompt, failed or were cancelled), the status bar's Cancel for a job that
//! only fetches and its size check, and the confirmation before a large Fetch full history.
//! Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_history::with_file_view;
use crate::msg::{JobMsg, Msg, PartialMsg};
use crate::state::blame::BlameView;
use crate::state::history::TextState;
use crate::state::job_stop::JobStop;
use crate::state::partial::{ContentPhase, ContentView, PendingContent, Sizing, Wait};
use crate::state::{AppState, Modal, RunningJob};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::path::{Path, PathBuf};
use trellis_core::history_size::{HistorySize, SizeSource};
use trellis_core::settings::ThemeChoice;

type H = Harness<'static, (AppState, Vec<Msg>)>;

fn pending(dir: &Path, view: ContentView, path: &str, phase: ContentPhase) -> PendingContent {
    let oids = vec![git2::Oid::from_str("3f2a9c41d07be5a8c6f19e2d4b7a0c3e5f816d29").unwrap()];
    PendingContent { workdir: dir.to_path_buf(), view, path: path.into(), remote: "origin".into(), oids, retry: None, phase, held: false }
}

/// The repository with the diff of assets/logo.svg waiting in `phase`.
fn waiting_diff(theme: ThemeChoice, name: &str, phase: ContentPhase) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    st.diff_gen = 3;
    st.partial.content = Some(pending(&dir, ContentView::Diff { generation: 3 }, "assets/logo.svg", phase));
    (st, dir)
}

/// Clicks the button labelled `label` and returns what it pushed.
fn click(h: &mut H, label: &str) -> Vec<Msg> {
    h.state_mut().1.clear();
    h.get_by_label(label).click();
    h.run_steps(4);
    std::mem::take(&mut h.state_mut().1)
}

fn job(label: &str) -> RunningJob {
    RunningJob {
        label: label.into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: true,
    }
}

#[test]
fn content_downloading_dark() {
    let (st, dir) = waiting_diff(ThemeChoice::Dark, "partial-downloading", ContentPhase::Fetching { id: 1, stopping: false });
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("content_downloading_dark");
    assert!(matches!(&click(&mut h, "Cancel")[..], [Msg::Partial(PartialMsg::StopContent)]));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn content_needs_trust_light() {
    let (st, dir) = waiting_diff(ThemeChoice::Light, "partial-trust", ContentPhase::Waiting(Wait::Trust));
    let mut h = harness(st, false);
    h.run_steps(4);
    h.check_snapshot("content_needs_trust_light");
    assert!(matches!(&click(&mut h, "Download")[..], [Msg::Partial(PartialMsg::DownloadContent)]));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn content_failed_dark() {
    let why = "fatal: unable to access 'https://git.example.com/acme/site.git/': Could not resolve host: git.example.com";
    let (st, dir) = waiting_diff(ThemeChoice::Dark, "partial-failed", ContentPhase::Waiting(Wait::Failed(why.into())));
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("content_failed_dark");
    assert!(matches!(&click(&mut h, "Try again")[..], [Msg::Partial(PartialMsg::DownloadContent)]));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn content_cancelled_light() {
    let (st, dir) = waiting_diff(ThemeChoice::Light, "partial-cancelled", ContentPhase::Waiting(Wait::Cancelled));
    let mut h = harness(st, false);
    h.run_steps(4);
    h.check_snapshot("content_cancelled_light");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_view_downloading() {
    let (mut st, dir) = with_file_view("partial-file-view", ThemeChoice::Dark);
    let text = st.history.as_mut().unwrap().text.as_mut().unwrap();
    text.state = TextState::Loading;
    let view = ContentView::FileText { oid: text.oid, path: text.path.clone(), before: text.before };
    st.partial.content = Some(pending(&dir, view, "lib.rs", ContentPhase::Fetching { id: 2, stopping: false }));
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("file_view_downloading");
    assert!(h.query_by_label("Downloading the contents of lib.rs…").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn blame_downloading() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "partial-blame");
    st.blame_gen = 2;
    st.blame = Some(BlameView::loading("src/main.rs".into(), None));
    st.job = Some(job(crate::state::partial::DOWNLOAD_LABEL));
    st.job_stop = Some(JobStop::default());
    st.partial.content = Some(pending(&dir, ContentView::Blame { generation: 2 }, "src/main.rs", ContentPhase::FetchingAsJob));
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("blame_downloading");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn status_job_cancel() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "partial-status-cancel");
    st.job = Some(job("Fetch full history"));
    st.job_stop = Some(JobStop::default());
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("status_job_cancel");
    assert!(h.query_by_label("Fetch full history…").is_some());
    assert!(matches!(&click(&mut h, "Cancel")[..], [Msg::Job(JobMsg::Cancel)]));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn status_job_cancelling() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "partial-status-cancelling");
    st.job = Some(job("Fetch"));
    st.job_stop = Some(JobStop { requested: true, stopped: false });
    let mut h = harness(st, false);
    h.run_steps(4);
    h.check_snapshot("status_job_cancelling");
    assert!(h.query_by_label("Cancelling Fetch…").is_some());
    assert!(h.query_by_label("Cancel").is_none(), "asked once");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn status_sizing() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "partial-status-sizing");
    st.partial.sizing = Some(Sizing { id: 1, workdir: dir.clone(), remote: "origin".into() });
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("status_sizing");
    assert!(h.query_by_label("Checking the full history's size…").is_some());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn shallow_fetch_full_confirm() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "partial-confirm");
    let size = HistorySize::Known { bytes: 2_400_000_000, source: SizeSource::Service("GitHub".into()) };
    let (title, body, button) = crate::state::partial_confirm::confirm(&size, "origin", 1, false).unwrap();
    let then = Box::new(Msg::Job(JobMsg::DeepenNow(trellis_core::shallow_ops::Deepen::Full)));
    st.modal = Some(Modal::Confirm { title, body, button, then });
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("shallow_fetch_full_confirm");
    drop(h);
    remove_fixture(&dir);
}
