//! Headless tests for the diff viewer's Ignore whitespace toggle: saving, reloading the open
//! diff, and refusing hunk and line operations while whitespace changes are hidden.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::DiffMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::state::AppState;
use crate::state::lines::LineClick::Only;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::{self, DiffLine, DiffTarget, FileDiff, Hunk, HunkOp, LoadOptions};

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
    st.diff.as_ref().map(|(_, d)| d.hunks.len())
}

fn whitespace_refusals(st: &AppState) -> usize {
    st.log.iter().filter(|l| !l.ok && l.text.contains("Ignore whitespace")).count()
}

fn has_local_op(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::LocalOp { .. }))
}

fn read(dir: &Path, file: &str) -> String {
    std::fs::read_to_string(dir.join(file)).unwrap()
}

#[test]
fn toggle_saves_and_reloads_the_open_diff() {
    let dir = scratch_repo("ws-toggle");
    let mut st = state();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    st.diff = Some((DiffTarget::Unstaged("a.txt".into()), one_hunk().into()));
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
    assert_eq!(st.diff.as_ref().map(|(t, _)| t), Some(&ws), "the diff stays open");
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
fn hunk_and_line_ops_are_refused_while_whitespace_is_hidden() {
    let dir = scratch_repo("ws-refuse");
    git(&dir, &["config", "core.autocrlf", "false"]);
    let a = DiffTarget::Unstaged("a.txt".into());
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(a.clone())));
    drive(&mut st, Msg::Diff(DiffMsg::SetIgnoreWhitespace(true)));
    let g = st.hunk_ops_gen().expect("the reload landed");
    assert!(st.hunk_ops_blocked().is_some());

    let stage = |generation| Msg::Stage(StageMsg::Hunk { op: HunkOp::Stage, path: "a.txt".into(), index: 0, generation });
    assert!(!has_local_op(&run(&mut st, stage(g))));
    assert_eq!(whitespace_refusals(&st), 1);

    // Clicks don't select lines; a selection made before the toggle can't be applied either.
    let (_, d) = st.diff.clone().unwrap();
    let two = d.hunks[0].lines.iter().position(|l| l.origin == '+').unwrap();
    run(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line: two, how: Only }));
    assert!(st.view.diff_lines.is_empty());
    st.view.diff_lines.click(&a, &d, 0, two, Only);
    assert!(!has_local_op(&run(&mut st, Msg::Stage(StageMsg::ApplyLines(HunkOp::Stage)))));
    assert_eq!(whitespace_refusals(&st), 2);

    // A discard waiting in a confirmation dialog is refused when accepted.
    let discard = Msg::Stage(StageMsg::Hunk { op: HunkOp::Discard, path: "a.txt".into(), index: 0, generation: g });
    run(&mut st, Msg::confirm("Discard hunk", "Discard?".into(), "Discard", discard));
    assert!(!has_local_op(&run(&mut st, Msg::ConfirmAccepted)));
    assert_eq!(whitespace_refusals(&st), 3);
    assert_eq!(read(&dir, "a.txt"), "one\ntwo\n", "nothing discarded");
    assert!(repo::load_diff(&dir, &DiffTarget::Staged("a.txt".into())).is_err(), "nothing staged");

    // Whole-file operations don't use hunk indexes and still work.
    assert!(has_local_op(&run(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())))));

    // Showing whitespace again re-enables hunk staging on the reloaded diff.
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(a.clone())));
    drive(&mut st, Msg::Diff(DiffMsg::SetIgnoreWhitespace(true)));
    drive(&mut st, Msg::Diff(DiffMsg::SetIgnoreWhitespace(false)));
    assert_eq!(st.hunk_ops_blocked(), None);
    let fresh = st.hunk_ops_gen().expect("enabled again");
    drive(&mut st, stage(fresh));
    assert!(repo::load_diff(&dir, &DiffTarget::Staged("a.txt".into())).is_ok(), "the hunk was staged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commit_diffs_block_revert_while_whitespace_is_hidden() {
    let dir = scratch_repo("ws-commit");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let head = st.snap.as_ref().unwrap().commits[0].oid;
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Commit(head, "a.txt".into()))));
    let fx = run(&mut st, Msg::Diff(DiffMsg::SetIgnoreWhitespace(true)));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadDiff { ignore_whitespace: true, target: DiffTarget::Commit(..), .. })));
    assert_eq!(st.hunk_ops_blocked(), Some(crate::state::WHITESPACE_BLOCKS_REVERT));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_load_started_before_the_toggle_is_dropped() {
    let dir = ws_repo("ws-late");
    let a = DiffTarget::Unstaged("a.txt".into());
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(a.clone())));
    let before = st.diff.clone();

    // ws.txt is requested with whitespace shown; the toggle lands before its result.
    let held = run(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("ws.txt".into()))));
    let toggled = run(&mut st, Msg::Diff(DiffMsg::SetIgnoreWhitespace(true)));
    for e in held {
        let late = crate::runner::execute(e).expect("diff result");
        run(&mut st, late);
    }
    assert_eq!(st.diff, before, "the stale result is dropped");
    assert_eq!(st.hunk_ops_gen(), None);

    // The toggle's reload of the diff on screen does land.
    for e in toggled.into_iter().filter(|e| matches!(e, Effect::LoadDiff { .. })) {
        let loaded = crate::runner::execute(e).expect("diff result");
        run(&mut st, loaded);
    }
    assert_eq!(st.diff.as_ref().map(|(t, _)| t), Some(&a));
    assert!(st.hunk_ops_gen().is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hunk_ops_are_not_blocked_with_whitespace_shown() {
    let mut st = state();
    st.diff = Some((DiffTarget::Staged("f.txt".into()), one_hunk().into()));
    assert_eq!(st.hunk_ops_blocked(), None);
    st.settings.diff_ignore_whitespace = true;
    assert!(st.hunk_ops_blocked().is_some());
    st.diff = None;
    assert_eq!(st.hunk_ops_blocked(), None);
}
