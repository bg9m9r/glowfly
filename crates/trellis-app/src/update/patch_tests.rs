//! Headless tests for saving commits as a patch file: the dialogs asked for, the order and
//! name of an export, refusals, the toasts and log lines, and end-to-end exports against a real
//! repository. Shared helpers for the other patch tests (update/patch_*_tests.rs).

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, PaletteMsg, RepoMsg};
use crate::patch::{PatchEffect, PatchMsg};
use crate::state::AppState;
use crate::state::palette::{PaletteCommand as C, entries};
use crate::state::patch::PATCH_BLOCKED_MERGE;
use git2::Oid;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::patch_ops::PatchSource;

pub(crate) fn run(st: &mut AppState, m: PatchMsg) -> Vec<Effect> {
    update(st, Msg::Patch(m), Instant::now())
}

/// `scratch_repo` with its change committed as "Add the second line" and a third commit
/// "Third: more (lines)", open in a fresh state.
pub(crate) fn three_commits(name: &str) -> (AppState, PathBuf) {
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
pub(crate) fn commit(st: &AppState, summary: &str) -> Oid {
    st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == summary).unwrap().oid
}

/// A file next to the fixture `dir`, outside its working tree.
pub(crate) fn beside(dir: &Path, name: &str) -> PathBuf {
    let folder = dir.with_file_name(format!("{}-files", dir.file_name().unwrap().to_string_lossy()));
    std::fs::create_dir_all(&folder).unwrap();
    folder.join(name)
}

pub(crate) fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(beside(dir, "x").parent().unwrap());
    let _ = std::fs::remove_dir_all(dir);
}

pub(crate) fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

pub(crate) fn last_toast(st: &AppState) -> (bool, String) {
    st.toasts.last().map(|t| (t.ok, t.text.clone())).unwrap_or_default()
}

#[test]
fn export_asks_where_to_save_oldest_first() {
    let (mut st, dir) = three_commits("patch-ui-order");
    let (oldest, newest) = (commit(&st, "first"), commit(&st, "Third: more (lines)"));
    let fx = run(&mut st, PatchMsg::Export(vec![newest, oldest]));
    match &fx[..] {
        [Effect::Patch(PatchEffect::PickSave { workdir, what: PatchSource::Commits { oids, paths }, name })] => {
            assert_eq!(Some(workdir), st.workdir().as_ref());
            assert_eq!(oids, &[oldest, newest]);
            assert!(paths.is_empty());
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
    let what = PatchSource::Commits { oids: vec![commit(&st, "first")], paths: Vec::new() };
    assert!(run(&mut st, PatchMsg::SaveTo { workdir, what, dest: None }).is_empty());
    assert!(st.toasts.is_empty() && st.log.is_empty());
    cleanup(&dir);
}

#[test]
fn save_writes_the_patch_end_to_end() {
    let (mut st, dir) = three_commits("patch-ui-save");
    let workdir = st.workdir().unwrap();
    let oids = vec![commit(&st, "Add the second line"), commit(&st, "Third: more (lines)")];
    let dest = beside(&dir, "x.patch");
    let what = PatchSource::Commits { oids, paths: Vec::new() };
    drive(&mut st, Msg::Patch(PatchMsg::SaveTo { workdir, what, dest: Some(dest.clone()) }));
    let text = std::fs::read_to_string(&dest).unwrap();
    let subjects: Vec<&str> = text.lines().filter(|l| l.starts_with("Subject: [PATCH]")).collect();
    assert_eq!(subjects, ["Subject: [PATCH] Add the second line", "Subject: [PATCH] Third: more (lines)"]);
    assert_eq!(last_toast(&st), (true, "Saved 2 commits to x.patch".to_owned()));
    // The toast shows the file in the file manager, whatever tab is active then.
    let toast = st.toasts.last().unwrap();
    let action = toast.action.as_ref().expect("a show-in-folder button");
    assert_eq!(action.label, crate::state::files::SHOW_IN_FOLDER);
    assert!(action.workdir.is_none());
    let id = toast.id;
    let fx = update(&mut st, Msg::ToastAction(id), Instant::now());
    assert!(matches!(&fx[..], [Effect::RevealPath(p)] if *p == dest), "{fx:?}");
    assert!(last_log(&st).contains(&dest.display().to_string()), "{}", last_log(&st));
    assert!(st.job.is_none());
    cleanup(&dir);
}

#[test]
fn exported_error_is_toasted_and_logged() {
    let mut st = state();
    let fx = run(&mut st, PatchMsg::Exported { dest: "y.patch".into(), result: Err("boom".into()) });
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
