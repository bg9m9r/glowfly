//! Headless tests for the diff viewer's Ignore whitespace toggle: saving, reloading the open
//! diff, and dropping a load started before the toggle. Hunk and line operations on a diff
//! with whitespace hidden are in `whitespace_ops_tests.rs`.

use super::tests::{drive, finish, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::DiffMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::state::AppState;
use crate::state::lines::LineClick::Only;
use glowfly_core::repo::{self, DiffLine, DiffTarget, FileDiff, Hunk, LoadOptions};
use std::path::PathBuf;
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn one_hunk() -> FileDiff {
    let line = |origin: char, text: &str| DiffLine { origin, old: Some(1), new: Some(1), text: text.into() };
    let hunks = vec![Hunk { header: "@@ -1 +1 @@".into(), lines: vec![line('-', "b"), line('+', "c")] }];
    FileDiff { binary: false, hunks, highlight: None, intraline: Default::default(), whole: None }
}

/// The scratch repository plus `ws.txt`, committed and then changed only in whitespace
/// (trailing spaces, a trailing tab and a re-indent). `a.txt` keeps its real change.
fn ws_repo(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.autocrlf", "false"]);
    std::fs::write(dir.join("ws.txt"), "foo\nbar\n    baz\n").unwrap();
    git(&dir, &["add", "ws.txt"]);
    git(&dir, &["commit", "-q", "-m", "ws"]);
    std::fs::write(dir.join("ws.txt"), "foo  \nbar\t\n        baz\n").unwrap();
    dir
}

fn hunks(st: &AppState) -> Option<usize> {
    st.session.diff.view.as_ref().map(|(_, d)| d.hunks.len())
}

#[test]
fn toggle_saves_and_reloads_the_open_diff() {
    let dir = scratch_repo("ws-toggle");
    let mut st = state();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    st.session.diff.view = Some((DiffTarget::Unstaged("a.txt".into()), one_hunk().into()));
    assert!(run(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line: 1, how: Only })).is_empty());
    assert!(!st.view.diff_lines.is_empty());
    assert!(st.hunk_ops_gen().is_some());

    let fx = run(&mut st, Msg::Diff(DiffMsg::SetIgnoreWhitespace(true)));
    assert!(matches!(&fx[..], [Effect::SaveSettings(s), Effect::LoadDiff { ignore_whitespace: true, reload: false, .. }]
        if s.diff_ignore_whitespace));
    assert!(st.view.diff_lines.is_empty(), "hunk rows change, so the selection goes");
    assert_eq!(st.hunk_ops_gen(), None, "hunk buttons wait for the reload");
    assert_eq!(st.saved_settings, serde_json::to_string(&st.settings).unwrap());

    assert!(run(&mut st, Msg::Diff(DiffMsg::SetIgnoreWhitespace(true))).is_empty(), "no change, nothing to do");

    // With no diff open only the setting changes.
    let mut st = state();
    let fx = run(&mut st, Msg::Diff(DiffMsg::SetIgnoreWhitespace(true)));
    assert!(matches!(&fx[..], [Effect::SaveSettings(s)] if s.diff_ignore_whitespace));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn whitespace_toggle_filters_a_real_repo() {
    let dir = ws_repo("ws-filter");
    let ws = DiffTarget::Unstaged("ws.txt".into());
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(ws.clone())));
    assert!(hunks(&st) >= Some(1));

    drive(&mut st, Msg::Diff(DiffMsg::SetIgnoreWhitespace(true)));
    assert_eq!(st.session.diff.view.as_ref().map(|(t, _)| t), Some(&ws), "the diff stays open");
    assert_eq!(hunks(&st), Some(0));

    drive(&mut st, Msg::Diff(DiffMsg::SetIgnoreWhitespace(false)));
    assert!(hunks(&st) >= Some(1), "the hunks come back");

    // Opening a diff, and reloading it after a status refresh, both carry the setting.
    drive(&mut st, Msg::Diff(DiffMsg::SetIgnoreWhitespace(true)));
    let fx = run(&mut st, Msg::Repo(RepoMsg::OpenDiff(ws.clone())));
    assert!(matches!(&fx[..], [Effect::LoadDiff { ignore_whitespace: true, reload: false, .. }]));
    for e in fx {
        let loaded = crate::runner::execute(e).expect("diff result");
        run(&mut st, loaded);
    }
    let status =
        Effect::LoadStatus { workdir: dir.clone(), generation: st.load_gen, submodules: false, open: None, git_status: true, paths: None };
    let status = crate::runner::execute(status).expect("status result");
    let fx = run(&mut st, status);
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadDiff { ignore_whitespace: true, reload: true, .. })));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commit_diffs_reload_with_whitespace_hidden_and_keep_revert() {
    let dir = scratch_repo("ws-commit");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let head = st.snap.as_ref().unwrap().commits[0].oid;
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Commit(head, "a.txt".into()))));
    let fx = run(&mut st, Msg::Diff(DiffMsg::SetIgnoreWhitespace(true)));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadDiff { ignore_whitespace: true, target: DiffTarget::Commit(..), .. })));
    assert_eq!(st.hunk_ops_gen(), None, "Revert Hunk waits for the reload");
    finish(&mut st, fx);
    let generation = st.hunk_ops_gen().expect("the reload landed");
    let fx = run(&mut st, Msg::Stage(StageMsg::RevertHunk { index: 0, generation }));
    assert!(matches!(&fx[..], [Effect::Revert { req, .. }] if req.ignore_whitespace), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_load_started_before_the_toggle_is_dropped() {
    let dir = ws_repo("ws-late");
    let a = DiffTarget::Unstaged("a.txt".into());
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(a.clone())));
    let before = st.session.diff.view.clone();

    // ws.txt is requested with whitespace shown; the toggle lands before its result.
    let held = run(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("ws.txt".into()))));
    let toggled = run(&mut st, Msg::Diff(DiffMsg::SetIgnoreWhitespace(true)));
    for e in held {
        let late = crate::runner::execute(e).expect("diff result");
        run(&mut st, late);
    }
    assert_eq!(st.session.diff.view, before, "the stale result is dropped");
    assert_eq!(st.hunk_ops_gen(), None);

    // The toggle asks again for the file still opening, and that lands.
    for e in toggled.into_iter().filter(|e| matches!(e, Effect::LoadDiff { .. })) {
        let loaded = crate::runner::execute(e).expect("diff result");
        run(&mut st, loaded);
    }
    assert_eq!(st.session.diff.view.as_ref().map(|(t, _)| t), Some(&DiffTarget::Unstaged("ws.txt".into())));
    assert!(st.hunk_ops_gen().is_some());
    let _ = std::fs::remove_dir_all(&dir);
}
