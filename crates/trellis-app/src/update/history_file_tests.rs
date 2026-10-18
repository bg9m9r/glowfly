//! Headless tests for the history's File view: asking for the selected version's text, ignoring
//! stale results, asking again after a tab switch, and reading a real old version.

use super::history_tests::{entry, oid, with_view};
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{HistoryMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::history::{FileTextData, HistoryPane, TextState};
use std::sync::Arc;
use std::time::Instant;
use trellis_core::repo::DiffTarget;

fn send(st: &mut AppState, m: HistoryMsg) -> Vec<Effect> {
    update(st, Msg::History(m), Instant::now())
}

/// The (oid, path) of each text request among `fx`.
fn text_requests(fx: &[Effect]) -> Vec<(git2::Oid, String)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::LoadFileText(r) => Some((r.oid, r.path.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn file_pane_requests_the_selected_version() {
    let (mut st, dir) = with_view("history-file-pane");
    send(&mut st, HistoryMsg::SelectHistoryEntry(0));
    let fx = send(&mut st, HistoryMsg::SetHistoryPane(HistoryPane::File));
    assert_eq!(text_requests(&fx), [(oid(1), "a.txt".to_owned())]);
    let v = st.history.as_ref().unwrap();
    assert_eq!(v.pane, HistoryPane::File);
    assert!(matches!(v.text.as_ref().map(|t| &t.state), Some(TextState::Loading)));
    assert!(send(&mut st, HistoryMsg::SetHistoryPane(HistoryPane::File)).is_empty(), "already on its way");
    // Back to the diff: nothing to read.
    assert!(send(&mut st, HistoryMsg::SetHistoryPane(HistoryPane::Diff)).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn selecting_in_file_pane_requests_text_and_diff() {
    let (mut st, dir) = with_view("history-file-select");
    send(&mut st, HistoryMsg::SetHistoryPane(HistoryPane::File));
    let fx = send(&mut st, HistoryMsg::SelectHistoryEntry(1));
    assert_eq!(text_requests(&fx), [(oid(2), "old.txt".to_owned())]);
    assert!(
        fx.iter().any(|e| matches!(e, Effect::LoadDiff { target: DiffTarget::Commit(o, p), .. } if *o == oid(2) && p == "old.txt")),
        "the diff loads underneath: {fx:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deleted_entry_requests_no_text() {
    let (mut st, dir) = with_view("history-file-deleted");
    let mut gone = entry(3, "a.txt");
    gone.status = 'D';
    st.history.as_mut().unwrap().entries.insert(0, gone);
    send(&mut st, HistoryMsg::SelectHistoryEntry(0));
    let fx = send(&mut st, HistoryMsg::SetHistoryPane(HistoryPane::File));
    assert!(text_requests(&fx).is_empty(), "{fx:?}");
    assert!(st.history.as_ref().unwrap().text.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stale_text_is_ignored() {
    let (mut st, dir) = with_view("history-file-stale");
    send(&mut st, HistoryMsg::SelectHistoryEntry(0));
    send(&mut st, HistoryMsg::SetHistoryPane(HistoryPane::File));
    let data = Arc::new(FileTextData { lines: vec!["x".into()], spans: None });
    send(&mut st, HistoryMsg::FileTextLoaded { oid: oid(2), path: "a.txt".into(), result: Ok(data.clone()) });
    send(&mut st, HistoryMsg::FileTextLoaded { oid: oid(1), path: "b.txt".into(), result: Ok(data.clone()) });
    let text = st.history.as_ref().unwrap().text.as_ref().unwrap();
    assert!(matches!(text.state, TextState::Loading));
    send(&mut st, HistoryMsg::FileTextLoaded { oid: oid(1), path: "a.txt".into(), result: Err("Binary file not shown".into()) });
    let text = st.history.as_ref().unwrap().text.as_ref().unwrap();
    assert!(matches!(&text.state, TextState::Failed(e) if e == "Binary file not shown"));
    // A late duplicate doesn't replace what arrived.
    send(&mut st, HistoryMsg::FileTextLoaded { oid: oid(1), path: "a.txt".into(), result: Ok(data) });
    assert!(matches!(&st.history.as_ref().unwrap().text.as_ref().unwrap().state, TextState::Failed(_)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tab_switch_rerequests_loading_text() {
    let (mut st, dir) = with_view("history-file-reload");
    send(&mut st, HistoryMsg::SelectHistoryEntry(1));
    send(&mut st, HistoryMsg::SetHistoryPane(HistoryPane::File));
    let mut fx = Vec::new();
    super::history::reload_pending(&mut st, &mut fx);
    assert_eq!(text_requests(&fx), [(oid(2), "old.txt".to_owned())]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_pane_e2e_reads_the_old_version() {
    let dir = scratch_repo("history-file-e2e");
    git(&dir, &["commit", "-q", "-am", "edit a"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::History(HistoryMsg::OpenFileHistory { path: "a.txt".into(), at: None }));
    drive(&mut st, Msg::History(HistoryMsg::SetHistoryPane(HistoryPane::File)));
    let lines = |st: &AppState| match st.history.as_ref().and_then(|v| v.text.as_ref()).map(|t| &t.state) {
        Some(TextState::Ready(d)) => d.lines.clone(),
        other => panic!("not ready: {other:?}"),
    };
    assert_eq!(lines(&st), ["one", "two"]);
    drive(&mut st, Msg::History(HistoryMsg::SelectHistoryEntry(1)));
    assert_eq!(lines(&st), ["one"], "the first commit's version");
    let _ = std::fs::remove_dir_all(&dir);
}
