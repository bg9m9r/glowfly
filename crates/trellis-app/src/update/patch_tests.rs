//! Headless tests for saving commits as a patch file and applying one: the dialogs asked for,
//! the order and name of an export, refusals, the toasts and log lines, and end-to-end exports
//! and applies against a real repository.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{JobMsg, Msg, PaletteMsg, RepoMsg};
use crate::patch::{PatchEffect, PatchMsg};
use crate::state::AppState;
use crate::state::palette::{PaletteCommand as C, entries};
use crate::state::patch::{APPLY_LABEL, PATCH_BLOCKED_MERGE, PendingApply};
use git2::Oid;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::patch_ops;

fn run(st: &mut AppState, m: PatchMsg) -> Vec<Effect> {
    update(st, Msg::Patch(m), Instant::now())
}

/// `scratch_repo` with its change committed as "Add the second line" and a third commit
/// "Third: more (lines)", open in a fresh state.
fn three_commits(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    // Files are compared byte for byte; keep a global autocrlf from rewriting line ends.
    git(&dir, &["config", "core.autocrlf", "false"]);
    git(&dir, &["commit", "-q", "-am", "Add the second line"]);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "Third: more (lines)"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.snap.is_some());
    (st, dir)
}

/// The commit with this summary.
fn commit(st: &AppState, summary: &str) -> Oid {
    st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == summary).unwrap().oid
}

/// A file next to the fixture `dir`, outside its working tree.
fn beside(dir: &Path, name: &str) -> PathBuf {
    let folder = dir.with_file_name(format!("{}-files", dir.file_name().unwrap().to_string_lossy()));
    std::fs::create_dir_all(&folder).unwrap();
    folder.join(name)
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(beside(dir, "x").parent().unwrap());
    let _ = std::fs::remove_dir_all(dir);
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

fn last_toast(st: &AppState) -> (bool, String) {
    st.toasts.last().map(|t| (t.ok, t.text.clone())).unwrap_or_default()
}

#[test]
fn export_asks_where_to_save_oldest_first() {
    let (mut st, dir) = three_commits("patch-ui-order");
    let (oldest, newest) = (commit(&st, "first"), commit(&st, "Third: more (lines)"));
    let fx = run(&mut st, PatchMsg::Export(vec![newest, oldest]));
    match &fx[..] {
        [Effect::Patch(PatchEffect::PickSave { workdir, oids, name })] => {
            assert_eq!(Some(workdir), st.workdir().as_ref());
            assert_eq!(oids, &[oldest, newest]);
            assert_eq!(name, "first-and-1-more.patch");
        }
        other => panic!("unexpected effects {other:?}"),
    }
    assert!(st.job.is_none(), "saving a patch is no job");
    cleanup(&dir);
}

#[test]
fn export_one_commit_named_after_subject() {
    let (mut st, dir) = three_commits("patch-ui-one");
    let third = commit(&st, "Third: more (lines)");
    let fx = run(&mut st, PatchMsg::Export(vec![third]));
    assert!(matches!(&fx[..], [Effect::Patch(PatchEffect::PickSave { name, .. })] if name == "Third-more-lines.patch"), "{fx:?}");
    cleanup(&dir);
}

#[test]
fn export_ignores_commits_not_in_the_graph() {
    let (mut st, dir) = three_commits("patch-ui-unknown");
    assert!(run(&mut st, PatchMsg::Export(vec![Oid::zero()])).is_empty());
    assert!(run(&mut st, PatchMsg::Export(Vec::new())).is_empty());
    cleanup(&dir);
}

#[test]
fn export_refuses_merges() {
    let (mut st, dir) = three_commits("patch-ui-merge");
    git(&dir, &["checkout", "-q", "-b", "side", "HEAD~1"]);
    std::fs::write(dir.join("c.txt"), "c\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "side"]);
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["merge", "-q", "--no-ff", "-m", "Merge side", "side"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let (merge, third) = (commit(&st, "Merge side"), commit(&st, "Third: more (lines)"));
    assert!(run(&mut st, PatchMsg::Export(vec![merge])).is_empty());
    assert_eq!(last_log(&st), PATCH_BLOCKED_MERGE);
    assert!(run(&mut st, PatchMsg::Export(vec![third, merge])).is_empty(), "one merge blocks a multi-selection");
    cleanup(&dir);
}

#[test]
fn save_cancelled_does_nothing() {
    let (mut st, dir) = three_commits("patch-ui-cancel");
    let workdir = st.workdir().unwrap();
    let oids = vec![commit(&st, "first")];
    assert!(run(&mut st, PatchMsg::SaveTo { workdir, oids, dest: None }).is_empty());
    assert!(st.toasts.is_empty() && st.log.is_empty());
    cleanup(&dir);
}

#[test]
fn save_writes_the_patch_end_to_end() {
    let (mut st, dir) = three_commits("patch-ui-save");
    let workdir = st.workdir().unwrap();
    let oids = vec![commit(&st, "Add the second line"), commit(&st, "Third: more (lines)")];
    let dest = beside(&dir, "x.patch");
    drive(&mut st, Msg::Patch(PatchMsg::SaveTo { workdir, oids, dest: Some(dest.clone()) }));
    let text = std::fs::read_to_string(&dest).unwrap();
    let subjects: Vec<&str> = text.lines().filter(|l| l.starts_with("Subject: [PATCH]")).collect();
    assert_eq!(subjects, ["Subject: [PATCH] Add the second line", "Subject: [PATCH] Third: more (lines)"]);
    assert_eq!(last_toast(&st), (true, "Saved 2 commits to x.patch".to_owned()));
    assert!(last_log(&st).contains(&dest.display().to_string()), "{}", last_log(&st));
    assert!(st.job.is_none());
    cleanup(&dir);
}

#[test]
fn exported_error_is_toasted_and_logged() {
    let mut st = state();
    let fx = run(&mut st, PatchMsg::Exported { dest: "C:/x/y.patch".into(), result: Err("boom".into()) });
    assert!(fx.is_empty());
    assert_eq!(last_toast(&st), (false, "Could not create the patch: boom".to_owned()));
    assert_eq!(last_log(&st), "Could not create the patch: boom");
    assert!(st.log.last().is_some_and(|l| !l.ok));
    assert!(st.view.show_log);
}

#[test]
fn palette_offers_apply_patch_with_a_repo() {
    let st = state();
    assert!(!entries(&st).iter().any(|e| e.command == C::ApplyPatch), "not without a repository");
    let (mut st, dir) = three_commits("patch-ui-palette");
    let e = entries(&st).into_iter().find(|e| e.command == C::ApplyPatch).expect("Apply patch…");
    assert_eq!(e.label, "Apply patch…");
    assert_eq!(e.shortcut(&st.keymap), None);

    update(&mut st, Msg::Palette(PaletteMsg::TogglePalette), Instant::now());
    let fx = update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::ApplyPatch)), Instant::now());
    assert!(st.palette.is_none());
    let workdir = st.workdir().unwrap();
    assert!(matches!(&fx[..], [Effect::Patch(PatchEffect::PickOpen { workdir: w })] if *w == workdir), "{fx:?}");
    assert_eq!(st.palette_recent.position("apply-patch"), Some(0));
    cleanup(&dir);
}

#[test]
fn pick_to_apply_while_busy_only_logs() {
    let (mut st, dir) = three_commits("patch-ui-busy");
    assert!(!update(&mut st, Msg::git("Fetch", &["fetch"]), Instant::now()).is_empty());
    assert!(run(&mut st, PatchMsg::PickToApply).is_empty());
    assert_eq!(last_log(&st), "Another operation is still running.");
    cleanup(&dir);
}

#[test]
fn apply_from_starts_check_then_apply_job() {
    let (mut st, dir) = three_commits("patch-ui-apply-job");
    let workdir = st.workdir().unwrap();
    let p = beside(&dir, "p.patch");
    assert!(run(&mut st, PatchMsg::ApplyFrom { workdir: workdir.clone(), path: None }).is_empty());
    let other = dir.with_file_name("someone-else");
    assert!(run(&mut st, PatchMsg::ApplyFrom { workdir: other, path: Some(p.clone()) }).is_empty());
    assert!(last_log(&st).contains("no longer the active tab"), "{}", last_log(&st));
    assert!(st.job.is_none());

    let fx = run(&mut st, PatchMsg::ApplyFrom { workdir, path: Some(p.clone()) });
    match &fx[..] {
        [Effect::RunGit { cmds, .. }] => assert_eq!(cmds, &patch_ops::apply_cmds(p.to_str().unwrap())),
        other => panic!("unexpected effects {other:?}"),
    }
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some(APPLY_LABEL));
    assert_eq!(st.patch_apply.as_ref().map(|a| a.name.as_str()), Some("p.patch"));
    cleanup(&dir);
}

/// `three_commits`' second commit saved as p.patch, then the branch reset to the first.
fn patch_then_reset(name: &str) -> (AppState, PathBuf, PathBuf) {
    let (mut st, dir) = three_commits(name);
    let p = beside(&dir, "p.patch");
    patch_ops::export(&dir, &[commit(&st, "Add the second line")], &p).unwrap();
    git(&dir, &["reset", "-q", "--hard", "HEAD~2"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    (st, dir, p)
}

#[test]
fn apply_end_to_end_success() {
    let (mut st, dir, p) = patch_then_reset("patch-ui-apply-ok");
    let workdir = st.workdir().unwrap();
    drive(&mut st, Msg::Patch(PatchMsg::ApplyFrom { workdir, path: Some(p) }));
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "one\ntwo\n");
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.unstaged.iter().any(|f| f.path == "a.txt"), "the status was refreshed");
    assert!(snap.staged.is_empty(), "the index is untouched");
    assert_eq!(last_toast(&st), (true, "Applied p.patch to the working tree.".to_owned()));
    assert!(st.job.is_none() && st.patch_apply.is_none());
    cleanup(&dir);
}

#[test]
fn apply_end_to_end_failure_names_file_and_hunk() {
    let (mut st, dir, p) = patch_then_reset("patch-ui-apply-fail");
    std::fs::write(dir.join("a.txt"), "uno\n").unwrap();
    let workdir = st.workdir().unwrap();
    drive(&mut st, Msg::Patch(PatchMsg::ApplyFrom { workdir, path: Some(p) }));
    let (ok, toast) = last_toast(&st);
    assert!(!ok);
    assert_eq!(toast, "p.patch doesn't apply: a.txt has changed where the hunk at line 1 goes. Nothing was changed.");
    for text in ["a.txt", "line 1", "Nothing was changed"] {
        assert!(last_log(&st).contains(text), "{text} in {}", last_log(&st));
    }
    assert_eq!(std::fs::read(dir.join("a.txt")).unwrap(), b"uno\n");
    assert!(st.job.is_none() && st.patch_apply.is_none());
    cleanup(&dir);
}

#[test]
fn other_job_ending_clears_pending_apply() {
    let (mut st, dir) = three_commits("patch-ui-stale");
    assert!(!update(&mut st, Msg::git("Fetch", &["fetch"]), Instant::now()).is_empty());
    st.patch_apply = Some(PendingApply { label: APPLY_LABEL.into(), name: "p.patch".into() });
    update(&mut st, Msg::Job(JobMsg::JobDone { ok: false, output: "offline".into(), undo: None }), Instant::now());
    assert_eq!(last_toast(&st), (false, "Fetch failed: offline".to_owned()));
    assert!(st.patch_apply.is_none());
    cleanup(&dir);
}
